// App boot and dependency wiring

use crate::ui;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            ui::commands::generate_replies,
        ])
        .setup(|_app| {
            tracing::info!("EchoMate starting up");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running EchoMate");
}
