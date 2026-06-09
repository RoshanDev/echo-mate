use crate::agent::orchestrator::{AppConfig, OrchestratorState};
use crate::domain::{
    ContactInput, ContactRecord, MacosContextSnapshot, MemoryCandidate, MemoryItemRecord,
    PermissionStatus, PlatformSignal, PlatformSignalResult, ReminderCandidate, ReminderDetail,
    ReplyFeedbackRecord, StyleProfileRecord,
};
use crate::platform::macos_context::MacosContextHelper;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

/// Generate replies from clipboard text or clipboard image.
#[tauri::command]
pub async fn generate_replies(
    app: AppHandle,
    state: State<'_, OrchestratorState>,
) -> Result<(), String> {
    state.0.trigger(&app).await.map(|_| ())
}

/// Generate replies from a chat context screenshot.
#[tauri::command]
pub async fn generate_replies_from_screenshot(
    app: AppHandle,
    state: State<'_, OrchestratorState>,
) -> Result<(), String> {
    state.0.trigger_from_screenshot(&app).await.map(|_| ())
}

/// Generate proactive topic starters without relying on the latest message.
#[tauri::command]
pub async fn generate_topics(
    app: AppHandle,
    state: State<'_, OrchestratorState>,
) -> Result<(), String> {
    state.0.trigger_topics(&app).await.map(|_| ())
}

/// Regenerate with a style modifier
#[tauri::command]
pub async fn regenerate_candidates(
    app: AppHandle,
    state: State<'_, OrchestratorState>,
) -> Result<(), String> {
    state.0.regenerate_last(&app).await.map(|_| ())
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

    let result = state.0.regenerate_last(&app).await;

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
    if let Some(state) = app.try_state::<OrchestratorState>() {
        let _ = state
            .0
            .record_reply_feedback("copy", candidate_index as i64)
            .map_err(|e| tracing::warn!("Failed to record copy feedback: {e}"));
    }
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
    state: State<'_, OrchestratorState>,
    candidate_index: usize,
    text: String,
) -> Result<(), String> {
    app.clipboard()
        .write_text(&text)
        .map_err(|e| format!("Clipboard write error: {}", e))?;
    tracing::info!("User copied candidate {}", candidate_index);
    let _ = state
        .0
        .record_reply_feedback_with_text("copy", candidate_index as i64, &text)
        .map_err(|e| tracing::warn!("Failed to record copy feedback: {e}"));
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
        let _ = window.set_always_on_top(false);
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
        let _ = window.set_always_on_top(false);
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

/// Save a user-confirmed memory candidate.
#[tauri::command]
pub async fn save_memory_candidate(
    state: State<'_, OrchestratorState>,
    candidate: MemoryCandidate,
) -> Result<MemoryItemRecord, String> {
    state.0.save_memory_candidate(candidate)
}

/// Create a reminder from a user-confirmed reminder candidate.
#[tauri::command]
pub async fn create_reminder_from_candidate(
    app: AppHandle,
    state: State<'_, OrchestratorState>,
    candidate: ReminderCandidate,
    trigger_at: Option<String>,
) -> Result<ReminderDetail, String> {
    state
        .0
        .create_reminder_from_candidate(&app, candidate, trigger_at)
        .await
}

/// Record that the user ignored a memory candidate without saving it.
#[tauri::command]
pub async fn ignore_memory_candidate(
    state: State<'_, OrchestratorState>,
    candidate_index: usize,
) -> Result<ReplyFeedbackRecord, String> {
    state
        .0
        .record_reply_feedback("ignore_memory", candidate_index as i64)
}

/// Record that the user ignored a reminder candidate without scheduling it.
#[tauri::command]
pub async fn ignore_reminder_candidate(
    state: State<'_, OrchestratorState>,
    candidate_index: usize,
) -> Result<ReplyFeedbackRecord, String> {
    state
        .0
        .record_reply_feedback("ignore_reminder", candidate_index as i64)
}

/// Soft-delete a confirmed memory item and cancel its reminders.
#[tauri::command]
pub async fn delete_memory(state: State<'_, OrchestratorState>, id: String) -> Result<(), String> {
    state.0.delete_memory(&id)
}

/// Cancel a scheduled or notified reminder.
#[tauri::command]
pub async fn delete_reminder(
    state: State<'_, OrchestratorState>,
    id: String,
) -> Result<(), String> {
    state.0.delete_reminder(&id)
}

/// Return the latest notified reminder so the panel can recover after navigation.
#[tauri::command]
pub async fn get_latest_notified_reminder(
    state: State<'_, OrchestratorState>,
) -> Result<Option<ReminderDetail>, String> {
    state.0.latest_notified_reminder()
}

/// List local contacts and their allowlist state.
#[tauri::command]
pub async fn list_contacts(
    state: State<'_, OrchestratorState>,
) -> Result<Vec<ContactRecord>, String> {
    state.0.list_contacts()
}

/// Create or update a local contact.
#[tauri::command]
pub async fn upsert_contact(
    state: State<'_, OrchestratorState>,
    contact: ContactInput,
) -> Result<ContactRecord, String> {
    state.0.upsert_contact(contact)
}

/// Delete a contact and clear its local context.
#[tauri::command]
pub async fn delete_contact(state: State<'_, OrchestratorState>, id: String) -> Result<(), String> {
    state.0.delete_contact(&id)
}

/// Clear recent messages and context summaries for a contact.
#[tauri::command]
pub async fn clear_contact_context(
    state: State<'_, OrchestratorState>,
    id: String,
) -> Result<(), String> {
    state.0.clear_contact_context(&id)
}

/// Set the active contact used for prompt context and allowlist gating.
#[tauri::command]
pub async fn set_active_contact(
    state: State<'_, OrchestratorState>,
    contact_id: String,
) -> Result<(), String> {
    state.0.set_active_contact(contact_id)
}

/// Delete a persisted context summary from the popup.
#[tauri::command]
pub async fn delete_context_summary(
    state: State<'_, OrchestratorState>,
    id: String,
) -> Result<(), String> {
    state.0.delete_context_summary(&id)
}

/// Return the local style profile summary used for prompt context.
#[tauri::command]
pub async fn get_style_profile(
    state: State<'_, OrchestratorState>,
) -> Result<Option<StyleProfileRecord>, String> {
    state.0.style_profile()
}

/// Rebuild the local style profile from adopted replies already saved on disk.
#[tauri::command]
pub async fn refresh_style_profile(
    state: State<'_, OrchestratorState>,
) -> Result<Option<StyleProfileRecord>, String> {
    state.0.refresh_style_profile()
}

/// Clear the local style profile.
#[tauri::command]
pub async fn reset_style_profile(state: State<'_, OrchestratorState>) -> Result<(), String> {
    state.0.reset_style_profile()
}

/// Return transparent platform permission/fallback state.
#[tauri::command]
pub async fn get_permission_status(
    state: State<'_, OrchestratorState>,
) -> Result<PermissionStatus, String> {
    Ok(state.0.permission_status())
}

/// Return a one-shot macOS approximate context snapshot without saving it.
#[tauri::command]
pub async fn get_macos_context_snapshot(
    app: AppHandle,
    state: State<'_, OrchestratorState>,
) -> Result<MacosContextSnapshot, String> {
    let config = state.0.config.lock().unwrap().clone();
    let pasteboard_text = if cfg!(target_os = "macos") && config.macos_context_helper_enabled {
        app.clipboard().read_text().ok()
    } else {
        None
    };
    Ok(MacosContextHelper::collect(
        config.macos_context_helper_enabled,
        config.macos_accessibility_enabled,
        pasteboard_text,
    ))
}

/// Accept a local approximate inbound signal without auto-generating or sending.
#[tauri::command]
pub async fn ingest_platform_signal(
    app: AppHandle,
    state: State<'_, OrchestratorState>,
    signal: PlatformSignal,
) -> Result<PlatformSignalResult, String> {
    state.0.ingest_platform_signal(&app, signal)
}
