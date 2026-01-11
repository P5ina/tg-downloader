mod commands;
mod errors;
mod handlers;
pub mod queue;
mod schema;
pub mod subscription;
mod utils;
mod video;

use std::sync::Arc;

use teloxide::{dispatching::dialogue::InMemStorage, prelude::*};

use crate::{
    queue::TaskQueue,
    schema::{State, schema},
    subscription::SubscriptionManager,
    utils::clear_dir,
};

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    pretty_env_logger::init();
    log::info!("Starting command bot...");

    let bot = Bot::from_env();

    // Initialize the subscription manager
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:subscriptions.db".to_string());
    let subscription_manager = Arc::new(
        SubscriptionManager::new(&database_url)
            .await
            .expect("Failed to initialize subscription manager"),
    );
    log::info!("Subscription manager initialized");

    // Initialize the task queue
    let task_queue = TaskQueue::new(bot.clone());
    log::info!("Task queue initialized");

    Dispatcher::builder(bot, schema())
        .dependencies(dptree::deps![
            InMemStorage::<State>::new(),
            task_queue,
            subscription_manager
        ])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    clear_dir("videos").await.unwrap();
    clear_dir("converted").await.unwrap();
}
