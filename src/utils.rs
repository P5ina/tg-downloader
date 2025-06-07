use strum::{Display, EnumIter, EnumString};

pub fn is_youtube_video_link(url: &str) -> bool {
    let url = url.trim().to_lowercase();

    let is_youtube_domain = url.starts_with("https://www.youtube.com/watch?")
        || url.starts_with("http://www.youtube.com/watch?")
        || url.starts_with("https://youtube.com/watch?")
        || url.starts_with("http://youtube.com/watch?")
        || url.starts_with("https://youtu.be/")
        || url.starts_with("http://youtu.be/");

    if !is_youtube_domain {
        return false;
    }

    // Проверим наличие параметра v= (для youtube.com/watch?v=)
    if url.contains("youtube.com/watch?") {
        return url.contains("v=") && url.find("v=").unwrap() < 100;
    }

    // Для коротких ссылок youtu.be/ должно быть хотя бы что-то после слэша
    if url.contains("youtu.be/") {
        let parts: Vec<&str> = url.split("youtu.be/").collect();
        return parts.len() == 2 && !parts[1].is_empty();
    }

    false
}

#[derive(EnumIter, Display, EnumString, Debug)]
pub enum MediaFormatType {
    #[strum(to_string = "🎥 Видео")]
    Video,
    #[strum(to_string = "🔈 Аудио")]
    Audio,
    #[strum(to_string = "📷 Кружочек")]
    VideoNote,
    #[strum(to_string = "🎙️ Войс")]
    Voice,
}
