use teloxide::prelude::*;
use tokio::fs;

use crate::{errors::HandlerResult, schema::MyDialogue};

pub async fn cancel(bot: Bot, dialogue: MyDialogue, msg: Message) -> HandlerResult {
    bot.send_message(msg.chat.id, "Загрузка отменена.").await?;
    let state_or_none = dialogue.get().await?;
    if let Some(state) = state_or_none {
        match state {
            crate::schema::State::Start => (),
            crate::schema::State::ReceiveFormat { filename } => {
                fs::remove_file(filename).await?;
            }
        }
    }
    dialogue
        .exit()
        .await
        .map_err(|e| crate::errors::BotError::general(format!("Failed to exit dialogue: {}", e)))?;
    Ok(())
}
