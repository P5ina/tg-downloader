use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

use tokio::{fs, process};

use crate::errors::{BotError, BotResult, ConversionError};

const MAX_FILE_SIZE: u64 = 200 * 1024 * 1024; // 200MB in bytes

pub async fn convert_video_note<P: AsRef<Path>>(file: P) -> BotResult<String> {
    convert(
        file,
        "mp4",
        &[
            "-t",
            "60",
            "-vf",
            "scale=(iw*sar)*max(512/(iw*sar)\\,512/ih):ih*max(512/(iw*sar)\\,512/ih), crop=512:512",
        ],
    )
    .await
}

pub async fn convert_video<P: AsRef<Path>>(file: P) -> BotResult<String> {
    // First try normal conversion
    let converted_file = convert(file.as_ref(), "mp4", &["-fs", "240M"]).await?;

    // Check file size
    let file_size = fs::metadata(&converted_file).await?.len();

    if file_size <= MAX_FILE_SIZE {
        return Ok(converted_file);
    }

    // File is too big, remove it and try compression
    fs::remove_file(&converted_file).await?;

    // Return error to signal that compression is needed
    Err(BotError::file_too_large(format!(
        "File size {} bytes exceeds {} bytes limit",
        file_size, MAX_FILE_SIZE
    )))
}

pub async fn compress_video<P: AsRef<Path>>(file: P) -> BotResult<String> {
    // Try compression with reduced quality
    let compressed_file = convert(
        file,
        "mp4",
        &[
            "-crf",
            "28", // Higher CRF = lower quality, smaller file
            "-preset",
            "medium", // Encoding speed vs compression efficiency
            "-vf",
            "scale=iw*min(1280/iw\\,720/ih):ih*min(1280/iw\\,720/ih)", // Scale down if needed
        ],
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
    convert(file, "mp3", &[]).await
}

pub async fn convert<P: AsRef<Path>>(file: P, ext: &str, args: &[&str]) -> BotResult<String> {
    let input_path = file.as_ref();

    fs::create_dir_all("converted").await?;
    let output_path = move_to_new_folder(&input_path.with_extension(ext), "converted");
    let output = process::Command::new("ffmpeg")
        .args(["-y", "-i"])
        .arg(&input_path)
        .args(args)
        .arg(&output_path)
        .stdout(Stdio::null())
        .output()
        .await?;

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

fn move_to_new_folder(path: &Path, new_folder: &str) -> PathBuf {
    let filename = match path.file_name() {
        Some(name) => name,
        None => return PathBuf::from(new_folder), // fallback, если путь не содержит файла
    };

    Path::new(new_folder).join(filename)
}
