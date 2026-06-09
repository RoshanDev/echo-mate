use crate::agent::parser::OutputParser;
use crate::agent::schema;
use crate::agent::PromptComposer;
use crate::domain::{
    Candidate, CandidateEnvelope, ContactInput, ContactRecord, ContextPolicy,
    ContextSummaryCandidate, ContextSummaryRecord, MemoryCandidate, NextAction, PermissionStatus,
    PlatformSignal, PlatformSignalResult, ReminderCandidate, StyleProfileRecord,
};
use crate::platform::clipboard::{ClipboardImage, ClipboardManager};
use crate::platform::hotkey::HotkeyManager;
use crate::platform::input::InputSimulator;
use crate::platform::screenshot::ScreenCapture;
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
        }
    }

    fn config_path() -> PathBuf {
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

    pub async fn trigger_topics(&self, app: &AppHandle) -> Result<CandidateEnvelope, String> {
        self.generate_with_guard(app, GenerationInput::Topic).await
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
            GenerationInput::Topic => self.generate_from_topic(app, input).await,
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
                let context_record = self.persist_generation_artifacts(
                    &envelope,
                    "clipboard",
                    &generation_context,
                    Some(&text),
                );
                self.emit_candidates_ready(
                    app,
                    &envelope,
                    &config.primary_provider,
                    "standard",
                    &generation_context.policy,
                    context_record.as_ref(),
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
        let system_prompt = self.prompt_composer.system_prompt();
        let task_prompt = self.prompt_composer.screenshot_task_prompt(
            screenshot.width,
            screenshot.height,
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
                let envelope = self.apply_context_policy(envelope, &generation_context);
                let context_record = self.persist_generation_artifacts(
                    &envelope,
                    "screenshot",
                    &generation_context,
                    None,
                );
                self.emit_candidates_ready(
                    app,
                    &envelope,
                    &provider,
                    "screenshot",
                    &generation_context.policy,
                    context_record.as_ref(),
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
                let context_record = self.persist_generation_artifacts(
                    &envelope,
                    "topic",
                    &generation_context,
                    None,
                );
                self.emit_candidates_ready(
                    app,
                    &envelope,
                    &config.primary_provider,
                    "topic",
                    &generation_context.policy,
                    context_record.as_ref(),
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

    fn emit_candidates_ready(
        &self,
        app: &AppHandle,
        envelope: &CandidateEnvelope,
        provider: &str,
        mode: &str,
        policy: &ContextPolicy,
        context_record: Option<&ContextSummaryRecord>,
    ) {
        if let Err(errs) = self.parser.validate(envelope) {
            tracing::warn!("Validation warnings: {:?}", errs);
        }
        let _ = app.emit(
            "candidates-ready",
            serde_json::json!({
                "candidates": &envelope.candidates,
                "action_card": &envelope.action_card,
                "memory_candidates": &envelope.memory_candidates,
                "reminder_candidates": &envelope.reminder_candidates,
                "context_summary": &envelope.context_summary,
                "context_policy": policy,
                "context_record": context_record,
                "provider": provider,
                "mode": mode,
            }),
        );
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
        let context_block = self.build_context_block(contact.as_ref(), &policy);
        GenerationContext {
            contact,
            policy,
            context_block,
        }
    }

    fn build_context_block(
        &self,
        contact: Option<&ContactRecord>,
        policy: &ContextPolicy,
    ) -> String {
        if !policy.can_save_context {
            return format!(
                "- {reason}\n- 必须让 memory_candidates 和 reminder_candidates 返回空数组。\n- 可以继续生成 5 条候选回复，但不要声称已保存任何信息。",
                reason = policy.reason
            );
        }

        let contact = match contact {
            Some(contact) => contact,
            None => return policy.reason.clone(),
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
        let style_profile = self.memory_repo.style_profile().ok().flatten();

        let mut block = format!(
            "- 当前联系人：{} / {}\n- 白名单：已启用\n- 保存策略：只保存用户可见来源，用户可删除；记忆/提醒仍需用户确认。",
            contact.alias, contact.channel
        );
        if let Some(profile) = style_profile {
            let guide = style_profile_prompt_guide(&profile.profile_json);
            block.push_str(&format!(
                "\n- 风格画像指南：{}",
                truncate_for_prompt(&guide, 560)
            ));
        }
        if !memories.is_empty() {
            block.push_str("\n- 已确认联系人记忆：");
            for memory in memories {
                block.push_str(&format!(
                    "\n  - [{}] {}（来源：{}）",
                    memory.memory_type,
                    truncate_for_prompt(&memory.value, 80),
                    truncate_for_prompt(&memory.source_excerpt, 60)
                ));
            }
        }
        if !recent_messages.is_empty() {
            block.push_str("\n- 最近上下文：");
            for message in recent_messages {
                block.push_str(&format!(
                    "\n  - {} / {}：{}",
                    message.role,
                    message.source,
                    truncate_for_prompt(&message.text, 100)
                ));
            }
        }
        block
    }

    fn apply_context_policy(
        &self,
        mut envelope: CandidateEnvelope,
        context: &GenerationContext,
    ) -> CandidateEnvelope {
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
        context: &GenerationContext,
        incoming_text: Option<&str>,
    ) -> Option<ContextSummaryRecord> {
        if !context.policy.can_save_context {
            return None;
        }
        let contact = context.contact.as_ref()?;
        let config = self.config.lock().unwrap().clone();
        if let Err(e) = self
            .memory_repo
            .apply_retention(config.context_retention_days)
        {
            tracing::warn!("Failed to apply retention: {e}");
        }

        if let Some(text) = incoming_text.filter(|text| !text.trim().is_empty()) {
            if let Err(e) = self.memory_repo.append_message(
                &contact.id,
                "other",
                text,
                fallback_source_kind,
                false,
            ) {
                tracing::warn!("Failed to append inbound message: {e}");
            }
        }

        let mut summary = envelope.context_summary.clone();
        if summary.source_kind.trim().is_empty() {
            summary.source_kind = fallback_source_kind.to_string();
        }
        if summary.summary.trim().is_empty() {
            return None;
        }
        if incoming_text.is_none() && !matches!(fallback_source_kind, "topic") {
            if let Err(e) = self.memory_repo.append_message(
                &contact.id,
                "other",
                &summary.summary,
                fallback_source_kind,
                false,
            ) {
                tracing::warn!("Failed to append summarized message: {e}");
            }
        }
        match self
            .memory_repo
            .insert_context_summary_for_contact(Some(&contact.id), &summary)
        {
            Ok(record) => Some(record),
            Err(e) => {
                tracing::warn!("Failed to persist context summary: {e}");
                None
            }
        }
    }

    pub fn save_memory_candidate(
        &self,
        candidate: crate::domain::MemoryCandidate,
    ) -> Result<crate::domain::MemoryItemRecord, String> {
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

    pub async fn create_reminder_from_candidate(
        &self,
        app: &AppHandle,
        candidate: crate::domain::ReminderCandidate,
        trigger_at: Option<String>,
    ) -> Result<crate::domain::ReminderDetail, String> {
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
                    tracing::error!("Orchestrator trigger error: {}", e);
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
    Topic,
}

impl GenerationInput {
    fn source_kind(&self) -> &'static str {
        match self {
            GenerationInput::Text(_) => "text",
            GenerationInput::Screenshot(_) => "screenshot",
            GenerationInput::Topic => "topic",
        }
    }
}

struct GenerationContext {
    contact: Option<ContactRecord>,
    policy: ContextPolicy,
    context_block: String,
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

fn image_signature(image: &ClipboardImage) -> u64 {
    let mut hasher = DefaultHasher::new();
    image.width.hash(&mut hasher);
    image.height.hash(&mut hasher);
    image.rgba.hash(&mut hasher);
    hasher.finish()
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

        let cooldown_since = now - ChronoDuration::minutes(RECENT_CONTACT_COOLDOWN_MINUTES);
        if repo.has_recent_copy_feedback(cooldown_since)? && !e2e_disable_cooldown_enabled() {
            repo.snooze_reminder(
                &reminder_id,
                now + ChronoDuration::minutes(RECENT_CONTACT_SNOOZE_MINUTES),
            )?;
            continue;
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
    let trigger_at = (Utc::now() - ChronoDuration::seconds(5)).to_rfc3339();
    CandidateEnvelope {
        candidates: vec![
            Candidate {
                text: "那你明天面试加油，结束后好好休息。".to_string(),
                style_tags: vec!["温柔".to_string()],
                risk_flags: vec!["none".to_string()],
                reason: "接住明确事件，保持低压".to_string(),
            },
            Candidate {
                text: "明天面试顺利，别给自己太大压力。".to_string(),
                style_tags: vec!["稳妥".to_string()],
                risk_flags: vec!["none".to_string()],
                reason: "轻鼓励，不追问细节".to_string(),
            },
            Candidate {
                text: "冲，明天就当去聊一聊。".to_string(),
                style_tags: vec!["轻松".to_string()],
                risk_flags: vec!["none".to_string()],
                reason: "缓解紧张感".to_string(),
            },
            Candidate {
                text: "早点睡，明天保持状态就好。".to_string(),
                style_tags: vec!["关心".to_string()],
                risk_flags: vec!["none".to_string()],
                reason: "自然提醒休息".to_string(),
            },
            Candidate {
                text: "那我不打扰你准备啦，明天顺利。".to_string(),
                style_tags: vec!["收束".to_string()],
                risk_flags: vec!["none".to_string()],
                reason: "适合在事件前自然收束".to_string(),
            },
        ],
        action_card: NextAction {
            action_type: "wrap_up".to_string(),
            reason: "她明确提到明天面试，适合轻鼓励后收束，不继续追问。".to_string(),
            confidence: 0.86,
        },
        memory_candidates: vec![MemoryCandidate {
            memory_type: "event".to_string(),
            value: "她明天有面试".to_string(),
            source_kind: source_kind.to_string(),
            source_ref: "e2e-mock".to_string(),
            source_excerpt: "我明天面试".to_string(),
            confidence: 0.9,
            sensitivity: "normal".to_string(),
            expires_at: String::new(),
        }],
        reminder_candidates: vec![ReminderCandidate {
            memory_type: "event".to_string(),
            memory_value: "她明天有面试".to_string(),
            source_kind: source_kind.to_string(),
            source_ref: "e2e-mock".to_string(),
            source_excerpt: "我明天面试".to_string(),
            recommended_time: "面试后".to_string(),
            trigger_at,
            reason: "面试后适合轻问结果。".to_string(),
            suggested_follow_up: "今天面试还顺利吗？".to_string(),
            confidence: 0.86,
            sensitivity: "normal".to_string(),
        }],
        context_summary: ContextSummaryCandidate {
            source_kind: source_kind.to_string(),
            source_ref: "e2e-mock".to_string(),
            summary: "对方明确提到明天有面试。".to_string(),
        },
    }
}
