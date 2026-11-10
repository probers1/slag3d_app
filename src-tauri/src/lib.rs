use tauri::Builder;

pub mod commands;
pub mod db;
pub mod protocol;
pub mod services;

pub fn run() {
    protocol::register_asset_protocol(Builder::default())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::models::get_models,
            commands::models::get_model_detail,
            commands::models::import_models,
            commands::models::update_model,
            commands::models::delete_model,
            commands::models::toggle_favorite,
            commands::models::save_thumbnail,
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
