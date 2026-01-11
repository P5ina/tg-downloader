use std::path::{Path, PathBuf};

use strum::IntoEnumIterator;
use teloxide::{prelude::*, types::{InlineKeyboardButton, InlineKeyboardMarkup, Video}};
use tokio::fs;

use crate::{
    errors::{BotError, HandlerResult},
    schema::{MyDialogue, State},
    utils::{get_unique_file_id, replace_path_keep_extension_inplace, MediaFormatType},
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
    let host_path = "/bot-api-data";
    let local_path = file.path.replace(container_path, host_path);
    let telegram_path = Path::new(&local_path);
    let output_path = replace_path_keep_extension_inplace(
        telegram_path,
        "videos",
        &format!("custom_{unique_id}"),
    );
    log::info!(
        "Downloading video: file.path={}, local_path={}, output_path={}",
        file.path,
        local_path,
        output_path.display()
    );
    let download_result = fs::copy(&local_path, &output_path).await;
    if let Err(e) = download_result {
        log::error!("Error downloading file from {} to {}: {:?}", local_path, output_path.display(), e);
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

async fn send_format_message(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    filename: impl Into<PathBuf>,
) -> HandlerResult {
    let formats: Vec<InlineKeyboardButton> = MediaFormatType::iter()
        .map(|f| format!("{}", f))
        .map(|f| InlineKeyboardButton::callback(&f, &f))
        .collect();

    bot.send_message(
        msg.chat.id,
        "Видео загружено. Теперь выбери формат в котором ты хочешь получить это видео",
    )
    .reply_markup(
        InlineKeyboardMarkup::default()
            .append_row([formats[0].clone(), formats[1].clone()])
            .append_row([formats[2].clone(), formats[3].clone()]),
    )
    .await?;
    dialogue
        .update(State::ReceiveFormat {
            filename: filename.into().to_str().unwrap().to_owned(),
        })
        .await
        .map_err(|e| BotError::general(format!("Failed to update dialogue: {}", e)))?;
    Ok(())
}
