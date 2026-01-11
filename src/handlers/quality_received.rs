use std::sync::Arc;

use log::info;
use teloxide::{
    prelude::*,
    types::MaybeInaccessibleMessage,
};

use crate::{
    errors::{BotError, HandlerResult},
    queue::{Task, TaskId, TaskQueue, TaskType},
};

/// Handle quality selection callback
/// Callback format: q:short_id:height
pub async fn quality_received(
    bot: Bot,
    query: CallbackQuery,
    task_queue: Arc<TaskQueue>,
) -> HandlerResult {
    let data = query
        .data
        .as_ref()
        .ok_or_else(|| BotError::general("No callback data"))?;

    let message = query
        .message
        .ok_or_else(|| BotError::general("Couldn't find message"))?;

    let chat_id = match &message {
        MaybeInaccessibleMessage::Inaccessible(m) => m.chat.id,
        MaybeInaccessibleMessage::Regular(m) => m.chat.id,
    };

    let message_id = match &message {
        MaybeInaccessibleMessage::Inaccessible(m) => m.message_id,
        MaybeInaccessibleMessage::Regular(m) => m.id,
    };

    bot.answer_callback_query(&query.id).await?;

    // Parse callback data: q:short_id:height
    let stripped = data.strip_prefix("q:").ok_or_else(|| {
        BotError::general(format!("Invalid quality callback: {}", data))
    })?;

    let parts: Vec<&str> = stripped.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(BotError::general(format!(
            "Invalid quality callback structure: {}",
            data
        )));
    }

    let short_id = parts[0];
    let height: u32 = parts[1].parse().map_err(|_| {
        BotError::general(format!("Invalid quality: {}", parts[1]))
    })?;

    // Get URL from pending downloads
    let pending = task_queue.take_pending_download(short_id).await.ok_or_else(|| {
        BotError::general("Download session expired. Please send the link again.")
    })?;

    info!("User selected quality: {}p for URL: {}", height, pending.url);

    let unique_file_id = format!("chat{}_msg{}", chat_id, message_id);

    // Create download task
    let task = Task {
        id: TaskId::new(),
        task_type: TaskType::Download {
            url: pending.url,
            quality: height,
        },
        chat_id,
        message_id,
        unique_file_id,
    };

    // Submit to queue
    match task_queue.submit(task).await {
        Ok(position) => {
            let queue_msg = if position > 1 {
                format!(
                    "⏳ Задача добавлена в очередь (позиция: {})\nСкачиваем видео в {}p...",
                    position, height
                )
            } else {
                format!("⏳ Скачиваем видео в {}p...", height)
            };

            if let MaybeInaccessibleMessage::Regular(m) = &message {
                let _ = bot.edit_message_text(chat_id, m.id, queue_msg).await;
            }
        }
        Err(e) => {
            log::error!("Failed to submit task: {}", e);
            if let MaybeInaccessibleMessage::Regular(m) = &message {
                let _ = bot
                    .edit_message_text(chat_id, m.id, "❌ Ошибка добавления в очередь")
                    .await;
            }
        }
    }

    Ok(())
}
