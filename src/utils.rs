use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use strum::{Display, EnumIter, EnumString};
use teloxide::prelude::*;
use teloxide::types::{ChatId, Message, MessageId};
use tokio::fs;
use tokio::sync::mpsc;
use tokio::time::sleep;

use crate::video::ProgressInfo;

pub fn is_youtube_video_link(url: &str) -> bool {
    let url = url.trim().to_lowercase();

    // Regular watch links
    let is_watch_link = url.starts_with("https://www.youtube.com/watch?")
        || url.starts_with("http://www.youtube.com/watch?")
        || url.starts_with("https://youtube.com/watch?")
        || url.starts_with("http://youtube.com/watch?");

    // Short links (youtu.be)
    let is_short_link = url.starts_with("https://youtu.be/")
        || url.starts_with("http://youtu.be/");

    // Shorts links
    let is_shorts_link = url.starts_with("https://www.youtube.com/shorts/")
        || url.starts_with("http://www.youtube.com/shorts/")
        || url.starts_with("https://youtube.com/shorts/")
        || url.starts_with("http://youtube.com/shorts/");

    if is_watch_link {
        return url.contains("v=") && url.find("v=").unwrap() < 100;
    }

    if is_short_link {
        let parts: Vec<&str> = url.split("youtu.be/").collect();
        return parts.len() == 2 && !parts[1].is_empty();
    }

    if is_shorts_link {
        let parts: Vec<&str> = url.split("/shorts/").collect();
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

pub async fn loading_screen_with_progress(
    bot: Bot,
    chat_id: ChatId,
    message_id: MessageId,
    should_stop: Arc<AtomicBool>,
    mut progress_receiver: mpsc::UnboundedReceiver<ProgressInfo>,
) {
    let loading_messages = [
        "🚀 Почти готово...",
        "🔄 Еще конвертируем...",
        "⚡ Обрабатываем видео...",
        "🎬 Творим магию...",
        "🛠️ Работаем над этим...",
        "⏳ Терпение, волшебство требует времени...",
        "🎯 Доводим до совершенства...",
        "🔥 Скоро будет готово...",
        "⚙️ Крутим-вертим...",
        "🌟 Добавляем последние штрихи...",
        "🎪 Устраиваем представление...",
        "🔮 Колдуем над файлом...",
    ];

    // Ждем 3 секунды перед началом анимации
    sleep(Duration::from_secs(3)).await;

    let mut current_index = 0;
    let mut last_progress: Option<ProgressInfo> = None;

    loop {
        // Проверяем новые обновления прогресса
        while let Ok(progress) = progress_receiver.try_recv() {
            last_progress = Some(progress);
        }

        if should_stop.load(Ordering::Relaxed) {
            break;
        }

        let base_message = loading_messages[current_index % loading_messages.len()];

        let message = if let Some(ref progress) = last_progress {
            if progress.percentage > 0.0 {
                let progress_bar = create_progress_bar(progress.percentage);
                let time_info = if let Some(eta) = progress.estimated_time_remaining {
                    if eta.as_secs() > 0 {
                        format!(" (осталось ~{})", format_duration(eta))
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                format!(
                    "{}\n{} {:.1}%{}",
                    base_message, progress_bar, progress.percentage, time_info
                )
            } else {
                base_message.to_string()
            }
        } else {
            base_message.to_string()
        };

        // Обновляем сообщение
        let _ = bot.edit_message_text(chat_id, message_id, &message).await;

        current_index += 1;
        sleep(Duration::from_secs(3)).await;
    }
}

pub async fn compression_loading_screen_with_progress(
    bot: Bot,
    chat_id: ChatId,
    message_id: MessageId,
    should_stop: Arc<AtomicBool>,
    mut progress_receiver: mpsc::UnboundedReceiver<ProgressInfo>,
) {
    let compression_messages = [
        "🔧 Сжимаем видео...",
        "🗜️ Уменьшаем размер...",
        "📦 Упаковываем покрепче...",
        "⚡ Применяем компрессию...",
        "🎯 Оптимизируем качество...",
        "🔄 Пережимаем пикселы...",
        "⚙️ Настраиваем битрейт...",
        "🚀 Делаем файл легче...",
        "🌟 Сохраняем качество...",
        "🎪 Творим чудеса сжатия...",
        "🔮 Магия компрессии в действии...",
        "💎 Превращаем в алмаз размера...",
    ];

    // Ждем 3 секунды перед началом анимации
    sleep(Duration::from_secs(3)).await;

    let mut current_index = 0;
    let mut last_progress: Option<ProgressInfo> = None;

    loop {
        // Проверяем новые обновления прогресса
        while let Ok(progress) = progress_receiver.try_recv() {
            last_progress = Some(progress);
        }

        if should_stop.load(Ordering::Relaxed) {
            break;
        }

        let base_message = compression_messages[current_index % compression_messages.len()];

        let message = if let Some(ref progress) = last_progress {
            if progress.percentage > 0.0 {
                let progress_bar = create_progress_bar(progress.percentage);
                let time_info = if let Some(eta) = progress.estimated_time_remaining {
                    if eta.as_secs() > 0 {
                        format!(" (осталось ~{})", format_duration(eta))
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                format!(
                    "{}\n{} {:.1}%{}",
                    base_message, progress_bar, progress.percentage, time_info
                )
            } else {
                base_message.to_string()
            }
        } else {
            base_message.to_string()
        };

        // Обновляем сообщение
        let _ = bot.edit_message_text(chat_id, message_id, &message).await;

        current_index += 1;
        sleep(Duration::from_secs(3)).await;
    }
}

fn create_progress_bar(percentage: f32) -> String {
    let filled = (percentage / 10.0) as usize;
    let empty = 10_usize.saturating_sub(filled);

    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;

    if minutes > 0 {
        format!("{}м {}с", minutes, seconds)
    } else {
        format!("{}с", seconds)
    }
}

pub async fn clear_dir<P: AsRef<Path>>(dir: P) -> std::io::Result<()> {
    let mut entries = fs::read_dir(dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if path.is_dir() {
            fs::remove_dir_all(&path).await?;
        } else {
            fs::remove_file(&path).await?;
        }
    }

    Ok(())
}
