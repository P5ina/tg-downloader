//! Database migrations using sqlx built-in migration system.
//!
//! Migrations are stored in the `migrations/` directory.
//! Each migration file is named `NNNN_description.sql`.

use sqlx::migrate::Migrator;
use sqlx::SqlitePool;

use crate::errors::{BotError, BotResult};

// Embed migrations at compile time
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Run all pending migrations using sqlx migrate
pub async fn run_migrations(pool: &SqlitePool) -> BotResult<()> {
    // Handle legacy databases that existed before sqlx migrations
    handle_legacy_database(pool).await?;

    // Run sqlx migrations
    MIGRATOR
        .run(pool)
        .await
        .map_err(|e| BotError::general(format!("Failed to run migrations: {}", e)))?;

    log::info!("Database migrations completed successfully");
    Ok(())
}

/// Handle legacy databases that were created before the migration system.
/// This ensures existing databases with data are properly migrated.
async fn handle_legacy_database(pool: &SqlitePool) -> BotResult<()> {
    // Check if this is a legacy database (has tables but no _sqlx_migrations table)
    let has_subscriptions = table_exists(pool, "subscriptions").await;
    let has_sqlx_migrations = table_exists(pool, "_sqlx_migrations").await;

    if has_subscriptions && !has_sqlx_migrations {
        log::info!("Detected legacy database. Preparing for migration...");

        // Create the sqlx migrations table manually
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS _sqlx_migrations (
                version BIGINT PRIMARY KEY,
                description TEXT NOT NULL,
                installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                success BOOLEAN NOT NULL,
                checksum BLOB NOT NULL,
                execution_time BIGINT NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| BotError::general(format!("Failed to create migrations table: {}", e)))?;

        // Get checksums from the compiled migrator
        let migrations: Vec<_> = MIGRATOR.iter().collect();

        // Mark migration 0001 as already applied (tables exist)
        if let Some(m) = migrations.get(0) {
            sqlx::query(
                "INSERT OR IGNORE INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (?, ?, ?, ?, ?)"
            )
            .bind(m.version)
            .bind(m.description.as_ref())
            .bind(true)
            .bind(m.checksum.as_ref())
            .bind(0i64)
            .execute(pool)
            .await
            .map_err(|e| BotError::general(format!("Failed to mark migration 0001: {}", e)))?;

            log::info!("Marked migration {} as applied (legacy database)", m.description);
        }

        // Check if format column already exists in pending_downloads
        if column_exists(pool, "pending_downloads", "format").await {
            if let Some(m) = migrations.get(1) {
                sqlx::query(
                    "INSERT OR IGNORE INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (?, ?, ?, ?, ?)"
                )
                .bind(m.version)
                .bind(m.description.as_ref())
                .bind(true)
                .bind(m.checksum.as_ref())
                .bind(0i64)
                .execute(pool)
                .await
                .map_err(|e| BotError::general(format!("Failed to mark migration 0002: {}", e)))?;

                log::info!("Marked migration {} as applied (column exists)", m.description);
            }
        }
    }

    Ok(())
}

/// Check if a table exists in the database
async fn table_exists(pool: &SqlitePool, table: &str) -> bool {
    let result = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?"
    )
    .bind(table)
    .fetch_one(pool)
    .await;

    matches!(result, Ok(count) if count > 0)
}

/// Check if a column exists in a table
async fn column_exists(pool: &SqlitePool, table: &str, column: &str) -> bool {
    let sql = format!("SELECT COUNT(*) FROM pragma_table_info('{}') WHERE name = ?", table);
    let result = sqlx::query_scalar::<_, i64>(&sql)
        .bind(column)
        .fetch_one(pool)
        .await;

    matches!(result, Ok(count) if count > 0)
}
