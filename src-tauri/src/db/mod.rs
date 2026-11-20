pub mod schema;

pub use schema::init_schema;

#[cfg(test)]
mod tests;

use std::sync::{Mutex, MutexGuard, OnceLock};
use tauri::{AppHandle, Manager};

static DB_CONN: OnceLock<Mutex<rusqlite::Connection>> = OnceLock::new();

/// Seed the global shared database connection initialized during app setup.
pub fn init_db_connection(conn: rusqlite::Connection) {
    let _ = DB_CONN.set(Mutex::new(conn));
}

/// Retrieve the shared, persistent SQLite connection across all Tauri commands.
/// Reusing a single open connection avoids disk open/close thrashing and ensures
/// WAL mode and busy_timeout stay permanently in effect.
pub fn get_db_connection(app: &AppHandle) -> Result<MutexGuard<'static, rusqlite::Connection>, String> {
    let mutex = if let Some(m) = DB_CONN.get() {
        m
    } else {
        let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;
        let db_path = app_dir.join("vault.db");
        let conn = rusqlite::Connection::open(&db_path).map_err(|e| e.to_string())?;
        conn.busy_timeout(std::time::Duration::from_millis(5000)).map_err(|e| e.to_string())?;
        init_schema(&conn).map_err(|e| e.to_string())?;
        let _ = DB_CONN.set(Mutex::new(conn));
        DB_CONN.get().expect("DB_CONN must be initialized")
    };
    mutex.lock().map_err(|e| e.to_string())
}

// Placeholder query helpers - minimal implementations to satisfy compilation
pub fn list_models(_conn: &rusqlite::Connection) -> rusqlite::Result<Vec<String>> {
    Ok(vec![])
}

pub fn get_model_detail(_conn: &rusqlite::Connection, _id: &str) -> rusqlite::Result<Option<()>> {
    Ok(None)
}

pub fn insert_model(_conn: &rusqlite::Connection, _id: &str) -> rusqlite::Result<()> {
    Ok(())
}

pub fn update_model(_conn: &rusqlite::Connection, _id: &str) -> rusqlite::Result<()> {
    Ok(())
}

pub fn delete_model(_conn: &rusqlite::Connection, _id: &str) -> rusqlite::Result<()> {
    Ok(())
}

pub fn list_collections(_conn: &rusqlite::Connection) -> rusqlite::Result<Vec<String>> {
    Ok(vec![])
}

pub fn create_collection(_conn: &rusqlite::Connection, _name: &str) -> rusqlite::Result<()> {
    Ok(())
}

pub fn delete_collection(_conn: &rusqlite::Connection, _id: &str) -> rusqlite::Result<()> {
    Ok(())
}

pub fn list_tags(_conn: &rusqlite::Connection) -> rusqlite::Result<Vec<String>> {
    Ok(vec![])
}
