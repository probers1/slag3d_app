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
            blob TEXT, -- Base64-encoded read_file result for cached thumbnail rendering
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE
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
            PRIMARY KEY (model_id, tag_id),
            FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE,
            FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
        );",
    )?;

    // Favorites
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS favorites (
            user_id TEXT NOT NULL,
            model_id TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (user_id, model_id),
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
            FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE
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
            PRIMARY KEY (collection_id, model_id),
            FOREIGN KEY (collection_id) REFERENCES collections(id) ON DELETE CASCADE,
            FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE
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

    // Migration v6: the v5 schema carried `is_deleted`/`deleted_at` columns
    // from the server's v4 migration, but the Tauri app hard-deletes models
    // (see `delete_model` in commands/models.rs) — there is no soft-delete
    // path, so those columns are dead weight that every query filters
    // `is_deleted = 0` unconditionally. Drop them on existing vaults so the
    // schema matches the fresh-table definition above and there's no stale
    // column to confuse future migrations.
    //
    // SQLite 3.35+ supports ALTER TABLE ... DROP COLUMN. rusqlite 0.31 ships
    // with SQLite 3.31+ (bundled), so DROP COLUMN is available on every
    // supported platform.
    drop_soft_delete_columns(conn, "models")?;
    drop_soft_delete_columns(conn, "files")?;
    drop_soft_delete_columns(conn, "collections")?;
    drop_soft_delete_columns(conn, "users")?;

    // Migration v7: declare foreign keys that the pre-v7 schema omitted, so
    // deletes cascade structurally instead of relying on hand-written cleanup
    // in individual commands (delete_model, delete_collection, ...).
    migrate_foreign_keys(conn)?;

    // Set schema version to 7 (upsert)
    conn.execute_batch(
        "INSERT INTO schema_versions (id, version) VALUES (1, 7)
         ON CONFLICT(id) DO UPDATE SET version = 7;",
    )?;

    Ok(())
}

/// Drop `is_deleted` / `deleted_at` columns from `table` if they exist.
/// Safe to call on tables that don't have the columns (no-op).
fn drop_soft_delete_columns(conn: &Connection, table: &str) -> Result<()> {
    // PRAGMA table_info returns columns: cid, name, type, notnull, dflt_value, pk
    // The column name is at index 1 (name), not 0 (cid).
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let names: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;

    for col in ["is_deleted", "deleted_at"] {
        if names.iter().any(|c| c == col) {
            conn.execute_batch(&format!(
                "ALTER TABLE {} DROP COLUMN {}",
                table, col
            ))?;
        }
    }
    Ok(())
}

/// One expected foreign key: (referenced table, column, ON DELETE action).
struct ExpectedFk {
    table: String,
    from: String,
    on_delete: String,
}

/// Foreign keys declared on `table` as (from column, referenced table, action).
pub fn foreign_keys(conn: &Connection, table: &str) -> Result<Vec<(String, String, String)>> {
    let mut stmt = conn.prepare(&format!("PRAGMA foreign_key_list({})", table))?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(3)?, // from
            row.get::<_, String>(2)?, // table
            row.get::<_, String>(6)?, // on_delete
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (from, table, on_delete) = row?;
        out.push((from, table, on_delete));
    }
    Ok(out)
}

fn has_expected_fks(conn: &Connection, table: &str, expected: &[ExpectedFk]) -> Result<bool> {
    let actual = foreign_keys(conn, table)?;
    Ok(expected.iter().all(|e| {
        actual.iter().any(|f| f.0 == e.from && f.1 == e.table && f.2 == e.on_delete)
    }))
}

/// Rebuild `table` as `table_new` using `create_sql`, copy data over, then swap.
/// Used by migration v7, which cannot add FKs via ALTER TABLE in SQLite.
fn rebuild_table(conn: &Connection, table: &str, create_sql: &str, columns: &[&str]) -> Result<()> {
    let new_name = format!("{}_new", table);
    conn.execute_batch(&format!("DROP TABLE IF EXISTS {}", new_name))?;
    conn.execute_batch(create_sql)?;
    let cols = columns
        .iter()
        .map(|c| format!("\"{}\"", c))
        .collect::<Vec<_>>()
        .join(", ");
    conn.execute_batch(&format!(
        "INSERT INTO {} ({}) SELECT {} FROM {};",
        new_name, cols, cols, table
    ))?;
    conn.execute_batch(&format!("DROP TABLE {};", table))?;
    conn.execute_batch(&format!("ALTER TABLE {} RENAME TO {};", new_name, table))?;
    Ok(())
}

/// Migration v7: add the foreign keys the pre-v7 schema omitted. Existing
/// vaults are rebuilt (SQLite cannot ALTER TABLE ADD CONSTRAINT); fresh vaults
/// already get the FK clauses from the CREATE TABLE statements above, so this is
/// a no-op for them.
fn migrate_foreign_keys(conn: &Connection) -> Result<()> {
    if !has_expected_fks(
        conn,
        "files",
        &[ExpectedFk {
            table: "models".into(),
            from: "model_id".into(),
            on_delete: "CASCADE".into(),
        }],
    )? {
        rebuild_table(
            conn,
            "files",
            "CREATE TABLE files_new (
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
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE
            )",
            &[
                "id", "model_id", "filename", "original_name", "mime_type", "file_size",
                "file_path", "width", "height", "is_primary_3d", "created_at",
            ],
        )?;
        conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_files_model_id ON files(model_id);")?;
        conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_files_is_primary ON files(is_primary_3d);")?;
    }

    if !has_expected_fks(
        conn,
        "model_tags",
        &[
            ExpectedFk { table: "models".into(), from: "model_id".into(), on_delete: "CASCADE".into() },
            ExpectedFk { table: "tags".into(), from: "tag_id".into(), on_delete: "CASCADE".into() },
        ],
    )? {
        rebuild_table(
            conn,
            "model_tags",
            "CREATE TABLE model_tags_new (
                model_id TEXT NOT NULL,
                tag_id TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (model_id, tag_id),
                FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE,
                FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
            )",
            &["model_id", "tag_id", "created_at"],
        )?;
    }

    if !has_expected_fks(
        conn,
        "favorites",
        &[
            ExpectedFk { table: "users".into(), from: "user_id".into(), on_delete: "CASCADE".into() },
            ExpectedFk { table: "models".into(), from: "model_id".into(), on_delete: "CASCADE".into() },
        ],
    )? {
        rebuild_table(
            conn,
            "favorites",
            "CREATE TABLE favorites_new (
                user_id TEXT NOT NULL,
                model_id TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (user_id, model_id),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE
            )",
            &["user_id", "model_id", "created_at"],
        )?;
    }

    if !has_expected_fks(
        conn,
        "collection_models",
        &[
            ExpectedFk { table: "collections".into(), from: "collection_id".into(), on_delete: "CASCADE".into() },
            ExpectedFk { table: "models".into(), from: "model_id".into(), on_delete: "CASCADE".into() },
        ],
    )? {
        rebuild_table(
            conn,
            "collection_models",
            "CREATE TABLE collection_models_new (
                collection_id TEXT NOT NULL,
                model_id TEXT NOT NULL,
                added_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (collection_id, model_id),
                FOREIGN KEY (collection_id) REFERENCES collections(id) ON DELETE CASCADE,
                FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE
            )",
            &["collection_id", "model_id", "added_at"],
        )?;
    }

    Ok(())
}
