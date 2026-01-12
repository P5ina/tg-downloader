//! Database layer for task queue persistence.
//! Works with raw SQL and primitive types only.

use std::sync::Arc;

use chrono::Utc;
use sqlx::{Row, SqlitePool};

/// TTL for pending tasks in seconds (24 hours)
const TASK_TTL_SECONDS: i64 = 24 * 60 * 60;

/// Raw pending download row from database
#[derive(Debug, Clone)]
pub struct PendingDownloadRow {
    pub short_id: String,
    pub url: String,
    pub chat_id: i64,
    pub message_id: i32,
}

/// Raw pending conversion row from database
#[derive(Debug, Clone)]
pub struct PendingConversionRow {
    pub short_id: String,
    pub filename: String,
    pub thumbnail_path: Option<String>,
    pub chat_id: i64,
    pub message_id: i32,
}

/// Raw task row from database
#[derive(Debug, Clone)]
pub struct TaskRow {
    pub id: String,
    pub task_type: String,
    pub chat_id: i64,
    pub message_id: i32,
    pub unique_file_id: String,
    pub status: String,
    pub url: Option<String>,
    pub quality: Option<i32>,
    pub filename: Option<String>,
    pub thumbnail_path: Option<String>,
    pub format: Option<String>,
}

/// Database operations for task queue persistence
#[derive(Clone)]
pub struct TaskDb {
    pool: Arc<SqlitePool>,
}

impl TaskDb {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        Self { pool }
    }

    // ==================== Pending Downloads ====================

    pub async fn insert_pending_download(
        &self,
        short_id: &str,
        url: &str,
        chat_id: i64,
        message_id: i32,
    ) -> Result<(), String> {
        let now = Utc::now().timestamp();

        sqlx::query(
            "INSERT INTO pending_downloads (short_id, url, chat_id, message_id, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(short_id)
        .bind(url)
        .bind(chat_id)
        .bind(message_id)
        .bind(now)
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| format!("Failed to insert pending download: {}", e))?;

        Ok(())
    }

    pub async fn delete_pending_download(&self, short_id: &str) -> Result<(), String> {
        sqlx::query("DELETE FROM pending_downloads WHERE short_id = ?")
            .bind(short_id)
            .execute(self.pool.as_ref())
            .await
            .map_err(|e| format!("Failed to delete pending download: {}", e))?;

        Ok(())
    }

    pub async fn get_all_pending_downloads(&self) -> Result<Vec<PendingDownloadRow>, String> {
        let cutoff = Utc::now().timestamp() - TASK_TTL_SECONDS;

        let rows = sqlx::query(
            "SELECT short_id, url, chat_id, message_id FROM pending_downloads WHERE created_at > ?",
        )
        .bind(cutoff)
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(|e| format!("Failed to load pending downloads: {}", e))?;

        Ok(rows
            .iter()
            .map(|row| PendingDownloadRow {
                short_id: row.get("short_id"),
                url: row.get("url"),
                chat_id: row.get("chat_id"),
                message_id: row.get("message_id"),
            })
            .collect())
    }

    pub async fn delete_expired_pending_downloads(&self) -> Result<usize, String> {
        let cutoff = Utc::now().timestamp() - TASK_TTL_SECONDS;

        let result = sqlx::query("DELETE FROM pending_downloads WHERE created_at <= ?")
            .bind(cutoff)
            .execute(self.pool.as_ref())
            .await
            .map_err(|e| format!("Failed to cleanup expired pending downloads: {}", e))?;

        Ok(result.rows_affected() as usize)
    }

    // ==================== Pending Conversions ====================

    pub async fn insert_pending_conversion(
        &self,
        short_id: &str,
        filename: &str,
        thumbnail_path: Option<&str>,
        chat_id: i64,
        message_id: i32,
    ) -> Result<(), String> {
        let now = Utc::now().timestamp();

        sqlx::query(
            "INSERT INTO pending_conversions (short_id, filename, thumbnail_path, chat_id, message_id, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(short_id)
        .bind(filename)
        .bind(thumbnail_path)
        .bind(chat_id)
        .bind(message_id)
        .bind(now)
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| format!("Failed to insert pending conversion: {}", e))?;

        Ok(())
    }

    pub async fn delete_pending_conversion(&self, short_id: &str) -> Result<(), String> {
        sqlx::query("DELETE FROM pending_conversions WHERE short_id = ?")
            .bind(short_id)
            .execute(self.pool.as_ref())
            .await
            .map_err(|e| format!("Failed to delete pending conversion: {}", e))?;

        Ok(())
    }

    pub async fn get_all_pending_conversions(&self) -> Result<Vec<PendingConversionRow>, String> {
        let cutoff = Utc::now().timestamp() - TASK_TTL_SECONDS;

        let rows = sqlx::query(
            "SELECT short_id, filename, thumbnail_path, chat_id, message_id FROM pending_conversions WHERE created_at > ?",
        )
        .bind(cutoff)
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(|e| format!("Failed to load pending conversions: {}", e))?;

        Ok(rows
            .iter()
            .map(|row| PendingConversionRow {
                short_id: row.get("short_id"),
                filename: row.get("filename"),
                thumbnail_path: row.get("thumbnail_path"),
                chat_id: row.get("chat_id"),
                message_id: row.get("message_id"),
            })
            .collect())
    }

    /// Returns filenames of expired conversions for cleanup
    pub async fn delete_expired_pending_conversions(&self) -> Result<Vec<String>, String> {
        let cutoff = Utc::now().timestamp() - TASK_TTL_SECONDS;

        // Get filenames before deletion
        let rows = sqlx::query(
            "SELECT filename, thumbnail_path FROM pending_conversions WHERE created_at <= ?",
        )
        .bind(cutoff)
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(|e| format!("Failed to get expired conversions: {}", e))?;

        let mut files: Vec<String> = rows
            .iter()
            .map(|row| row.get::<String, _>("filename"))
            .collect();

        for row in &rows {
            if let Some(thumb) = row.get::<Option<String>, _>("thumbnail_path") {
                files.push(thumb);
            }
        }

        // Delete from database
        sqlx::query("DELETE FROM pending_conversions WHERE created_at <= ?")
            .bind(cutoff)
            .execute(self.pool.as_ref())
            .await
            .map_err(|e| format!("Failed to cleanup expired pending conversions: {}", e))?;

        Ok(files)
    }

    // ==================== Tasks ====================

    pub async fn insert_task(
        &self,
        id: &str,
        task_type: &str,
        chat_id: i64,
        message_id: i32,
        unique_file_id: &str,
        status: &str,
        url: Option<&str>,
        quality: Option<i32>,
        filename: Option<&str>,
        thumbnail_path: Option<&str>,
        format: Option<&str>,
    ) -> Result<(), String> {
        let now = Utc::now().timestamp();

        sqlx::query(
            r#"
            INSERT INTO tasks (id, task_type, chat_id, message_id, unique_file_id, status, url, quality, filename, thumbnail_path, format, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET status = excluded.status
            "#,
        )
        .bind(id)
        .bind(task_type)
        .bind(chat_id)
        .bind(message_id)
        .bind(unique_file_id)
        .bind(status)
        .bind(url)
        .bind(quality)
        .bind(filename)
        .bind(thumbnail_path)
        .bind(format)
        .bind(now)
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| format!("Failed to insert task: {}", e))?;

        Ok(())
    }

    pub async fn update_task_status(&self, task_id: &str, status: &str) -> Result<(), String> {
        sqlx::query("UPDATE tasks SET status = ? WHERE id = ?")
            .bind(status)
            .bind(task_id)
            .execute(self.pool.as_ref())
            .await
            .map_err(|e| format!("Failed to update task status: {}", e))?;

        Ok(())
    }

    pub async fn delete_task(&self, task_id: &str) -> Result<(), String> {
        sqlx::query("DELETE FROM tasks WHERE id = ?")
            .bind(task_id)
            .execute(self.pool.as_ref())
            .await
            .map_err(|e| format!("Failed to delete task: {}", e))?;

        Ok(())
    }

    pub async fn get_all_tasks(&self) -> Result<Vec<TaskRow>, String> {
        let cutoff = Utc::now().timestamp() - TASK_TTL_SECONDS;

        let rows = sqlx::query(
            r#"
            SELECT id, task_type, chat_id, message_id, unique_file_id, status, url, quality, filename, thumbnail_path, format
            FROM tasks
            WHERE created_at > ?
            "#,
        )
        .bind(cutoff)
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(|e| format!("Failed to load tasks: {}", e))?;

        Ok(rows
            .iter()
            .map(|row| TaskRow {
                id: row.get("id"),
                task_type: row.get("task_type"),
                chat_id: row.get("chat_id"),
                message_id: row.get("message_id"),
                unique_file_id: row.get("unique_file_id"),
                status: row.get("status"),
                url: row.get("url"),
                quality: row.get("quality"),
                filename: row.get("filename"),
                thumbnail_path: row.get("thumbnail_path"),
                format: row.get("format"),
            })
            .collect())
    }

    /// Returns filenames of expired tasks for cleanup
    pub async fn delete_expired_tasks(&self) -> Result<Vec<String>, String> {
        let cutoff = Utc::now().timestamp() - TASK_TTL_SECONDS;

        // Get filenames from convert tasks
        let rows = sqlx::query(
            "SELECT filename, thumbnail_path FROM tasks WHERE created_at <= ? AND task_type = 'convert'",
        )
        .bind(cutoff)
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(|e| format!("Failed to get expired tasks: {}", e))?;

        let mut files: Vec<String> = rows
            .iter()
            .filter_map(|row| row.get::<Option<String>, _>("filename"))
            .collect();

        for row in &rows {
            if let Some(thumb) = row.get::<Option<String>, _>("thumbnail_path") {
                files.push(thumb);
            }
        }

        // Delete expired tasks
        sqlx::query("DELETE FROM tasks WHERE created_at <= ?")
            .bind(cutoff)
            .execute(self.pool.as_ref())
            .await
            .map_err(|e| format!("Failed to cleanup expired tasks: {}", e))?;

        Ok(files)
    }

    /// Get all filenames currently in use (to prevent deletion)
    pub async fn get_active_filenames(&self) -> Result<Vec<String>, String> {
        let mut filenames = Vec::new();

        // From pending_conversions
        let rows = sqlx::query("SELECT filename, thumbnail_path FROM pending_conversions")
            .fetch_all(self.pool.as_ref())
            .await
            .map_err(|e| format!("Failed to get pending conversion filenames: {}", e))?;

        for row in rows {
            filenames.push(row.get::<String, _>("filename"));
            if let Some(thumb) = row.get::<Option<String>, _>("thumbnail_path") {
                filenames.push(thumb);
            }
        }

        // From convert tasks
        let rows =
            sqlx::query("SELECT filename, thumbnail_path FROM tasks WHERE task_type = 'convert'")
                .fetch_all(self.pool.as_ref())
                .await
                .map_err(|e| format!("Failed to get task filenames: {}", e))?;

        for row in rows {
            if let Some(filename) = row.get::<Option<String>, _>("filename") {
                filenames.push(filename);
            }
            if let Some(thumb) = row.get::<Option<String>, _>("thumbnail_path") {
                filenames.push(thumb);
            }
        }

        Ok(filenames)
    }
}
