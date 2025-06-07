use strum::IntoEnumIterator;
use teloxide::{
    prelude::*,
    types::{ChatAction, InlineKeyboardButton, InlineKeyboardMarkup},
};

use crate::{
    schema::{HandlerResult, MyDialogue, State},
    utils::MediaFormatType,
    youtube::{download_video, get_filename},
};

pub async fn link_received(bot: Bot, dialogue: MyDialogue, msg: Message) -> HandlerResult {
    let text = msg
        .text()
        .ok_or("Text should be here. It's invalid state")?;

    let unique_id = format!("chat{}_msg{}", msg.chat.id, msg.id);

    bot.send_chat_action(msg.chat.id, ChatAction::Typing)
        .await?;
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
            dialogue.update(State::ReceiveFormat { filename }).await?;
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
