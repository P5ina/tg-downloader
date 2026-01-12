use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use teloxide::prelude::*;
use teloxide::types::{ChatId, MessageId};
use tokio::sync::{mpsc, Mutex, Semaphore};

use crate::db::TaskDb;
use crate::utils::MediaFormatType;

/// Maximum number of concurrent tasks (downloads + conversions)
const MAX_CONCURRENT_TASKS: usize = 2;

/// Short ID for callback data (8 chars max)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ShortId(pub String);

impl ShortId {
    pub fn new() -> Self {
        // Use first 8 chars of UUID for short callback-safe ID
        Self(uuid::Uuid::new_v4().to_string()[..8].to_string())
    }
}

impl std::fmt::Display for ShortId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Pending download waiting for quality selection
#[derive(Debug, Clone)]
pub struct PendingDownload {
    pub url: String,
    pub chat_id: ChatId,
    pub message_id: MessageId,
}

/// Pending conversion waiting for format selection
#[derive(Debug, Clone)]
pub struct PendingConversion {
    pub filename: String,
    pub thumbnail_path: Option<String>,
    pub chat_id: ChatId,
    pub message_id: MessageId,
}

/// Unique task identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskId(pub String);

impl TaskId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn from_short(short: &ShortId) -> Self {
        Self(short.0.clone())
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Task types that can be queued
#[derive(Debug, Clone)]
pub enum TaskType {
    /// Download video from YouTube
    Download {
        url: String,
        quality: u32,
    },
    /// Convert downloaded video to specific format
    Convert {
        filename: String,
        thumbnail_path: Option<String>,
        format: MediaFormatType,
    },
}

/// A task in the queue
#[derive(Debug, Clone)]
pub struct Task {
    pub id: TaskId,
    pub task_type: TaskType,
    pub chat_id: ChatId,
    pub message_id: MessageId,
    pub unique_file_id: String,
}

/// Task status for tracking
#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    /// Waiting in queue
    Queued { position: usize },
    /// Currently being processed
    Processing,
    /// Completed successfully
    Completed,
    /// Failed with error
    Failed(String),
}

/// Information about a queued task for the user
#[derive(Debug, Clone)]
pub struct QueuedTaskInfo {
    pub task_id: TaskId,
    pub status: TaskStatus,
    pub task_type: String,
}

/// Global task queue manager
pub struct TaskQueue {
    /// Channel sender for submitting tasks
    sender: mpsc::UnboundedSender<Task>,
    /// Semaphore to limit concurrent tasks
    semaphore: Arc<Semaphore>,
    /// Track tasks per user for status queries
    user_tasks: Arc<Mutex<HashMap<ChatId, Vec<TaskId>>>>,
    /// Track task statuses
    task_statuses: Arc<Mutex<HashMap<TaskId, QueuedTaskInfo>>>,
    /// Number of tasks waiting in queue (not yet being processed)
    pending_count: Arc<AtomicUsize>,
    /// Pending downloads waiting for quality selection (short_id -> PendingDownload)
    pending_downloads: Arc<Mutex<HashMap<String, PendingDownload>>>,
    /// Pending conversions waiting for format selection (short_id -> PendingConversion)
    pending_conversions: Arc<Mutex<HashMap<String, PendingConversion>>>,
    /// Database for persistence
    db: TaskDb,
}

impl TaskQueue {
    /// Create a new task queue and start the worker
    pub async fn new(bot: Bot, db: TaskDb) -> Arc<Self> {
        let (sender, receiver) = mpsc::unbounded_channel();
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_TASKS));
        let user_tasks = Arc::new(Mutex::new(HashMap::new()));
        let task_statuses = Arc::new(Mutex::new(HashMap::new()));
        let pending_count = Arc::new(AtomicUsize::new(0));
        let pending_downloads = Arc::new(Mutex::new(HashMap::new()));
        let pending_conversions = Arc::new(Mutex::new(HashMap::new()));

        // Load pending data from database
        if let Ok(downloads) = db.get_all_pending_downloads().await {
            let mut pd = pending_downloads.lock().await;
            for row in downloads {
                pd.insert(
                    row.short_id,
                    PendingDownload {
                        url: row.url,
                        chat_id: ChatId(row.chat_id),
                        message_id: MessageId(row.message_id),
                    },
                );
            }
            log::info!("Loaded {} pending downloads from database", pd.len());
        }

        if let Ok(conversions) = db.get_all_pending_conversions().await {
            let mut pc = pending_conversions.lock().await;
            for row in conversions {
                pc.insert(
                    row.short_id,
                    PendingConversion {
                        filename: row.filename,
                        thumbnail_path: row.thumbnail_path,
                        chat_id: ChatId(row.chat_id),
                        message_id: MessageId(row.message_id),
                    },
                );
            }
            log::info!("Loaded {} pending conversions from database", pc.len());
        }

        let queue = Arc::new(Self {
            sender,
            semaphore,
            user_tasks,
            task_statuses,
            pending_count,
            pending_downloads,
            pending_conversions,
            db,
        });

        // Start the worker
        let queue_clone = queue.clone();
        tokio::spawn(async move {
            queue_clone.run_worker(receiver, bot).await;
        });

        queue
    }

    /// Store a pending download and return short ID for callback
    pub async fn add_pending_download(&self, url: String, chat_id: ChatId, message_id: MessageId) -> ShortId {
        let short_id = ShortId::new();
        let pending = PendingDownload {
            url: url.clone(),
            chat_id,
            message_id,
        };

        // Save to database
        if let Err(e) = self.db.insert_pending_download(
            &short_id.0,
            &url,
            chat_id.0,
            message_id.0,
        ).await {
            log::error!("Failed to save pending download to DB: {}", e);
        }

        let mut pending_downloads = self.pending_downloads.lock().await;
        pending_downloads.insert(short_id.0.clone(), pending);

        short_id
    }

    /// Get and remove a pending download by short ID
    pub async fn take_pending_download(&self, short_id: &str) -> Option<PendingDownload> {
        // Delete from database
        if let Err(e) = self.db.delete_pending_download(short_id).await {
            log::error!("Failed to delete pending download from DB: {}", e);
        }

        let mut pending_downloads = self.pending_downloads.lock().await;
        pending_downloads.remove(short_id)
    }

    /// Store a pending conversion and return short ID for callback
    pub async fn add_pending_conversion(&self, filename: String, thumbnail_path: Option<String>, chat_id: ChatId, message_id: MessageId) -> ShortId {
        let short_id = ShortId::new();
        let pending = PendingConversion {
            filename: filename.clone(),
            thumbnail_path: thumbnail_path.clone(),
            chat_id,
            message_id,
        };

        // Save to database
        if let Err(e) = self.db.insert_pending_conversion(
            &short_id.0,
            &filename,
            thumbnail_path.as_deref(),
            chat_id.0,
            message_id.0,
        ).await {
            log::error!("Failed to save pending conversion to DB: {}", e);
        }

        let mut pending_conversions = self.pending_conversions.lock().await;
        pending_conversions.insert(short_id.0.clone(), pending);

        short_id
    }

    /// Get and remove a pending conversion by short ID
    pub async fn take_pending_conversion(&self, short_id: &str) -> Option<PendingConversion> {
        // Delete from database
        if let Err(e) = self.db.delete_pending_conversion(short_id).await {
            log::error!("Failed to delete pending conversion from DB: {}", e);
        }

        let mut pending_conversions = self.pending_conversions.lock().await;
        pending_conversions.remove(short_id)
    }

    /// Submit a task to the queue
    pub async fn submit(&self, task: Task) -> Result<usize, String> {
        // Position is number of tasks already waiting + 1
        let position = self.pending_count.fetch_add(1, Ordering::SeqCst) + 1;

        // Save task to database
        let (task_type_str, url, quality, filename, thumbnail_path, format) = match &task.task_type {
            TaskType::Download { url, quality } => {
                ("download", Some(url.as_str()), Some(*quality as i32), None, None, None)
            }
            TaskType::Convert { filename, thumbnail_path, format } => {
                ("convert", None, None, Some(filename.as_str()), thumbnail_path.as_deref(), Some(format.to_string()))
            }
        };

        if let Err(e) = self.db.insert_task(
            &task.id.0,
            task_type_str,
            task.chat_id.0,
            task.message_id.0,
            &task.unique_file_id,
            "queued",
            url,
            quality,
            filename,
            thumbnail_path,
            format.as_deref(),
        ).await {
            log::error!("Failed to save task to DB: {}", e);
        }

        // Track task for user
        {
            let mut user_tasks = self.user_tasks.lock().await;
            user_tasks
                .entry(task.chat_id)
                .or_insert_with(Vec::new)
                .push(task.id.clone());
        }

        // Track task status
        {
            let mut statuses = self.task_statuses.lock().await;
            let task_type = match &task.task_type {
                TaskType::Download { quality, .. } => format!("📥 {}p", quality),
                TaskType::Convert { format, .. } => format!("{} {}", format.emoji(), format),
            };
            statuses.insert(
                task.id.clone(),
                QueuedTaskInfo {
                    task_id: task.id.clone(),
                    status: TaskStatus::Queued { position },
                    task_type,
                },
            );
        }

        self.sender
            .send(task)
            .map_err(|e| format!("Failed to submit task: {}", e))?;

        Ok(position)
    }

    /// Get number of tasks waiting in queue
    pub fn pending_count(&self) -> usize {
        self.pending_count.load(Ordering::SeqCst)
    }

    /// Get tasks for a user
    pub async fn get_user_tasks(&self, chat_id: ChatId) -> Vec<QueuedTaskInfo> {
        let user_tasks = self.user_tasks.lock().await;
        let statuses = self.task_statuses.lock().await;

        user_tasks
            .get(&chat_id)
            .map(|task_ids| {
                task_ids
                    .iter()
                    .filter_map(|id| statuses.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Restore state after bot restart and notify affected users
    pub async fn restore_on_startup(&self, bot: &Bot) {
        use strum::IntoEnumIterator;
        use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};
        use tokio::fs;

        log::info!("Starting restore_on_startup...");

        // 1. Cleanup expired data
        if let Err(e) = self.db.delete_expired_pending_downloads().await {
            log::error!("Failed to cleanup expired pending downloads: {}", e);
        }

        if let Ok(expired_files) = self.db.delete_expired_pending_conversions().await {
            for file in expired_files {
                let _ = fs::remove_file(&file).await;
            }
        }

        if let Ok(expired_files) = self.db.delete_expired_tasks().await {
            for file in expired_files {
                let _ = fs::remove_file(&file).await;
            }
        }

        // 2. Handle tasks that were processing when bot crashed
        if let Ok(tasks) = self.db.get_all_tasks().await {
            for task_row in tasks {
                if task_row.status == "processing" {
                    // Task was interrupted - notify user
                    let _ = bot
                        .send_message(
                            ChatId(task_row.chat_id),
                            "❌ Бот был перезапущен во время обработки вашего запроса. Пожалуйста, отправьте ссылку заново.",
                        )
                        .await;

                    // Delete task and associated file
                    if let Some(filename) = &task_row.filename {
                        let _ = fs::remove_file(filename).await;
                    }
                    if let Some(thumbnail) = &task_row.thumbnail_path {
                        let _ = fs::remove_file(thumbnail).await;
                    }
                    let _ = self.db.delete_task(&task_row.id).await;
                } else if task_row.status == "queued" {
                    // Task was in queue - we could restart it but for simplicity notify user
                    let _ = bot
                        .send_message(
                            ChatId(task_row.chat_id),
                            "⚠️ Бот был перезапущен. Ваш запрос из очереди был сброшен. Пожалуйста, отправьте ссылку заново.",
                        )
                        .await;

                    if let Some(filename) = &task_row.filename {
                        let _ = fs::remove_file(filename).await;
                    }
                    if let Some(thumbnail) = &task_row.thumbnail_path {
                        let _ = fs::remove_file(thumbnail).await;
                    }
                    let _ = self.db.delete_task(&task_row.id).await;
                }
            }
        }

        // 3. Handle pending downloads (user needs to send link again)
        let pending_downloads = self.pending_downloads.lock().await;
        for (_, pending) in pending_downloads.iter() {
            let _ = bot
                .send_message(
                    pending.chat_id,
                    "⚠️ Бот был перезапущен. Пожалуйста, отправьте ссылку заново.",
                )
                .await;
        }
        drop(pending_downloads);

        // Clear pending downloads from both memory and DB
        {
            let mut pd = self.pending_downloads.lock().await;
            for short_id in pd.keys().cloned().collect::<Vec<_>>() {
                let _ = self.db.delete_pending_download(&short_id).await;
            }
            pd.clear();
        }

        // 4. Handle pending conversions (file downloaded, waiting for format selection)
        let pending_conversions = self.pending_conversions.lock().await;
        let mut to_notify: Vec<(String, PendingConversion, bool)> = Vec::new();

        for (short_id, pending) in pending_conversions.iter() {
            let file_exists = fs::metadata(&pending.filename).await.is_ok();
            to_notify.push((short_id.clone(), pending.clone(), file_exists));
        }
        drop(pending_conversions);

        for (short_id, pending, file_exists) in to_notify {
            if file_exists {
                // File exists - show format selection again
                let formats: Vec<InlineKeyboardButton> = MediaFormatType::iter()
                    .enumerate()
                    .map(|(idx, f)| {
                        let label = format!("{}", f);
                        let callback = format!("fmt:{}:{}", idx, short_id);
                        InlineKeyboardButton::callback(label, callback)
                    })
                    .collect();

                let keyboard = InlineKeyboardMarkup::default()
                    .append_row([formats[0].clone(), formats[1].clone()])
                    .append_row([formats[2].clone(), formats[3].clone()]);

                let _ = bot
                    .send_message(
                        pending.chat_id,
                        "⚠️ Бот был перезапущен. Ваше видео сохранено. Выберите формат:",
                    )
                    .reply_markup(keyboard)
                    .await;
            } else {
                // File doesn't exist - notify user
                let _ = bot
                    .send_message(
                        pending.chat_id,
                        "❌ Бот был перезапущен и файл был потерян. Пожалуйста, отправьте ссылку заново.",
                    )
                    .await;

                // Remove from DB
                let _ = self.db.delete_pending_conversion(&short_id).await;

                // Remove from memory
                let mut pc = self.pending_conversions.lock().await;
                pc.remove(&short_id);
            }
        }

        log::info!("restore_on_startup completed");
    }

    /// Update task status (in-memory and database)
    async fn update_status(&self, task_id: &TaskId, status: TaskStatus) {
        // Update in-memory
        let mut statuses = self.task_statuses.lock().await;
        if let Some(info) = statuses.get_mut(task_id) {
            info.status = status.clone();
        }

        // Update in database
        let status_str = match status {
            TaskStatus::Queued { .. } => "queued",
            TaskStatus::Processing => "processing",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed(_) => "failed",
        };
        if let Err(e) = self.db.update_task_status(&task_id.0, status_str).await {
            log::error!("Failed to update task status in DB: {}", e);
        }
    }

    /// Main worker loop
    async fn run_worker(&self, mut receiver: mpsc::UnboundedReceiver<Task>, bot: Bot) {
        while let Some(task) = receiver.recv().await {
            let permit = self.semaphore.clone().acquire_owned().await.unwrap();
            self.pending_count.fetch_sub(1, Ordering::SeqCst);

            // Update status to processing
            self.update_status(&task.id, TaskStatus::Processing).await;

            let bot_clone = bot.clone();
            let task_id = task.id.clone();
            let task_statuses = self.task_statuses.clone();
            let user_tasks = self.user_tasks.clone();
            let pending_conversions = self.pending_conversions.clone();
            let db = self.db.clone();

            // Spawn task handler
            tokio::spawn(async move {
                log::info!("Processing task {}: {:?}", task_id, task.task_type);
                let result = process_task(&bot_clone, &task, &pending_conversions, &db).await;

                match &result {
                    Ok(_) => log::info!("Task {} completed successfully", task_id),
                    Err(e) => log::error!("Task {} failed: {}", task_id, e),
                }

                // Update status based on result
                {
                    let mut statuses = task_statuses.lock().await;
                    if let Some(info) = statuses.get_mut(&task_id) {
                        info.status = match &result {
                            Ok(_) => TaskStatus::Completed,
                            Err(e) => TaskStatus::Failed(e.clone()),
                        };
                    }
                }

                // Delete task from database (it's done)
                if let Err(e) = db.delete_task(&task_id.0).await {
                    log::error!("Failed to delete task from DB: {}", e);
                }

                // Clean up after a delay
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

                // Remove from in-memory tracking
                {
                    let mut user_tasks = user_tasks.lock().await;
                    if let Some(tasks) = user_tasks.get_mut(&task.chat_id) {
                        tasks.retain(|id| id != &task_id);
                    }
                }
                {
                    let mut statuses = task_statuses.lock().await;
                    statuses.remove(&task_id);
                }

                drop(permit);
            });
        }
    }
}

/// Process a single task
async fn process_task(
    bot: &Bot,
    task: &Task,
    pending_conversions: &Arc<Mutex<HashMap<String, PendingConversion>>>,
    db: &TaskDb,
) -> Result<(), String> {
    match &task.task_type {
        TaskType::Download { url, quality } => {
            process_download_task(bot, task, url, *quality, pending_conversions, db).await
        }
        TaskType::Convert { filename, thumbnail_path, format } => {
            process_convert_task(bot, task, filename, thumbnail_path.clone(), format.clone()).await
        }
    }
}

/// Process download task
async fn process_download_task(
    bot: &Bot,
    task: &Task,
    url: &str,
    quality: u32,
    pending_conversions: &Arc<Mutex<HashMap<String, PendingConversion>>>,
    db: &TaskDb,
) -> Result<(), String> {
    use crate::video::youtube::download_video;
    use strum::IntoEnumIterator;
    use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

    log::info!("Starting download task: {} at {}p", url, quality);

    // Update message to show downloading
    let _ = bot
        .edit_message_text(
            task.chat_id,
            task.message_id,
            format!("⏳ Скачиваем видео в {}p...", quality),
        )
        .await;

    match download_video(url, &task.unique_file_id, Some(quality)).await {
        Ok(result) => {
            log::info!("Downloaded file: {}", result.video_path);

            // Store pending conversion and get short ID
            let short_id = ShortId::new();

            // Save to database
            if let Err(e) = db.insert_pending_conversion(
                &short_id.0,
                &result.video_path,
                result.thumbnail_path.as_deref(),
                task.chat_id.0,
                task.message_id.0,
            ).await {
                log::error!("Failed to save pending conversion to DB: {}", e);
            }

            {
                let mut conversions = pending_conversions.lock().await;
                conversions.insert(
                    short_id.0.clone(),
                    PendingConversion {
                        filename: result.video_path.clone(),
                        thumbnail_path: result.thumbnail_path.clone(),
                        chat_id: task.chat_id,
                        message_id: task.message_id,
                    },
                );
            }

            // Show format selection with short callback: fmt:format_index:short_id
            let formats: Vec<InlineKeyboardButton> = MediaFormatType::iter()
                .enumerate()
                .map(|(idx, f)| {
                    let label = format!("{}", f);
                    let callback = format!("fmt:{}:{}", idx, short_id);
                    InlineKeyboardButton::callback(label, callback)
                })
                .collect();

            let keyboard = InlineKeyboardMarkup::default()
                .append_row([formats[0].clone(), formats[1].clone()])
                .append_row([formats[2].clone(), formats[3].clone()]);

            let _ = bot
                .edit_message_text(
                    task.chat_id,
                    task.message_id,
                    "Видео загружено. Теперь выбери формат:",
                )
                .reply_markup(keyboard)
                .await;

            Ok(())
        }
        Err(e) => {
            log::error!("Download error: {}", e);
            let _ = bot
                .edit_message_text(
                    task.chat_id,
                    task.message_id,
                    "❌ Не могу скачать это видео, попробуй другое.",
                )
                .await;
            Err(format!("Download failed: {}", e))
        }
    }
}

/// Process conversion task
async fn process_convert_task(
    bot: &Bot,
    task: &Task,
    filename: &str,
    thumbnail_path: Option<String>,
    format: MediaFormatType,
) -> Result<(), String> {
    use crate::video::convert::{convert_audio, convert_video_note};
    use crate::video::{VideoInfo, compress_video_with_progress, generate_thumbnail};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use teloxide::types::{InputFile, ParseMode};
    use teloxide::{ApiError, RequestError};
    use tokio::fs;
    use tokio::sync::mpsc;

    use crate::utils::loading_screen_with_progress;

    // For Video format, just send without conversion
    if format == MediaFormatType::Video {
        let _ = bot
            .edit_message_text(task.chat_id, task.message_id, "📤 Отправляем видео...")
            .await;

        let video_info = VideoInfo::from_file(filename)
            .await
            .map_err(|e| e.to_string())?;

        // Use YouTube thumbnail if available, otherwise generate one
        let thumbnail = if thumbnail_path.is_some() {
            thumbnail_path.clone()
        } else {
            generate_thumbnail(filename).await.ok()
        };

        let mut request = bot
            .send_video(task.chat_id, InputFile::file(filename))
            .width(video_info.width)
            .height(video_info.height)
            .duration(video_info.duration as u32)
            .supports_streaming(true);

        if let Some(ref thumb_path) = thumbnail {
            request = request.thumbnail(InputFile::file(thumb_path));
        }

        let result = request.await;

        // Clean up thumbnail
        if let Some(thumb_path) = thumbnail {
            let _ = fs::remove_file(&thumb_path).await;
        }

        match result {
            Ok(_) => {
                let _ = bot
                    .edit_message_text(
                        task.chat_id,
                        task.message_id,
                        "✅ Готово! Ваше видео отправлено!",
                    )
                    .await;
            }
            Err(RequestError::Api(ApiError::RequestEntityTooLarge)) => {
                // Try compression
                let _ = bot
                    .edit_message_text(
                        task.chat_id,
                        task.message_id,
                        "🔧 Видео слишком большое, сжимаем...",
                    )
                    .await;

                match compress_video_with_progress(filename, None).await {
                    Ok(compressed) => {
                        let video_info = VideoInfo::from_file(&compressed)
                            .await
                            .map_err(|e| e.to_string())?;

                        // Use original thumbnail or generate from compressed video
                        let thumb = if thumbnail_path.is_some() {
                            thumbnail_path.clone()
                        } else {
                            generate_thumbnail(&compressed).await.ok()
                        };

                        let mut request = bot
                            .send_video(task.chat_id, InputFile::file(&compressed))
                            .width(video_info.width)
                            .height(video_info.height)
                            .duration(video_info.duration as u32)
                            .supports_streaming(true);

                        if let Some(ref thumb_path) = thumb {
                            request = request.thumbnail(InputFile::file(thumb_path));
                        }

                        let send_result = request.await;

                        let _ = fs::remove_file(&compressed).await;
                        if let Some(thumb_path) = thumb {
                            let _ = fs::remove_file(&thumb_path).await;
                        }

                        match send_result {
                            Ok(_) => {
                                let _ = bot
                                    .edit_message_text(
                                        task.chat_id,
                                        task.message_id,
                                        "✅ Видео сжато и отправлено!",
                                    )
                                    .await;
                            }
                            Err(_) => {
                                let _ = bot
                                    .edit_message_text(
                                        task.chat_id,
                                        task.message_id,
                                        "❌ Не удалось отправить видео даже после сжатия.",
                                    )
                                    .await;
                            }
                        }
                    }
                    Err(_) => {
                        let _ = bot
                            .edit_message_text(
                                task.chat_id,
                                task.message_id,
                                "❌ Не удалось сжать видео.",
                            )
                            .await;
                    }
                }
            }
            Err(e) => {
                let _ = fs::remove_file(filename).await;
                return Err(format!("Send error: {}", e));
            }
        }

        let _ = fs::remove_file(filename).await;
        return Ok(());
    }

    // For other formats, need conversion
    let _ = bot
        .edit_message_text(
            task.chat_id,
            task.message_id,
            "🚀 Начинаем конвертацию...",
        )
        .await;

    // Start loading screen
    let should_stop_loading = Arc::new(AtomicBool::new(false));
    let (_progress_tx, progress_rx) = mpsc::unbounded_channel();
    let loading_task = {
        let bot_clone = bot.clone();
        let should_stop_clone = should_stop_loading.clone();
        let chat_id = task.chat_id;
        let message_id = task.message_id;
        tokio::spawn(async move {
            loading_screen_with_progress(
                bot_clone,
                chat_id,
                message_id,
                should_stop_clone,
                progress_rx,
            )
            .await;
        })
    };

    let conversion_result = match format {
        MediaFormatType::Video => Ok(filename.to_string()),
        MediaFormatType::VideoNote => {
            let _ = bot
                .send_message(
                    task.chat_id,
                    "<b>⚠️ Внимание</b> кружочек будет обрезан до 1 минуты.",
                )
                .parse_mode(ParseMode::Html)
                .await;
            convert_video_note(filename).await
        }
        MediaFormatType::Audio | MediaFormatType::Voice => convert_audio(filename).await,
    };

    // Stop loading
    should_stop_loading.store(true, Ordering::Relaxed);
    loading_task.abort();

    match conversion_result {
        Ok(converted_file) => {
            let send_result = match format {
                MediaFormatType::Video => {
                    let video_info = VideoInfo::from_file(&converted_file)
                        .await
                        .map_err(|e| e.to_string())?;

                    // Use original thumbnail or generate from converted video
                    let thumb = if thumbnail_path.is_some() {
                        thumbnail_path.clone()
                    } else {
                        generate_thumbnail(&converted_file).await.ok()
                    };

                    let mut request = bot
                        .send_video(task.chat_id, InputFile::file(&converted_file))
                        .width(video_info.width)
                        .height(video_info.height)
                        .duration(video_info.duration as u32)
                        .supports_streaming(true);

                    if let Some(ref thumb_path) = thumb {
                        request = request.thumbnail(InputFile::file(thumb_path));
                    }

                    let result = request.await.map(|_| ());

                    // Clean up thumbnail
                    if let Some(thumb_path) = thumb {
                        let _ = fs::remove_file(&thumb_path).await;
                    }

                    result
                }
                MediaFormatType::Audio => bot
                    .send_audio(task.chat_id, InputFile::file(&converted_file))
                    .await
                    .map(|_| ()),
                MediaFormatType::VideoNote => bot
                    .send_video_note(task.chat_id, InputFile::file(&converted_file))
                    .await
                    .map(|_| ()),
                MediaFormatType::Voice => bot
                    .send_voice(task.chat_id, InputFile::file(&converted_file))
                    .await
                    .map(|_| ()),
            };

            match send_result {
                Ok(_) => {
                    let _ = bot
                        .edit_message_text(
                            task.chat_id,
                            task.message_id,
                            "✅ Готово! Файл отправлен!",
                        )
                        .await;
                }
                Err(RequestError::Api(ApiError::RequestEntityTooLarge)) => {
                    let _ = bot
                        .edit_message_text(
                            task.chat_id,
                            task.message_id,
                            "❌ Файл слишком большой для отправки.",
                        )
                        .await;
                }
                Err(e) => {
                    let _ = bot
                        .edit_message_text(
                            task.chat_id,
                            task.message_id,
                            format!("❌ Ошибка отправки: {}", e),
                        )
                        .await;
                }
            }

            // Cleanup
            if converted_file != filename {
                let _ = fs::remove_file(&converted_file).await;
            }
            let _ = fs::remove_file(filename).await;

            Ok(())
        }
        Err(e) => {
            let _ = bot
                .edit_message_text(
                    task.chat_id,
                    task.message_id,
                    "❌ Ошибка конвертации. Попробуйте другой формат.",
                )
                .await;
            let _ = fs::remove_file(filename).await;
            Err(format!("Conversion error: {}", e))
        }
    }
}
