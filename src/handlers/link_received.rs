use std::sync::Arc;

use strum::IntoEnumIterator;
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode},
};

use crate::{
    errors::{BotError, HandlerResult},
    queue::TaskQueue,
    utils::MediaFormatType,
    video::youtube::{
        MAX_VIDEO_DURATION_SECONDS, format_duration, get_video_duration,
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

    // Send immediate feedback
    let status_msg = bot
        .send_message(msg.chat.id, "🔍 Получаю информацию о видео...")
        .await?;

    // Check video duration first
    match get_video_duration(text).await {
        Ok(duration) => {
            if is_video_too_long(duration) {
                let formatted_duration = format_duration(duration);
                let max_duration = format_duration(MAX_VIDEO_DURATION_SECONDS);
                bot.edit_message_text(
                    msg.chat.id,
                    status_msg.id,
                    format!(
                        "❌ <b>Видео слишком длинное</b> ({}).\nМаксимальная длительность: {}",
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

    // Show format selection first
    send_format_message(&bot, &msg, &status_msg, text, &task_queue).await?;

    Ok(())
}

/// Show format selection (Video, Audio, VideoNote, Voice)
async fn send_format_message(
    bot: &Bot,
    msg: &Message,
    status_msg: &Message,
    url: &str,
    task_queue: &Arc<TaskQueue>,
) -> HandlerResult {
    // Store URL in pending downloads and get short ID (format will be set later)
    let short_id = task_queue
        .add_pending_download(url.to_string(), msg.chat.id, status_msg.id, None)
        .await;

    // Create format buttons with callback: ff:format_index:short_id
    // ff = "format first" to distinguish from fmt (format after download)
    let formats: Vec<InlineKeyboardButton> = MediaFormatType::iter()
        .enumerate()
        .map(|(idx, f)| {
            let label = format!("{}", f);
            let callback = format!("ff:{}:{}", idx, short_id);
            InlineKeyboardButton::callback(label, callback)
        })
        .collect();

    let keyboard = InlineKeyboardMarkup::default()
        .append_row([formats[0].clone(), formats[1].clone()])
        .append_row([formats[2].clone(), formats[3].clone()]);

    // Show queue status if there are pending tasks
    let pending = task_queue.pending_count();
    let queue_info = if pending > 0 {
        format!("\n\n📊 В очереди сейчас {} задач", pending)
    } else {
        String::new()
    };

    bot.edit_message_text(
        msg.chat.id,
        status_msg.id,
        format!("🎬 Выбери формат:{}", queue_info),
    )
    .reply_markup(keyboard)
    .await?;

    Ok(())
}
