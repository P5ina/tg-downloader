mod commands;
mod errors;
mod handlers;
mod schema;
mod utils;
mod video;

use teloxide::{dispatching::dialogue::InMemStorage, prelude::*};

use crate::{
    schema::{State, schema},
    utils::clear_dir,
};

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    pretty_env_logger::init();
    log::info!("Starting command bot...");

    let bot = Bot::from_env();

    Dispatcher::builder(bot, schema())
        .dependencies(dptree::deps![InMemStorage::<State>::new()])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    clear_dir("videos").await.unwrap();
    clear_dir("converted").await.unwrap();
}
