use teloxide::prelude::*;

use crate::schema::{HandlerResult, MyDialogue};

pub async fn cancel(bot: Bot, dialogue: MyDialogue, msg: Message) -> HandlerResult {
    bot.send_message(msg.chat.id, "Загрузка отменена.").await?;
    dialogue.exit().await?;
    Ok(())
}
