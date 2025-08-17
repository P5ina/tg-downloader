use teloxide::prelude::*;

use crate::errors::HandlerResult;

pub async fn duplicated_link_received(bot: Bot, msg: Message) -> HandlerResult {
    bot.send_message(
        msg.chat.id,
        "❌ Выберите формат или отмените с помощью /cancel",
    )
    .await?;
    Ok(())
}
