mod commands;
pub mod db;
mod errors;
mod handlers;
mod migrations;
pub mod queue;
mod schema;
pub mod subscription;
mod utils;
mod video;

use std::sync::Arc;

use teloxide::{dispatching::dialogue::InMemStorage, prelude::*};

use crate::{
    db::TaskDb,
    queue::TaskQueue,
    schema::{State, schema},
    subscription::SubscriptionManager,
};

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    pretty_env_logger::init();
    log::info!("Starting command bot...");

    let bot = Bot::from_env();

    // Initialize the subscription manager
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:subscriptions.db?mode=rwc".to_string());
    let subscription_manager = Arc::new(
        SubscriptionManager::new(&database_url)
            .await
            .expect("Failed to initialize subscription manager"),
    );
    log::info!("Subscription manager initialized");

    // Initialize the task database and queue
    let task_db = TaskDb::new(subscription_manager.pool());
    let task_queue = TaskQueue::new(bot.clone(), task_db.clone()).await;
    log::info!("Task queue initialized");

    // Restore state after restart and notify affected users
    task_queue.restore_on_startup(&bot).await;

    // Clean up orphaned files (not referenced by any pending task)
    cleanup_orphaned_files(&task_db).await;

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
}

/// Clean up files that are not referenced by any pending task
async fn cleanup_orphaned_files(db: &TaskDb) {
    use std::collections::HashSet;
    use std::path::Path;
    use tokio::fs;

    let active_files: HashSet<String> = match db.get_active_filenames().await {
        Ok(files) => files
            .into_iter()
            .filter_map(|f| {
                // Extract just the filename for comparison
                Path::new(&f)
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
            })
            .collect(),
        Err(e) => {
            log::error!("Failed to get active filenames: {}", e);
            return;
        }
    };

    // Clean videos directory
    if let Ok(mut entries) = fs::read_dir("videos").await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_file() {
                let filename = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if !active_files.contains(&filename) {
                    if let Err(e) = fs::remove_file(&path).await {
                        log::warn!("Failed to remove orphaned file {:?}: {}", path, e);
                    } else {
                        log::info!("Removed orphaned file: {:?}", path);
                    }
                }
            }
        }
    }

    // Clean converted directory
    if let Ok(mut entries) = fs::read_dir("converted").await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_file() {
                let filename = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if !active_files.contains(&filename) {
                    if let Err(e) = fs::remove_file(&path).await {
                        log::warn!("Failed to remove orphaned file {:?}: {}", path, e);
                    } else {
                        log::info!("Removed orphaned file: {:?}", path);
                    }
                }
            }
        }
    }
}
