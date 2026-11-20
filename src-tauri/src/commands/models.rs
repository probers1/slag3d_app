use crate::commands::{
    ModelDetail, ModelFile, ModelItem, PaginatedModels, CollectionRef,
};
use crate::db::schema::DEFAULT_USER_ID;
use crate::services::archive::{extract_7z, extract_zip};
use crate::services::storage::ExtractedFile;
use base64::{engine::general_purpose, Engine as _};
use rusqlite::OptionalExtension;
use std::fs;
use std::path::Path;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

fn get_db_connection(app: &AppHandle) -> rusqlite::Result<rusqlite::Connection> {
    let path_resolver = app.path();
    let db_path = path_resolver.app_data_dir().unwrap().join("vault.db");
    let conn = rusqlite::Connection::open(db_path)?;
    conn.busy_timeout(std::time::Duration::from_millis(5000))?;
    Ok(conn)
}

#[tauri::command]
pub async fn get_models(
    app: AppHandle,
    page: u32,
    limit: u32,
    search: Option<String>,
    tag: Option<String>,
    collection_id: Option<String>,
    filter: Option<String>,
) -> Result<PaginatedModels, String> {
    let page = page.max(1);
    let limit = limit.max(1).min(100);
    let offset = (page - 1) * limit;

    let conn = get_db_connection(&app).map_err(|e| e.to_string())?;

    // Build WHERE clause and owned params
    let mut where_clauses = vec!["m.is_deleted = 0"];
    let mut params_list: Vec<String> = Vec::new();

    if let Some(ref s) = search {
        where_clauses.push("(m.title LIKE ? OR m.description LIKE ?)");
        let sp = format!("%{}%", s);
        params_list.push(sp.clone());
        params_list.push(sp);
    }

    if let Some(ref t) = tag {
        where_clauses.push("EXISTS (SELECT 1 FROM model_tags mt JOIN tags tg ON tg.id = mt.tag_id WHERE mt.model_id = m.id AND tg.name = ?)");
        params_list.push(t.clone());
    }

    if let Some(ref c) = collection_id {
        where_clauses.push("EXISTS (SELECT 1 FROM collection_models cm WHERE cm.collection_id = ? AND cm.model_id = m.id)");
        params_list.push(c.clone());
    }

    if let Some(ref f) = filter {
        if f == "favorites" {
            where_clauses.push("EXISTS (SELECT 1 FROM favorites fav WHERE fav.user_id = ? AND fav.model_id = m.id)");
            params_list.push(DEFAULT_USER_ID.to_string());
        }
    }

    let where_sql = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    // Count total
    let count_sql = format!("SELECT COUNT(*) FROM models m {}", where_sql);
    let mut stmt = conn.prepare(&count_sql).map_err(|e| e.to_string())?;
    let total: u32 = stmt.query_row(rusqlite::params_from_iter(params_list.iter()), |row| row.get(0))
        .map_err(|e| e.to_string())?;

    // Fetch models
    let select_sql = format!(
        r#"
        SELECT m.id, m.user_id, m.title, m.description, m.thumbnail_path, m.thumbnail_status,
               m.created_at, m.updated_at, u.username as owner_username,
               COUNT(DISTINCT f.id) as files_count,
               GROUP_CONCAT(DISTINCT tg.name) as tags,
               MAX(CASE WHEN f.is_primary_3d = 1 THEN f.id END) as primary_file_id,
               MAX(CASE WHEN f.is_primary_3d = 1 THEN f.filename END) as primary_filename,
               MAX(CASE WHEN f.is_primary_3d = 1 THEN f.original_name END) as primary_original_name,
               MAX(CASE WHEN f.is_primary_3d = 1 THEN f.mime_type END) as primary_mime_type,
               MAX(CASE WHEN f.is_primary_3d = 1 THEN f.file_size END) as primary_file_size,
               MAX(CASE WHEN f.is_primary_3d = 1 THEN f.file_path END) as primary_file_path,
               EXISTS(SELECT 1 FROM favorites fav WHERE fav.user_id = ? AND fav.model_id = m.id) as is_favorited
        FROM models m
        LEFT JOIN users u ON u.id = m.user_id
        LEFT JOIN files f ON f.model_id = m.id
        LEFT JOIN model_tags mt ON mt.model_id = m.id
        LEFT JOIN tags tg ON tg.id = mt.tag_id
        {}
        GROUP BY m.id
        ORDER BY m.created_at DESC
        LIMIT ? OFFSET ?
        "#,
        where_sql
    );

    // Order matters: the favorites `?` sits in the SELECT list, before the WHERE clause.
    let mut all_params = vec![DEFAULT_USER_ID.to_string()];
    all_params.extend(params_list);
    all_params.push(limit.to_string());
    all_params.push(offset.to_string());

    let mut stmt = conn.prepare(&select_sql).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(rusqlite::params_from_iter(all_params.iter()), |row| {
        let tags_str: Option<String> = row.get(10)?;
        let tags = tags_str.map(|s| s.split(',').map(|s| s.to_string()).collect()).unwrap_or_default();

        let primary_file = if let Some(id) = row.get::<_, Option<String>>(11)? {
            Some(ModelFile {
                id,
                filename: row.get(12)?,
                original_name: row.get(13)?,
                mime_type: row.get(14)?,
                file_size: row.get(15)?,
                file_path: row.get(16)?,
                is_primary_3d: 1,
            })
        } else {
            None
        };

        Ok(ModelItem {
            id: row.get(0)?,
            user_id: row.get(1)?,
            title: row.get(2)?,
            description: row.get(3)?,
            thumbnail_path: row.get(4)?,
            thumbnail_status: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
            owner_username: row.get(8)?,
            files_count: row.get(9)?,
            tags,
            primary_file,
            is_favorited: row.get(17)?,
        })
    }).map_err(|e| e.to_string())?;

    let models = rows.map(|r| r.map_err(|e| e.to_string())).collect::<Result<_, _>>()?;

    Ok(PaginatedModels { models, total, page, limit })
}

#[tauri::command]
pub async fn get_model_detail(
    app: AppHandle,
    id: String,
) -> Result<ModelDetail, String> {
    let conn = get_db_connection(&app).map_err(|e| e.to_string())?;
    let user_id = DEFAULT_USER_ID.to_string();

    let model = conn.query_row(
        r#"SELECT m.id, m.user_id, m.title, m.description, m.thumbnail_path, m.thumbnail_status,
               m.created_at, m.updated_at, u.username as owner_username,
               EXISTS(SELECT 1 FROM favorites fav WHERE fav.user_id = ? AND fav.model_id = m.id) as is_favorited
        FROM models m
        LEFT JOIN users u ON u.id = m.user_id
        WHERE m.id = ? AND m.is_deleted = 0"#,
        rusqlite::params![&user_id, &id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,   // m.id
                row.get::<_, String>(1)?,   // m.user_id
                row.get::<_, String>(2)?,   // m.title
                row.get::<_, String>(3)?,   // m.description
                row.get::<_, Option<String>>(4)?,  // m.thumbnail_path
                row.get::<_, String>(5)?,   // m.thumbnail_status
                row.get::<_, String>(6)?,   // m.created_at
                row.get::<_, String>(7)?,   // m.updated_at
                row.get::<_, String>(8)?,   // u.username as owner_username
                row.get::<_, bool>(9)?,     // is_favorited
            ))
        },
    ).map_err(|e| e.to_string())?;

    // Tags
    let tags: Vec<String> = conn.prepare("SELECT tg.name FROM tags tg JOIN model_tags mt ON mt.tag_id = tg.id WHERE mt.model_id = ?")
        .map_err(|e| e.to_string())?
        .query_map(rusqlite::params![&id], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();

    // Files
    let files: Vec<ModelFile> = conn.prepare("SELECT id, filename, original_name, mime_type, file_size, file_path, is_primary_3d FROM files WHERE model_id = ?")
        .map_err(|e| e.to_string())?
        .query_map(rusqlite::params![&id], |row| {
            Ok(ModelFile {
                id: row.get(0)?,
                filename: row.get(1)?,
                original_name: row.get(2)?,
                mime_type: row.get(3)?,
                file_size: row.get(4)?,
                file_path: row.get(5)?,
                is_primary_3d: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();

    // Collections
    let collections: Vec<CollectionRef> = conn.prepare("SELECT c.id, c.name FROM collections c JOIN collection_models cm ON cm.collection_id = c.id WHERE cm.model_id = ?")
        .map_err(|e| e.to_string())?
        .query_map(rusqlite::params![&id], |row| {
            Ok(CollectionRef { id: row.get(0)?, name: row.get(1)? })
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();

    Ok(ModelDetail {
        id: model.0,
        user_id: model.1,
        title: model.2,
        description: model.3,
        thumbnail_path: model.4,
        thumbnail_status: model.5,
        created_at: model.6,
        updated_at: model.7,
        owner_username: model.8,
        is_favorited: model.9,
        tags,
        files,
        collections,
    })
}

#[tauri::command]
pub async fn import_models(
    app: AppHandle,
    file_paths: Vec<String>,
    title: Option<String>,
    description: Option<String>,
    tags: Option<String>,
    collection_id: Option<String>,
) -> Result<ModelDetail, String> {
    let conn = get_db_connection(&app).map_err(|e| e.to_string())?;
    let model_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let user_id = DEFAULT_USER_ID.to_string();

    // Use the same app_data_dir as the DB and asset protocol (com.slag3d.app)
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::env::set_var("SLAG3D_APP_DATA_DIR", &app_data_dir);
    let models_dir = app_data_dir.join("uploads").join("models").join(&model_id);
    fs::create_dir_all(&models_dir).map_err(|e| e.to_string())?;

    let mut all_extracted_files = Vec::new();

    for file_path_str in &file_paths {
        let path = Path::new(file_path_str);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

        let extracted = match ext.as_str() {
            "zip" => {
                extract_zip(path, &model_id)
                    .map_err(|e| format!("Failed to extract {}: {}", file_path_str, e))?
            },
            "7z" => {
                extract_7z(path, &model_id)
                    .map_err(|e| format!("Failed to extract {}: {}", file_path_str, e))?
            },
            _ => {
                // Single file - copy to model dir
                let filename = match path.file_name() {
                    Some(name) => name.to_string_lossy().to_string(),
                    None => continue,
                };
                let dest_path = models_dir.join(&filename);
                fs::copy(path, &dest_path).map_err(|e| e.to_string())?;
                let size = fs::metadata(&dest_path).map_err(|e| e.to_string())?.len();
                vec![ExtractedFile {
                    filename,
                    destination_path: dest_path,
                    size,
                }]
            }
        };

        all_extracted_files.extend(extracted);
    }

    if all_extracted_files.is_empty() {
        return Err("No valid 3D files found in archives".to_string());
    }

    // Insert model
    let model_title = title.unwrap_or_else(|| "Untitled Model".to_string());
    let model_description = description.unwrap_or_default();

    conn.execute(
        "INSERT INTO models (id, user_id, title, description, thumbnail_status, created_at, updated_at) VALUES (?, ?, ?, ?, 'pending', ?, ?)",
        rusqlite::params![&model_id, &user_id, &model_title, &model_description, &now, &now],
    ).map_err(|e| e.to_string())?;

    // Insert files with public paths so the asset protocol can serve them
    for (idx, file) in all_extracted_files.iter().enumerate() {
        let file_id = Uuid::new_v4().to_string();
        let mime_type = mime_guess::from_path(&file.destination_path).first_or_octet_stream().to_string();
        let is_primary = if idx == 0 { 1 } else { 0 };
        let public_path = format!("/uploads/models/{}/{}", model_id, file.filename);

        conn.execute(
            "INSERT INTO files (id, model_id, filename, original_name, mime_type, file_size, file_path, is_primary_3d) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                &file_id, &model_id, &file.filename, &file.filename,
                &mime_type, file.size as i64, &public_path, is_primary
            ],
        ).map_err(|e| e.to_string())?;
    }

    // Handle tags
    if let Some(tag_string) = tags {
        for tag_name in tag_string.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            let new_tag_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT OR IGNORE INTO tags (id, name) VALUES (?, ?)",
                rusqlite::params![&new_tag_id, tag_name],
            ).map_err(|e| e.to_string())?;

            let tag_id: String = conn.query_row(
                "SELECT id FROM tags WHERE name = ?",
                rusqlite::params![tag_name],
                |row| row.get(0),
            ).map_err(|e| e.to_string())?;

            conn.execute(
                "INSERT OR IGNORE INTO model_tags (model_id, tag_id) VALUES (?, ?)",
                rusqlite::params![&model_id, &tag_id],
            ).map_err(|e| e.to_string())?;
        }
    }

    // Add to collection if provided
    if let Some(cid) = collection_id {
        conn.execute(
            "INSERT OR IGNORE INTO collection_models (collection_id, model_id) VALUES (?, ?)",
            rusqlite::params![&cid, &model_id],
        ).map_err(|e| e.to_string())?;
    }

    get_model_detail(app, model_id).await
}

#[tauri::command]
pub async fn update_model(
    app: AppHandle,
    id: String,
    title: Option<String>,
    description: Option<String>,
    tags: Option<String>,
    primary_file_id: Option<String>,
) -> Result<(), String> {
    let conn = get_db_connection(&app).map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();

    if let Some(t) = title {
        conn.execute("UPDATE models SET title = ?, updated_at = ? WHERE id = ?", rusqlite::params![&t, &now, &id]).map_err(|e| e.to_string())?;
    }
    if let Some(d) = description {
        conn.execute("UPDATE models SET description = ?, updated_at = ? WHERE id = ?", rusqlite::params![&d, &now, &id]).map_err(|e| e.to_string())?;
    }

    if let Some(tag_string) = tags {
        conn.execute("DELETE FROM model_tags WHERE model_id = ?", rusqlite::params![&id]).map_err(|e| e.to_string())?;
        for tag_name in tag_string.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            let new_tag_id = Uuid::new_v4().to_string();
            conn.execute("INSERT OR IGNORE INTO tags (id, name) VALUES (?, ?)", rusqlite::params![&new_tag_id, tag_name]).map_err(|e| e.to_string())?;
            let tag_id: String = conn.query_row("SELECT id FROM tags WHERE name = ?", rusqlite::params![tag_name], |row| row.get(0)).map_err(|e| e.to_string())?;
            conn.execute("INSERT OR IGNORE INTO model_tags (model_id, tag_id) VALUES (?, ?)", rusqlite::params![&id, &tag_id]).map_err(|e| e.to_string())?;
        }
    }

    if let Some(pfid) = primary_file_id {
        conn.execute("UPDATE files SET is_primary_3d = 0 WHERE model_id = ?", rusqlite::params![&id]).map_err(|e| e.to_string())?;
        conn.execute("UPDATE files SET is_primary_3d = 1 WHERE id = ? AND model_id = ?", rusqlite::params![&pfid, &id]).map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Add supplementary files to an existing model. Mirrors the web server's
/// `POST /api/models/{id}/files` flow: extract archives, copy direct files,
/// designate the largest 3D file as primary when it beats the current one.
#[tauri::command]
pub async fn add_model_files(
    app: AppHandle,
    model_id: String,
    file_paths: Vec<String>,
) -> Result<Vec<ModelFile>, String> {
    let conn = get_db_connection(&app).map_err(|e| e.to_string())?;
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::env::set_var("SLAG3D_APP_DATA_DIR", &app_data_dir);
    let models_dir = app_data_dir.join("uploads").join("models").join(&model_id);
    fs::create_dir_all(&models_dir).map_err(|e| e.to_string())?;

    let mut all_extracted: Vec<ExtractedFile> = Vec::new();
    for file_path_str in &file_paths {
        let path = Path::new(file_path_str);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        let extracted = match ext.as_str() {
            "zip" => extract_zip(path, &model_id)
                .map_err(|e| format!("Failed to extract {}: {}", file_path_str, e))?,
            "7z" => extract_7z(path, &model_id)
                .map_err(|e| format!("Failed to extract {}: {}", file_path_str, e))?,
            _ => {
                let filename = match path.file_name() {
                    Some(name) => name.to_string_lossy().to_string(),
                    None => continue,
                };
                let dest_path = models_dir.join(&filename);
                fs::copy(path, &dest_path).map_err(|e| e.to_string())?;
                let size = fs::metadata(&dest_path).map_err(|e| e.to_string())?.len();
                vec![ExtractedFile {
                    filename,
                    destination_path: dest_path,
                    size,
                }]
            }
        };
        all_extracted.extend(extracted);
    }

    if all_extracted.is_empty() {
        return Err("No valid 3D files found in archives".to_string());
    }

    let mut inserted: Vec<ModelFile> = Vec::new();
    for file in &all_extracted {
        let file_id = Uuid::new_v4().to_string();
        let mime_type = mime_guess::from_path(&file.destination_path).first_or_octet_stream().to_string();
        let public_path = format!("/uploads/models/{}/{}", model_id, file.filename);
        conn.execute(
            "INSERT INTO files (id, model_id, filename, original_name, mime_type, file_size, file_path, is_primary_3d) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                &file_id, &model_id, &file.filename, &file.filename,
                &mime_type, file.size as i64, &public_path, 0,
            ],
        ).map_err(|e| e.to_string())?;
        inserted.push(ModelFile {
            id: file_id,
            filename: file.filename.clone(),
            original_name: file.filename.clone(),
            mime_type,
            file_size: file.size,
            file_path: public_path,
            is_primary_3d: 0,
        });
    }

    // Recompute primary: promote the largest new 3D file if it beats the current primary
    let current_primary = conn
        .prepare("SELECT id, file_size FROM files WHERE model_id = ? AND is_primary_3d = 1")
        .map_err(|e| e.to_string())?
        .query_row(rusqlite::params![&model_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
        .optional()
        .map_err(|e| e.to_string())?;

    let is_3d = |name: &str| {
        matches!(
            Path::new(name).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase().as_str(),
            "stl" | "3mf" | "obj"
        )
    };

    let largest_new = inserted
        .iter()
        .filter(|f| is_3d(&f.original_name))
        .max_by_key(|f| f.file_size);

    if let Some(largest) = largest_new {
        if current_primary.map_or(true, |(_, size)| largest.file_size as u64 > size as u64) {
            conn.execute("UPDATE files SET is_primary_3d = 0 WHERE model_id = ?", rusqlite::params![&model_id])
                .map_err(|e| e.to_string())?;
            conn.execute("UPDATE files SET is_primary_3d = 1 WHERE id = ? AND model_id = ?", rusqlite::params![&largest.id, &model_id])
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(inserted)
}

#[tauri::command]
pub async fn delete_model(app: AppHandle, id: String) -> Result<(), String> {
    // Hard delete — matches the Express server's `DELETE /api/models/:id`
    // (purge files from disk, then cascade the DB row). The spec for the
    // Tauri build also specifies "Removes DB records and deletes files from
    // disk" (design doc §5.2), so soft-delete was an implementation
    // inconsistency with the server. Aligning on hard delete means the same
    // model can never be recovered from either surface.
    let mut conn = get_db_connection(&app).map_err(|e| e.to_string())?;

    // 1. Purge model files directory
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let models_dir = app_data_dir.join("uploads").join("models").join(&id);
    if models_dir.exists() {
        fs::remove_dir_all(&models_dir).map_err(|e| e.to_string())?;
    }

    // 2. Purge cached thumbnail
    let thumb_path = app_data_dir.join("uploads").join("thumbnails").join(format!("{}.png", id));
    if thumb_path.exists() {
        fs::remove_file(&thumb_path).map_err(|e| e.to_string())?;
    }

    // 3. Delete database record. The Tauri schema declares `files.model_id`
    // etc. without REFERENCES, so SQLite won't cascade — clean up the
    // dependents explicitly or they'd orphan.
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM model_tags WHERE model_id = ?", rusqlite::params![&id])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM favorites WHERE model_id = ?", rusqlite::params![&id])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM collection_models WHERE model_id = ?", rusqlite::params![&id])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM files WHERE model_id = ?", rusqlite::params![&id])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM models WHERE id = ?", rusqlite::params![&id])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn delete_model_file(app: AppHandle, model_id: String, file_id: String) -> Result<(), String> {
    let conn = get_db_connection(&app).map_err(|e| e.to_string())?;
    
    // Get the file to delete
    let file = conn.query_row(
        "SELECT file_path, is_primary_3d, original_name FROM files WHERE id = ? AND model_id = ?",
        rusqlite::params![&file_id, &model_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,  // file_path
                row.get::<_, i32>(1)?,     // is_primary_3d
                row.get::<_, String>(2)?,  // original_name
            ))
        },
    ).map_err(|e| e.to_string())?;
    
    // Delete physical file from disk
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let uploads_dir = app_data_dir.join("uploads");
    let file_path = file.0.strip_prefix("/uploads/").unwrap_or(&file.0);
    let full_path = uploads_dir.join(file_path);
    
    if full_path.exists() {
        fs::remove_file(&full_path).map_err(|e| e.to_string())?;
    }
    
    // Delete database record
    conn.execute("DELETE FROM files WHERE id = ?", rusqlite::params![&file_id]).map_err(|e| e.to_string())?;
    
    // If this was the primary 3D file, select a new primary from remaining files
    if file.1 == 1 {
        let remaining = conn.query_row(
            "SELECT id, file_size, original_name, file_path FROM files WHERE model_id = ? AND is_primary_3d = 0 ORDER BY file_size DESC LIMIT 1",
            rusqlite::params![&model_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,  // id
                    row.get::<_, i64>(1)?,     // file_size
                    row.get::<_, String>(2)?,  // original_name
                    row.get::<_, String>(3)?,  // file_path
                ))
            },
        ).optional().map_err(|e| e.to_string())?;
        
        if let Some(rem) = remaining {
            conn.execute("UPDATE files SET is_primary_3d = 1 WHERE id = ?", rusqlite::params![&rem.0]).map_err(|e| e.to_string())?;
            
            // Re-queue thumbnail render for the new primary 3D file
            // Note: In Tauri, thumbnail generation is handled differently (client-side)
            // The frontend will re-render the thumbnail when needed
        } else {
            // No remaining 3D files - update thumbnail status
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute("UPDATE models SET thumbnail_status = 'failed', updated_at = ? WHERE id = ?", rusqlite::params![&now, &model_id]).map_err(|e| e.to_string())?;
        }
    }
    
    Ok(())
}

#[tauri::command]
pub async fn toggle_favorite(app: AppHandle, id: String) -> Result<bool, String> {
    let conn = get_db_connection(&app).map_err(|e| e.to_string())?;
    let user_id = DEFAULT_USER_ID.to_string();

    let exists: bool = conn.query_row(
        "SELECT 1 FROM favorites WHERE user_id = ? AND model_id = ?",
        rusqlite::params![&user_id, &id],
        |_| Ok(true),
    ).unwrap_or(false);

    if exists {
        conn.execute("DELETE FROM favorites WHERE user_id = ? AND model_id = ?", rusqlite::params![&user_id, &id]).map_err(|e| e.to_string())?;
        Ok(false)
    } else {
        conn.execute("INSERT INTO favorites (user_id, model_id) VALUES (?, ?)", rusqlite::params![&user_id, &id]).map_err(|e| e.to_string())?;
        Ok(true)
    }
}

/// Read an uploaded asset off disk and return its bytes as base64.
/// The frontend wraps these in a Blob URL so Three.js loaders don't have to
/// `fetch()` a custom-scheme URL (which WebView2 on Windows intercepts
/// differently from WKWebView, leaving the viewer and thumbnail renderer blank).
#[tauri::command]
pub async fn read_file(app: AppHandle, path: String) -> Result<String, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let uploads_dir = app_data_dir.join("uploads");

    // Accept either the mapped `slag3d-asset://uploads/...` form or a raw
    // `/uploads/...` / `uploads/...` public path.
    let relative = path
        .strip_prefix("slag3d-asset://")
        .or_else(|| path.strip_prefix("slag3d-asset:"))
        .unwrap_or(&path);
    let relative = relative
        .strip_prefix("localhost/")
        .unwrap_or(relative)
        .trim_start_matches('/');
    // Drop the leading `uploads/` so we join directly under uploads_dir.
    let relative = relative.strip_prefix("uploads/").unwrap_or(relative);

    let file_path = uploads_dir.join(relative);

    // Security: the resolved path must stay inside the uploads tree.
    // On Windows, canonicalize() may prepend a \\?\ long-path prefix, so
    // compare using the normalized string form instead of raw Path::starts_with.
    let canonical_uploads = fs::canonicalize(&uploads_dir).map_err(|e| e.to_string())?;
    let canonical_file = fs::canonicalize(&file_path).map_err(|e| e.to_string())?;
    if !path_starts_with(&canonical_file, &canonical_uploads) || !canonical_file.is_file() {
        return Err("Access denied".to_string());
    }

    let content = fs::read(&canonical_file).map_err(|e| e.to_string())?;
    Ok(general_purpose::STANDARD.encode(&content))
}

/// Persist a downloaded asset to a user-chosen path.
///
/// The native save dialog runs on the main thread and returns a path the user
/// picked. Writing the file ourselves (rather than relying on a blob-URL anchor
/// click) is the only reliable way to trigger a real download in a Tauri
/// WebView — on Linux AppImage the anchor click silently does nothing, which
/// is why the Download button appeared to be dead.
#[tauri::command]
pub async fn save_downloaded_file(
    app: AppHandle,
    source_path: String,
    dest_path: String,
) -> Result<(), String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let uploads_dir = app_data_dir.join("uploads");

    // Resolve the source exactly like read_file does, so the security
    // containment check applies to downloads too.
    let relative = source_path
        .strip_prefix("slag3d-asset://")
        .or_else(|| source_path.strip_prefix("slag3d-asset:"))
        .unwrap_or(&source_path);
    let relative = relative
        .strip_prefix("localhost/")
        .unwrap_or(relative)
        .trim_start_matches('/');
    let relative = relative.strip_prefix("uploads/").unwrap_or(relative);

    let file_path = uploads_dir.join(relative);
    let canonical_uploads = fs::canonicalize(&uploads_dir).map_err(|e| e.to_string())?;
    let canonical_file = fs::canonicalize(&file_path).map_err(|e| e.to_string())?;
    if !path_starts_with(&canonical_file, &canonical_uploads) || !canonical_file.is_file() {
        return Err("Access denied".to_string());
    }

    let content = fs::read(&canonical_file).map_err(|e| e.to_string())?;
    // The parent directory of the user's chosen path may not exist yet.
    if let Some(parent) = std::path::Path::new(&dest_path).parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&dest_path, &content).map_err(|e| e.to_string())?;
    Ok(())
}

/// Compare two paths for prefix containment, normalizing Windows \\?\ long-path
/// prefixes so a canonicalized existing directory and a plain file path
/// compare correctly.
fn path_starts_with(path: &Path, base: &Path) -> bool {
    // Strip Windows \\?\ long-path prefix BEFORE normalizing separators,
    // otherwise the prefix becomes //?/ after the replace and won't match.
    let p = path.to_string_lossy();
    let p = p.strip_prefix(r"\\?\").unwrap_or(&p);
    let b = base.to_string_lossy();
    let b = b.strip_prefix(r"\\?\").unwrap_or(&b);
    // Normalize backslashes to forward slashes for consistent comparison
    p.replace('\\', "/").starts_with(&b.replace('\\', "/"))
}

#[tauri::command]
pub async fn save_thumbnail(app: AppHandle, id: String, image_base64: String) -> Result<String, String> {
    let conn = get_db_connection(&app).map_err(|e| e.to_string())?;

    // Decode base64
    let image_data = general_purpose::STANDARD.decode(&image_base64).map_err(|e| e.to_string())?;

    // Save thumbnail to uploads/thumbnails/<id>.png
    let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let uploads_dir = app_data.join("uploads");
    let thumbnails_dir = uploads_dir.join("thumbnails");
    fs::create_dir_all(&thumbnails_dir).map_err(|e| e.to_string())?;
    let thumbnail_path = thumbnails_dir.join(format!("{}.png", id));
    fs::write(&thumbnail_path, &image_data).map_err(|e| e.to_string())?;

    // Store public path so the asset protocol can serve it
    let public_path = format!("/uploads/thumbnails/{}.png", id);

    // Update model
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE models SET thumbnail_path = ?, thumbnail_status = 'completed', updated_at = ? WHERE id = ?",
        rusqlite::params![&public_path, &now, &id],
    ).map_err(|e| e.to_string())?;

    Ok(public_path)
}
