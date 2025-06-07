use std::time::Duration;

use teloxide::{prelude::*, types::ChatAction};
use tokio::time::sleep;

use crate::{
    schema::{HandlerResult, MyDialogue},
    youtube::{download_video, get_filename},
};

pub async fn link_received(bot: Bot, dialogue: MyDialogue, msg: Message) -> HandlerResult {
    let text = msg
        .text()
        .ok_or("Text should be here. It's invalid state")?;
    bot.send_chat_action(msg.chat.id, ChatAction::Typing)
        .await?;
    let filename = match get_filename(text).await {
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

    match download_video(text).await {
        Ok(_) => {
            bot.send_message(msg.chat.id, "Готово!").await?;
        }
        Err(_) => {
            bot.send_message(msg.chat.id, "Не могу скачать это видео, попробуй другое.")
                .await?;
        }
    }

    Ok(())
}
