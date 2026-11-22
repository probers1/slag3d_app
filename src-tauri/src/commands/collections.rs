use crate::commands::CollectionItem;
use crate::errors::AppError;
use crate::db::schema::DEFAULT_USER_ID;
use crate::db::get_db_connection;
use tauri::AppHandle;
use uuid::Uuid;

#[tauri::command]
pub async fn get_collections(app: AppHandle) -> Result<Vec<CollectionItem>, AppError> {
    let conn = get_db_connection(&app).map_err(|e| AppError::from(e.to_string()))?;
    let user_id = DEFAULT_USER_ID.to_string();

    let mut stmt = conn.prepare(
        r#"
        SELECT c.id, c.name, c.description, c.parent_folder_id, c.created_at,
               COUNT(m.id) as model_count
        FROM collections c
        LEFT JOIN collection_models cm ON cm.collection_id = c.id
        LEFT JOIN models m ON m.id = cm.model_id
        WHERE c.user_id = ?
        GROUP BY c.id
        ORDER BY c.name
        "#,
    ).map_err(|e| AppError::from(e.to_string()))?;

    let rows = stmt.query_map([&user_id], |row| {
        Ok(CollectionItem {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            parent_folder_id: row.get(3)?,
            created_at: row.get(4)?,
            model_count: row.get(5)?,
            children: None,
        })
    }).map_err(|e| AppError::from(e.to_string()))?;

    let mut collections = Vec::new();
    for row in rows {
        collections.push(row.map_err(|e| AppError::from(e.to_string()))?);
    }

    // Order collections hierarchically (depth-first) so children appear directly under their parents
    // in flat dropdowns and menus, while still preserving all items in the array for the frontend tree view.
    let mut flat_hierarchical = Vec::new();
    let mut children_map: std::collections::HashMap<Option<String>, Vec<CollectionItem>> = std::collections::HashMap::new();

    for c in collections {
        children_map.entry(c.parent_folder_id.clone()).or_default().push(c);
    }

    fn flatten_tree(
        parent_id: Option<String>,
        children_map: &mut std::collections::HashMap<Option<String>, Vec<CollectionItem>>,
        out: &mut Vec<CollectionItem>,
    ) {
        if let Some(items) = children_map.remove(&parent_id) {
            for item in items {
                let id = item.id.clone();
                out.push(item);
                flatten_tree(Some(id), children_map, out);
            }
        }
    }

    flatten_tree(None, &mut children_map, &mut flat_hierarchical);

    // Append any orphaned collections (where parent_folder_id does not exist)
    for (_, remaining) in children_map {
        flat_hierarchical.extend(remaining);
    }

    Ok(flat_hierarchical)
}

#[tauri::command]
pub async fn create_collection(
    app: AppHandle,
    name: String,
    description: Option<String>,
    parent_folder_id: Option<String>,
) -> Result<CollectionItem, AppError> {
    let conn = get_db_connection(&app).map_err(|e| AppError::from(e.to_string()))?;
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let user_id = DEFAULT_USER_ID.to_string();
    conn.execute(
        "INSERT INTO collections (id, user_id, name, description, parent_folder_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        rusqlite::params![&id, &user_id, &name, &description.clone().unwrap_or_default(), parent_folder_id.clone(), &now, &now],
    ).map_err(|e| AppError::from(e.to_string()))?;

    Ok(CollectionItem {
        id,
        name,
        description: description.unwrap_or_default(),
        parent_folder_id,
        created_at: now,
        model_count: 0,
        children: None,
    })
}

#[tauri::command]
pub async fn rename_collection(app: AppHandle, id: String, name: String) -> Result<(), AppError> {
    let conn = get_db_connection(&app).map_err(|e| AppError::from(e.to_string()))?;
    let now = chrono::Utc::now().to_rfc3339();
    let user_id = DEFAULT_USER_ID.to_string();

    conn.execute(
        "UPDATE collections SET name = ?, updated_at = ? WHERE id = ? AND user_id = ?",
        rusqlite::params![&name, &now, &id, &user_id],
    ).map_err(|e| AppError::from(e.to_string()))?;

    Ok(())
}

#[tauri::command]
pub async fn delete_collection(app: AppHandle, id: String) -> Result<(), AppError> {
    let conn = get_db_connection(&app).map_err(|e| AppError::from(e.to_string()))?;
    let user_id = DEFAULT_USER_ID.to_string();

    // Re-parent any sub-folders to the deleted folder's parent (or NULL if root) to prevent orphaned collections
    conn.execute(
        "UPDATE collections SET parent_folder_id = (SELECT parent_folder_id FROM collections WHERE id = ?1) WHERE parent_folder_id = ?1",
        rusqlite::params![&id],
    ).map_err(|e| AppError::from(e.to_string()))?;

    // Delete collection models first
    conn.execute("DELETE FROM collection_models WHERE collection_id = ?", rusqlite::params![&id]).map_err(|e| AppError::from(e.to_string()))?;
    // Delete collection
    conn.execute("DELETE FROM collections WHERE id = ? AND user_id = ?", rusqlite::params![&id, &user_id]).map_err(|e| AppError::from(e.to_string()))?;

    Ok(())
}

#[tauri::command]
pub async fn add_model_to_collection(app: AppHandle, collection_id: String, model_id: String) -> Result<(), AppError> {
    let conn = get_db_connection(&app).map_err(|e| AppError::from(e.to_string()))?;

    conn.execute(
        "INSERT OR IGNORE INTO collection_models (collection_id, model_id) VALUES (?, ?)",
        [&collection_id, &model_id],
    ).map_err(|e| AppError::from(e.to_string()))?;

    Ok(())
}

#[tauri::command]
pub async fn remove_model_from_collection(app: AppHandle, collection_id: String, model_id: String) -> Result<(), AppError> {
    let conn = get_db_connection(&app).map_err(|e| AppError::from(e.to_string()))?;

    conn.execute(
        "DELETE FROM collection_models WHERE collection_id = ? AND model_id = ?",
        [&collection_id, &model_id],
    ).map_err(|e| AppError::from(e.to_string()))?;

    Ok(())
}