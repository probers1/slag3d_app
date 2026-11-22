use crate::db::schema::init_schema;

#[test]
fn test_db_init_and_migrations() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    let version: i32 = conn.query_row("SELECT version FROM schema_versions WHERE id = 1", [], |r| r.get(0)).unwrap();
    assert_eq!(version, 8);
}

#[test]
fn test_fresh_schema_declares_foreign_keys() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();

    // files.model_id -> models ON DELETE CASCADE
    let fks = crate::db::schema::foreign_keys(&conn, "files").unwrap();
    assert!(fks.iter().any(|f| f.0 == "model_id" && f.1 == "models" && f.2 == "CASCADE"));

    // model_tags -> models and tags
    let model_tags_fks = crate::db::schema::foreign_keys(&conn, "model_tags").unwrap();
    assert!(model_tags_fks
        .iter()
        .any(|f| f.0 == "model_id" && f.1 == "models" && f.2 == "CASCADE"));
    assert!(model_tags_fks
        .iter()
        .any(|f| f.0 == "tag_id" && f.1 == "tags" && f.2 == "CASCADE"));

    // favorites -> users and models
    let favorites_fks = crate::db::schema::foreign_keys(&conn, "favorites").unwrap();
    assert!(favorites_fks
        .iter()
        .any(|f| f.0 == "user_id" && f.1 == "users" && f.2 == "CASCADE"));
    assert!(favorites_fks
        .iter()
        .any(|f| f.0 == "model_id" && f.1 == "models" && f.2 == "CASCADE"));

    // collection_models -> collections and models
    let cm_fks = crate::db::schema::foreign_keys(&conn, "collection_models").unwrap();
    assert!(cm_fks
        .iter()
        .any(|f| f.0 == "collection_id" && f.1 == "collections" && f.2 == "CASCADE"));
    assert!(cm_fks
        .iter()
        .any(|f| f.0 == "model_id" && f.1 == "models" && f.2 == "CASCADE"));
}

#[test]
fn test_model_delete_cascades_dependent_rows() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();

    conn.execute(
        "INSERT INTO models (id, user_id, title) VALUES (?1, ?2, ?3)",
        rusqlite::params!["m1", crate::db::schema::DEFAULT_USER_ID, "Test"],
    ).unwrap();
    conn.execute(
        "INSERT INTO files (id, model_id, filename, original_name, mime_type, file_size, blob, file_path) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params!["f1", "m1", "a.stl", "a.stl", "model/stl", 10, "", "/tmp/a.stl"],
    ).unwrap();
    conn.execute(
        "INSERT INTO tags (id, name) VALUES (?1, ?2)",
        rusqlite::params!["t1", "tag1"],
    ).unwrap();
    conn.execute(
        "INSERT INTO model_tags (model_id, tag_id) VALUES (?1, ?2)",
        rusqlite::params!["m1", "t1"],
    ).unwrap();
    conn.execute(
        "INSERT INTO favorites (user_id, model_id) VALUES (?1, ?2)",
        rusqlite::params![crate::db::schema::DEFAULT_USER_ID, "m1"],
    ).unwrap();
    conn.execute(
        "INSERT INTO collections (id, user_id, name) VALUES (?1, ?2, ?3)",
        rusqlite::params!["c1", crate::db::schema::DEFAULT_USER_ID, "Col"],
    ).unwrap();
    conn.execute(
        "INSERT INTO collection_models (collection_id, model_id) VALUES (?1, ?2)",
        rusqlite::params!["c1", "m1"],
    ).unwrap();

    conn.execute("DELETE FROM models WHERE id = ?1", rusqlite::params!["m1"]).unwrap();

    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get::<_, i64>(0)).unwrap(),
        0,
        "files should cascade on model delete"
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM model_tags", [], |r| r.get::<_, i64>(0)).unwrap(),
        0,
        "model_tags should cascade on model delete"
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM favorites", [], |r| r.get::<_, i64>(0)).unwrap(),
        0,
        "favorites should cascade on model delete"
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM collection_models", [], |r| r.get::<_, i64>(0)).unwrap(),
        0,
        "collection_models should cascade on model delete"
    );
}

#[test]
fn test_v6_vault_migrates_to_v7_foreign_keys() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    // Hand-build a legacy v6 database: no FKs, plus the pre-v6 soft-delete columns.
    conn.execute_batch(
        "CREATE TABLE schema_versions (id INTEGER PRIMARY KEY CHECK(id = 1), version INTEGER NOT NULL UNIQUE, applied_at DATETIME DEFAULT CURRENT_TIMESTAMP);
         CREATE TABLE users (id TEXT PRIMARY KEY, username TEXT NOT NULL UNIQUE, password_hash TEXT NOT NULL, role TEXT NOT NULL DEFAULT 'user' CHECK(role IN ('user','admin')), created_at DATETIME DEFAULT CURRENT_TIMESTAMP, updated_at DATETIME DEFAULT CURRENT_TIMESTAMP, is_deleted INTEGER NOT NULL DEFAULT 0, deleted_at DATETIME);
         CREATE TABLE models (id TEXT PRIMARY KEY, user_id TEXT NOT NULL, title TEXT NOT NULL, description TEXT DEFAULT '', thumbnail_path TEXT, thumbnail_status TEXT NOT NULL CHECK(thumbnail_status IN ('pending','processing','completed','failed')) DEFAULT 'pending', width INTEGER NOT NULL DEFAULT 800, height INTEGER NOT NULL DEFAULT 600, created_at DATETIME DEFAULT CURRENT_TIMESTAMP, updated_at DATETIME DEFAULT CURRENT_TIMESTAMP, is_deleted INTEGER NOT NULL DEFAULT 0, deleted_at DATETIME);
         CREATE TABLE files (id TEXT PRIMARY KEY, model_id TEXT NOT NULL, filename TEXT NOT NULL, original_name TEXT NOT NULL, mime_type TEXT NOT NULL, file_size INTEGER NOT NULL, file_path TEXT NOT NULL, width INTEGER NOT NULL DEFAULT 800, height INTEGER NOT NULL DEFAULT 600, is_primary_3d INTEGER NOT NULL DEFAULT 0 CHECK(is_primary_3d IN (0,1)), created_at DATETIME DEFAULT CURRENT_TIMESTAMP, is_deleted INTEGER NOT NULL DEFAULT 0, deleted_at DATETIME);
         CREATE TABLE tags (id TEXT PRIMARY KEY, name TEXT UNIQUE NOT NULL COLLATE NOCASE, created_at DATETIME DEFAULT CURRENT_TIMESTAMP);
         CREATE TABLE model_tags (model_id TEXT NOT NULL, tag_id TEXT NOT NULL, created_at DATETIME DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (model_id, tag_id));
         CREATE TABLE favorites (user_id TEXT NOT NULL, model_id TEXT NOT NULL, created_at DATETIME DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (user_id, model_id));
         CREATE TABLE collections (id TEXT PRIMARY KEY, user_id TEXT NOT NULL, name TEXT NOT NULL, description TEXT DEFAULT '', parent_folder_id TEXT, created_at DATETIME DEFAULT CURRENT_TIMESTAMP, updated_at DATETIME DEFAULT CURRENT_TIMESTAMP, is_deleted INTEGER NOT NULL DEFAULT 0, deleted_at DATETIME);
         CREATE TABLE collection_models (collection_id TEXT NOT NULL, model_id TEXT NOT NULL, added_at DATETIME DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (collection_id, model_id));
         INSERT INTO schema_versions (id, version) VALUES (1, 6);",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO models (id, user_id, title) VALUES (?1, ?2, ?3)",
        rusqlite::params!["m1", crate::db::schema::DEFAULT_USER_ID, "Legacy"],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO files (id, model_id, filename, original_name, mime_type, file_size, file_path) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params!["f1", "m1", "a.stl", "a.stl", "model/stl", 10, "/tmp/a.stl"],
    )
    .unwrap();

    init_schema(&conn).unwrap();

    let version: i32 = conn.query_row("SELECT version FROM schema_versions WHERE id = 1", [], |r| r.get(0)).unwrap();
    assert_eq!(version, 8);

    // The migration must keep the data intact while adding the FK.
    let (title, file_id): (String, String) = conn
        .query_row(
            "SELECT m.title, f.id FROM models m JOIN files f ON f.model_id = m.id WHERE m.id = ?1",
            rusqlite::params!["m1"],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(title, "Legacy");
    assert_eq!(file_id, "f1");

    // Verify the blob column was added by migration v8
    let has_blob: bool = conn
        .prepare("PRAGMA table_info(files)")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .any(|name| name.unwrap() == "blob");
    assert!(has_blob, "migration v8 should add blob column to files table");
}

