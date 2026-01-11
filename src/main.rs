mod commands;
mod errors;
mod handlers;
pub mod queue;
mod schema;
mod utils;
mod video;

use teloxide::{dispatching::dialogue::InMemStorage, prelude::*};

use crate::{
    queue::TaskQueue,
    schema::{State, schema},
    utils::clear_dir,
};

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    pretty_env_logger::init();
    log::info!("Starting command bot...");

    let bot = Bot::from_env();

    // Initialize the task queue
    let task_queue = TaskQueue::new(bot.clone());
    log::info!("Task queue initialized");

    Dispatcher::builder(bot, schema())
        .dependencies(dptree::deps![InMemStorage::<State>::new(), task_queue])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    clear_dir("videos").await.unwrap();
    clear_dir("converted").await.unwrap();
}
