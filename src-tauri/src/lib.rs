use tauri::{Builder, Manager};

pub mod commands;
pub mod db;
pub mod protocol;
pub mod services;

use rusqlite;

pub fn run() {
    protocol::register_asset_protocol(Builder::default())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // On a fresh machine the app data directory does not exist yet.
            // SQLite cannot create parent directories, so opening the DB there
            // would panic and leave the window blank. Create the tree first.
            let app_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_dir)?;
            std::env::set_var("SLAG3D_APP_DATA_DIR", &app_dir);
            crate::services::storage::ensure_dirs()?;

            let db_path = app_dir.join("vault.db");
            println!("[SLAG3D] Initializing DB at: {:?}", db_path);
            let conn = rusqlite::Connection::open(&db_path)?;
            // Without a busy_timeout, concurrent writes (unlikely in a
            // single-user desktop app, but real during thumbnail renders that
            // update the same row) would return SQLITE_BUSY immediately and
            // leave the UI spinning. Five seconds is generous enough to wait
            // out any short-lived writer instead of failing outright.
            conn.busy_timeout(std::time::Duration::from_millis(5000))?;
            crate::db::schema::init_schema(&conn)?;
            println!("[SLAG3D] DB initialized successfully");

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::models::get_models,
            commands::models::get_model_detail,
            commands::models::import_models,
            commands::models::update_model,
            commands::models::delete_model,
            commands::models::delete_model_file,
            commands::models::toggle_favorite,
            commands::models::save_thumbnail,
            commands::models::add_model_files,
            commands::models::read_file,
            commands::collections::get_collections,
            commands::collections::create_collection,
            commands::collections::rename_collection,
            commands::collections::delete_collection,
            commands::collections::add_model_to_collection,
            commands::collections::remove_model_from_collection,
            commands::tags::get_tags,
            commands::app::init_db,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
