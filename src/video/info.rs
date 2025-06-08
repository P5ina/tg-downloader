use serde_json::Value;
use tokio::process::Command;

use crate::errors::{BotError, BotResult};

#[derive(Debug, Clone)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub duration: f64,
}

impl VideoInfo {
    /// Extract video info using JSON parsing with async tokio
    pub async fn from_file(path: &str) -> BotResult<Self> {
        let output = Command::new("ffprobe")
            .args([
                "-v",
                "quiet",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
                path,
            ])
            .output()
            .await
            .map_err(|e| BotError::external_command_error("ffprobe", e.to_string()))?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(BotError::external_command_error("ffprobe", error_msg));
        }

        let json_str = String::from_utf8(output.stdout).map_err(|e| {
            BotError::ParseError(format!("Failed to parse ffprobe output as UTF-8: {}", e))
        })?;

        let json: Value = serde_json::from_str(&json_str)?;

        Self::parse_json(json)
    }

    /// Get only duration of a video file
    pub async fn get_duration(path: &str) -> BotResult<f64> {
        let output = Command::new("ffprobe")
            .args([
                "-v",
                "quiet",
                "-show_entries",
                "format=duration",
                "-of",
                "csv=p=0",
                path,
            ])
            .output()
            .await
            .map_err(|e| BotError::external_command_error("ffprobe", e.to_string()))?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(BotError::external_command_error("ffprobe", error_msg));
        }

        let duration_str = String::from_utf8(output.stdout)
            .map_err(|e| {
                BotError::ParseError(format!("Failed to parse ffprobe output as UTF-8: {}", e))
            })?
            .trim()
            .to_string();

        duration_str.parse::<f64>().map_err(|e| {
            BotError::ParseError(format!(
                "Failed to parse duration '{}': {}",
                duration_str, e
            ))
        })
    }

    /// Parse JSON output from ffprobe
    fn parse_json(json: Value) -> BotResult<Self> {
        // Find video stream
        let streams = json["streams"].as_array().ok_or_else(|| {
            BotError::ParseError("No streams found in ffprobe output".to_string())
        })?;

        let video_stream = streams
            .iter()
            .find(|s| s["codec_type"] == "video")
            .ok_or_else(|| BotError::ParseError("No video stream found".to_string()))?;

        let width = video_stream["width"]
            .as_u64()
            .ok_or_else(|| BotError::ParseError("Width not found in video stream".to_string()))?
            as u32;

        let height = video_stream["height"]
            .as_u64()
            .ok_or_else(|| BotError::ParseError("Height not found in video stream".to_string()))?
            as u32;

        // Get duration from format section
        let duration_str = json["format"]["duration"].as_str().ok_or_else(|| {
            BotError::ParseError("Duration not found in format section".to_string())
        })?;

        let duration = duration_str.parse::<f64>().map_err(|e| {
            BotError::ParseError(format!(
                "Failed to parse duration '{}': {}",
                duration_str, e
            ))
        })?;

        Ok(VideoInfo {
            width,
            height,
            duration,
        })
    }
}
