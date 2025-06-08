use std::path::{Path, PathBuf};

use teloxide::{net::Download, prelude::*, types::Video};
use tokio::fs;

use crate::{
    errors::{BotError, HandlerResult},
    handlers::link_received::send_format_message,
    schema::MyDialogue,
    utils::{get_unique_file_id, replace_path_keep_extension_inplace},
};

pub async fn video_received(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    video: Video,
) -> HandlerResult {
    let file = bot.get_file(video.file.id).await?;

    let unique_id = get_unique_file_id(msg.clone());
    let telegram_path = Path::new(&file.path);
    let output_path = replace_path_keep_extension_inplace(
        telegram_path,
        "videos",
        &format!("custom_{unique_id}"),
    );
    log::debug!("Starting downloading video... {}", telegram_path.display());
    let mut dst = fs::File::create(output_path).await?;
    let download_result = bot.download_file(&file.path, &mut dst).await;
    if let Err(e) = download_result {
        log::error!("Error downloading file: {:?}", e);
        bot.send_message(
            msg.chat.id,
            "⚠️ Мы не смогли скачать ваше видео, попробуйте еще раз.",
        )
        .await?;
        return Err(BotError::general("Error downloading file"));
    }
    log::debug!("Video downloaded");

    let filename = output_path
        .to_str()
        .ok_or_else(|| BotError::general("Path should be valid"))?;
    send_format_message(bot, dialogue, msg, filename).await?;
    Ok(())
}

pub async fn download_file_locally(file_path: &str, output_path: &Path) -> HandlerResult {
    // Проверяем, что это локальный путь Local Bot API
    if file_path.starts_with("/var/lib/telegram-bot-api") {
        // Файл уже есть локально, просто копируем его
        let local_path = convert_api_path_to_local(file_path);

        if local_path.exists() {
            let destination = output_path;

            // Копируем файл
            log::debug!(
                "Copy video file from {} to {}",
                local_path.display(),
                destination.display()
            );
            fs::copy(&local_path, &destination).await?;

            log::info!(
                "✅ Файл скопирован: {} -> {}",
                file_path,
                destination.display()
            );
            return Ok(());
        } else {
            return Err(BotError::file_not_found(file_path));
        }
    }

    // Если путь не локальный, используем стандартную загрузку
    Err(BotError::general("Файл не доступен локально"))
}

fn convert_api_path_to_local(api_path: &str) -> PathBuf {
    // Убираем префикс /var/lib/telegram-bot-api и заменяем на bot-api-data
    if let Some(relative_path) = api_path.strip_prefix("/var/lib/telegram-bot-api/") {
        Path::new("bot-api-data").join(relative_path)
    } else {
        // Если путь не содержит стандартный префикс, возвращаем как есть
        PathBuf::from(api_path)
    }
}
