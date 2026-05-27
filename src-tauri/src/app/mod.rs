use crate::agent::orchestrator::{Orchestrator, OrchestratorState};
use crate::ui;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let orchestrator = Orchestrator::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_shell::init())
        .manage(OrchestratorState(orchestrator))
        .invoke_handler(tauri::generate_handler![
            ui::commands::generate_replies,
            ui::commands::regenerate_candidates,
            ui::commands::regenerate_with_style,
            ui::commands::record_copy,
            ui::commands::hide_window,
            ui::commands::open_settings,
            ui::commands::show_popup,
            ui::commands::get_settings,
            ui::commands::save_settings,
            ui::commands::reset_settings,
            ui::commands::record_hotkey,
        ])
        .setup(|app| {
            tracing::info!("EchoMate starting up");

            if let Err(e) = ui::tray::setup_tray(app.handle()) {
                tracing::warn!("Tray setup failed: {}", e);
            }

            // Initialize orchestrator via handle
            let handle = app.handle().clone();
            let state = handle.try_state::<OrchestratorState>()
                .expect("OrchestratorState not found");
            state.0.init(&handle);

            tracing::info!("EchoMate ready");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running EchoMate");
}
