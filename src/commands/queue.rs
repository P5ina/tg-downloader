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
            let status = match &task.status {
                TaskStatus::Queued { position } => format!("⏳ #{}", position),
                TaskStatus::Processing => "🔄 обработка".to_string(),
                _ => continue,
            };
            response.push_str(&format!("• {} — {}\n", task.task_type, status));
        }
    }

    bot.send_message(msg.chat.id, response).await?;
    Ok(())
}
