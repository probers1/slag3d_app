use crate::db::{get_db_connection, schema::init_schema};
use tauri::AppHandle;

#[tauri::command]
pub async fn init_db(app: AppHandle) -> Result<(), String> {
    let conn = get_db_connection(&app)?;
    init_schema(&conn).map_err(|e| e.to_string())?;
    
    Ok(())
}