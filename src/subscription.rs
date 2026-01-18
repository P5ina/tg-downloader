use chrono::{DateTime, Duration, Utc};
use sqlx::{Row, SqlitePool};
use std::sync::Arc;

use crate::errors::{BotError, BotResult};
use crate::migrations;

/// Subscription manager handles premium subscriptions storage
#[derive(Clone)]
pub struct SubscriptionManager {
    pool: Arc<SqlitePool>,
}

impl SubscriptionManager {
    /// Create a new subscription manager and initialize the database
    pub async fn new(database_url: &str) -> BotResult<Self> {
        let pool = SqlitePool::connect(database_url)
            .await
            .map_err(|e| BotError::general(format!("Failed to connect to database: {}", e)))?;

        // Run database migrations
        migrations::run_migrations(&pool).await?;

        Ok(Self {
            pool: Arc::new(pool),
        })
    }

    /// Get the database pool for sharing with other components
    pub fn pool(&self) -> Arc<SqlitePool> {
        self.pool.clone()
    }

    /// Check if a user has an active subscription
    pub async fn is_subscribed(&self, user_id: i64) -> bool {
        let now = Utc::now().timestamp();

        let result = sqlx::query("SELECT expires_at FROM subscriptions WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(self.pool.as_ref())
            .await;

        match result {
            Ok(Some(row)) => {
                let expires_at: i64 = row.get("expires_at");
                expires_at > now
            }
            _ => false,
        }
    }

    /// Add or extend subscription for a user
    pub async fn add_subscription(&self, user_id: i64, days: i64) -> BotResult<DateTime<Utc>> {
        let now = Utc::now();

        // Get current expiration or use now as base
        let base_time = if let Some(current_expires) = self.get_expiration(user_id).await {
            if current_expires > now {
                current_expires // Extend from current expiration
            } else {
                now // Expired, start from now
            }
        } else {
            now // No subscription, start from now
        };

        let new_expires = base_time + Duration::days(days);
        let expires_timestamp = new_expires.timestamp();

        sqlx::query(
            r#"
            INSERT INTO subscriptions (user_id, expires_at) VALUES (?, ?)
            ON CONFLICT(user_id) DO UPDATE SET expires_at = ?
            "#,
        )
        .bind(user_id)
        .bind(expires_timestamp)
        .bind(expires_timestamp)
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| BotError::general(format!("Failed to add subscription: {}", e)))?;

        log::info!(
            "Subscription added for user {}: expires at {}",
            user_id,
            new_expires
        );

        Ok(new_expires)
    }

    /// Get subscription expiration date for a user
    pub async fn get_expiration(&self, user_id: i64) -> Option<DateTime<Utc>> {
        let result = sqlx::query("SELECT expires_at FROM subscriptions WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(self.pool.as_ref())
            .await;

        match result {
            Ok(Some(row)) => {
                let expires_at: i64 = row.get("expires_at");
                DateTime::from_timestamp(expires_at, 0)
            }
            _ => None,
        }
    }

    /// Get subscription info for display
    pub async fn get_subscription_info(&self, user_id: i64) -> SubscriptionInfo {
        let now = Utc::now();

        if let Some(expires_at) = self.get_expiration(user_id).await {
            if expires_at > now {
                let days_left = (expires_at - now).num_days();
                SubscriptionInfo::Active {
                    expires_at,
                    days_left,
                }
            } else {
                SubscriptionInfo::Expired { expired_at: expires_at }
            }
        } else {
            SubscriptionInfo::None
        }
    }
}

#[derive(Debug)]
pub enum SubscriptionInfo {
    Active {
        expires_at: DateTime<Utc>,
        days_left: i64,
    },
    Expired {
        expired_at: DateTime<Utc>,
    },
    None,
}

/// Premium features configuration
pub mod premium {
    use crate::utils::MediaFormatType;

    /// Check if a media format requires premium subscription
    pub fn is_premium_format(format: &MediaFormatType) -> bool {
        matches!(format, MediaFormatType::VideoNote | MediaFormatType::Voice)
    }

    /// Price in Telegram Stars for subscription
    pub const SUBSCRIPTION_PRICE_STARS: i32 = 50;

    /// Subscription duration in days
    pub const SUBSCRIPTION_DAYS: i64 = 30;

    /// Payload prefix for identifying our payments
    pub const PAYMENT_PAYLOAD_PREFIX: &str = "premium_sub_";
}
