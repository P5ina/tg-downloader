use teloxide::prelude::*;

use crate::{errors::HandlerResult, schema::MyDialogue};

pub async fn cancel(bot: Bot, dialogue: MyDialogue, msg: Message) -> HandlerResult {
    bot.send_message(msg.chat.id, "Загрузка отменена.").await?;
    dialogue
        .exit()
        .await
        .map_err(|e| crate::errors::BotError::general(format!("Failed to exit dialogue: {}", e)))?;
    Ok(())
}
