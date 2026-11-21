use std::fs;
use std::path::{Path, PathBuf};

use tauri::AppHandle;
use uuid::Uuid;

use crate::db::{get_db_connection, schema::init_schema};
use crate::services::backup::{
    extract_backup_archive, replace_uploads_dir, restore_database, restore_previous_uploads,
    schema_version, snapshot_database, validate_backup_db, BackupInfo, DB_NAME,
};
use crate::services::storage::{ensure_dirs, get_app_data_dir, get_backup_work_dir};

#[tauri::command]
pub async fn create_backup(app: AppHandle, destination: String) -> Result<BackupInfo, String> {
    let conn = get_db_connection(&app)?;
    let app_data = get_app_data_dir();
    let work = get_backup_work_dir().join(format!("export-{}", Uuid::new_v4()));

    let result = crate::services::backup::write_backup_archive(
        &conn,
        &app_data.join("uploads"),
        &work,
        Path::new(&destination),
        env!("CARGO_PKG_VERSION"),
    );
    let _ = fs::remove_dir_all(&work);
    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_backup(app: AppHandle, archive_path: String) -> Result<BackupInfo, String> {
    let archive = PathBuf::from(&archive_path);
    if !archive.is_file() {
        return Err("Backup file not found".to_string());
    }

    let app_data = get_app_data_dir();
    let work = get_backup_work_dir().join(format!("import-{}", Uuid::new_v4()));

    let manifest = extract_backup_archive(&archive, &work).map_err(|e| e.to_string())?;
    validate_backup_db(&work.join(DB_NAME)).map_err(|e| e.to_string())?;

    let mut conn = get_db_connection(&app)?;
    if manifest.schema_version > schema_version(&conn) {
        let _ = fs::remove_dir_all(&work);
        return Err(format!(
            "Backup needs a newer SLAG3D version (schema v{}); update the app first.",
            manifest.schema_version
        ));
    }
    let rollback_db = work.join("pre-restore.db");
    let mut result = snapshot_database(&conn, &rollback_db).map_err(|e| e.to_string());
    let mut staged_uploads: Option<PathBuf> = None;

    if result.is_ok() {
        result = (|| -> Result<(), String> {
            restore_database(&work.join(DB_NAME), &mut conn).map_err(|e| e.to_string())?;
            // A backup made by an older version may need migrations (e.g. v5 -> v6).
            init_schema(&conn).map_err(|e| e.to_string())?;
            staged_uploads =
                replace_uploads_dir(&app_data, &work.join("uploads")).map_err(|e| e.to_string())?;
            ensure_dirs().map_err(|e| e.to_string())?;
            Ok(())
        })();

        if result.is_err() {
            // Roll back both halves so the DB never points at uploads that are
            // gone (or at a partially replaced tree).
            let _ = restore_database(&rollback_db, &mut conn);
            if let Err(e) = restore_previous_uploads(&app_data, staged_uploads.as_deref()) {
                eprintln!("[SLAG3D] Failed to roll back uploads after restore error: {}", e);
            }
        }
    }

    // Only discard the pre-restore tree once the whole operation succeeded.
    if result.is_ok() {
        if let Some(staged) = staged_uploads.as_deref() {
            let _ = fs::remove_dir_all(staged);
        }
    }

    let _ = fs::remove_dir_all(&work);
    result?;

    Ok(BackupInfo {
        path: archive_path,
        size: fs::metadata(&archive).map_err(|e| e.to_string())?.len(),
        created_at: manifest.created_at,
        model_count: manifest.model_count,
    })
}
