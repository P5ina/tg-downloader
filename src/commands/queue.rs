use std::sync::Arc;

use teloxide::prelude::*;

use crate::{errors::HandlerResult, queue::TaskQueue};

pub async fn queue(bot: Bot, msg: Message, task_queue: Arc<TaskQueue>) -> HandlerResult {
    let pending = task_queue.pending_count();
    let user_tasks = task_queue.get_user_tasks(msg.chat.id).await;

    let mut response = String::new();

    // Global queue status
    if pending > 0 {
        response.push_str(&format!("📊 В очереди: {} задач\n\n", pending));
    } else {
        response.push_str("📊 Очередь пуста\n\n");
    }

    // User's tasks
    if user_tasks.is_empty() {
        response.push_str("У вас нет активных задач.");
    } else {
        response.push_str("Ваши задачи:\n");
        for task in user_tasks {
            let status_emoji = match &task.status {
                crate::queue::TaskStatus::Queued { position } => {
                    format!("⏳ В очереди (позиция: {})", position)
                }
                crate::queue::TaskStatus::Processing => "🔄 Обрабатывается".to_string(),
                crate::queue::TaskStatus::Completed => "✅ Завершено".to_string(),
                crate::queue::TaskStatus::Failed(e) => format!("❌ Ошибка: {}", e),
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
