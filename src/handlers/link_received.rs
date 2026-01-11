use teloxide::{
    prelude::*,
    types::{ChatAction, InlineKeyboardButton, InlineKeyboardMarkup, ParseMode},
};

use crate::{
    errors::{BotError, HandlerResult},
    schema::{MyDialogue, State},
    video::youtube::{
        MAX_VIDEO_DURATION_SECONDS, format_duration, get_available_qualities, get_video_duration,
        is_video_too_long,
    },
};

pub async fn link_received(bot: Bot, dialogue: MyDialogue, msg: Message) -> HandlerResult {
    let text = msg.text().ok_or_else(|| {
        BotError::general("Text should be here. It's invalid state")
    })?;

    bot.send_chat_action(msg.chat.id, ChatAction::Typing)
        .await?;

    // Check video duration first
    match get_video_duration(text).await {
        Ok(duration) => {
            if is_video_too_long(duration) {
                let formatted_duration = format_duration(duration);
                let max_duration = format_duration(MAX_VIDEO_DURATION_SECONDS);
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

    // Get available qualities
    log::info!("Getting available qualities for URL: {}", text);
    match get_available_qualities(text).await {
        Ok(qualities) => {
            log::info!("Found {} quality options", qualities.len());
            send_quality_message(&bot, &dialogue, &msg, text, &qualities).await?;
        }
        Err(e) => {
            log::error!("Failed to get video qualities: {e}");
            bot.send_message(
                msg.chat.id,
                "❌ Не могу получить информацию о видео, попробуй другую ссылку.",
            )
            .await?;
        }
    }

    Ok(())
}

async fn send_quality_message(
    bot: &Bot,
    dialogue: &MyDialogue,
    msg: &Message,
    url: &str,
    qualities: &[crate::video::VideoQuality],
) -> HandlerResult {
    // Create quality buttons (2 per row)
    let buttons: Vec<InlineKeyboardButton> = qualities
        .iter()
        .map(|q| InlineKeyboardButton::callback(&q.label, q.callback_data()))
        .collect();

    let mut keyboard = InlineKeyboardMarkup::default();
    for chunk in buttons.chunks(2) {
        keyboard = keyboard.append_row(chunk.to_vec());
    }

    bot.send_message(
        msg.chat.id,
        "🎬 Выбери качество видео:",
    )
    .reply_markup(keyboard)
    .await?;

    dialogue
        .update(State::ReceiveQuality {
            url: url.to_owned(),
        })
        .await
        .map_err(|e| BotError::general(format!("Failed to update dialogue: {}", e)))?;

    Ok(())
}
