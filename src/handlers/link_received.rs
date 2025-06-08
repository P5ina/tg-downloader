use strum::IntoEnumIterator;
use teloxide::{
    prelude::*,
    types::{ChatAction, InlineKeyboardButton, InlineKeyboardMarkup, ParseMode},
};

use crate::{
    errors::HandlerResult,
    schema::{MyDialogue, State},
    utils::MediaFormatType,
    video::youtube::{
        download_video, format_duration, get_filename, get_video_duration, is_video_too_long,
    },
};

pub async fn link_received(bot: Bot, dialogue: MyDialogue, msg: Message) -> HandlerResult {
    let text = msg.text().ok_or_else(|| {
        crate::errors::BotError::general("Text should be here. It's invalid state")
    })?;

    let unique_id = format!("chat{}_msg{}", msg.chat.id, msg.id);

    bot.send_chat_action(msg.chat.id, ChatAction::Typing)
        .await?;

    // Check video duration first
    match get_video_duration(text).await {
        Ok(duration) => {
            if is_video_too_long(duration) {
                let formatted_duration = format_duration(duration);
                let max_duration = format_duration(3600); // 1 hour
                bot.send_message(
                    msg.chat.id,
                    format!(
                        "<b>❌ Видео слишком длинное</b> ({}).\nМаксимальная длительность: {}",
                        formatted_duration, max_duration
                    ),
                )
                .parse_mode(ParseMode::Html)
                .await?;
                return Ok(());
            }
        }
        Err(_) => {
            // If we can't get duration, we'll still try to process the video
            // This handles cases where duration might not be available but video is valid
            log::warn!("Could not get video duration for URL: {}", text);
        }
    }

    let filename = match get_filename(text, &unique_id).await {
        Ok(f) => f,
        Err(_) => {
            bot.send_message(
                msg.chat.id,
                "Не могу найти это видео, попробуй другую ссылку.",
            )
            .await?;
            return Ok(());
        }
    };
    log::info!("Downloading file: {filename}");

    bot.send_chat_action(msg.chat.id, ChatAction::UploadVideo)
        .await?;

    match download_video(text, &unique_id).await {
        Ok(_) => {
            send_format_message(bot, dialogue, msg, &filename).await?;
        }
        Err(e) => {
            log::error!("yt-dlp error: {e}");
            bot.send_message(
                msg.chat.id,
                "❌ Не могу скачать это видео, попробуй другое.",
            )
            .await?;
        }
    }
    Ok(())
}

pub async fn send_format_message(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    filename: &str,
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
            filename: filename.to_owned(),
        })
        .await
        .map_err(|e| {
            crate::errors::BotError::general(format!("Failed to update dialogue: {}", e))
        })?;
    Ok(())
}
