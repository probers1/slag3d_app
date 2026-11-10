use crate::db::schema::init_schema;
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn init_db(app: AppHandle) -> Result<(), String> {
    let path_resolver = app.path();
    let db_path = path_resolver.app_data_dir().map_err(|e| e.to_string())?.join("vault.db");
    
    let conn = rusqlite::Connection::open(&db_path).map_err(|e| e.to_string())?;
    init_schema(&conn).map_err(|e| e.to_string())?;
    
    Ok(())
}