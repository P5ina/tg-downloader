use tokio::{fs, io, process};

const VIDEO_OUTPUT_FORMAT: &str = "videos/%(id)s.%(ext)s";

pub async fn get_filename(url: &str) -> io::Result<String> {
    let output = process::Command::new("yt-dlp")
        .arg("--no-playlist")
        .arg("--print")
        .arg("filename")
        .arg("-o")
        .arg(VIDEO_OUTPUT_FORMAT) // чтобы точно знать шаблон
        .arg(url)
        .output()
        .await?;

    if output.status.success() {
        let filename = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(filename)
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            String::from_utf8_lossy(&output.stderr),
        ))
    }
}

pub async fn download_video(url: &str) -> io::Result<()> {
    fs::create_dir_all("videos").await?;

    let output = process::Command::new("yt-dlp")
        .arg("--no-playlist")
        .arg("-o")
        .arg(VIDEO_OUTPUT_FORMAT) // тот же шаблон
        .arg(url)
        .output()
        .await?;

    if output.status.success() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            String::from_utf8_lossy(&output.stderr),
        ))
    }
}
