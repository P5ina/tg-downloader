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
    video::youtube::get_available_qualities,
};

/// Handle format selection callback (first step after receiving link)
/// Callback format: ff:format_index:short_id
pub async fn format_first_received(
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

    bot.answer_callback_query(query.id.clone()).await?;

    // Parse callback data: ff:format_index:short_id
    let stripped = data.strip_prefix("ff:").ok_or_else(|| {
        BotError::general(format!("Invalid format first callback: {}", data))
    })?;

    let parts: Vec<&str> = stripped.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(BotError::general(format!(
            "Invalid format first callback structure: {}",
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

    // Get pending download
    let pending = task_queue.get_pending_download(short_id).await.ok_or_else(|| {
        BotError::general("Download session expired. Please send the link again.")
    })?;

    log::info!("Format first selected: {:?} for URL: {}", format, pending.url);

    // Update format in pending download
    task_queue.update_pending_download_format(short_id, format.clone()).await;

    // For Video and VideoNote, show quality selection
    // For Audio and Voice, start download immediately (no quality needed)
    match format {
        MediaFormatType::Video | MediaFormatType::VideoNote => {
            // Get available qualities
            if let MaybeInaccessibleMessage::Regular(m) = &message {
                let _ = bot
                    .edit_message_text(chat_id, m.id, "🔍 Получаю доступные качества...")
                    .await;
            }

            match get_available_qualities(&pending.url).await {
                Ok(qualities) => {
                    log::info!("Found {} quality options", qualities.len());

                    // Create quality buttons with short callback: q:short_id:height
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

                    if let MaybeInaccessibleMessage::Regular(m) = &message {
                        let _ = bot
                            .edit_message_text(chat_id, m.id, "🎬 Выбери качество видео:")
                            .reply_markup(keyboard)
                            .await;
                    }
                }
                Err(e) => {
                    log::error!("Failed to get video qualities: {}", e);
                    if let MaybeInaccessibleMessage::Regular(m) = &message {
                        let _ = bot
                            .edit_message_text(
                                chat_id,
                                m.id,
                                "❌ Не могу получить информацию о видео, попробуй другую ссылку.",
                            )
                            .await;
                    }
                }
            }
        }
        MediaFormatType::Audio | MediaFormatType::Voice => {
            // For audio formats, start download immediately without quality selection
            // Take the pending download (removes it from pending)
            let pending = task_queue.take_pending_download(short_id).await.ok_or_else(|| {
                BotError::general("Download session expired. Please send the link again.")
            })?;

            let unique_file_id = format!("chat{}_msg{}", chat_id, message_id);

            // Create download task with no quality (audio only)
            let task = Task {
                id: TaskId::new(),
                task_type: TaskType::Download {
                    url: pending.url,
                    quality: None, // No quality for audio
                    format,
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
                            "⏳ Задача добавлена в очередь (позиция: {})\nСкачиваем аудио...",
                            position
                        )
                    } else {
                        "⏳ Скачиваем аудио...".to_string()
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
        }
    }

    Ok(())
}
