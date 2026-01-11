use std::path::PathBuf;

use log::info;
use strum::IntoEnumIterator;
use teloxide::{
    prelude::*,
    types::{ChatAction, InlineKeyboardButton, InlineKeyboardMarkup, MaybeInaccessibleMessage},
};

use crate::{
    errors::{BotError, HandlerResult},
    schema::{MyDialogue, State},
    utils::MediaFormatType,
    video::youtube::{VideoQuality, download_video},
};

pub async fn quality_received(
    bot: Bot,
    dialogue: MyDialogue,
    url: String,
    query: CallbackQuery,
) -> HandlerResult {
    if let Some(data) = &query.data {
        let message = query
            .message
            .ok_or_else(|| BotError::general("Couldn't find message"))?;

        let chat_id = match &message {
            MaybeInaccessibleMessage::Inaccessible(m) => m.chat.id,
            MaybeInaccessibleMessage::Regular(m) => m.chat.id,
        };

        bot.answer_callback_query(&query.id).await?;

        // Parse quality from callback data
        let height = VideoQuality::from_callback_data(data).ok_or_else(|| {
            BotError::general(format!("Invalid quality callback data: {}", data))
        })?;

        info!("User selected quality: {}p", height);

        // Update message to show downloading status
        if let MaybeInaccessibleMessage::Regular(m) = &message {
            bot.edit_message_text(chat_id, m.id, format!("⏳ Скачиваем видео в {}p...", height))
                .await?;
        }

        bot.send_chat_action(chat_id, ChatAction::UploadVideo)
            .await?;

        let unique_id = format!(
            "chat{}_msg{}",
            chat_id,
            match &message {
                MaybeInaccessibleMessage::Inaccessible(m) => m.message_id,
                MaybeInaccessibleMessage::Regular(m) => m.id,
            }
        );

        match download_video(&url, &unique_id, Some(height)).await {
            Ok(file) => {
                info!("Downloaded file with name {}", file);
                send_format_message(bot, dialogue, chat_id, &message, &file).await?;
            }
            Err(e) => {
                log::error!("yt-dlp error: {e}");
                if let MaybeInaccessibleMessage::Regular(m) = &message {
                    bot.edit_message_text(
                        chat_id,
                        m.id,
                        "❌ Не могу скачать это видео, попробуй другое.",
                    )
                    .await?;
                } else {
                    bot.send_message(
                        chat_id,
                        "❌ Не могу скачать это видео, попробуй другое.",
                    )
                    .await?;
                }
                dialogue
                    .exit()
                    .await
                    .map_err(|e| BotError::general(format!("Failed to exit dialogue: {}", e)))?;
            }
        }
    }

    Ok(())
}

async fn send_format_message(
    bot: Bot,
    dialogue: MyDialogue,
    chat_id: ChatId,
    message: &MaybeInaccessibleMessage,
    filename: impl Into<PathBuf>,
) -> HandlerResult {
    let formats: Vec<InlineKeyboardButton> = MediaFormatType::iter()
        .map(|f| format!("{}", f))
        .map(|f| InlineKeyboardButton::callback(&f, &f))
        .collect();

    let text = "Видео загружено. Теперь выбери формат в котором ты хочешь получить это видео";

    if let MaybeInaccessibleMessage::Regular(m) = message {
        bot.edit_message_text(chat_id, m.id, text)
            .reply_markup(
                InlineKeyboardMarkup::default()
                    .append_row([formats[0].clone(), formats[1].clone()])
                    .append_row([formats[2].clone(), formats[3].clone()]),
            )
            .await?;
    } else {
        bot.send_message(chat_id, text)
            .reply_markup(
                InlineKeyboardMarkup::default()
                    .append_row([formats[0].clone(), formats[1].clone()])
                    .append_row([formats[2].clone(), formats[3].clone()]),
            )
            .await?;
    }

    dialogue
        .update(State::ReceiveFormat {
            filename: filename.into().to_str().unwrap().to_owned(),
        })
        .await
        .map_err(|e| BotError::general(format!("Failed to update dialogue: {}", e)))?;

    Ok(())
}
