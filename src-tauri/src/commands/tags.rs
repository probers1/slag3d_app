use crate::commands::TagItem;
use crate::errors::AppError;
use crate::db::get_db_connection;
use rusqlite::OptionalExtension;
use tauri::AppHandle;

#[tauri::command]
pub async fn get_tags(app: AppHandle) -> Result<Vec<TagItem>, AppError> {
    let conn = get_db_connection(&app).map_err(|e| AppError::from(e.to_string()))?;

    let mut stmt = conn.prepare(
        r#"
        SELECT t.id, t.name, COUNT(mt.model_id) as count
        FROM tags t
        LEFT JOIN model_tags mt ON mt.tag_id = t.id
        GROUP BY t.id, t.name
        ORDER BY count DESC, t.name
        "#,
    ).map_err(|e| AppError::from(e.to_string()))?;

    let rows = stmt.query_map([], |row| {
        Ok(TagItem {
            id: row.get(0)?,
            name: row.get(1)?,
            count: row.get(2)?,
        })
    }).map_err(|e| AppError::from(e.to_string()))?;

    let mut tags = Vec::new();
    for row in rows {
        tags.push(row.map_err(|e| AppError::from(e.to_string()))?);
    }

    Ok(tags)
}

#[tauri::command]
pub async fn add_tag(app: AppHandle, name: String) -> Result<TagItem, AppError> {
    let conn = get_db_connection(&app).map_err(|e| AppError::from(e.to_string()))?;

    // Check if tag already exists (using optional to handle duplicates)
    let existing: Option<String> = conn.query_row(
        "SELECT id FROM tags WHERE name = ?1 COLLATE NOCASE",
        rusqlite::params![&name],
        |row| row.get::<_, String>(0),
    ).optional().map_err(|e| AppError::from(e.to_string()))?;

    if let Some(id) = existing {
        // Tag already exists, return it with count 0 (no models yet attached)
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM model_tags WHERE tag_id = ?",
            rusqlite::params![&id],
            |row| row.get::<_, i64>(0),
        ).map_err(|e| AppError::from(e.to_string()))?;

        Ok(TagItem {
            id,
            name: name.to_string(),
            count: count as u32,
        })
    } else {
        // Insert new tag and get its id back
        let id = conn.query_row(
            "INSERT INTO tags (name) VALUES (?1) RETURNING id",
            rusqlite::params![&name],
            |row| row.get::<_, String>(0),
        ).map_err(|e| AppError::from(e.to_string()))?;

        // Get the count after insertion
        let count = conn.query_row(
            "SELECT COUNT(*) FROM model_tags WHERE tag_id = ?",
            rusqlite::params![&id],
            |row| row.get::<_, i64>(0),
        ).map_err(|e| AppError::from(e.to_string()))?;

        Ok(TagItem {
            id,
            name: name.to_string(),
            count: count as u32,
        })
    }
}

#[tauri::command]
pub async fn delete_tag(app: AppHandle, id: String) -> Result<TagItem, AppError> {
    let conn = get_db_connection(&app).map_err(|e| AppError::from(e.to_string()))?;

    // Get tag info before deleting
    let name = conn.query_row(
        "SELECT name FROM tags WHERE id = ?1",
        rusqlite::params![&id],
        |row| row.get::<_, String>(0),
    ).map_err(|e| AppError::from(e.to_string()))?;

    // Get count before deletion
    let count = conn.query_row(
        "SELECT COUNT(*) FROM model_tags WHERE tag_id = ?",
        rusqlite::params![&id],
        |row| row.get::<_, i64>(0),
    ).map_err(|e| AppError::from(e.to_string()))?;

    // Delete the tag and its associations (FK CASCADE handles model_tags)
    conn.execute("DELETE FROM tags WHERE id = ?1", rusqlite::params![&id])
        .map_err(|e| AppError::from(e.to_string()))?;

    Ok(TagItem {
        id,
        name,
        count: count as u32,
    })
}