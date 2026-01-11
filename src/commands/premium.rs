use std::sync::Arc;

use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, LabeledPrice, ParseMode},
};

use crate::{
    errors::HandlerResult,
    subscription::{
        premium::{SUBSCRIPTION_DAYS, SUBSCRIPTION_PRICE_STARS},
        SubscriptionInfo, SubscriptionManager,
    },
};

pub async fn premium(
    bot: Bot,
    msg: Message,
    subscription_manager: Arc<SubscriptionManager>,
) -> HandlerResult {
    let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    let info = subscription_manager.get_subscription_info(user_id).await;

    let (status_text, show_buy_button) = match info {
        SubscriptionInfo::Active {
            expires_at,
            days_left,
        } => {
            let text = format!(
                "<b>Premium-подписка активна</b>\n\n\
                Осталось дней: <b>{}</b>\n\
                Действует до: {}\n\n\
                <b>Доступные функции:</b>\n\
                - Конвертация в кружочки\n\
                - Конвертация в войсы",
                days_left,
                expires_at.format("%d.%m.%Y %H:%M UTC")
            );
            (text, true) // Can extend subscription
        }
        SubscriptionInfo::Expired { expired_at } => {
            let text = format!(
                "<b>Подписка истекла</b>\n\n\
                Истекла: {}\n\n\
                Продлите подписку, чтобы получить доступ к:\n\
                - Конвертация в кружочки\n\
                - Конвертация в войсы",
                expired_at.format("%d.%m.%Y %H:%M UTC")
            );
            (text, true)
        }
        SubscriptionInfo::None => {
            let text = "<b>У вас нет Premium-подписки</b>\n\n\
                Оформите подписку, чтобы получить доступ к:\n\
                - Конвертация в кружочки\n\
                - Конвертация в войсы"
                .to_string();
            (text, true)
        }
    };

    let mut keyboard_buttons = vec![];

    if show_buy_button {
        keyboard_buttons.push(vec![InlineKeyboardButton::callback(
            format!("Купить за {} Stars ({} дней)", SUBSCRIPTION_PRICE_STARS, SUBSCRIPTION_DAYS),
            "buy_premium",
        )]);
    }

    let keyboard = InlineKeyboardMarkup::new(keyboard_buttons);

    bot.send_message(msg.chat.id, status_text)
        .parse_mode(ParseMode::Html)
        .reply_markup(keyboard)
        .await?;

    Ok(())
}

/// Handle the buy_premium callback - send invoice
pub async fn handle_buy_premium_callback(
    bot: Bot,
    query: CallbackQuery,
) -> HandlerResult {
    bot.answer_callback_query(query.id.clone()).await?;

    let chat_id = query.message.as_ref().map(|m| match m {
        teloxide::types::MaybeInaccessibleMessage::Regular(msg) => msg.chat.id,
        teloxide::types::MaybeInaccessibleMessage::Inaccessible(msg) => msg.chat.id,
    });

    let user_id = query.from.id.0;

    if let Some(chat_id) = chat_id {
        // Create invoice payload with user_id
        let payload = format!("premium_sub_{}", user_id);

        // Send invoice with Telegram Stars
        let prices = vec![LabeledPrice::new(
            "Premium-подписка",
            SUBSCRIPTION_PRICE_STARS as u32,
        )];

        bot.send_invoice(
            chat_id,
            "Premium-подписка",
            format!(
                "Доступ к премиум-функциям на {} дней:\n- Конвертация в кружочки\n- Конвертация в войсы",
                SUBSCRIPTION_DAYS
            ),
            payload,
            "XTR", // Telegram Stars currency
            prices,
        )
        .await?;
    }

    Ok(())
}
