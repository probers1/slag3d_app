use rusqlite::{Connection, Result};

pub const DEFAULT_USER_ID: &str = "00000000-0000-0000-0000-000000000000";

pub fn init_schema(conn: &Connection) -> Result<()> {
    // Enable pragmas
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

    // Schema versions
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_versions (
            id INTEGER PRIMARY KEY CHECK(id = 1),
            version INTEGER NOT NULL UNIQUE,
            applied_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );",
    )?;

    // Users table (needed for FKs, minimal)
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'user' CHECK(role IN ('user','admin')),
            is_deleted INTEGER NOT NULL DEFAULT 0,
            deleted_at TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );",
    )?;

    // Models
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS models (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            title TEXT NOT NULL,
            description TEXT DEFAULT '',
            thumbnail_path TEXT,
            thumbnail_status TEXT NOT NULL CHECK(thumbnail_status IN ('pending','processing','completed','failed')) DEFAULT 'pending',
            width INTEGER NOT NULL DEFAULT 800,
            height INTEGER NOT NULL DEFAULT 600,
            is_deleted INTEGER NOT NULL DEFAULT 0,
            deleted_at TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );",
    )?;

    // Files
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS files (
            id TEXT PRIMARY KEY,
            model_id TEXT NOT NULL,
            filename TEXT NOT NULL,
            original_name TEXT NOT NULL,
            mime_type TEXT NOT NULL,
            file_size INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            width INTEGER NOT NULL DEFAULT 800,
            height INTEGER NOT NULL DEFAULT 600,
            is_primary_3d INTEGER NOT NULL DEFAULT 0 CHECK(is_primary_3d IN (0,1)),
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );",
    )?;

    // Tags
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS tags (
            id TEXT PRIMARY KEY,
            name TEXT UNIQUE NOT NULL COLLATE NOCASE,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );",
    )?;

    // Model tags
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS model_tags (
            model_id TEXT NOT NULL,
            tag_id TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (model_id, tag_id)
        );",
    )?;

    // Favorites
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS favorites (
            user_id TEXT NOT NULL,
            model_id TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (user_id, model_id)
        );",
    )?;

    // Collections
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS collections (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT DEFAULT '',
            parent_folder_id TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );",
    )?;

    // Collection models
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS collection_models (
            collection_id TEXT NOT NULL,
            model_id TEXT NOT NULL,
            added_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (collection_id, model_id)
        );",
    )?;

    // Indexes
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_models_user_id ON models(user_id);
         CREATE INDEX IF NOT EXISTS idx_models_title ON models(title);
         CREATE INDEX IF NOT EXISTS idx_models_created_at ON models(created_at);
         CREATE INDEX IF NOT EXISTS idx_files_model_id ON files(model_id);
         CREATE INDEX IF NOT EXISTS idx_files_is_primary ON files(is_primary_3d);
         CREATE INDEX IF NOT EXISTS idx_collections_user_id ON collections(user_id);
         CREATE INDEX IF NOT EXISTS idx_collections_name ON collections(name);
         CREATE INDEX IF NOT EXISTS idx_collections_parent ON collections(parent_folder_id);",
    )?;

    // Migration v5: ensure default user exists
    conn.execute(
        "INSERT OR IGNORE INTO users (id, username, password_hash, role) VALUES (?1, ?2, ?3, ?4)",
        [&DEFAULT_USER_ID, "", "", "user"],
    )?;

    // Migration v5b: the server's v4 migration only added `is_deleted` to the
    // `users` table; the Tauri app filters `models.is_deleted = 0` in every
    // query, so a v4 database migrated to Tauri would crash on first load.
    // Add the column (and `deleted_at`) idempotently so an existing v4 vault
    // upgrades in place without losing data.
    add_soft_delete_columns(conn, "models")?;
    add_soft_delete_columns(conn, "files")?;
    add_soft_delete_columns(conn, "collections")?;

    // Set schema version to 5 (upsert)
    conn.execute_batch(
        "INSERT INTO schema_versions (id, version) VALUES (1, 5)
         ON CONFLICT(id) DO UPDATE SET version = 5;",
    )?;

    Ok(())
}

/// Add `is_deleted` / `deleted_at` columns to `table` if they are missing.
/// Safe to call on tables that already have the columns (no-op).
fn add_soft_delete_columns(conn: &Connection, table: &str) -> Result<()> {
    // PRAGMA table_info returns columns: cid, name, type, notnull, dflt_value, pk
    // The column name is at index 1 (name), not 0 (cid).
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let names: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;

    if !names.iter().any(|c| c == "is_deleted") {
        conn.execute_batch(&format!(
            "ALTER TABLE {} ADD COLUMN is_deleted INTEGER NOT NULL DEFAULT 0",
            table
        ))?;
    }
    if !names.iter().any(|c| c == "deleted_at") {
        conn.execute_batch(&format!(
            "ALTER TABLE {} ADD COLUMN deleted_at TEXT",
            table
        ))?;
    }
    Ok(())
}
