#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Native <select> popups are drawn by GTK on Linux; keep them dark.
    if std::env::var("GTK_THEME").is_err() {
        std::env::set_var("GTK_THEME", "Adwaita:dark");
    }
    slag3d::run()
}
