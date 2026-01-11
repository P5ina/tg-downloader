use std::sync::Arc;

use teloxide::prelude::*;

use crate::{errors::HandlerResult, queue::{TaskQueue, TaskStatus}};

/// Generate a progress bar string
fn progress_bar(progress: Option<u8>, width: usize) -> String {
    match progress {
        Some(p) => {
            let filled = (p as usize * width) / 100;
            let empty = width - filled;
            format!("{}{} {}%", "▓".repeat(filled), "░".repeat(empty), p)
        }
        None => format!("{} ожидает", "░".repeat(width))
    }
}

pub async fn queue(bot: Bot, msg: Message, task_queue: Arc<TaskQueue>) -> HandlerResult {
    let pending = task_queue.pending_count();
    let user_tasks = task_queue.get_user_tasks(msg.chat.id).await;

    // Filter only active tasks (queued or processing)
    let active_tasks: Vec<_> = user_tasks
        .into_iter()
        .filter(|t| matches!(t.status, TaskStatus::Queued { .. } | TaskStatus::Processing))
        .collect();

    let mut response = String::new();

    // Global queue status - compact header
    response.push_str(&format!("📊 Очередь ({})\n\n", pending));

    // User's active tasks with progress bars
    if active_tasks.is_empty() {
        response.push_str("У вас нет активных задач.");
    } else {
        for task in active_tasks {
            let emoji = task.description.emoji();
            let label = task.description.to_string();

            let progress_display = match &task.status {
                TaskStatus::Processing => progress_bar(task.progress.or(Some(0)), 10),
                TaskStatus::Queued { position } => format!("{} #{}", "░".repeat(10), position),
                _ => continue,
            };

            response.push_str(&format!("{} {} {}\n", emoji, label, progress_display));
        }
    }

    bot.send_message(msg.chat.id, response).await?;
    Ok(())
}
