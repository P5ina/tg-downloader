use std::sync::Arc;

use teloxide::{
    prelude::*,
    types::{ChatAction, InlineKeyboardButton, InlineKeyboardMarkup, ParseMode},
};

use crate::{
    errors::{BotError, HandlerResult},
    queue::TaskQueue,
    video::youtube::{
        MAX_VIDEO_DURATION_SECONDS, format_duration, get_available_qualities, get_video_duration,
        is_video_too_long,
    },
};

pub async fn link_received(
    bot: Bot,
    msg: Message,
    task_queue: Arc<TaskQueue>,
) -> HandlerResult {
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
            send_quality_message(&bot, &msg, text, &qualities, &task_queue).await?;
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
    msg: &Message,
    url: &str,
    qualities: &[crate::video::VideoQuality],
    task_queue: &Arc<TaskQueue>,
) -> HandlerResult {
    // Send message first to get message_id
    let sent_msg = bot
        .send_message(msg.chat.id, "🎬 Загрузка...")
        .await?;

    // Store URL in pending downloads and get short ID
    let short_id = task_queue
        .add_pending_download(url.to_string(), msg.chat.id, sent_msg.id)
        .await;

    // Create quality buttons with short callback: q:short_id:height
    // Total callback length: "q:" (2) + short_id (8) + ":" (1) + height (max 4) = max 15 chars
    let buttons: Vec<InlineKeyboardButton> = qualities
        .iter()
        .map(|q| {
            let callback = format!("q:{}:{}", short_id, q.height);
            InlineKeyboardButton::callback(&q.label, callback)
        })
        .collect();

    let mut keyboard = InlineKeyboardMarkup::default();
    for chunk in buttons.chunks(2) {
        keyboard = keyboard.append_row(chunk.to_vec());
    }

    // Show queue status if there are pending tasks
    let queue_size = task_queue.queue_size();
    let queue_info = if queue_size > 0 {
        format!("\n\n📊 В очереди сейчас {} задач", queue_size)
    } else {
        String::new()
    };

    bot.edit_message_text(
        msg.chat.id,
        sent_msg.id,
        format!("🎬 Выбери качество видео:{}", queue_info),
    )
    .reply_markup(keyboard)
    .await?;

    Ok(())
}
