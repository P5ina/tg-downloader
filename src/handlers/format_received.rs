use std::str::FromStr;

use teloxide::{dispatching::dialogue::GetChatId, prelude::*};

use crate::{
    schema::{HandlerResult, MyDialogue},
    utils::MediaFormatType,
};

pub async fn format_received(
    bot: Bot,
    dialogue: MyDialogue,
    filename: String,
    query: CallbackQuery,
) -> HandlerResult {
    if let Some(s) = &query.data {
        let media_format = MediaFormatType::from_str(s)?;

        bot.answer_callback_query(&query.id).await?;
        log::info!("Found media format {:?}", media_format);

        let chat_id = query.chat_id().ok_or("Couldn't find message")?;
        bot.send_message(chat_id, format!("Надпись на курточке {s}"))
            .await?;
    }

    Ok(())
}
