use crate::agent::orchestrator::{AppConfig, OrchestratorState};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

/// Generate replies from clipboard text
#[tauri::command]
pub async fn generate_replies(
    app: AppHandle,
    state: State<'_, OrchestratorState>,
) -> Result<(), String> {
    state.0.trigger(&app).await.map(|_| ())
}

/// Regenerate with a style modifier
#[tauri::command]
pub async fn regenerate_candidates(
    app: AppHandle,
    state: State<'_, OrchestratorState>,
) -> Result<(), String> {
    state.0.trigger(&app).await.map(|_| ())
}

/// Regenerate with specific style
#[tauri::command]
pub async fn regenerate_with_style(
    app: AppHandle,
    state: State<'_, OrchestratorState>,
    style: String,
) -> Result<(), String> {
    // Temporarily override style
    let original_tone = {
        let mut config = state.0.config.lock().unwrap();
        let original = config.tone.clone();
        match style.as_str() {
            "conservative" => config.tone = "formal".into(),
            "fun" => config.tone = "humorous".into(),
            _ => {}
        }
        original
    };

    let result = state.0.trigger(&app).await;

    // Restore original tone
    {
        let mut config = state.0.config.lock().unwrap();
        config.tone = original_tone;
    }

    result.map(|_| ())
}

/// Record that user copied a candidate
#[tauri::command]
pub async fn record_copy(app: AppHandle, candidate_index: usize) -> Result<(), String> {
    tracing::info!("User copied candidate {}", candidate_index);
    let _ = app.emit(
        "copy-recorded",
        serde_json::json!({"index": candidate_index}),
    );
    Ok(())
}

/// Copy a candidate through Tauri's clipboard plugin.
#[tauri::command]
pub async fn copy_candidate(
    app: AppHandle,
    candidate_index: usize,
    text: String,
) -> Result<(), String> {
    app.clipboard()
        .write_text(text)
        .map_err(|e| format!("Clipboard write error: {}", e))?;
    tracing::info!("User copied candidate {}", candidate_index);
    let _ = app.emit(
        "copy-recorded",
        serde_json::json!({"index": candidate_index}),
    );
    Ok(())
}

/// Hide the popup window
#[tauri::command]
pub async fn hide_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    Ok(())
}

/// Open settings window
#[tauri::command]
pub async fn open_settings(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        // Navigate to settings page within the same window
        let _ = window.eval("window.location.href = 'settings.html'");
        let _ = window.show();
        let _ = window.set_focus();
    }
    Ok(())
}

/// Show main popup
#[tauri::command]
pub async fn show_popup(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.eval("window.location.href = 'index.html'");
        let _ = window.show();
        let _ = window.set_focus();
    }
    Ok(())
}

/// Get current settings
#[tauri::command]
pub async fn get_settings(state: State<'_, OrchestratorState>) -> Result<AppConfig, String> {
    Ok(state.0.config.lock().unwrap().clone())
}

/// Save settings
#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    state: State<'_, OrchestratorState>,
    settings: AppConfig,
) -> Result<(), String> {
    {
        let mut config = state.0.config.lock().unwrap();
        *config = settings;
    }
    state.0.reload_hotkey(&app);
    state.0.save_config_to_disk();
    tracing::info!("Settings saved");
    Ok(())
}

/// Reset settings to defaults
#[tauri::command]
pub async fn reset_settings(
    app: AppHandle,
    state: State<'_, OrchestratorState>,
) -> Result<(), String> {
    {
        let mut config = state.0.config.lock().unwrap();
        *config = AppConfig::default();
    }
    state.0.reload_hotkey(&app);
    state.0.save_config_to_disk();
    tracing::info!("Settings reset to defaults");
    Ok(())
}

/// Record a new hotkey (called when user presses record)
#[tauri::command]
pub async fn record_hotkey(_app: AppHandle) -> Result<String, String> {
    // For MVP, return current hotkey — actual recording requires
    // listening for next keypress which is complex with global shortcuts
    Ok("CmdOrCtrl+Shift+Space".into())
}
