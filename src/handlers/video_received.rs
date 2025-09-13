use std::path::Path;

use teloxide::{prelude::*, types::Video};
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
    let container_path = "/var/lib/telegram-bot-api";
    let host_path = "bot-api-data";
    let local_path = file.path.replace(container_path, host_path);
    let telegram_path = Path::new(&local_path);
    let output_path = replace_path_keep_extension_inplace(
        telegram_path,
        "videos",
        &format!("custom_{unique_id}"),
    );
    log::debug!("Starting downloading video... {}", telegram_path.display());
    let download_result = fs::copy(local_path, &output_path).await;
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

    send_format_message(bot, dialogue, msg, &output_path).await?;
    Ok(())
}
