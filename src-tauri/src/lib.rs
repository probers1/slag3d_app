use tauri::Builder;

pub mod db;
pub mod protocol;
pub mod services;

pub fn run() {
    protocol::register_asset_protocol(Builder::default())
        .plugin(tauri_plugin_dialog::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
