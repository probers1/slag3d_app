use crate::commands::TagItem;
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn get_tags(app: AppHandle) -> Result<Vec<TagItem>, String> {
    let conn = get_db_connection(&app).map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        r#"
        SELECT t.id, t.name, COUNT(mt.model_id) as count
        FROM tags t
        LEFT JOIN model_tags mt ON mt.tag_id = t.id
        GROUP BY t.id, t.name
        ORDER BY count DESC, t.name
        "#,
    ).map_err(|e| e.to_string())?;

    let rows = stmt.query_map([], |row| {
        Ok(TagItem {
            id: row.get(0)?,
            name: row.get(1)?,
            count: row.get(2)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut tags = Vec::new();
    for row in rows {
        tags.push(row.map_err(|e| e.to_string())?);
    }

    Ok(tags)
}

fn get_db_connection(app: &AppHandle) -> rusqlite::Result<rusqlite::Connection> {
    let path_resolver = app.path();
    let db_path = path_resolver.app_data_dir().unwrap().join("vault.db");
    rusqlite::Connection::open(db_path)
}