use crate::agent::orchestrator::{AppConfig, OrchestratorState};
use crate::domain::{
    ContactFactCandidate, ContactFactClassification, ContactFactRecord, ContactInput,
    ContactRecord, DataAuditReport, MacosContextSnapshot, MemoryCandidate, MemoryCandidateRecord,
    MemoryItemRecord, PermissionStatus, PlatformSignal, PlatformSignalResult, PrivacyGuideStatus,
    RelationshipCard, ReminderCandidate, ReminderCenterItem, ReminderDetail, ReplyFeedbackRecord,
    StyleProfileRecord,
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
    topic_hint: Option<String>,
) -> Result<(), String> {
    state.0.trigger_topics(&app, topic_hint).await.map(|_| ())
}

/// Return the latest generated candidate view so navigation back from settings can restore it.
#[tauri::command]
pub async fn get_last_generation_view(
    state: State<'_, OrchestratorState>,
) -> Result<Option<serde_json::Value>, String> {
    Ok(state.0.last_generation_view())
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
    mut settings: AppConfig,
) -> Result<(), String> {
    {
        let mut config = state.0.config.lock().unwrap();
        if settings.active_contact_id.trim().is_empty() {
            settings.active_contact_id = config.active_contact_id.clone();
        }
        if !settings.privacy_onboarding_completed {
            settings.privacy_onboarding_completed = config.privacy_onboarding_completed;
        }
        *config = settings;
    }
    state.0.clear_last_generation_view();
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
    state.0.clear_last_generation_view();
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

/// List pending memory candidates persisted from previous provider runs.
#[tauri::command]
pub async fn list_memory_candidate_inbox(
    state: State<'_, OrchestratorState>,
    contact_id: Option<String>,
) -> Result<Vec<MemoryCandidateRecord>, String> {
    state.0.list_memory_candidate_inbox(contact_id)
}

/// Confirm one pending memory candidate and save it as long-term memory.
#[tauri::command]
pub async fn confirm_memory_candidate_record(
    state: State<'_, OrchestratorState>,
    id: String,
) -> Result<MemoryItemRecord, String> {
    state.0.confirm_memory_candidate_record(&id)
}

/// Ignore one pending memory candidate.
#[tauri::command]
pub async fn ignore_memory_candidate_record(
    state: State<'_, OrchestratorState>,
    id: String,
) -> Result<(), String> {
    state.0.ignore_memory_candidate_record(&id)
}

/// List scheduled/notified reminders for the active or selected contact.
#[tauri::command]
pub async fn list_reminders(
    state: State<'_, OrchestratorState>,
    contact_id: Option<String>,
) -> Result<Vec<ReminderCenterItem>, String> {
    state.0.list_reminders(contact_id)
}

/// Mark a reminder as completed.
#[tauri::command]
pub async fn complete_reminder(
    state: State<'_, OrchestratorState>,
    id: String,
) -> Result<(), String> {
    state.0.complete_reminder(&id)
}

/// Snooze a reminder by minutes.
#[tauri::command]
pub async fn snooze_reminder(
    state: State<'_, OrchestratorState>,
    id: String,
    minutes: i64,
) -> Result<(), String> {
    state.0.snooze_reminder_minutes(&id, minutes)
}

/// Mute reminders by contact and/or reminder kind.
#[tauri::command]
pub async fn mute_reminders(
    state: State<'_, OrchestratorState>,
    contact_id: Option<String>,
    kind: Option<String>,
    hours: i64,
) -> Result<(), String> {
    state.0.mute_reminders(contact_id, kind, hours)
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
    state.0.clear_last_generation_view();
    state.0.upsert_contact(contact)
}

/// Classify a user-entered contact note into structured manual facts.
#[tauri::command]
pub async fn classify_contact_facts(
    state: State<'_, OrchestratorState>,
    contact_id: String,
    note: String,
) -> Result<ContactFactClassification, String> {
    state.0.classify_contact_facts(&contact_id, &note).await
}

/// Save user-approved manual facts for a contact.
#[tauri::command]
pub async fn save_contact_facts(
    state: State<'_, OrchestratorState>,
    contact_id: String,
    facts: Vec<ContactFactCandidate>,
) -> Result<Vec<ContactFactRecord>, String> {
    state.0.clear_last_generation_view();
    state.0.save_contact_facts(&contact_id, facts)
}

/// List saved manual/structured facts for a contact.
#[tauri::command]
pub async fn list_contact_facts(
    state: State<'_, OrchestratorState>,
    contact_id: String,
) -> Result<Vec<ContactFactRecord>, String> {
    state.0.list_contact_facts(&contact_id)
}

/// Return a single-contact relationship card.
#[tauri::command]
pub async fn get_relationship_card(
    state: State<'_, OrchestratorState>,
    contact_id: Option<String>,
) -> Result<RelationshipCard, String> {
    state.0.relationship_card(contact_id)
}

/// Return local data audit counts and contamination findings.
#[tauri::command]
pub async fn get_data_audit_report(
    state: State<'_, OrchestratorState>,
) -> Result<DataAuditReport, String> {
    state.0.data_audit_report()
}

/// Export a local JSON snapshot through the controlled backend.
#[tauri::command]
pub async fn export_data_snapshot(
    state: State<'_, OrchestratorState>,
) -> Result<serde_json::Value, String> {
    state.0.export_data_snapshot()
}

/// Clear all local EchoMate data.
#[tauri::command]
pub async fn clear_all_data(state: State<'_, OrchestratorState>) -> Result<(), String> {
    state.0.clear_all_data()
}

/// Clear local EchoMate log files.
#[tauri::command]
pub async fn clear_logs(state: State<'_, OrchestratorState>) -> Result<(), String> {
    state.0.clear_logs()
}

/// Return privacy guide status and data/log paths.
#[tauri::command]
pub async fn get_privacy_guide_status(
    state: State<'_, OrchestratorState>,
) -> Result<PrivacyGuideStatus, String> {
    Ok(state.0.privacy_guide_status())
}

/// Mark the privacy guide as acknowledged.
#[tauri::command]
pub async fn acknowledge_privacy_guide(state: State<'_, OrchestratorState>) -> Result<(), String> {
    state.0.acknowledge_privacy_guide();
    Ok(())
}

/// Delete one saved contact fact.
#[tauri::command]
pub async fn delete_contact_fact(
    state: State<'_, OrchestratorState>,
    id: String,
) -> Result<(), String> {
    state.0.clear_last_generation_view();
    state.0.delete_contact_fact(&id)
}

/// Delete a contact and clear its local context.
#[tauri::command]
pub async fn delete_contact(state: State<'_, OrchestratorState>, id: String) -> Result<(), String> {
    state.0.clear_last_generation_view();
    state.0.delete_contact(&id)
}

/// Clear recent messages and context summaries for a contact.
#[tauri::command]
pub async fn clear_contact_context(
    state: State<'_, OrchestratorState>,
    id: String,
) -> Result<(), String> {
    state.0.clear_last_generation_view();
    state.0.clear_contact_context(&id)
}

/// Set the active contact used for prompt context and allowlist gating.
#[tauri::command]
pub async fn set_active_contact(
    state: State<'_, OrchestratorState>,
    contact_id: String,
) -> Result<(), String> {
    state.0.clear_last_generation_view();
    state.0.set_active_contact(contact_id)
}

/// Delete a persisted context summary from the popup.
#[tauri::command]
pub async fn delete_context_summary(
    state: State<'_, OrchestratorState>,
    id: String,
) -> Result<(), String> {
    state.0.clear_last_generation_view();
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
