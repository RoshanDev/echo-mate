use crate::agent::orchestrator::{log_dir_path, Orchestrator, OrchestratorState};
use crate::ui;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing subscriber — log to file + stderr
    let log_dir = log_dir_path();
    let _ = std::fs::create_dir_all(&log_dir);
    let file_appender = tracing_appender::rolling::daily(&log_dir, "echomate.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(true)
        .with_thread_ids(true)
        .with_writer(non_blocking)
        .init();

    let orchestrator = Orchestrator::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .manage(OrchestratorState(orchestrator))
        .invoke_handler(tauri::generate_handler![
            ui::commands::generate_replies,
            ui::commands::generate_replies_from_screenshot,
            ui::commands::add_screenshot_to_batch,
            ui::commands::clear_screenshot_batch,
            ui::commands::generate_replies_from_screenshot_batch,
            ui::commands::generate_topics,
            ui::commands::get_last_generation_view,
            ui::commands::regenerate_candidates,
            ui::commands::regenerate_with_style,
            ui::commands::record_copy,
            ui::commands::copy_candidate,
            ui::commands::hide_window,
            ui::commands::open_settings,
            ui::commands::show_popup,
            ui::commands::get_settings,
            ui::commands::save_settings,
            ui::commands::reset_settings,
            ui::commands::record_hotkey,
            ui::commands::save_memory_candidate,
            ui::commands::create_reminder_from_candidate,
            ui::commands::ignore_memory_candidate,
            ui::commands::ignore_reminder_candidate,
            ui::commands::list_memory_candidate_inbox,
            ui::commands::confirm_memory_candidate_record,
            ui::commands::confirm_memory_candidate_record_with_edits,
            ui::commands::ignore_memory_candidate_record,
            ui::commands::list_reminders,
            ui::commands::complete_reminder,
            ui::commands::snooze_reminder,
            ui::commands::mute_reminders,
            ui::commands::delete_memory,
            ui::commands::delete_reminder,
            ui::commands::get_latest_notified_reminder,
            ui::commands::list_contacts,
            ui::commands::upsert_contact,
            ui::commands::classify_contact_facts,
            ui::commands::save_contact_facts,
            ui::commands::list_contact_facts,
            ui::commands::get_relationship_card,
            ui::commands::get_data_audit_report,
            ui::commands::export_data_snapshot,
            ui::commands::clear_all_data,
            ui::commands::clear_logs,
            ui::commands::get_privacy_guide_status,
            ui::commands::acknowledge_privacy_guide,
            ui::commands::delete_contact_fact,
            ui::commands::delete_contact,
            ui::commands::clear_contact_context,
            ui::commands::set_active_contact,
            ui::commands::delete_context_summary,
            ui::commands::get_style_profile,
            ui::commands::refresh_style_profile,
            ui::commands::reset_style_profile,
            ui::commands::get_permission_status,
            ui::commands::get_macos_context_snapshot,
            ui::commands::ingest_platform_signal,
        ])
        .setup(|app| {
            tracing::info!("EchoMate starting up");

            if let Err(e) = ui::tray::setup_tray(app.handle()) {
                tracing::warn!("Tray setup failed: {}", e);
            }

            // Initialize orchestrator via handle
            let handle = app.handle().clone();
            let state = handle
                .try_state::<OrchestratorState>()
                .expect("OrchestratorState not found");
            state.0.init(&handle);

            tracing::info!("EchoMate ready");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running EchoMate");
}
