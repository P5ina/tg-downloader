use std::sync::Arc;

use strum::IntoEnumIterator;
use teloxide::{
    prelude::*,
    types::MaybeInaccessibleMessage,
};

use crate::{
    errors::{BotError, HandlerResult},
    queue::{Task, TaskId, TaskQueue, TaskType},
    utils::MediaFormatType,
};

/// Handle format selection callback from queue-based download
/// Callback format: fmt:format_index:task_id:filename
pub async fn format_callback_received(
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

    // Parse callback data: fmt:format_index:task_id:filename
    let stripped = data.strip_prefix("fmt:").ok_or_else(|| {
        BotError::general(format!("Invalid format callback: {}", data))
    })?;

    // Split into parts: format_index:task_id:filename
    let parts: Vec<&str> = stripped.splitn(3, ':').collect();
    if parts.len() != 3 {
        return Err(BotError::general(format!(
            "Invalid format callback structure: {}",
            data
        )));
    }

    let format_index: usize = parts[0].parse().map_err(|_| {
        BotError::general(format!("Invalid format index: {}", parts[0]))
    })?;
    let task_id_str = parts[1];
    let filename = parts[2];

    // Get format from index
    let format = MediaFormatType::iter()
        .nth(format_index)
        .ok_or_else(|| BotError::general(format!("Invalid format index: {}", format_index)))?;

    log::info!(
        "Format callback: format={:?}, task_id={}, filename={}",
        format,
        task_id_str,
        filename
    );

    // Create conversion task
    let task = Task {
        id: TaskId(task_id_str.to_string()),
        task_type: TaskType::Convert {
            filename: filename.to_string(),
            format,
        },
        chat_id,
        message_id,
        unique_file_id: format!("chat{}_msg{}", chat_id, message_id),
    };

    // Submit to queue
    match task_queue.submit(task).await {
        Ok(position) => {
            let queue_msg = if position > 1 {
                format!("⏳ Задача добавлена в очередь (позиция: {})", position)
            } else {
                "📤 Обрабатываем...".to_string()
            };

            if let MaybeInaccessibleMessage::Regular(m) = &message {
                let _ = bot.edit_message_text(chat_id, m.id, queue_msg).await;
            }
        }
        Err(e) => {
            log::error!("Failed to submit conversion task: {}", e);
            if let MaybeInaccessibleMessage::Regular(m) = &message {
                let _ = bot
                    .edit_message_text(chat_id, m.id, "❌ Ошибка добавления в очередь")
                    .await;
            }
        }
    }

    Ok(())
}
