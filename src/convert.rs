use std::{
    fmt,
    path::{Path, PathBuf},
    process::Stdio,
};

use tokio::{fs, process};

#[derive(Debug)]
pub enum ConversionError {
    NonUtf8Path,
    IOError(std::io::Error),
    FfmpegFailed(std::process::ExitStatus, String),
}

impl From<std::io::Error> for ConversionError {
    fn from(e: std::io::Error) -> Self {
        Self::IOError(e)
    }
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ConversionError::*;
        match self {
            NonUtf8Path => write!(f, "path is not valid UTF-8"),
            IOError(e) => write!(f, "failed to spawn ffmpeg: {e}"),
            FfmpegFailed(code, stderr) => {
                write!(f, "ffmpeg exited with {code} - stderr: {stderr}")
            }
        }
    }
}

impl std::error::Error for ConversionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        use ConversionError::*;
        match self {
            IOError(e) => Some(e),
            _ => None,
        }
    }
}

pub async fn convert_video_note<P: AsRef<Path>>(file: P) -> Result<String, ConversionError> {
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

pub async fn convert_video<P: AsRef<Path>>(file: P) -> Result<String, ConversionError> {
    convert(file, "mp4", &[]).await
}

pub async fn convert_audio<P: AsRef<Path>>(file: P) -> Result<String, ConversionError> {
    convert(file, "mp3", &[]).await
}

pub async fn convert<P: AsRef<Path>>(
    file: P,
    ext: &str,
    args: &[&str],
) -> Result<String, ConversionError> {
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
        ));
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
