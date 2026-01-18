-- Initial database schema

CREATE TABLE IF NOT EXISTS subscriptions (
    user_id INTEGER PRIMARY KEY,
    expires_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    task_type TEXT NOT NULL,
    chat_id INTEGER NOT NULL,
    message_id INTEGER NOT NULL,
    unique_file_id TEXT NOT NULL,
    status TEXT NOT NULL,
    url TEXT,
    quality INTEGER,
    filename TEXT,
    thumbnail_path TEXT,
    format TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS pending_downloads (
    short_id TEXT PRIMARY KEY,
    url TEXT NOT NULL,
    chat_id INTEGER NOT NULL,
    message_id INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS pending_conversions (
    short_id TEXT PRIMARY KEY,
    filename TEXT NOT NULL,
    thumbnail_path TEXT,
    chat_id INTEGER NOT NULL,
    message_id INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);
