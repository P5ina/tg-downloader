use std::sync::Arc;

use teloxide::prelude::*;

use crate::{
    errors::HandlerResult,
    subscription::{premium::SUBSCRIPTION_DAYS, SubscriptionManager},
};

/// Handle pre-checkout query - approve the payment
pub async fn handle_pre_checkout_query(bot: Bot, query: PreCheckoutQuery) -> HandlerResult {
    // Verify the payload starts with our prefix
    if query.invoice_payload.starts_with("premium_sub_") {
        bot.answer_pre_checkout_query(query.id.clone(), true).await?;
    } else {
        bot.answer_pre_checkout_query(query.id.clone(), false)
            .error_message("Unknown payment type")
            .await?;
    }
    Ok(())
}

/// Handle successful payment - activate subscription
pub async fn handle_successful_payment(
    bot: Bot,
    msg: Message,
    subscription_manager: Arc<SubscriptionManager>,
) -> HandlerResult {
    if let Some(payment) = msg.successful_payment() {
        // Extract user_id from payload
        if let Some(user_id_str) = payment.invoice_payload.strip_prefix("premium_sub_") {
            if let Ok(user_id) = user_id_str.parse::<i64>() {
                // Add subscription
                match subscription_manager
                    .add_subscription(user_id, SUBSCRIPTION_DAYS)
                    .await
                {
                    Ok(expires_at) => {
                        let text = format!(
                            "Спасибо за покупку!\n\n\
                            Premium-подписка активирована.\n\
                            Действует до: {}\n\n\
                            Теперь вам доступны:\n\
                            - Конвертация в кружочки\n\
                            - Конвертация в войсы",
                            expires_at.format("%d.%m.%Y %H:%M UTC")
                        );
                        bot.send_message(msg.chat.id, text).await?;
                    }
                    Err(e) => {
                        log::error!("Failed to activate subscription: {}", e);
                        bot.send_message(
                            msg.chat.id,
                            "Произошла ошибка при активации подписки. Обратитесь в поддержку.",
                        )
                        .await?;
                    }
                }
            }
        }
    }
    Ok(())
}
