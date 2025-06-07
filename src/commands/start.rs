use teloxide::prelude::*;

use crate::schema::HandlerResult;

pub async fn start(bot: Bot, msg: Message) -> HandlerResult {
    bot.send_message(
        msg.chat.id,
        "Привет 👋\n\nОтправь мне ссылку на YouTube видео, и я превращу его в любой формат, который ты захочешь.",
    )
    .await?;
    Ok(())
}
