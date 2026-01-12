use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};

use tokio::{fs, process, sync::mpsc};

use crate::errors::{BotError, BotResult, ConversionError};

const MAX_FILE_SIZE: u64 = 200 * 1024 * 1024; // 200MB in bytes

#[derive(Debug, Clone)]
pub struct ProgressInfo {
    pub percentage: f32,
    pub estimated_time_remaining: Option<Duration>,
}

pub async fn convert_video_note<P: AsRef<Path>>(file: P) -> BotResult<String> {
    convert_with_progress(
        file,
        "mp4",
        &[
            "-t",
            "60",
            "-vf",
            "scale=(iw*sar)*max(512/(iw*sar)\\,512/ih):ih*max(512/(iw*sar)\\,512/ih), crop=512:512",
        ],
        None,
    )
    .await
}

pub async fn compress_video_with_progress<P: AsRef<Path>>(
    file: P,
    progress_sender: Option<mpsc::UnboundedSender<ProgressInfo>>,
) -> BotResult<String> {
    // Try compression with reduced quality
    let compressed_file = convert_with_progress(
        file,
        "mp4",
        &[
            "-crf",
            "32", // Higher CRF = lower quality, smaller file
            "-preset",
            "fast", // Encoding speed vs compression efficiency
            "-vf",
            "scale=iw*min(1280/iw\\,720/ih):ih*min(1280/iw\\,720/ih)", // Scale down if needed
        ],
        progress_sender,
    )
    .await?;

    // Check if compressed file is still too big
    let file_size = fs::metadata(&compressed_file).await?.len();

    if file_size > MAX_FILE_SIZE {
        fs::remove_file(&compressed_file).await?;
        return Err(BotError::file_too_large(format!(
            "Even compressed file size {} bytes exceeds {} bytes limit",
            file_size, MAX_FILE_SIZE
        )));
    }

    Ok(compressed_file)
}

pub async fn convert_audio<P: AsRef<Path>>(file: P) -> BotResult<String> {
    convert_with_progress(file, "mp3", &[], None).await
}

pub async fn convert_with_progress<P: AsRef<Path>>(
    file: P,
    ext: &str,
    args: &[&str],
    progress_sender: Option<mpsc::UnboundedSender<ProgressInfo>>,
) -> BotResult<String> {
    let input_path = file.as_ref();

    fs::create_dir_all("converted").await?;
    let output_path = move_to_new_folder(&input_path.with_extension(ext), "converted");

    // Create progress file for ffmpeg progress reporting
    let progress_file = format!("/tmp/ffmpeg_progress_{}.txt", std::process::id());

    let mut cmd = process::Command::new("ffmpeg");
    cmd.args(["-y", "-i"])
        .arg(&input_path)
        .args(args);

    // Add faststart flag for MP4 files to enable streaming before full download
    if ext == "mp4" {
        cmd.args(["-movflags", "+faststart"]);
    }

    cmd.arg("-progress")
        .arg(&progress_file)
        .arg(&output_path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let child = cmd.spawn()?;

    // Spawn task to monitor progress if sender is provided
    let progress_task = if let Some(sender) = progress_sender {
        let progress_file_clone = progress_file.clone();
        let input_path_str = input_path.to_string_lossy().to_string();

        // Get video duration for video formats only
        let video_duration = if ext == "mp4" {
            // Try to get duration, but don't fail if we can't
            match crate::video::VideoInfo::get_duration(&input_path_str).await {
                Ok(duration) => Some(Duration::from_secs_f64(duration)),
                Err(_) => None,
            }
        } else {
            None
        };

        Some(tokio::spawn(async move {
            monitor_progress(progress_file_clone, sender, video_duration).await;
        }))
    } else {
        None
    };

    let output = child.wait_with_output().await?;

    // Clean up progress file
    let _ = fs::remove_file(&progress_file).await;

    // Stop progress task if it exists and wait briefly for it to finish
    if let Some(task) = progress_task {
        task.abort();
        let _ = tokio::time::timeout(Duration::from_secs(2), task).await;
    }

    if !output.status.success() {
        return Err(ConversionError::FfmpegFailed(
            output.status,
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
        .into());
    }

    let path = output_path.to_str().ok_or(ConversionError::NonUtf8Path)?;
    Ok(path.to_owned())
}

async fn monitor_progress(
    progress_file: String,
    sender: mpsc::UnboundedSender<ProgressInfo>,
    video_duration: Option<Duration>,
) {
    let start_time = Instant::now();
    let mut last_time = Duration::ZERO;
    let mut total_duration: Option<Duration> = video_duration; // Use real duration if available
    let mut no_update_count = 0;

    // Wait a bit for the file to be created
    tokio::time::sleep(Duration::from_millis(500)).await;

    loop {
        // Exit if we've been running too long without updates
        if no_update_count > 30 {
            // 30 * 2 seconds = 1 minute without updates
            break;
        }

        // Check if progress file still exists
        if !tokio::fs::try_exists(&progress_file).await.unwrap_or(false) {
            break;
        }

        if let Ok(content) = fs::read_to_string(&progress_file).await {
            let mut current_time = Duration::ZERO;
            let mut out_time_us: Option<u64> = None;
            let mut ffmpeg_finished = false;

            for line in content.lines() {
                if line.starts_with("out_time_us=") {
                    if let Ok(us) = line.replace("out_time_us=", "").parse::<u64>() {
                        out_time_us = Some(us);
                        current_time = Duration::from_micros(us);
                    }
                } else if line.starts_with("progress=") && line.contains("end") {
                    ffmpeg_finished = true;
                }
            }

            if ffmpeg_finished {
                // FFmpeg finished
                let _ = sender.send(ProgressInfo {
                    percentage: 100.0,
                    estimated_time_remaining: Some(Duration::ZERO),
                });
                break;
            }

            if let Some(_) = out_time_us {
                if current_time > last_time {
                    no_update_count = 0; // Reset counter on progress update
                    let elapsed = start_time.elapsed();
                    let processed_duration = current_time;

                    // Only try to estimate total duration if we don't have it from video info
                    if total_duration.is_none() && elapsed.as_secs() > 10 {
                        // Conservative estimation based on processing speed (only as fallback)
                        let processing_rate =
                            processed_duration.as_secs_f64() / elapsed.as_secs_f64();
                        if processing_rate > 0.5 {
                            // More conservative estimate - assume we're about 40% done
                            total_duration = Some(Duration::from_secs_f64(
                                processed_duration.as_secs_f64() / 0.4,
                            ));
                        }
                    }

                    let (percentage, eta) = if let Some(total) = total_duration {
                        let pct = (processed_duration.as_secs_f64() / total.as_secs_f64() * 100.0)
                            .min(100.0);
                        let remaining_work = total.saturating_sub(processed_duration);
                        let estimated_time = if processed_duration.as_secs() > 0 && pct < 99.0 {
                            // Use real processing speed to estimate remaining time
                            let processing_speed =
                                processed_duration.as_secs_f64() / elapsed.as_secs_f64();
                            if processing_speed > 0.0 {
                                let remaining_seconds =
                                    remaining_work.as_secs_f64() / processing_speed;
                                Some(Duration::from_secs_f64(remaining_seconds))
                            } else {
                                None
                            }
                        } else {
                            Some(Duration::ZERO)
                        };
                        (pct as f32, estimated_time)
                    } else {
                        // If we don't know total duration, show progress based on elapsed time
                        let rough_percentage = (elapsed.as_secs() as f32 * 2.0).min(95.0); // Very rough estimate
                        (rough_percentage, None)
                    };

                    let _ = sender.send(ProgressInfo {
                        percentage,
                        estimated_time_remaining: eta,
                    });

                    last_time = current_time;
                } else {
                    no_update_count += 1;
                }
            } else {
                no_update_count += 1;
            }
        } else {
            no_update_count += 1;
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    // Send final 100% completion
    let _ = sender.send(ProgressInfo {
        percentage: 100.0,
        estimated_time_remaining: Some(Duration::ZERO),
    });
}

fn move_to_new_folder(path: &Path, new_folder: &str) -> PathBuf {
    let filename = match path.file_name() {
        Some(name) => name,
        None => return PathBuf::from(new_folder), // fallback, если путь не содержит файла
    };

    Path::new(new_folder).join(filename)
}
