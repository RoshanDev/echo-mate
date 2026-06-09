use crate::agent::parser::OutputParser;
use crate::agent::schema;
use crate::agent::PromptComposer;
use crate::domain::{
    BoundingBox, Candidate, CandidateEnvelope, ContactFactCandidate, ContactFactClassification,
    ContactFactRecord, ContactInput, ContactRecord, ContextPolicy, ContextSummaryCandidate,
    ContextSummaryRecord, DataAuditReport, MemoryCandidateRecord, MemoryItemRecord, NextAction,
    PermissionStatus, PlatformSignal, PlatformSignalResult, PrivacyGuideStatus, RelationshipCard,
    ReminderCenterItem, ScreenshotAnalysis, ScreenshotTurn, SourceCard, SourceContextRecord,
    StyleProfileRecord, SuggestionRunRecord,
};
use crate::platform::clipboard::{ClipboardImage, ClipboardManager};
use crate::platform::hotkey::HotkeyManager;
use crate::platform::input::InputSimulator;
use crate::platform::screenshot::ScreenCapture;
use crate::platform::vision_ocr::{self, OcrLine};
use crate::provider::claude::ClaudeProvider;
use crate::provider::codex::CodexProvider;
use crate::store::memory_repo::MemoryRepository;
use crate::ui::window::WindowManager;
use chrono::{DateTime, Duration as ChronoDuration, Local, TimeZone, Timelike, Utc};
use image::{ImageBuffer, ImageFormat, Rgba};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

const SELECTION_COPY_SETTLE: Duration = Duration::from_millis(250);
const CLIPBOARD_POLL_INTERVAL: Duration = Duration::from_millis(50);
const CLIPBOARD_POLL_ATTEMPTS: usize = 40;
const SCREENSHOT_POLL_INTERVAL: Duration = Duration::from_millis(250);
const SCREENSHOT_POLL_ATTEMPTS: usize = 240;
const REMINDER_POLL_INTERVAL: Duration = Duration::from_secs(30);
const RECENT_CONTACT_COOLDOWN_MINUTES: i64 = 20;
const RECENT_CONTACT_SNOOZE_MINUTES: i64 = 30;
const RECENT_CONTEXT_LIMIT: usize = 8;
const CONTACT_MEMORY_LIMIT: usize = 8;
const CONTACT_FACT_LIMIT: usize = 8;
const SOURCE_CARD_LIMIT: usize = 8;

fn default_context_retention_days() -> i64 {
    30
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub hotkey: String,
    pub primary_provider: String,
    pub fallback_provider: String,
    pub candidate_count: usize,
    pub timeout_seconds: u64,
    pub strict_privacy: bool,
    pub sqlcipher: bool,
    pub tone: String,
    pub length: String,
    pub emoji_level: f64,
    pub humor_level: f64,
    #[serde(default)]
    pub active_contact_id: String,
    #[serde(default)]
    pub global_privacy_mode: bool,
    #[serde(default = "default_context_retention_days")]
    pub context_retention_days: i64,
    #[serde(default)]
    pub windows_notification_helper_enabled: bool,
    #[serde(default)]
    pub macos_context_helper_enabled: bool,
    #[serde(default)]
    pub macos_accessibility_enabled: bool,
    #[serde(default)]
    pub privacy_onboarding_completed: bool,
    #[serde(default)]
    pub debug_log_body_enabled: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            hotkey: "CmdOrCtrl+Shift+Space".into(),
            primary_provider: "claude".into(),
            fallback_provider: "claude".into(),
            candidate_count: 5,
            timeout_seconds: 180,
            strict_privacy: true,
            sqlcipher: false,
            tone: "warm_calm".into(),
            length: "short_to_medium".into(),
            emoji_level: 0.2,
            humor_level: 0.3,
            active_contact_id: String::new(),
            global_privacy_mode: false,
            context_retention_days: default_context_retention_days(),
            windows_notification_helper_enabled: false,
            macos_context_helper_enabled: false,
            macos_accessibility_enabled: false,
            privacy_onboarding_completed: false,
            debug_log_body_enabled: false,
        }
    }
}

pub struct Orchestrator {
    pub config: Arc<Mutex<AppConfig>>,
    hotkey: HotkeyManager,
    clipboard: ClipboardManager,
    input: InputSimulator,
    screen_capture: ScreenCapture,
    window: WindowManager,
    prompt_composer: PromptComposer,
    parser: OutputParser,
    memory_repo: MemoryRepository,
    schema_dir: PathBuf,
    generation_in_progress: AtomicBool,
    last_generation_input: Mutex<Option<GenerationInput>>,
    last_generation_view: Mutex<Option<serde_json::Value>>,
}

impl Orchestrator {
    pub fn new() -> Self {
        let schema_dir = std::env::temp_dir().join("echomate-schemas");
        let config = Self::load_config();
        tracing::info!(
            "Config loaded: hotkey={}, provider={}",
            config.hotkey,
            config.primary_provider
        );
        let memory_repo = MemoryRepository::open_default().unwrap_or_else(|e| {
            tracing::error!("Failed to open EchoMate database: {e}; falling back to temp DB");
            let fallback = std::env::temp_dir().join("echomate-fallback.db");
            MemoryRepository::new(fallback).expect("fallback EchoMate database should open")
        });
        tracing::info!("Memory DB: {}", memory_repo.db_path().display());
        Self {
            config: Arc::new(Mutex::new(config)),
            hotkey: HotkeyManager::new(),
            clipboard: ClipboardManager::new(),
            input: InputSimulator::new(),
            screen_capture: ScreenCapture::new(),
            window: WindowManager::new(),
            prompt_composer: PromptComposer::new(),
            parser: OutputParser::new(),
            memory_repo,
            schema_dir,
            generation_in_progress: AtomicBool::new(false),
            last_generation_input: Mutex::new(None),
            last_generation_view: Mutex::new(None),
        }
    }

    fn config_path() -> PathBuf {
        if e2e_mock_provider_enabled() {
            return e2e_profile_dir().join("config.json");
        }
        // On Windows: %APPDATA%\EchoMate\config.json
        // On Linux/macOS: ~/.echomate/config.json
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("EchoMate").join("config.json");
        }
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join(".echomate").join("config.json")
    }

    fn load_config() -> AppConfig {
        let path = Self::config_path();
        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<AppConfig>(&content) {
                Ok(config) => {
                    tracing::info!("Loaded config from {}", path.display());
                    return config;
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse config file {}: {e}, using defaults",
                        path.display()
                    );
                }
            },
            Err(e) => {
                tracing::info!("No config file at {} ({e}), using defaults", path.display());
            }
        }
        AppConfig::default()
    }

    pub fn save_config_to_disk(&self) {
        let config = self.config.lock().unwrap().clone();
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match serde_json::to_string_pretty(&config) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, &json) {
                    tracing::error!("Failed to write config to {}: {e}", path.display());
                } else {
                    tracing::info!("Config saved to {}", path.display());
                }
            }
            Err(e) => {
                tracing::error!("Failed to serialize config: {e}");
            }
        }
    }

    pub async fn trigger(&self, app: &AppHandle) -> Result<CandidateEnvelope, String> {
        self.trigger_auto(app, TriggerInput::Clipboard).await
    }

    pub async fn trigger_from_selection(
        &self,
        app: &AppHandle,
    ) -> Result<CandidateEnvelope, String> {
        self.trigger_auto(app, TriggerInput::Selection).await
    }

    pub async fn regenerate_last(&self, app: &AppHandle) -> Result<CandidateEnvelope, String> {
        let input = self
            .last_generation_input
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| "还没有可重新生成的上下文，请先触发一次生成。".to_string())?;
        self.generate_with_guard(app, input).await
    }

    pub async fn trigger_topics(
        &self,
        app: &AppHandle,
        topic_hint: Option<String>,
    ) -> Result<CandidateEnvelope, String> {
        self.generate_with_guard(app, GenerationInput::Topic(topic_hint))
            .await
    }

    pub async fn trigger_from_screenshot(
        &self,
        app: &AppHandle,
    ) -> Result<CandidateEnvelope, String> {
        if self.generation_in_progress.swap(true, Ordering::AcqRel) {
            let message = "已有一次生成正在进行，请等当前请求完成或超时后再试".to_string();
            self.emit_generation_started(app, "busy", None);
            return Err(message);
        }
        let _generation_guard = GenerationGuard::new(&self.generation_in_progress);
        self.emit_generation_started(app, "screenshot", None);

        let screenshot = match self.capture_chat_screenshot(app).await {
            Ok(screenshot) => screenshot,
            Err(e) => {
                self.emit_generation_error(app, &e);
                return Err(e);
            }
        };

        self.generate_from_input(app, GenerationInput::Screenshot(screenshot))
            .await
    }

    async fn trigger_auto(
        &self,
        app: &AppHandle,
        source: TriggerInput,
    ) -> Result<CandidateEnvelope, String> {
        if self.generation_in_progress.swap(true, Ordering::AcqRel) {
            let message = "已有一次生成正在进行，请等当前请求完成或超时后再试".to_string();
            self.emit_generation_started(app, "busy", None);
            return Err(message);
        }
        let _generation_guard = GenerationGuard::new(&self.generation_in_progress);
        if !matches!(source, TriggerInput::Selection) {
            self.emit_generation_started(app, source.source_kind(), None);
        }

        let input = match self.resolve_trigger_input(app, source).await {
            Ok(input) => input,
            Err(e) => {
                self.emit_generation_error(app, &e);
                return Err(e);
            }
        };

        self.generate_from_input(app, input).await
    }

    async fn generate_with_guard(
        &self,
        app: &AppHandle,
        input: GenerationInput,
    ) -> Result<CandidateEnvelope, String> {
        if self.generation_in_progress.swap(true, Ordering::AcqRel) {
            let message = "已有一次生成正在进行，请等当前请求完成或超时后再试".to_string();
            self.emit_generation_started(app, "busy", None);
            return Err(message);
        }
        let _generation_guard = GenerationGuard::new(&self.generation_in_progress);
        self.emit_generation_started(app, input.source_kind(), None);

        self.generate_from_input(app, input).await
    }

    async fn generate_from_input(
        &self,
        app: &AppHandle,
        input: GenerationInput,
    ) -> Result<CandidateEnvelope, String> {
        match input.clone() {
            GenerationInput::Text(text) => self.generate_from_text(app, text, input).await,
            GenerationInput::Screenshot(screenshot) => {
                self.generate_from_screenshot_input(app, screenshot, input)
                    .await
            }
            GenerationInput::Topic(topic_hint) => {
                self.generate_from_topic(app, input, topic_hint).await
            }
        }
    }

    async fn generate_from_text(
        &self,
        app: &AppHandle,
        text: String,
        input: GenerationInput,
    ) -> Result<CandidateEnvelope, String> {
        if text.is_empty() {
            return Err("剪贴板为空，请先复制一段聊天内容再触发".to_string());
        }

        tracing::info!("Clipboard text length: {}", text.len());

        self.emit_generation_started(app, "text", Some(text.len()));

        let config = self.config.lock().unwrap().clone();
        let generation_context = self.generation_context(&config);
        let system_prompt = self.prompt_composer.system_prompt();
        let task_prompt = self.prompt_composer.task_prompt(
            &text,
            &generation_context.context_block,
            &config.tone,
            &config.length,
            config.emoji_level,
            config.humor_level,
        );
        let full_prompt = format!("{}\n\n---\n\n{}", system_prompt, task_prompt);

        let (schema_path, schema_json) = self.write_schema().await?;

        let result = if e2e_mock_provider_enabled() {
            Ok(mock_e2e_envelope("clipboard"))
        } else {
            self.call_provider(&config, &full_prompt, &schema_json, &schema_path)
                .await
        };

        match result {
            Ok(envelope) => {
                let envelope = self.apply_context_policy(envelope, &generation_context);
                let persisted = self.persist_generation_artifacts(
                    &envelope,
                    "clipboard",
                    &config.primary_provider,
                    &generation_context,
                    Some(&text),
                    None,
                );
                let source_cards = self.source_cards_for_view(
                    "clipboard",
                    &config.primary_provider,
                    &generation_context,
                    persisted.source_context.as_ref(),
                    Some(&text),
                    &envelope.context_summary.summary,
                );
                self.emit_candidates_ready(
                    app,
                    &envelope,
                    &config.primary_provider,
                    "standard",
                    &generation_context.policy,
                    persisted.context_record.as_ref(),
                    &source_cards,
                    persisted.suggestion_run.as_ref(),
                );
                self.remember_generation_input(input);
                Ok(envelope)
            }
            Err(e) => {
                self.emit_generation_error(app, &e);
                Err(e)
            }
        }
    }

    async fn generate_from_screenshot_input(
        &self,
        app: &AppHandle,
        screenshot: ScreenshotInput,
        input: GenerationInput,
    ) -> Result<CandidateEnvelope, String> {
        tracing::info!(
            "Screenshot context ready: {} ({}x{})",
            screenshot.path.display(),
            screenshot.width,
            screenshot.height
        );

        self.emit_generation_started(app, "screenshot", None);

        let config = self.config.lock().unwrap().clone();
        let generation_context = self.generation_context(&config);
        let local_screenshot_analysis = analyze_screenshot_locally(&screenshot);
        let system_prompt = self.prompt_composer.system_prompt();
        let task_prompt = self.prompt_composer.screenshot_task_prompt(
            screenshot.width,
            screenshot.height,
            &local_screenshot_analysis,
            &generation_context.context_block,
            &config.tone,
            &config.length,
            config.emoji_level,
            config.humor_level,
        );
        let full_prompt = format!("{}\n\n---\n\n{}", system_prompt, task_prompt);

        let (schema_path, _schema_json) = self.write_schema().await?;
        let result = if e2e_mock_provider_enabled() {
            Ok((mock_e2e_envelope("screenshot"), "e2e-mock".to_string()))
        } else {
            self.call_screenshot_provider(&config, &full_prompt, &schema_path, &screenshot.path)
                .await
        };

        match result {
            Ok((envelope, provider)) => {
                let mut envelope = self.apply_context_policy(envelope, &generation_context);
                envelope.screenshot_analysis = merge_screenshot_analysis(
                    local_screenshot_analysis,
                    envelope.screenshot_analysis,
                );
                let persisted = self.persist_generation_artifacts(
                    &envelope,
                    "screenshot",
                    &provider,
                    &generation_context,
                    None,
                    Some(&envelope.screenshot_analysis),
                );
                if let (Some(contact), Some(source_context)) = (
                    generation_context.contact.as_ref(),
                    persisted.source_context.as_ref(),
                ) {
                    if let Err(e) = self.memory_repo.insert_screenshot_analysis(
                        &contact.id,
                        Some(&source_context.id),
                        &screenshot.path.to_string_lossy(),
                        screenshot.width,
                        screenshot.height,
                        "apple_vision_or_provider",
                        &envelope.screenshot_analysis,
                    ) {
                        tracing::warn!("Failed to persist screenshot analysis: {e}");
                    }
                }
                let source_cards = self.source_cards_for_view(
                    "screenshot",
                    &provider,
                    &generation_context,
                    persisted.source_context.as_ref(),
                    None,
                    &envelope.context_summary.summary,
                );
                self.emit_candidates_ready(
                    app,
                    &envelope,
                    &provider,
                    "screenshot",
                    &generation_context.policy,
                    persisted.context_record.as_ref(),
                    &source_cards,
                    persisted.suggestion_run.as_ref(),
                );
                self.remember_generation_input(input);
                Ok(envelope)
            }
            Err(e) => {
                self.emit_generation_error(app, &e);
                Err(e)
            }
        }
    }

    async fn generate_from_topic(
        &self,
        app: &AppHandle,
        input: GenerationInput,
        topic_hint: Option<String>,
    ) -> Result<CandidateEnvelope, String> {
        tracing::info!("Generating proactive topic starters");
        self.emit_generation_started(app, "topic", None);

        let config = self.config.lock().unwrap().clone();
        let generation_context = self.generation_context(&config);
        let system_prompt = self.prompt_composer.system_prompt();
        let task_prompt = self.prompt_composer.topic_task_prompt(
            &generation_context.context_block,
            &config.tone,
            &config.length,
            config.emoji_level,
            config.humor_level,
            topic_hint.as_deref(),
        );
        let full_prompt = format!("{}\n\n---\n\n{}", system_prompt, task_prompt);
        let (schema_path, schema_json) = self.write_schema().await?;

        let result = if e2e_mock_provider_enabled() {
            Ok(mock_e2e_envelope("topic"))
        } else {
            self.call_provider(&config, &full_prompt, &schema_json, &schema_path)
                .await
        };

        match result {
            Ok(envelope) => {
                let envelope = self.apply_context_policy(envelope, &generation_context);
                let persisted = self.persist_generation_artifacts(
                    &envelope,
                    "topic",
                    &config.primary_provider,
                    &generation_context,
                    None,
                    None,
                );
                let source_cards = self.source_cards_for_view(
                    "topic",
                    &config.primary_provider,
                    &generation_context,
                    persisted.source_context.as_ref(),
                    topic_hint.as_deref(),
                    &envelope.context_summary.summary,
                );
                self.emit_candidates_ready(
                    app,
                    &envelope,
                    &config.primary_provider,
                    "topic",
                    &generation_context.policy,
                    persisted.context_record.as_ref(),
                    &source_cards,
                    persisted.suggestion_run.as_ref(),
                );
                self.remember_generation_input(input);
                Ok(envelope)
            }
            Err(e) => {
                self.emit_generation_error(app, &e);
                Err(e)
            }
        }
    }

    fn remember_generation_input(&self, input: GenerationInput) {
        *self.last_generation_input.lock().unwrap() = Some(input);
    }

    async fn write_schema(&self) -> Result<(PathBuf, serde_json::Value), String> {
        tokio::fs::create_dir_all(&self.schema_dir)
            .await
            .map_err(|e| e.to_string())?;
        let schema_path = self.schema_dir.join("reply_candidates.schema.json");
        let schema_json = schema::candidate_schema();
        tokio::fs::write(
            &schema_path,
            serde_json::to_string_pretty(&schema_json).unwrap(),
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok((schema_path, schema_json))
    }

    async fn write_contact_fact_schema(&self) -> Result<(PathBuf, serde_json::Value), String> {
        tokio::fs::create_dir_all(&self.schema_dir)
            .await
            .map_err(|e| e.to_string())?;
        let schema_path = self.schema_dir.join("contact_facts.schema.json");
        let schema_json = schema::contact_fact_schema();
        tokio::fs::write(
            &schema_path,
            serde_json::to_string_pretty(&schema_json).unwrap(),
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok((schema_path, schema_json))
    }

    fn emit_candidates_ready(
        &self,
        app: &AppHandle,
        envelope: &CandidateEnvelope,
        provider: &str,
        mode: &str,
        policy: &ContextPolicy,
        context_record: Option<&ContextSummaryRecord>,
        source_cards: &[SourceCard],
        suggestion_run: Option<&SuggestionRunRecord>,
    ) {
        if let Err(errs) = self.parser.validate(envelope) {
            tracing::warn!("Validation warnings: {:?}", errs);
        }
        let payload = serde_json::json!({
            "candidates": &envelope.candidates,
            "situation": &envelope.situation,
            "action_card": &envelope.action_card,
            "source_summary": &envelope.source_summary,
            "memory_candidates": &envelope.memory_candidates,
            "reminder_candidates": &envelope.reminder_candidates,
            "context_summary": &envelope.context_summary,
            "screenshot_analysis": &envelope.screenshot_analysis,
            "context_policy": policy,
            "context_record": context_record,
            "source_cards": source_cards,
            "suggestion_run": suggestion_run,
            "provider": provider,
            "mode": mode,
        });
        *self.last_generation_view.lock().unwrap() = Some(payload.clone());
        let _ = app.emit("candidates-ready", payload);
        self.window.show_popup(app);
    }

    async fn resolve_trigger_input(
        &self,
        app: &AppHandle,
        source: TriggerInput,
    ) -> Result<GenerationInput, String> {
        match source {
            TriggerInput::Clipboard => {
                if let Ok(text) = self.clipboard.read_text(app) {
                    if !text.trim().is_empty() {
                        return Ok(GenerationInput::Text(text));
                    }
                }

                let image = self.clipboard.read_image(app).map_err(|_| {
                    "剪贴板里没有可用的文字或图片。请复制文字，或先用微信/系统截图把聊天截图保存到剪贴板。"
                        .to_string()
                })?;
                let screenshot = self.persist_screenshot(image).await?;
                Ok(GenerationInput::Screenshot(screenshot))
            }
            TriggerInput::Selection => {
                let previous_image = self.clipboard.read_image(app).ok();
                match self.copy_selection_to_clipboard(app).await {
                    Ok(text) => Ok(GenerationInput::Text(text)),
                    Err(text_error) => {
                        if let Some(image) = previous_image {
                            let screenshot = self.persist_screenshot(image).await?;
                            Ok(GenerationInput::Screenshot(screenshot))
                        } else {
                            Err(text_error)
                        }
                    }
                }
            }
        }
    }

    async fn copy_selection_to_clipboard(&self, app: &AppHandle) -> Result<String, String> {
        let previous_text = self.clipboard.read_text(app).ok();
        let previous_image = self.clipboard.read_image(app).ok();
        let marker = clipboard_probe_marker();

        self.clipboard.write_text(app, &marker)?;
        tokio::time::sleep(SELECTION_COPY_SETTLE).await;

        if let Err(e) = self.input.copy_selection() {
            self.restore_clipboard(app, previous_text.as_deref(), previous_image.as_ref());
            return Err(format!(
                "无法自动复制当前选中文本：{e}。请手动复制后点击“生成回复”。"
            ));
        }

        let mut last_clipboard_error = None;
        for attempt in 0..CLIPBOARD_POLL_ATTEMPTS {
            tokio::time::sleep(CLIPBOARD_POLL_INTERVAL).await;

            let current = match self.clipboard.read_text(app) {
                Ok(current) => current,
                Err(e) => {
                    last_clipboard_error = Some(e);
                    tracing::debug!(
                        "Clipboard not readable after simulated copy, retrying: attempt {}/{}",
                        attempt + 1,
                        CLIPBOARD_POLL_ATTEMPTS
                    );
                    continue;
                }
            };

            if current != marker {
                let text = current.trim().to_string();
                if text.is_empty() {
                    self.restore_clipboard(app, previous_text.as_deref(), previous_image.as_ref());
                    return Err("复制到了空内容，请先选中一段文字再按快捷键".into());
                }
                tracing::info!("Copied selected text length: {}", text.len());
                return Ok(text);
            }
        }

        self.restore_clipboard(app, previous_text.as_deref(), previous_image.as_ref());
        if let Some(e) = last_clipboard_error {
            tracing::warn!("Clipboard stayed unreadable while copying selection: {e}");
        }
        Err("没有检测到选中的文本内容。请确认当前应用支持 Ctrl+C 复制文本；也可以手动复制后点击“生成回复”。".into())
    }

    fn restore_clipboard(
        &self,
        app: &AppHandle,
        previous_text: Option<&str>,
        previous_image: Option<&ClipboardImage>,
    ) {
        if let Some(text) = previous_text {
            let _ = self.clipboard.write_text(app, text);
        } else if let Some(image) = previous_image {
            let _ = self.clipboard.write_image(app, image);
        }
    }

    async fn capture_chat_screenshot(&self, app: &AppHandle) -> Result<ScreenshotInput, String> {
        let previous_signature = self
            .clipboard
            .read_image(app)
            .ok()
            .map(|image| image_signature(&image));

        self.window.hide_popup(app);
        if e2e_skip_screenclip_enabled() {
            let image = self
                .clipboard
                .read_image(app)
                .map_err(|_| "E2E 模式未检测到剪贴板图片，无法生成截图上下文。".to_string())?;
            return self.persist_screenshot(image).await;
        }
        let launched = self.screen_capture.start_region_capture()?;
        if launched {
            return self
                .wait_for_new_clipboard_image(app, previous_signature)
                .await;
        }

        let image = self.clipboard.read_image(app).map_err(|_| {
            "没有检测到剪贴板截图。请先用系统截图工具截取聊天上下文，再点击“截图上下文”。"
                .to_string()
        })?;
        self.persist_screenshot(image).await
    }

    async fn wait_for_new_clipboard_image(
        &self,
        app: &AppHandle,
        previous_signature: Option<u64>,
    ) -> Result<ScreenshotInput, String> {
        let mut last_error = None;
        for attempt in 0..SCREENSHOT_POLL_ATTEMPTS {
            tokio::time::sleep(SCREENSHOT_POLL_INTERVAL).await;

            let image = match self.clipboard.read_image(app) {
                Ok(image) => image,
                Err(e) => {
                    last_error = Some(e);
                    tracing::debug!(
                        "Clipboard image not ready after screenshot, retrying: attempt {}/{}",
                        attempt + 1,
                        SCREENSHOT_POLL_ATTEMPTS
                    );
                    continue;
                }
            };

            let signature = image_signature(&image);
            if previous_signature.map_or(true, |previous| previous != signature) {
                return self.persist_screenshot(image).await;
            }
        }

        if let Some(e) = last_error {
            tracing::warn!("Clipboard image stayed unreadable after screenshot request: {e}");
        }
        Err(
            "没有检测到新的聊天截图。请完成框选，或先用 Win+Shift+S 截图后再点击“截图上下文”。"
                .into(),
        )
    }

    async fn persist_screenshot(&self, image: ClipboardImage) -> Result<ScreenshotInput, String> {
        let expected_len = image.width as usize * image.height as usize * 4;
        if image.rgba.len() != expected_len {
            return Err("截图像素数据不完整，无法生成上下文。请重新截图。".into());
        }

        let dir = std::env::temp_dir().join("echomate-screenshots");
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| format!("无法创建截图缓存目录：{e}"))?;

        let path = dir.join(format!("chat-context-{}.png", timestamp_nanos()));
        let buffer =
            ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(image.width, image.height, image.rgba)
                .ok_or_else(|| "截图像素数据无法编码。请重新截图。".to_string())?;
        buffer
            .save_with_format(&path, ImageFormat::Png)
            .map_err(|e| format!("无法保存截图上下文：{e}"))?;

        Ok(ScreenshotInput {
            path,
            width: image.width,
            height: image.height,
        })
    }

    fn emit_generation_error(&self, app: &AppHandle, message: &str) {
        let _ = app.emit("generation-error", serde_json::json!({"message": message}));
        self.window.show_popup(app);
    }

    fn emit_generation_started(&self, app: &AppHandle, source: &str, length: Option<usize>) {
        *self.last_generation_view.lock().unwrap() = None;
        let _ = app.emit(
            "generation-started",
            serde_json::json!({
                "length": length.unwrap_or(0),
                "source": source,
            }),
        );
        self.window.show_popup(app);
    }

    fn generation_context(&self, config: &AppConfig) -> GenerationContext {
        let contact = if config.active_contact_id.trim().is_empty() {
            None
        } else {
            match self.memory_repo.get_contact(&config.active_contact_id) {
                Ok(contact) => contact,
                Err(e) => {
                    tracing::warn!("Failed to load active contact: {e}");
                    None
                }
            }
        };

        let allowlisted = contact
            .as_ref()
            .map(|contact| contact.is_allowlisted)
            .unwrap_or(false);
        let can_save_context = allowlisted && !config.global_privacy_mode;
        let reason = if config.global_privacy_mode {
            "全局隐私模式已开启：只生成候选，不保存上下文、记忆或提醒。".to_string()
        } else if let Some(contact) = contact.as_ref() {
            if contact.is_allowlisted {
                format!(
                    "当前联系人「{}」在白名单中，可保存用户确认的上下文。",
                    contact.alias
                )
            } else {
                format!(
                    "当前联系人「{}」未启用白名单：只生成候选，不保存上下文。",
                    contact.alias
                )
            }
        } else {
            "未选择白名单联系人：只生成候选，不保存上下文、记忆或提醒。".to_string()
        };

        let policy = ContextPolicy {
            allowlisted,
            contact_id: contact
                .as_ref()
                .map(|contact| contact.id.clone())
                .unwrap_or_default(),
            contact_alias: contact
                .as_ref()
                .map(|contact| contact.alias.clone())
                .unwrap_or_default(),
            reason: reason.clone(),
            can_save_context,
            global_privacy_mode: config.global_privacy_mode,
        };
        let (context_block, source_cards) = self.build_context_block(contact.as_ref(), &policy);
        GenerationContext {
            contact,
            policy,
            context_block,
            source_cards,
        }
    }

    fn build_context_block(
        &self,
        contact: Option<&ContactRecord>,
        policy: &ContextPolicy,
    ) -> (String, Vec<SourceCard>) {
        if !policy.can_save_context {
            return (
                format!(
                "- {reason}\n- 必须让 memory_candidates 和 reminder_candidates 返回空数组。\n- 可以继续生成 5 条候选回复，但不要声称已保存任何信息。",
                reason = policy.reason
                ),
                Vec::new(),
            );
        }

        let contact = match contact {
            Some(contact) => contact,
            None => return (policy.reason.clone(), Vec::new()),
        };
        let recent_messages = self
            .memory_repo
            .recent_messages(&contact.id, RECENT_CONTEXT_LIMIT)
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to load recent messages: {e}");
                Vec::new()
            });
        let memories = self
            .memory_repo
            .confirmed_memories_for_contact(&contact.id, CONTACT_MEMORY_LIMIT)
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to load contact memories: {e}");
                Vec::new()
            });
        let contact_facts = self
            .memory_repo
            .prompt_contact_facts(&contact.id, CONTACT_FACT_LIMIT)
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to load contact facts: {e}");
                Vec::new()
            });
        let mut source_cards = self
            .memory_repo
            .recent_source_cards(&contact.id, SOURCE_CARD_LIMIT)
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to load source cards: {e}");
                Vec::new()
            });
        let style_profile = self.memory_repo.style_profile().ok().flatten();

        let mut block = format!(
            "- 当前联系人：{} / {}\n- 白名单：已启用\n- 保存策略：只保存用户可见来源，用户可删除；记忆/提醒仍需用户确认。\n- 来源合同：只能使用当前输入和下面列出的本地上下文；不得使用 provider 自带示例、测试语料或未列出的背景。",
            contact.alias, contact.channel
        );
        if let Some(profile) = style_profile {
            let guide = style_profile_prompt_guide(&profile.profile_json);
            block.push_str(&format!(
                "\n- 风格画像指南：{}",
                truncate_for_prompt(&guide, 560)
            ));
        }
        if !contact_facts.is_empty() {
            block.push_str("\n- 用户手动补充资料（不是聊天记录，只能在相关场景谨慎使用；引用时标明“用户手动补充”）：");
            for fact in &contact_facts {
                source_cards.push(contact_fact_source_card(fact));
                block.push_str(&format!(
                    "\n  - [{}] {}（来源：用户手动补充；敏感度：{}；使用策略：{}）",
                    fact_type_label(&fact.fact_type),
                    truncate_for_prompt(&fact.value, 80),
                    fact.sensitivity,
                    fact.usage_policy
                ));
            }
        }
        if !memories.is_empty() {
            block.push_str("\n- 已确认联系人记忆：");
            for memory in memories {
                source_cards.push(SourceCard {
                    id: memory.id.clone(),
                    source_kind: "memory".to_string(),
                    title: format!("已批准记忆：{}", memory.memory_type),
                    detail: truncate_for_prompt(&memory.value, 100),
                    fact_source: non_empty_signal(&memory.source_kind, "memory"),
                    captured_at: memory.created_at.clone(),
                    visible_message_time: String::new(),
                    inferred_chat_time: String::new(),
                    source_confidence: memory.confidence,
                });
                block.push_str(&format!(
                    "\n  - [{}] {}（来源：{}）",
                    memory.memory_type,
                    truncate_for_prompt(&memory.value, 80),
                    truncate_for_prompt(&memory.source_excerpt, 60)
                ));
            }
        }
        if !recent_messages.is_empty() {
            if let Some(latest) = recent_messages.last() {
                block.push_str(&format!(
                    "\n- 最近上下文时间说明：下列时间是 EchoMate 本地读取/保存时间，不等于聊天发送时间；截图/剪贴板来源尤其不能据此判断对方刚刚说过。最后一条本地保存记录：{}。",
                    message_capture_label(latest)
                ));
            }
            block.push_str("\n- 最近上下文：");
            for message in recent_messages {
                let capture = message_capture_label(&message);
                block.push_str(&format!(
                    "\n  - {} / {} / {}：{}",
                    message.role,
                    message.source,
                    capture,
                    truncate_for_prompt(&message.text, 100)
                ));
            }
        }
        (block, source_cards)
    }

    fn apply_context_policy(
        &self,
        mut envelope: CandidateEnvelope,
        context: &GenerationContext,
    ) -> CandidateEnvelope {
        if e2e_mock_provider_enabled() || has_e2e_mock_artifacts(&envelope) {
            envelope.memory_candidates.clear();
            envelope.reminder_candidates.clear();
            envelope.context_summary.summary.clear();
            return envelope;
        }
        if !context.policy.can_save_context {
            envelope.memory_candidates.clear();
            envelope.reminder_candidates.clear();
            return envelope;
        }

        envelope.memory_candidates.retain(|candidate| {
            candidate.confidence >= 0.45
                && !matches!(candidate.sensitivity.as_str(), "high" | "forbidden")
        });
        envelope.reminder_candidates.retain(|candidate| {
            candidate.confidence >= 0.45
                && !matches!(candidate.sensitivity.as_str(), "high" | "forbidden")
        });
        envelope
    }

    fn persist_generation_artifacts(
        &self,
        envelope: &CandidateEnvelope,
        fallback_source_kind: &str,
        provider: &str,
        context: &GenerationContext,
        incoming_text: Option<&str>,
        screenshot_analysis: Option<&ScreenshotAnalysis>,
    ) -> PersistedGenerationArtifacts {
        let empty = || PersistedGenerationArtifacts {
            context_record: None,
            source_context: None,
            suggestion_run: None,
        };
        if e2e_mock_provider_enabled() || has_e2e_mock_artifacts(envelope) {
            tracing::warn!("Skipped persistence for e2e mock generation artifacts");
            return empty();
        }
        if !context.policy.can_save_context {
            return empty();
        }
        let Some(contact) = context.contact.as_ref() else {
            return empty();
        };
        let config = self.config.lock().unwrap().clone();
        if let Err(e) = self
            .memory_repo
            .apply_retention(config.context_retention_days)
        {
            tracing::warn!("Failed to apply retention: {e}");
        }

        let source_excerpt = incoming_text
            .filter(|text| !text.trim().is_empty())
            .unwrap_or(&envelope.context_summary.summary);
        let source_context = match self.memory_repo.insert_source_context(
            &contact.id,
            provider,
            fallback_source_kind,
            fallback_source_kind,
            input_source_title(fallback_source_kind),
            source_excerpt,
            None,
            screenshot_analysis.and_then(|analysis| {
                (!analysis.visible_time_label.trim().is_empty())
                    .then_some(analysis.visible_time_label.as_str())
            }),
            screenshot_analysis
                .map(|analysis| analysis.inferred_chat_time.as_str())
                .or_else(|| inferred_chat_time_label(fallback_source_kind)),
            screenshot_analysis
                .map(screenshot_source_confidence)
                .unwrap_or_else(|| input_source_confidence(fallback_source_kind)),
            &screenshot_source_metadata(screenshot_analysis),
        ) {
            Ok(record) => Some(record),
            Err(e) => {
                tracing::warn!("Failed to persist source context: {e}");
                None
            }
        };

        if let Some(text) = incoming_text.filter(|text| !text.trim().is_empty()) {
            if let Err(e) = self.memory_repo.append_message_with_source_context(
                &contact.id,
                "other",
                text,
                fallback_source_kind,
                false,
                source_context.as_ref().map(|record| record.id.as_str()),
                source_context
                    .as_ref()
                    .map(|record| record.captured_at.as_str()),
                source_context
                    .as_ref()
                    .map(|record| record.visible_message_time.as_str()),
                source_context
                    .as_ref()
                    .map(|record| record.inferred_chat_time.as_str()),
                source_context
                    .as_ref()
                    .map(|record| record.source_confidence)
                    .unwrap_or_default(),
            ) {
                tracing::warn!("Failed to append inbound message: {e}");
            }
        }

        let mut summary = envelope.context_summary.clone();
        if summary.source_kind.trim().is_empty() {
            summary.source_kind = fallback_source_kind.to_string();
        }
        if summary.summary.trim().is_empty() {
            return PersistedGenerationArtifacts {
                context_record: None,
                source_context,
                suggestion_run: None,
            };
        }
        if incoming_text.is_none() && !matches!(fallback_source_kind, "topic") {
            if let Err(e) = self.memory_repo.append_message_with_source_context(
                &contact.id,
                "other",
                &summary.summary,
                fallback_source_kind,
                false,
                source_context.as_ref().map(|record| record.id.as_str()),
                source_context
                    .as_ref()
                    .map(|record| record.captured_at.as_str()),
                source_context
                    .as_ref()
                    .map(|record| record.visible_message_time.as_str()),
                source_context
                    .as_ref()
                    .map(|record| record.inferred_chat_time.as_str()),
                source_context
                    .as_ref()
                    .map(|record| record.source_confidence)
                    .unwrap_or_default(),
            ) {
                tracing::warn!("Failed to append summarized message: {e}");
            }
        }
        let context_record = match self.memory_repo.insert_context_summary_with_source(
            Some(&contact.id),
            &summary,
            source_context.as_ref().map(|record| record.id.as_str()),
            source_context
                .as_ref()
                .map(|record| record.captured_at.as_str()),
            source_context
                .as_ref()
                .map(|record| record.visible_message_time.as_str()),
            source_context
                .as_ref()
                .map(|record| record.inferred_chat_time.as_str()),
            source_context
                .as_ref()
                .map(|record| record.source_confidence)
                .unwrap_or_default(),
        ) {
            Ok(record) => Some(record),
            Err(e) => {
                tracing::warn!("Failed to persist context summary: {e}");
                None
            }
        };

        let mut persisted_cards = context.source_cards.clone();
        if let Some(record) = source_context.as_ref() {
            persisted_cards.push(source_context_to_card(record));
        }
        let suggestion_run = match self.memory_repo.record_suggestion_run(
            &contact.id,
            provider,
            fallback_source_kind,
            source_context.as_ref().map(|record| record.id.as_str()),
            &persisted_cards,
            &summary.summary,
        ) {
            Ok(run) => Some(run),
            Err(e) => {
                tracing::warn!("Failed to persist suggestion run: {e}");
                None
            }
        };
        if let Some(run) = suggestion_run.as_ref() {
            if let Err(e) = self.memory_repo.record_memory_candidates_for_run(
                &contact.id,
                &run.id,
                source_context.as_ref().map(|record| record.id.as_str()),
                &envelope.memory_candidates,
            ) {
                tracing::warn!("Failed to persist memory candidates: {e}");
            }
        }

        PersistedGenerationArtifacts {
            context_record,
            source_context,
            suggestion_run,
        }
    }

    fn source_cards_for_view(
        &self,
        input_kind: &str,
        provider: &str,
        context: &GenerationContext,
        source_context: Option<&SourceContextRecord>,
        incoming_text: Option<&str>,
        summary: &str,
    ) -> Vec<SourceCard> {
        let detail = incoming_text
            .filter(|text| !text.trim().is_empty())
            .unwrap_or(summary)
            .trim();
        let current = source_context
            .map(source_context_to_card)
            .unwrap_or_else(|| SourceCard {
                id: format!("current-{input_kind}"),
                source_kind: input_kind.to_string(),
                title: input_source_title(input_kind).to_string(),
                detail: truncate_for_prompt(detail, 220),
                fact_source: input_kind.to_string(),
                captured_at: Utc::now().to_rfc3339(),
                visible_message_time: String::new(),
                inferred_chat_time: inferred_chat_time_label(input_kind)
                    .unwrap_or_default()
                    .to_string(),
                source_confidence: input_source_confidence(input_kind),
            });

        let mut cards = vec![current];
        for card in &context.source_cards {
            if cards.iter().any(|existing| existing.id == card.id) {
                continue;
            }
            cards.push(card.clone());
        }
        if !provider.trim().is_empty() {
            cards.push(SourceCard {
                id: format!("provider-{provider}"),
                source_kind: "provider_run".to_string(),
                title: "本次 Provider 调用".to_string(),
                detail: format!("Provider：{provider}；只允许使用本次 prompt 列出的来源。"),
                fact_source: "provider".to_string(),
                captured_at: Utc::now().to_rfc3339(),
                visible_message_time: String::new(),
                inferred_chat_time: String::new(),
                source_confidence: 1.0,
            });
        }
        cards
    }

    pub fn save_memory_candidate(
        &self,
        candidate: crate::domain::MemoryCandidate,
    ) -> Result<crate::domain::MemoryItemRecord, String> {
        if e2e_mock_provider_enabled() || candidate.source_ref == "e2e-mock" {
            return Err("测试 mock 记忆不允许保存到真实联系人上下文。".to_string());
        }
        let context = self.generation_context(&self.config.lock().unwrap().clone());
        let contact_id = context
            .policy
            .can_save_context
            .then_some(context.policy.contact_id.as_str())
            .filter(|id| !id.is_empty())
            .ok_or_else(|| "联系人不在白名单或隐私模式已开启，不能保存这条记忆。".to_string())?;
        self.memory_repo
            .save_memory_candidate_for_contact(Some(contact_id), &candidate)
            .map_err(|e| e.to_string())
    }

    pub fn list_memory_candidate_inbox(
        &self,
        contact_id: Option<String>,
    ) -> Result<Vec<MemoryCandidateRecord>, String> {
        let contact_id = self.resolve_contact_id_for_view(contact_id)?;
        self.memory_repo
            .list_memory_candidates(&contact_id, Some("candidate"), 50)
            .map_err(|e| e.to_string())
    }

    pub fn confirm_memory_candidate_record(&self, id: &str) -> Result<MemoryItemRecord, String> {
        self.memory_repo
            .confirm_memory_candidate(id)
            .map_err(|e| e.to_string())
    }

    pub fn ignore_memory_candidate_record(&self, id: &str) -> Result<(), String> {
        self.memory_repo
            .ignore_memory_candidate_record(id)
            .map_err(|e| e.to_string())
    }

    pub async fn create_reminder_from_candidate(
        &self,
        app: &AppHandle,
        candidate: crate::domain::ReminderCandidate,
        trigger_at: Option<String>,
    ) -> Result<crate::domain::ReminderDetail, String> {
        if e2e_mock_provider_enabled() || candidate.source_ref == "e2e-mock" {
            return Err("测试 mock 提醒不允许保存到真实联系人上下文。".to_string());
        }
        let context = self.generation_context(&self.config.lock().unwrap().clone());
        let contact_id = context
            .policy
            .can_save_context
            .then_some(context.policy.contact_id.as_str())
            .filter(|id| !id.is_empty())
            .ok_or_else(|| "联系人不在白名单或隐私模式已开启，不能创建这条提醒。".to_string())?;
        let detail = self
            .memory_repo
            .create_reminder_from_candidate_for_contact(Some(contact_id), &candidate, trigger_at)
            .map_err(|e| e.to_string())?;
        if let Err(e) = process_due_reminders(app, &self.memory_repo).await {
            tracing::warn!("Failed to process due reminders after create: {e}");
        }
        Ok(detail)
    }

    pub fn list_reminders(
        &self,
        contact_id: Option<String>,
    ) -> Result<Vec<ReminderCenterItem>, String> {
        let resolved = contact_id
            .as_deref()
            .filter(|id| !id.trim().is_empty())
            .map(str::to_string)
            .or_else(|| {
                let active = self.config.lock().unwrap().active_contact_id.clone();
                (!active.trim().is_empty()).then_some(active)
            });
        self.memory_repo
            .list_reminders(resolved.as_deref(), false, 100)
            .map_err(|e| e.to_string())
    }

    pub fn complete_reminder(&self, id: &str) -> Result<(), String> {
        self.memory_repo
            .complete_reminder(id)
            .map_err(|e| e.to_string())
    }

    pub fn snooze_reminder_minutes(&self, id: &str, minutes: i64) -> Result<(), String> {
        let minutes = minutes.clamp(5, 60 * 24 * 30);
        self.memory_repo
            .snooze_reminder(id, Utc::now() + ChronoDuration::minutes(minutes))
            .map_err(|e| e.to_string())
    }

    pub fn mute_reminders(
        &self,
        contact_id: Option<String>,
        kind: Option<String>,
        hours: i64,
    ) -> Result<(), String> {
        self.memory_repo
            .mute_reminders(
                contact_id
                    .as_deref()
                    .filter(|value| !value.trim().is_empty()),
                kind.as_deref().filter(|value| !value.trim().is_empty()),
                hours,
                "用户在提醒中心静默",
            )
            .map_err(|e| e.to_string())
    }

    pub fn delete_memory(&self, id: &str) -> Result<(), String> {
        self.memory_repo
            .delete_memory(id)
            .map_err(|e| e.to_string())
    }

    pub fn delete_reminder(&self, id: &str) -> Result<(), String> {
        self.memory_repo
            .delete_reminder(id)
            .map_err(|e| e.to_string())
    }

    pub fn record_reply_feedback(
        &self,
        action: &str,
        candidate_index: i64,
    ) -> Result<crate::domain::ReplyFeedbackRecord, String> {
        self.record_reply_feedback_with_text(action, candidate_index, "")
    }

    pub fn record_reply_feedback_with_text(
        &self,
        action: &str,
        candidate_index: i64,
        candidate_text: &str,
    ) -> Result<crate::domain::ReplyFeedbackRecord, String> {
        if e2e_mock_provider_enabled() {
            return self
                .memory_repo
                .record_reply_feedback_for_contact(
                    "current",
                    action,
                    candidate_index,
                    candidate_text,
                    None,
                )
                .map_err(|e| e.to_string());
        }
        let context = self.generation_context(&self.config.lock().unwrap().clone());
        let contact_id = context
            .policy
            .can_save_context
            .then_some(context.policy.contact_id.as_str())
            .filter(|id| !id.is_empty());
        if action == "copy" {
            if let (Some(contact_id), text) = (contact_id, candidate_text.trim()) {
                if !text.is_empty() {
                    if let Err(e) = self
                        .memory_repo
                        .append_message(contact_id, "me", text, "manual", true)
                    {
                        tracing::warn!("Failed to append adopted reply: {e}");
                    }
                    if let Err(e) = self.memory_repo.update_style_profile_from_reply(text) {
                        tracing::warn!("Failed to update style profile: {e}");
                    }
                }
            }
        }
        self.memory_repo
            .record_reply_feedback_for_contact(
                "current",
                action,
                candidate_index,
                candidate_text,
                contact_id,
            )
            .map_err(|e| e.to_string())
    }

    pub fn latest_notified_reminder(
        &self,
    ) -> Result<Option<crate::domain::ReminderDetail>, String> {
        self.memory_repo
            .latest_notified_reminder()
            .map_err(|e| e.to_string())
    }

    pub fn list_contacts(&self) -> Result<Vec<ContactRecord>, String> {
        self.memory_repo.list_contacts().map_err(|e| e.to_string())
    }

    pub fn upsert_contact(&self, contact: ContactInput) -> Result<ContactRecord, String> {
        self.memory_repo
            .upsert_contact(&contact)
            .map_err(|e| e.to_string())
    }

    pub async fn classify_contact_facts(
        &self,
        contact_id: &str,
        note: &str,
    ) -> Result<ContactFactClassification, String> {
        let note = note.trim();
        if note.is_empty() {
            return Err("请先输入要补充的联系人资料。".to_string());
        }
        let config = self.config.lock().unwrap().clone();
        if config.global_privacy_mode {
            return Err("全局隐私模式已开启，不能调用 Provider 归类联系人资料。".to_string());
        }
        let contact = self
            .memory_repo
            .get_contact(contact_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "联系人不存在，不能补充资料。".to_string())?;
        if !contact.is_allowlisted {
            return Err("联系人未启用白名单，不能保存或归类补充资料。".to_string());
        }

        let prompt = self
            .prompt_composer
            .contact_fact_classification_prompt(&contact.alias, note);
        let (schema_path, schema_json) = self.write_contact_fact_schema().await?;
        let mut classification = match config.primary_provider.as_str() {
            "codex" => {
                let provider = CodexProvider::new().with_timeout(config.timeout_seconds);
                provider
                    .classify_contact_facts(&prompt, &schema_path)
                    .await
                    .map_err(|e| Self::friendly_provider_error("Codex", e))?
            }
            "claude" => {
                let provider = ClaudeProvider::new().with_timeout(config.timeout_seconds);
                provider
                    .classify_contact_facts(&prompt, &schema_json)
                    .await
                    .map_err(|e| Self::friendly_provider_error("Claude", e))?
            }
            _ => return Err(format!("Unknown provider: {}", config.primary_provider)),
        };
        normalize_manual_fact_classification(&mut classification, note);
        Ok(classification)
    }

    pub fn save_contact_facts(
        &self,
        contact_id: &str,
        mut facts: Vec<ContactFactCandidate>,
    ) -> Result<Vec<ContactFactRecord>, String> {
        let config = self.config.lock().unwrap().clone();
        if config.global_privacy_mode {
            return Err("全局隐私模式已开启，不能保存联系人补充资料。".to_string());
        }
        let contact = self
            .memory_repo
            .get_contact(contact_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "联系人不存在，不能保存补充资料。".to_string())?;
        if !contact.is_allowlisted {
            return Err("联系人未启用白名单，不能保存补充资料。".to_string());
        }
        for fact in &mut facts {
            fact.fact_source = "manual".to_string();
            if fact.source_note.trim().is_empty() {
                fact.source_note = "用户手动补充".to_string();
            }
        }
        self.memory_repo
            .save_contact_facts(contact_id, &facts)
            .map_err(|e| e.to_string())
    }

    pub fn list_contact_facts(&self, contact_id: &str) -> Result<Vec<ContactFactRecord>, String> {
        self.memory_repo
            .list_contact_facts(contact_id)
            .map_err(|e| e.to_string())
    }

    pub fn relationship_card(
        &self,
        contact_id: Option<String>,
    ) -> Result<RelationshipCard, String> {
        let contact_id = self.resolve_contact_id_for_view(contact_id)?;
        self.memory_repo
            .relationship_card(&contact_id)
            .map_err(|e| e.to_string())
    }

    pub fn data_audit_report(&self) -> Result<DataAuditReport, String> {
        let config = self.config.lock().unwrap().clone();
        self.memory_repo
            .data_audit_report(&config.active_contact_id, config.context_retention_days)
            .map_err(|e| e.to_string())
    }

    pub fn export_data_snapshot(&self) -> Result<serde_json::Value, String> {
        self.memory_repo
            .export_data_snapshot()
            .map_err(|e| e.to_string())
    }

    pub fn clear_all_data(&self) -> Result<(), String> {
        self.clear_last_generation_view();
        self.memory_repo.clear_all_data().map_err(|e| e.to_string())
    }

    pub fn clear_logs(&self) -> Result<(), String> {
        let dir = log_dir_path();
        if !dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.is_file() {
                std::fs::remove_file(&path).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    pub fn privacy_guide_status(&self) -> PrivacyGuideStatus {
        let config = self.config.lock().unwrap().clone();
        PrivacyGuideStatus {
            onboarding_completed: config.privacy_onboarding_completed,
            strict_privacy: config.strict_privacy,
            global_privacy_mode: config.global_privacy_mode,
            debug_log_body_enabled: config.debug_log_body_enabled,
            log_path: log_dir_path().display().to_string(),
            data_path: self.memory_repo.db_path().display().to_string(),
            shell_execute_exposed_to_frontend: false,
        }
    }

    pub fn acknowledge_privacy_guide(&self) {
        {
            let mut config = self.config.lock().unwrap();
            config.privacy_onboarding_completed = true;
        }
        self.save_config_to_disk();
    }

    pub fn delete_contact_fact(&self, id: &str) -> Result<(), String> {
        self.memory_repo
            .delete_contact_fact(id)
            .map_err(|e| e.to_string())
    }

    pub fn delete_contact(&self, id: &str) -> Result<(), String> {
        self.memory_repo
            .delete_contact(id)
            .map_err(|e| e.to_string())
    }

    pub fn clear_contact_context(&self, id: &str) -> Result<(), String> {
        self.memory_repo
            .clear_contact_context(id)
            .map_err(|e| e.to_string())
    }

    pub fn delete_context_summary(&self, id: &str) -> Result<(), String> {
        self.memory_repo
            .delete_context_summary(id)
            .map_err(|e| e.to_string())
    }

    pub fn style_profile(&self) -> Result<Option<StyleProfileRecord>, String> {
        self.memory_repo.style_profile().map_err(|e| e.to_string())
    }

    pub fn refresh_style_profile(&self) -> Result<Option<StyleProfileRecord>, String> {
        self.memory_repo
            .rebuild_style_profile_from_adopted_replies()
            .map_err(|e| e.to_string())
    }

    pub fn reset_style_profile(&self) -> Result<(), String> {
        self.memory_repo
            .reset_style_profile()
            .map_err(|e| e.to_string())
    }

    pub fn last_generation_view(&self) -> Option<serde_json::Value> {
        let snapshot = self.last_generation_view.lock().unwrap().clone()?;
        let snapshot_contact_id = snapshot
            .get("context_policy")
            .and_then(|policy| policy.get("contact_id"))
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let active_contact_id = self.config.lock().unwrap().active_contact_id.clone();
        if snapshot_contact_id != active_contact_id {
            return None;
        }
        Some(snapshot)
    }

    pub fn clear_last_generation_view(&self) {
        *self.last_generation_view.lock().unwrap() = None;
    }

    fn resolve_contact_id_for_view(&self, contact_id: Option<String>) -> Result<String, String> {
        let resolved = contact_id
            .as_deref()
            .filter(|id| !id.trim().is_empty())
            .map(str::to_string)
            .or_else(|| {
                let active = self.config.lock().unwrap().active_contact_id.clone();
                (!active.trim().is_empty()).then_some(active)
            })
            .ok_or_else(|| "请先选择一个联系人。".to_string())?;
        if self
            .memory_repo
            .get_contact(&resolved)
            .map_err(|e| e.to_string())?
            .is_none()
        {
            return Err("联系人不存在。".to_string());
        }
        Ok(resolved)
    }

    pub fn set_active_contact(&self, contact_id: String) -> Result<(), String> {
        if !contact_id.trim().is_empty()
            && self
                .memory_repo
                .get_contact(&contact_id)
                .map_err(|e| e.to_string())?
                .is_none()
        {
            return Err("联系人不存在，不能设为当前上下文。".to_string());
        }
        {
            let mut config = self.config.lock().unwrap();
            config.active_contact_id = contact_id;
        }
        self.save_config_to_disk();
        Ok(())
    }

    pub fn permission_status(&self) -> PermissionStatus {
        let config = self.config.lock().unwrap().clone();
        PermissionStatus {
            platform: std::env::consts::OS.to_string(),
            windows_notification_helper_enabled: config.windows_notification_helper_enabled,
            windows_notification_available: cfg!(target_os = "windows"),
            windows_notification_status: if cfg!(target_os = "windows") {
                "需要 packaged app capability 与用户授权；未授权时降级到热键/截图。".to_string()
            } else {
                "当前构建环境不是 Windows，Notification Listener 不可用，已降级到热键/截图。"
                    .to_string()
            },
            macos_context_helper_enabled: config.macos_context_helper_enabled,
            macos_accessibility_enabled: config.macos_accessibility_enabled,
            macos_context_status: if cfg!(target_os = "macos") {
                "可用前台应用、Pasteboard 和 Accessibility 近似上下文；不承诺后台通知读取。"
                    .to_string()
            } else {
                "当前构建环境不是 macOS，macOS 近似 helper 不运行。".to_string()
            },
            fallback_path: "权限关闭或平台能力不可用时，继续使用复制、选中文本热键和截图生成。"
                .to_string(),
        }
    }

    pub fn ingest_platform_signal(
        &self,
        app: &AppHandle,
        signal: PlatformSignal,
    ) -> Result<PlatformSignalResult, String> {
        let config = self.config.lock().unwrap().clone();
        if config.global_privacy_mode {
            return Ok(PlatformSignalResult {
                allowed: false,
                reason: "全局隐私模式已开启，入站信号不保存。".to_string(),
                contact: None,
                message: None,
            });
        }
        let source = non_empty_signal(&signal.source, "notification");
        let helper_enabled = match source.as_str() {
            "notification" => config.windows_notification_helper_enabled,
            "clipboard" | "manual" | "topic" | "screenshot" => true,
            _ => false,
        };
        if !helper_enabled {
            return Ok(PlatformSignalResult {
                allowed: false,
                reason: "对应平台 helper 未开启，已降级到热键/截图主流程。".to_string(),
                contact: None,
                message: None,
            });
        }
        let channel = non_empty_signal(&signal.channel, "wechat");
        let contact = self
            .memory_repo
            .find_allowlisted_contact(&signal.contact_alias, &channel)
            .map_err(|e| e.to_string())?;
        let Some(contact) = contact else {
            return Ok(PlatformSignalResult {
                allowed: false,
                reason: "联系人不在白名单，入站信号未保存，也不会触发生成。".to_string(),
                contact: None,
                message: None,
            });
        };
        let message = if signal.text.trim().is_empty() {
            None
        } else {
            Some(
                self.memory_repo
                    .append_message(&contact.id, "other", &signal.text, &source, false)
                    .map_err(|e| e.to_string())?,
            )
        };
        if let Err(e) = self.memory_repo.record_platform_signal_log(
            &contact.id,
            &contact.alias,
            &contact.channel,
            &source,
            &signal.app_name,
            &signal.text,
            true,
            "白名单联系人有新的近似入站信号；未自动生成或发送。",
        ) {
            tracing::warn!("Failed to record platform signal log: {e}");
        }
        let _ = app.emit(
            "inbound-signal",
            serde_json::json!({
                "contact": &contact,
                "source": source,
                "app_name": signal.app_name,
                "reason": "白名单联系人有新的近似入站信号；EchoMate 不会自动生成或发送。"
            }),
        );
        Ok(PlatformSignalResult {
            allowed: true,
            reason: "已记录白名单联系人入站信号；等待用户手动触发生成。".to_string(),
            contact: Some(contact),
            message,
        })
    }

    async fn call_provider(
        &self,
        config: &AppConfig,
        prompt: &str,
        schema_json: &serde_json::Value,
        schema_path: &PathBuf,
    ) -> Result<CandidateEnvelope, String> {
        match config.primary_provider.as_str() {
            "codex" => {
                let provider = CodexProvider::new().with_timeout(config.timeout_seconds);
                provider.generate(prompt, schema_path).await.map_err(|e| {
                    if config.fallback_provider == "claude" {
                        tracing::warn!("Codex failed, trying Claude fallback");
                        // Note: fallback would need async context, simplified for now
                        Self::friendly_provider_error("Codex", e)
                    } else {
                        Self::friendly_provider_error("Codex", e)
                    }
                })
            }
            "claude" => {
                let provider = ClaudeProvider::new().with_timeout(config.timeout_seconds);
                provider
                    .generate(prompt, schema_json)
                    .await
                    .map_err(|e| Self::friendly_provider_error("Claude", e))
            }
            _ => Err(format!("Unknown provider: {}", config.primary_provider)),
        }
    }

    async fn call_screenshot_provider(
        &self,
        config: &AppConfig,
        prompt: &str,
        schema_path: &PathBuf,
        image_path: &Path,
    ) -> Result<(CandidateEnvelope, String), String> {
        if config.primary_provider != "codex" {
            tracing::info!(
                "Screenshot context uses Codex image input instead of configured provider {}",
                config.primary_provider
            );
        }

        let provider = CodexProvider::new().with_timeout(config.timeout_seconds);
        provider
            .generate_with_images(prompt, schema_path, &[image_path.to_path_buf()])
            .await
            .map(|envelope| (envelope, "codex".to_string()))
            .map_err(|e| {
                let message = Self::friendly_provider_error("Codex", e);
                if config.primary_provider == "codex" {
                    message
                } else {
                    format!(
                        "截图上下文目前需要 Codex 的图片输入能力，已自动改用 Codex。\n{message}"
                    )
                }
            })
    }

    fn friendly_provider_error(provider: &str, error: anyhow::Error) -> String {
        let raw = error.to_string();
        let lower = raw.to_lowercase();
        let detail = truncate_error(&raw);

        if lower.contains("timed out") {
            return format!(
                "{provider} 生成超时，后台进程已终止。请稍后重试，或在设置里切换 Provider。\n技术信息：{detail}"
            );
        }

        if lower.contains("could not start")
            || lower.contains("no such file")
            || lower.contains("os error 2")
            || lower.contains("not found")
        {
            return format!(
                "{provider} 启动失败。请确认 WSL2 和 {provider} CLI 可用，并能在 WSL 终端里直接运行。\n技术信息：{detail}"
            );
        }

        format!("{provider} 生成失败。\n技术信息：{detail}")
    }

    pub fn init(&self, app: &AppHandle) {
        let config = self.config.lock().unwrap().clone();
        self.register_hotkey(app, &config.hotkey);
        self.start_reminder_loop(app);
        tracing::info!(
            "Orchestrator initialized with provider: {}",
            config.primary_provider
        );
    }

    pub fn reload_hotkey(&self, app: &AppHandle) {
        let config = self.config.lock().unwrap().clone();
        self.register_hotkey(app, &config.hotkey);
        tracing::info!("Hotkey reloaded: {}", config.hotkey);
    }

    fn register_hotkey(&self, app: &AppHandle, hotkey_str: &str) {
        let app_handle = app.clone();
        self.hotkey.register(app, hotkey_str, move || {
            let app = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<OrchestratorState>();
                if let Err(e) = state.0.trigger_from_selection(&app).await {
                    tracing::error!(
                        "Orchestrator trigger failed; detail omitted from logs ({} chars)",
                        e.chars().count()
                    );
                }
            });
        });
    }

    fn start_reminder_loop(&self, app: &AppHandle) {
        let app = app.clone();
        let repo = self.memory_repo.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                if let Err(e) = process_due_reminders(&app, &repo).await {
                    tracing::warn!("Reminder loop failed: {e}");
                }
                tokio::time::sleep(REMINDER_POLL_INTERVAL).await;
            }
        });
    }
}

pub struct OrchestratorState(pub Orchestrator);

#[derive(Clone, Copy)]
enum TriggerInput {
    Clipboard,
    Selection,
}

impl TriggerInput {
    fn source_kind(self) -> &'static str {
        match self {
            TriggerInput::Clipboard => "clipboard",
            TriggerInput::Selection => "selection",
        }
    }
}

#[derive(Clone)]
enum GenerationInput {
    Text(String),
    Screenshot(ScreenshotInput),
    Topic(Option<String>),
}

impl GenerationInput {
    fn source_kind(&self) -> &'static str {
        match self {
            GenerationInput::Text(_) => "text",
            GenerationInput::Screenshot(_) => "screenshot",
            GenerationInput::Topic(_) => "topic",
        }
    }
}

struct GenerationContext {
    contact: Option<ContactRecord>,
    policy: ContextPolicy,
    context_block: String,
    source_cards: Vec<SourceCard>,
}

struct PersistedGenerationArtifacts {
    context_record: Option<ContextSummaryRecord>,
    source_context: Option<SourceContextRecord>,
    suggestion_run: Option<SuggestionRunRecord>,
}

#[derive(Clone)]
struct ScreenshotInput {
    path: PathBuf,
    width: u32,
    height: u32,
}

struct GenerationGuard<'a> {
    flag: &'a AtomicBool,
}

impl<'a> GenerationGuard<'a> {
    fn new(flag: &'a AtomicBool) -> Self {
        Self { flag }
    }
}

impl Drop for GenerationGuard<'_> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Release);
    }
}

fn truncate_error(raw: &str) -> String {
    let trimmed = raw.trim();
    let mut chars = trimmed.chars();
    let head = chars.by_ref().take(500).collect::<String>();
    if chars.next().is_none() {
        trimmed.to_string()
    } else {
        format!("{head}...")
    }
}

fn truncate_for_prompt(raw: &str, max_chars: usize) -> String {
    let trimmed = raw.trim();
    let mut chars = trimmed.chars();
    let head = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_none() {
        trimmed.to_string()
    } else {
        format!("{head}...")
    }
}

fn style_profile_prompt_guide(profile_json: &str) -> String {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(profile_json) else {
        return profile_json.trim().to_string();
    };
    if let Some(guide) = json
        .get("prompt_guide")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
    {
        return guide.trim().to_string();
    }

    let summary = json
        .get("summary")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let rules = json
        .get("generation_rules")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str())
                .collect::<Vec<_>>()
                .join("；")
        })
        .unwrap_or_default();
    let guide = [summary, &rules]
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join("。");
    if guide.is_empty() {
        profile_json.trim().to_string()
    } else {
        guide
    }
}

fn source_context_to_card(record: &SourceContextRecord) -> SourceCard {
    SourceCard {
        id: record.id.clone(),
        source_kind: record.input_kind.clone(),
        title: if record.source_label.trim().is_empty() {
            input_source_title(&record.input_kind).to_string()
        } else {
            record.source_label.clone()
        },
        detail: truncate_for_prompt(&record.source_excerpt, 220),
        fact_source: record.fact_source.clone(),
        captured_at: record.captured_at.clone(),
        visible_message_time: record.visible_message_time.clone(),
        inferred_chat_time: record.inferred_chat_time.clone(),
        source_confidence: record.source_confidence,
    }
}

fn contact_fact_source_card(fact: &ContactFactRecord) -> SourceCard {
    SourceCard {
        id: fact.id.clone(),
        source_kind: "contact_fact".to_string(),
        title: format!("用户手动补充：{}", fact_type_label(&fact.fact_type)),
        detail: truncate_for_prompt(&fact.value, 120),
        fact_source: fact.fact_source.clone(),
        captured_at: if fact.captured_at.trim().is_empty() {
            fact.created_at.clone()
        } else {
            fact.captured_at.clone()
        },
        visible_message_time: fact.visible_message_time.clone(),
        inferred_chat_time: fact.inferred_chat_time.clone(),
        source_confidence: if fact.source_confidence > 0.0 {
            fact.source_confidence
        } else {
            fact.confidence
        },
    }
}

fn input_source_title(input_kind: &str) -> &'static str {
    match input_kind {
        "screenshot" => "当前截图",
        "clipboard" | "text" => "当前剪贴板文本",
        "topic" => "主动找话题",
        "notification" => "入站通知信号",
        "manual" => "用户手动输入",
        _ => "当前输入",
    }
}

fn inferred_chat_time_label(input_kind: &str) -> Option<&'static str> {
    match input_kind {
        "notification" => Some("inferred_from_notification"),
        "screenshot" | "clipboard" | "text" | "topic" | "manual" => Some("unknown"),
        _ => Some("unknown"),
    }
}

fn input_source_confidence(input_kind: &str) -> f64 {
    match input_kind {
        "notification" => 0.82,
        "manual" => 0.95,
        "screenshot" => 0.55,
        "clipboard" | "text" => 0.6,
        "topic" => 0.4,
        _ => 0.5,
    }
}

fn fact_type_label(fact_type: &str) -> &'static str {
    match fact_type {
        "birth_year" => "出生年份",
        "age_band" => "年龄段",
        "hometown" => "籍贯",
        "current_city" => "现居城市",
        "work_city" => "工作城市",
        "occupation" => "职业",
        "preference" => "偏好",
        "boundary" => "边界",
        "important_date" => "重要日期",
        "temporary_state" => "临时状态",
        _ => "资料",
    }
}

fn normalize_manual_fact_classification(
    classification: &mut ContactFactClassification,
    source_note: &str,
) {
    for fact in &mut classification.facts {
        fact.fact_source = "manual".to_string();
        fact.confidence = fact.confidence.clamp(0.0, 1.0);
        if fact.source_note.trim().is_empty() {
            fact.source_note = source_note.trim().to_string();
        }
        if fact.usage_policy.trim().is_empty() {
            fact.usage_policy = "contextual".to_string();
        }
        if fact.sensitivity.trim().is_empty() {
            fact.sensitivity = "normal".to_string();
        }
    }
    if classification.usage_guidance.trim().is_empty() {
        classification.usage_guidance =
            "只在当前话题相关时使用这些用户手动补充资料；敏感或无关资料默认不进入生成。"
                .to_string();
    }
}

fn message_capture_label(message: &crate::domain::MessageRecord) -> String {
    let time = local_saved_time_label(&message.created_at);
    match message.source.as_str() {
        "notification" => format!("通知收到于{time}，可近似视为消息时间"),
        "screenshot" => format!("截图摘要保存于{time}，不是聊天发送时间"),
        "clipboard" => format!("剪贴板内容保存于{time}，不是聊天发送时间"),
        "manual" => format!("用户采用回复保存于{time}，不是对方消息时间"),
        source => format!("本地来源 {source} 保存于{time}，不一定是聊天发送时间"),
    }
}

fn local_saved_time_label(created_at: &str) -> String {
    let Ok(parsed) = DateTime::parse_from_rfc3339(created_at) else {
        return created_at.to_string();
    };
    let utc = parsed.with_timezone(&Utc);
    let local = utc.with_timezone(&Local);
    let age = Utc::now().signed_duration_since(utc);
    format!(
        "{}（{}）",
        local.format("%m-%d %H:%M"),
        human_age_label(age)
    )
}

fn human_age_label(age: ChronoDuration) -> String {
    if age.num_seconds() < 0 {
        return "未来时间".to_string();
    }
    let minutes = age.num_minutes();
    if minutes < 1 {
        "刚刚".to_string()
    } else if minutes < 60 {
        format!("{minutes} 分钟前")
    } else if minutes < 24 * 60 {
        format!("{} 小时前", minutes / 60)
    } else {
        format!("{} 天前", minutes / (24 * 60))
    }
}

fn non_empty_signal(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn clipboard_probe_marker() -> String {
    format!(
        "ECHOMATE_COPY_PROBE_{}_{}",
        std::process::id(),
        timestamp_nanos()
    )
}

fn timestamp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

pub fn log_dir_path() -> PathBuf {
    if e2e_mock_provider_enabled() {
        return e2e_profile_dir().join("logs");
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("EchoMate").join("logs");
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".echomate").join("logs")
}

fn image_signature(image: &ClipboardImage) -> u64 {
    let mut hasher = DefaultHasher::new();
    image.width.hash(&mut hasher);
    image.height.hash(&mut hasher);
    image.rgba.hash(&mut hasher);
    hasher.finish()
}

fn analyze_screenshot_locally(screenshot: &ScreenshotInput) -> ScreenshotAnalysis {
    match vision_ocr::recognize_text(&screenshot.path) {
        Ok(lines) if !lines.is_empty() => screenshot_analysis_from_ocr(lines),
        Ok(_) => ScreenshotAnalysis {
            warnings: vec![
                "本地 OCR 未识别到文字；将依赖 provider 视觉输入并保守生成。".to_string(),
            ],
            ..ScreenshotAnalysis::default()
        },
        Err(e) => ScreenshotAnalysis {
            warnings: vec![format!("本地 Apple Vision OCR 不可用或失败：{e}")],
            ..ScreenshotAnalysis::default()
        },
    }
}

fn screenshot_analysis_from_ocr(lines: Vec<OcrLine>) -> ScreenshotAnalysis {
    let mut turns = Vec::new();
    let mut current_time_label = String::new();
    let mut warnings = Vec::new();
    for line in lines {
        let text = line.text.trim();
        if text.is_empty() {
            continue;
        }
        if is_visible_time_label(text) {
            current_time_label = text.to_string();
            turns.push(ScreenshotTurn {
                speaker: "system".to_string(),
                text: text.to_string(),
                media_kind: "system".to_string(),
                visible_time_label: text.to_string(),
                bbox: Some(ocr_bbox(&line)),
                confidence: line.confidence,
                warnings: Vec::new(),
            });
            continue;
        }
        let center_x = line.x + line.width / 2.0;
        let speaker = if center_x >= 0.58 {
            "me"
        } else if center_x <= 0.42 {
            "other"
        } else {
            "unknown"
        };
        let mut turn_warnings = Vec::new();
        if speaker == "unknown" {
            turn_warnings.push("气泡左右位置不明确".to_string());
        }
        if line.confidence < 0.55 {
            turn_warnings.push("OCR 置信度较低".to_string());
        }
        warnings.extend(turn_warnings.iter().cloned());
        turns.push(ScreenshotTurn {
            speaker: speaker.to_string(),
            text: text.to_string(),
            media_kind: screenshot_media_kind(text).to_string(),
            visible_time_label: current_time_label.clone(),
            bbox: Some(ocr_bbox(&line)),
            confidence: line.confidence,
            warnings: turn_warnings,
        });
    }
    let last_reply_target = turns
        .iter()
        .rev()
        .find(|turn| turn.speaker == "other" && !turn.text.trim().is_empty())
        .map(|turn| turn.text.clone())
        .unwrap_or_default();
    let visible_time_label = turns
        .iter()
        .rev()
        .find(|turn| !turn.visible_time_label.trim().is_empty())
        .map(|turn| turn.visible_time_label.clone())
        .unwrap_or_default();
    let inferred_chat_time = if visible_time_label.trim().is_empty() {
        "unknown".to_string()
    } else {
        format!("visible_time_label:{visible_time_label}")
    };
    let staleness = if visible_time_label.trim().is_empty() {
        "unknown"
    } else {
        "visible_time_only"
    };
    ScreenshotAnalysis {
        turns,
        last_reply_target,
        visible_time_label,
        inferred_chat_time,
        staleness: staleness.to_string(),
        warnings,
    }
}

fn ocr_bbox(line: &OcrLine) -> BoundingBox {
    BoundingBox {
        x: line.x,
        y: line.y,
        width: line.width,
        height: line.height,
    }
}

fn is_visible_time_label(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.contains("昨天")
        || trimmed.contains("今天")
        || trimmed.contains("前天")
        || trimmed.contains("上午")
        || trimmed.contains("下午")
        || trimmed.contains("晚上")
        || trimmed.contains("凌晨")
        || trimmed.contains("周")
    {
        return true;
    }
    let mut parts = trimmed.split(':');
    let Some(hour) = parts.next() else {
        return false;
    };
    let Some(minute) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && hour.chars().all(|c| c.is_ascii_digit())
        && minute.chars().all(|c| c.is_ascii_digit())
        && (1..=2).contains(&hour.len())
        && minute.len() == 2
}

fn screenshot_media_kind(text: &str) -> &'static str {
    if text.contains("[图片]") || text.contains("图片") {
        "image"
    } else if text.contains("[表情]") || text.contains("表情") {
        "emoji"
    } else if text.contains("引用") {
        "quote"
    } else {
        "text"
    }
}

fn merge_screenshot_analysis(
    local: ScreenshotAnalysis,
    mut provider: ScreenshotAnalysis,
) -> ScreenshotAnalysis {
    if provider.turns.is_empty()
        && provider.last_reply_target.trim().is_empty()
        && provider.visible_time_label.trim().is_empty()
    {
        return local;
    }
    if provider.visible_time_label.trim().is_empty() {
        provider.visible_time_label = local.visible_time_label;
    }
    if provider.inferred_chat_time.trim().is_empty() {
        provider.inferred_chat_time = local.inferred_chat_time;
    }
    if provider.staleness.trim().is_empty() {
        provider.staleness = local.staleness;
    }
    if provider.last_reply_target.trim().is_empty() {
        provider.last_reply_target = local.last_reply_target;
    }
    if provider.turns.is_empty() {
        provider.turns = local.turns;
    }
    provider.warnings.extend(local.warnings);
    provider
}

fn screenshot_source_confidence(analysis: &ScreenshotAnalysis) -> f64 {
    if analysis.turns.is_empty() {
        return 0.35;
    }
    (analysis
        .turns
        .iter()
        .map(|turn| turn.confidence)
        .sum::<f64>()
        / analysis.turns.len() as f64)
        .clamp(0.0, 1.0)
}

fn screenshot_source_metadata(analysis: Option<&ScreenshotAnalysis>) -> String {
    analysis
        .map(|analysis| {
            serde_json::to_string(&serde_json::json!({
                "last_reply_target": analysis.last_reply_target,
                "staleness": analysis.staleness,
                "warnings": analysis.warnings,
            }))
            .unwrap_or_else(|_| "{}".to_string())
        })
        .unwrap_or_else(|| "{}".to_string())
}

async fn process_due_reminders(app: &AppHandle, repo: &MemoryRepository) -> anyhow::Result<()> {
    let now = Utc::now();
    let due = repo.due_reminders(now)?;
    for detail in due {
        let reminder_id = detail.reminder.id.clone();
        if is_quiet_hour(Local::now()) && !e2e_disable_quiet_hours_enabled() {
            repo.snooze_reminder(&reminder_id, next_quiet_end(Local::now()))?;
            continue;
        }

        if repo.reminder_is_muted(&detail.reminder, now)? {
            repo.snooze_reminder(&reminder_id, now + ChronoDuration::hours(12))?;
            continue;
        }

        let cooldown_since = now - ChronoDuration::minutes(RECENT_CONTACT_COOLDOWN_MINUTES);
        if repo.has_recent_copy_feedback(cooldown_since)? && !e2e_disable_cooldown_enabled() {
            repo.snooze_reminder(
                &reminder_id,
                now + ChronoDuration::minutes(RECENT_CONTACT_SNOOZE_MINUTES),
            )?;
            continue;
        }

        if !detail.reminder.contact_id.trim().is_empty() && !e2e_disable_cooldown_enabled() {
            let daily_count = repo.recent_notified_reminder_count(
                &detail.reminder.contact_id,
                now - ChronoDuration::days(1),
            )?;
            let weekly_count = repo.recent_notified_reminder_count(
                &detail.reminder.contact_id,
                now - ChronoDuration::days(7),
            )?;
            if daily_count >= 1 {
                repo.snooze_reminder(&reminder_id, now + ChronoDuration::days(1))?;
                continue;
            }
            if weekly_count >= 2 {
                repo.snooze_reminder(&reminder_id, now + ChronoDuration::days(3))?;
                continue;
            }
        }

        repo.mark_reminder_notified(&reminder_id)?;
        emit_reminder_due(app, &detail);
        send_reminder_notification(app, &detail);
        show_reminder_panel(app);
    }
    Ok(())
}

fn emit_reminder_due(app: &AppHandle, detail: &crate::domain::ReminderDetail) {
    let _ = app.emit("reminder-due", detail);
}

fn send_reminder_notification(app: &AppHandle, detail: &crate::domain::ReminderDetail) {
    let body = if detail.reminder.reason.trim().is_empty() {
        format!(
            "{}。打开 EchoMate 看 3 条低压跟进候选。",
            detail.memory_item.value
        )
    } else {
        format!(
            "{}。打开 EchoMate 看 3 条低压跟进候选。",
            detail.reminder.reason
        )
    };
    if let Err(e) = app
        .notification()
        .builder()
        .title("EchoMate 跟进提醒")
        .body(truncate_notification_body(&body))
        .show()
    {
        tracing::warn!("Failed to show reminder notification: {e}");
    }
}

fn show_reminder_panel(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.eval(
            r#"
            if (window.location.pathname.endsWith('/settings.html')) {
              window.location.href = 'index.html#reminders';
            }
            "#,
        );
        let _ = window.set_always_on_top(false);
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn is_quiet_hour(now: DateTime<Local>) -> bool {
    let hour = now.hour();
    !(8..23).contains(&hour)
}

fn next_quiet_end(now: DateTime<Local>) -> DateTime<Utc> {
    let date = if now.hour() >= 23 {
        now.date_naive()
            .succ_opt()
            .unwrap_or_else(|| now.date_naive())
    } else {
        now.date_naive()
    };
    let naive = date
        .and_hms_opt(8, 30, 0)
        .unwrap_or_else(|| now.naive_local() + ChronoDuration::hours(8));
    Local
        .from_local_datetime(&naive)
        .single()
        .unwrap_or_else(|| now + ChronoDuration::hours(8))
        .with_timezone(&Utc)
}

fn truncate_notification_body(raw: &str) -> String {
    let mut chars = raw.chars();
    let head = chars.by_ref().take(110).collect::<String>();
    if chars.next().is_none() {
        raw.to_string()
    } else {
        format!("{head}...")
    }
}

fn e2e_mock_provider_enabled() -> bool {
    std::env::var("ECHOMATE_E2E_MOCK_PROVIDER")
        .map(|value| value == "1")
        .unwrap_or(false)
}

fn e2e_profile_dir() -> PathBuf {
    std::env::var("ECHOMATE_E2E_PROFILE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::temp_dir().join(format!("echomate-e2e-profile-{}", std::process::id()))
        })
}

fn has_e2e_mock_artifacts(envelope: &CandidateEnvelope) -> bool {
    envelope.context_summary.source_ref == "e2e-mock"
        || envelope
            .memory_candidates
            .iter()
            .any(|candidate| candidate.source_ref == "e2e-mock")
        || envelope
            .reminder_candidates
            .iter()
            .any(|candidate| candidate.source_ref == "e2e-mock")
}

fn e2e_skip_screenclip_enabled() -> bool {
    std::env::var("ECHOMATE_E2E_SKIP_SCREENCLIP")
        .map(|value| value == "1")
        .unwrap_or(false)
}

fn e2e_disable_quiet_hours_enabled() -> bool {
    std::env::var("ECHOMATE_E2E_DISABLE_QUIET_HOURS")
        .map(|value| value == "1")
        .unwrap_or(false)
}

fn e2e_disable_cooldown_enabled() -> bool {
    std::env::var("ECHOMATE_E2E_DISABLE_COOLDOWN")
        .map(|value| value == "1")
        .unwrap_or(false)
}

fn mock_e2e_envelope(source_kind: &str) -> CandidateEnvelope {
    CandidateEnvelope {
        candidates: vec![
            Candidate {
                text: "那你明天面试加油，结束后好好休息。".to_string(),
                intent_group: "支持".to_string(),
                style_tags: vec!["温柔".to_string()],
                risk_flags: vec!["none".to_string()],
                source_refs: vec!["e2e-mock".to_string()],
                reason: "接住明确事件，保持低压".to_string(),
            },
            Candidate {
                text: "明天面试顺利，别给自己太大压力。".to_string(),
                intent_group: "稳妥".to_string(),
                style_tags: vec!["稳妥".to_string()],
                risk_flags: vec!["none".to_string()],
                source_refs: vec!["e2e-mock".to_string()],
                reason: "轻鼓励，不追问细节".to_string(),
            },
            Candidate {
                text: "冲，明天就当去聊一聊。".to_string(),
                intent_group: "轻松".to_string(),
                style_tags: vec!["轻松".to_string()],
                risk_flags: vec!["none".to_string()],
                source_refs: vec!["e2e-mock".to_string()],
                reason: "缓解紧张感".to_string(),
            },
            Candidate {
                text: "早点睡，明天保持状态就好。".to_string(),
                intent_group: "温柔".to_string(),
                style_tags: vec!["关心".to_string()],
                risk_flags: vec!["none".to_string()],
                source_refs: vec!["e2e-mock".to_string()],
                reason: "自然提醒休息".to_string(),
            },
            Candidate {
                text: "那我不打扰你准备啦，明天顺利。".to_string(),
                intent_group: "收束".to_string(),
                style_tags: vec!["收束".to_string()],
                risk_flags: vec!["none".to_string()],
                source_refs: vec!["e2e-mock".to_string()],
                reason: "适合在事件前自然收束".to_string(),
            },
        ],
        situation: crate::domain::GenerationSituation {
            summary: "对方明确提到明天有面试。".to_string(),
            action_type: "wrap_up".to_string(),
            staleness: "unknown".to_string(),
            relationship_signal: "仅基于当前 fake fixture，不做关系判断。".to_string(),
            confidence: 0.86,
        },
        action_card: NextAction {
            action_type: "wrap_up".to_string(),
            reason: "她明确提到明天面试，适合轻鼓励后收束，不继续追问。".to_string(),
            confidence: 0.86,
        },
        source_summary: "e2e mock 输出，不能进入真实上下文。".to_string(),
        memory_candidates: Vec::new(),
        reminder_candidates: Vec::new(),
        context_summary: ContextSummaryCandidate {
            source_kind: source_kind.to_string(),
            source_ref: "e2e-mock".to_string(),
            summary: "对方明确提到明天有面试。".to_string(),
        },
        screenshot_analysis: ScreenshotAnalysis::default(),
    }
}
