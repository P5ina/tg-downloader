use log::info;
use serde::Deserialize;
use tokio::{fs, process};

use crate::errors::{BotError, BotResult};
use crate::utils::MediaFormatType;

pub const MAX_VIDEO_DURATION_SECONDS: u32 = 3600; // 1 hour

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VideoQuality {
    pub height: u32,
    pub label: String,
}

impl VideoQuality {
    pub fn new(height: u32) -> Self {
        let label = format!("{}p", height);
        Self { height, label }
    }
}

#[derive(Debug, Deserialize)]
struct YtDlpFormat {
    height: Option<u32>,
    vcodec: Option<String>,
}

#[derive(Debug, Deserialize)]
struct YtDlpInfo {
    formats: Vec<YtDlpFormat>,
}

/// Get available video qualities for a YouTube URL
pub async fn get_available_qualities(url: &str) -> BotResult<Vec<VideoQuality>> {
    let mut cmd = process::Command::new("yt-dlp");
    cmd.arg("--no-playlist")
        .args(["--socket-timeout", "5", "--retries", "3"])
        .args(["-J"]) // JSON output
        .arg(url);

    let output = cmd
        .output()
        .await
        .map_err(|e| BotError::external_command_error("yt-dlp", e.to_string()))?;

    if !output.status.success() {
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(BotError::youtube_error(stderr_str));
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    let info: YtDlpInfo = serde_json::from_str(&json_str)
        .map_err(|e| BotError::ParseError(format!("Failed to parse yt-dlp output: {}", e)))?;

    // Collect unique heights from video formats
    let mut heights: Vec<u32> = info
        .formats
        .iter()
        .filter(|f| {
            f.vcodec.as_ref().map_or(false, |v| v != "none")
                && f.height.map_or(false, |h| h > 0)
        })
        .filter_map(|f| f.height)
        .collect();

    heights.sort_unstable();
    heights.dedup();

    // Standard qualities to offer (filter by what's actually available)
    let standard_qualities = [360, 480, 720, 1080, 1440, 2160];
    let available: Vec<VideoQuality> = standard_qualities
        .iter()
        .filter(|&&h| heights.iter().any(|&available_h| available_h >= h))
        .map(|&h| VideoQuality::new(h))
        .collect();

    if available.is_empty() {
        // If no standard qualities match, return the best available
        if let Some(&max_height) = heights.last() {
            return Ok(vec![VideoQuality::new(max_height)]);
        }
        return Err(BotError::youtube_error(
            "No video formats available".to_string(),
        ));
    }

    Ok(available)
}

fn get_output_format(unique_id: &str) -> String {
    format!("videos/%(id)s_{unique_id}.%(ext)s")
}

fn build_video_command(url: &str, max_height: Option<u32>) -> process::Command {
    let mut cmd = process::Command::new("yt-dlp");
    cmd.arg("--no-playlist")
        .args(["--socket-timeout", "5", "--retries", "3"])
        // Download fragments concurrently to bypass YouTube throttling
        .args(["-N", "4"])
        // Always remux to mp4 to ensure faststart is applied
        .args(["--remux-video", "mp4"])
        // Add faststart for streaming compatibility (allows playback before full download)
        .args(["--ppa", "FFmpegVideoRemuxer:-movflags +faststart"]);

    // Apply quality filter - prefer H.264 (avc1) and AAC for Telegram compatibility
    // This avoids re-encoding since these codecs are natively supported
    if let Some(height) = max_height {
        // Prefer h264 video + aac/m4a audio, fall back to best available
        let format = format!(
            "bestvideo[height<={}][vcodec^=avc1]+bestaudio[acodec^=mp4a]/\
             bestvideo[height<={}][vcodec^=avc1]+bestaudio/\
             bestvideo[height<={}]+bestaudio/\
             best[height<={}]/best",
            height, height, height, height
        );
        cmd.args(["-f", &format]);
    } else {
        // No height limit - prefer h264 + aac for compatibility
        cmd.args(["-f",
            "bestvideo[vcodec^=avc1]+bestaudio[acodec^=mp4a]/\
             bestvideo[vcodec^=avc1]+bestaudio/\
             bestvideo+bestaudio/best"
        ]);
    }

    cmd.arg(url);
    cmd
}

fn build_audio_command(url: &str) -> process::Command {
    let mut cmd = process::Command::new("yt-dlp");
    cmd.arg("--no-playlist")
        .args(["--socket-timeout", "5", "--retries", "3"])
        // Download fragments concurrently
        .args(["-N", "4"])
        // Download only audio - prefer AAC for Telegram compatibility
        .args(["-f", "bestaudio[acodec^=mp4a]/bestaudio/best"])
        // Extract audio and convert to m4a (AAC container)
        .args(["-x", "--audio-format", "m4a"]);

    cmd.arg(url);
    cmd
}

// pub async fn get_filename(url: &str, unique_id: &str) -> BotResult<String> {
//     let mut cmd = build_base_command(url, unique_id);
//     let output = cmd
//         .output()
//         .await
//         .map_err(|e| BotError::external_command_error("yt-dlp", e.to_string()))?;

//     if output.status.success() {
//         let filename = String::from_utf8_lossy(&output.stdout).trim().to_string();
//         Ok(filename)
//     } else {
//         let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
//         Err(BotError::youtube_error(stderr_str))
//     }
// }

/// Result of video download containing video path and optional thumbnail path
#[derive(Debug, Clone)]
pub struct DownloadResult {
    pub video_path: String,
    pub thumbnail_path: Option<String>,
}

impl std::fmt::Display for DownloadResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.video_path)
    }
}

pub async fn download_video(
    url: &str,
    unique_id: &str,
    max_height: Option<u32>,
    format: &MediaFormatType,
) -> BotResult<DownloadResult> {
    fs::create_dir_all("videos").await?;

    let is_audio_only = matches!(format, MediaFormatType::Audio | MediaFormatType::Voice);

    let mut cmd = if is_audio_only {
        build_audio_command(url)
    } else {
        build_video_command(url, max_height)
    };

    cmd.args(["--no-simulate"])
        .args(["-o", &get_output_format(unique_id)])
        .args(["--print", "after_move:filepath"]);

    // Download thumbnail only for video formats
    if !is_audio_only {
        cmd.args(["--write-thumbnail"])
            .args(["--convert-thumbnails", "jpg"]);
    }

    info!(
        "Starting download: {} (quality: {:?}, format: {:?}, audio_only: {})",
        url, max_height, format, is_audio_only
    );

    let output = cmd
        .output()
        .await
        .map_err(|e| BotError::external_command_error("yt-dlp", e.to_string()))?;

    info!("yt-dlp exit code: {:?}", output.status.code());

    if output.status.success() {
        let file_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        info!("Download successful: {}", file_path);

        // Find thumbnail file only for video formats
        let thumbnail_path = if is_audio_only {
            None
        } else {
            find_thumbnail(&file_path).await
        };

        Ok(DownloadResult {
            video_path: file_path,
            thumbnail_path,
        })
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        log::error!("yt-dlp failed: {}", stderr);
        Err(BotError::youtube_error(stderr))
    }
}

/// Find thumbnail file for a video (yt-dlp saves it with same name but .jpg extension)
async fn find_thumbnail(video_path: &str) -> Option<String> {
    use std::path::Path;

    let video_path = Path::new(video_path);
    let stem = video_path.file_stem()?.to_str()?;
    let parent = video_path.parent()?;

    // yt-dlp may save thumbnail as .jpg
    let thumb_path = parent.join(format!("{}.jpg", stem));
    if fs::try_exists(&thumb_path).await.ok()? {
        return Some(thumb_path.to_string_lossy().into_owned());
    }

    None
}

pub async fn get_video_duration(url: &str) -> BotResult<u32> {
    let mut cmd = process::Command::new("yt-dlp");
    cmd.arg("--no-playlist")
        .args(["--socket-timeout", "5", "--retries", "3"])
        .args(["--print", "duration"])
        .arg(url);

    let output = cmd
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
