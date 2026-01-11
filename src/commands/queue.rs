use std::sync::Arc;

use teloxide::prelude::*;

use crate::{errors::HandlerResult, queue::{TaskQueue, TaskStatus}};

pub async fn queue(bot: Bot, msg: Message, task_queue: Arc<TaskQueue>) -> HandlerResult {
    let pending = task_queue.pending_count();
    let user_tasks = task_queue.get_user_tasks(msg.chat.id).await;

    // Filter only active tasks (queued or processing)
    let active_tasks: Vec<_> = user_tasks
        .into_iter()
        .filter(|t| matches!(t.status, TaskStatus::Queued { .. } | TaskStatus::Processing))
        .collect();

    let mut response = String::new();

    // Global queue status
    if pending > 0 {
        response.push_str(&format!("📊 В очереди: {} задач\n\n", pending));
    } else {
        response.push_str("📊 Очередь пуста\n\n");
    }

    // User's active tasks
    if active_tasks.is_empty() {
        response.push_str("У вас нет активных задач.");
    } else {
        response.push_str("Ваши задачи:\n");
        for task in active_tasks {
            let status_emoji = match &task.status {
                TaskStatus::Queued { .. } => "⏳ Ожидает".to_string(),
                TaskStatus::Processing => "🔄 Обрабатывается".to_string(),
                _ => continue,
            };

            let task_type = if task.task_type.starts_with("download") {
                "Скачивание"
            } else if task.task_type.starts_with("convert") {
                "Конвертация"
            } else {
                &task.task_type
            };

            response.push_str(&format!("• {} - {}\n", task_type, status_emoji));
        }
    }

    bot.send_message(msg.chat.id, response).await?;
    Ok(())
}
