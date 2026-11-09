use tauri::Builder;

pub mod db;

pub fn run() {
    Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
