use std::path::{Path, PathBuf};

use strum::{Display, EnumIter, EnumString};
use teloxide::types::Message;

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

pub fn get_unique_file_id(msg: Message) -> String {
    format!("chat{}_msg{}", msg.chat.id, msg.id)
}

pub fn replace_path_keep_extension_inplace(
    original_path: &Path,
    new_dir: &str,
    new_filename: &str,
) -> PathBuf {
    let extension = original_path.extension();
    let mut result = PathBuf::from(new_dir);

    if let Some(ext) = extension {
        result.push(format!("{}.{}", new_filename, ext.to_string_lossy()));
    } else {
        result.push(new_filename);
    }

    result
}

#[derive(EnumIter, Display, EnumString, Debug, Clone, PartialEq)]
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
