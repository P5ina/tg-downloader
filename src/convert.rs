use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

use tokio::{fs, process};

use crate::errors::{BotResult, ConversionError};

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
    convert(file, "mp4", &[]).await
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
