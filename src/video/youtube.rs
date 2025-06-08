use tokio::{fs, process};

use crate::errors::{BotError, BotResult};

const VIDEO_FORMAT: &str = "bestvideo[height<=720][ext=mp4]+bestaudio[ext=m4a]/mp4";
const MAX_VIDEO_DURATION_SECONDS: u32 = 3600; // 1 hour

fn get_output_format(unique_id: &str) -> String {
    format!("videos/%(id)s_{unique_id}.%(ext)s")
}

fn build_base_command(url: &str, unique_id: &str) -> process::Command {
    let mut cmd = process::Command::new("yt-dlp");
    cmd.arg("--no-playlist")
        .args(["--socket-timeout", "5", "--retries", "3"])
        .args(["-f", VIDEO_FORMAT])
        .args(["-o", &get_output_format(unique_id)])
        .arg(url);
    cmd
}

pub async fn get_filename(url: &str, unique_id: &str) -> BotResult<String> {
    let mut cmd = build_base_command(url, unique_id);
    let output = cmd
        .args(["--print", "filename"])
        .output()
        .await
        .map_err(|e| BotError::external_command_error("yt-dlp", e.to_string()))?;

    if output.status.success() {
        let filename = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(filename)
    } else {
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
        Err(BotError::youtube_error(stderr_str))
    }
}

pub async fn download_video(url: &str, unique_id: &str) -> BotResult<()> {
    fs::create_dir_all("videos").await?;

    let output = build_base_command(url, unique_id)
        .output()
        .await
        .map_err(|e| BotError::external_command_error("yt-dlp", e.to_string()))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
        Err(BotError::youtube_error(stderr_str))
    }
}

pub async fn get_video_duration(url: &str) -> BotResult<u32> {
    let mut cmd = process::Command::new("yt-dlp");
    let output = cmd
        .arg("--no-playlist")
        .args(["--socket-timeout", "5", "--retries", "3"])
        .args(["--print", "duration"])
        .arg(url)
        .output()
        .await
        .map_err(|e| BotError::external_command_error("yt-dlp", e.to_string()))?;

    if output.status.success() {
        let duration_str = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // yt-dlp can return "NA" for duration if it's not available
        if duration_str == "NA" || duration_str.is_empty() {
            return Err(BotError::youtube_error(
                "Video duration is not available".to_string(),
            ));
        }

        let duration = duration_str.parse::<f64>().map_err(|_| {
            BotError::youtube_error(format!("Invalid duration format: {}", duration_str))
        })?;

        Ok(duration as u32)
    } else {
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
        Err(BotError::youtube_error(stderr_str))
    }
}

pub fn is_video_too_long(duration_seconds: u32) -> bool {
    duration_seconds > MAX_VIDEO_DURATION_SECONDS
}

pub fn format_duration(seconds: u32) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, secs)
    } else {
        format!("{}:{:02}", minutes, secs)
    }
}
