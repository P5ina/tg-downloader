use std::sync::Arc;

use strum::IntoEnumIterator;
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, MaybeInaccessibleMessage, ParseMode},
};

use crate::{
    errors::{BotError, HandlerResult},
    queue::{Task, TaskId, TaskQueue, TaskType},
    subscription::{
        premium::{is_premium_format, SUBSCRIPTION_DAYS, SUBSCRIPTION_PRICE_STARS},
        SubscriptionManager,
    },
    utils::MediaFormatType,
};

/// Handle format selection callback from queue-based download
/// Callback format: fmt:format_index:short_id
pub async fn format_callback_received(
    bot: Bot,
    query: CallbackQuery,
    task_queue: Arc<TaskQueue>,
    subscription_manager: Arc<SubscriptionManager>,
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

    // Parse callback data: fmt:format_index:short_id
    let stripped = data.strip_prefix("fmt:").ok_or_else(|| {
        BotError::general(format!("Invalid format callback: {}", data))
    })?;

    // Split into parts: format_index:short_id
    let parts: Vec<&str> = stripped.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(BotError::general(format!(
            "Invalid format callback structure: {}",
            data
        )));
    }

    let format_index: usize = parts[0].parse().map_err(|_| {
        BotError::general(format!("Invalid format index: {}", parts[0]))
    })?;
    let short_id = parts[1];

    // Get format from index
    let format = MediaFormatType::iter()
        .nth(format_index)
        .ok_or_else(|| BotError::general(format!("Invalid format index: {}", format_index)))?;

    // Check if this is a premium format and user has subscription
    if is_premium_format(&format) {
        let user_id = query.from.id.0 as i64;
        if !subscription_manager.is_subscribed(user_id).await {
            // User doesn't have premium - show upgrade message
            let text = format!(
                "<b>Эта функция доступна только с Premium-подпиской</b>\n\n\
                Конвертация в {} требует подписки.\n\n\
                Стоимость: <b>{} Stars</b> за {} дней",
                format, SUBSCRIPTION_PRICE_STARS, SUBSCRIPTION_DAYS
            );

            let keyboard = InlineKeyboardMarkup::new(vec![vec![
                InlineKeyboardButton::callback("Купить Premium", "buy_premium"),
            ]]);

            if let MaybeInaccessibleMessage::Regular(m) = &message {
                bot.edit_message_text(chat_id, m.id, text)
                    .parse_mode(ParseMode::Html)
                    .reply_markup(keyboard)
                    .await?;
            }
            return Ok(());
        }
    }

    // Get pending conversion data
    let pending = task_queue.take_pending_conversion(short_id).await.ok_or_else(|| {
        BotError::general("Conversion session expired. Please download the video again.")
    })?;

    log::info!(
        "Format callback: format={:?}, filename={}",
        format,
        pending.filename
    );

    // Create conversion task
    let task = Task {
        id: TaskId::new(),
        task_type: TaskType::Convert {
            filename: pending.filename,
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
