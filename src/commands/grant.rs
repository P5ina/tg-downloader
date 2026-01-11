use std::sync::Arc;

use teloxide::prelude::*;

use crate::{errors::HandlerResult, subscription::SubscriptionManager};

/// Get admin user ID from environment
fn get_admin_id() -> Option<i64> {
    std::env::var("ADMIN_ID")
        .ok()
        .and_then(|s| s.parse().ok())
}

/// Handle /grant command - admin only
/// Usage: /grant <user_id> <days>
pub async fn grant(
    bot: Bot,
    msg: Message,
    subscription_manager: Arc<SubscriptionManager>,
) -> HandlerResult {
    let from_user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);

    // Check if user is admin
    let admin_id = get_admin_id();
    if admin_id.is_none() || admin_id != Some(from_user_id) {
        // Silently ignore for non-admins
        return Ok(());
    }

    // Parse command arguments
    let text = msg.text().unwrap_or("");
    let parts: Vec<&str> = text.split_whitespace().collect();

    if parts.len() != 3 {
        bot.send_message(
            msg.chat.id,
            "Usage: /grant <user_id> <days>\nExample: /grant 578503618 30",
        )
        .await?;
        return Ok(());
    }

    let target_user_id: i64 = match parts[1].parse() {
        Ok(id) => id,
        Err(_) => {
            bot.send_message(msg.chat.id, "Invalid user_id. Must be a number.")
                .await?;
            return Ok(());
        }
    };

    let days: i64 = match parts[2].parse() {
        Ok(d) => d,
        Err(_) => {
            bot.send_message(msg.chat.id, "Invalid days. Must be a number.")
                .await?;
            return Ok(());
        }
    };

    // Grant subscription
    match subscription_manager
        .add_subscription(target_user_id, days)
        .await
    {
        Ok(expires_at) => {
            let text = format!(
                "Subscription granted!\n\nUser: {}\nDays: {}\nExpires: {}",
                target_user_id,
                days,
                expires_at.format("%d.%m.%Y %H:%M UTC")
            );
            bot.send_message(msg.chat.id, text).await?;
        }
        Err(e) => {
            bot.send_message(msg.chat.id, format!("Error: {}", e))
                .await?;
        }
    }

    Ok(())
}
