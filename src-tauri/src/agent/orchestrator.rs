use crate::agent::parser::OutputParser;
use crate::agent::schema;
use crate::agent::PromptComposer;
use crate::domain::CandidateEnvelope;
use crate::platform::clipboard::ClipboardManager;
use crate::platform::hotkey::HotkeyManager;
use crate::platform::input::InputSimulator;
use crate::provider::claude::ClaudeProvider;
use crate::provider::codex::CodexProvider;
use crate::ui::window::WindowManager;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

const SELECTION_COPY_SETTLE: Duration = Duration::from_millis(250);
const CLIPBOARD_POLL_INTERVAL: Duration = Duration::from_millis(50);
const CLIPBOARD_POLL_ATTEMPTS: usize = 40;

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
        }
    }
}

pub struct Orchestrator {
    pub config: Arc<Mutex<AppConfig>>,
    hotkey: HotkeyManager,
    clipboard: ClipboardManager,
    input: InputSimulator,
    window: WindowManager,
    prompt_composer: PromptComposer,
    parser: OutputParser,
    schema_dir: PathBuf,
    generation_in_progress: AtomicBool,
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
        Self {
            config: Arc::new(Mutex::new(config)),
            hotkey: HotkeyManager::new(),
            clipboard: ClipboardManager::new(),
            input: InputSimulator::new(),
            window: WindowManager::new(),
            prompt_composer: PromptComposer::new(),
            parser: OutputParser::new(),
            schema_dir,
            generation_in_progress: AtomicBool::new(false),
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
        self.trigger_inner(app, TriggerInput::Clipboard).await
    }

    pub async fn trigger_from_selection(
        &self,
        app: &AppHandle,
    ) -> Result<CandidateEnvelope, String> {
        self.trigger_inner(app, TriggerInput::Selection).await
    }

    async fn trigger_inner(
        &self,
        app: &AppHandle,
        source: TriggerInput,
    ) -> Result<CandidateEnvelope, String> {
        if self.generation_in_progress.swap(true, Ordering::AcqRel) {
            let message = "已有一次生成正在进行，请等当前请求完成或超时后再试".to_string();
            self.emit_generation_error(app, &message);
            return Err(message);
        }
        let _generation_guard = GenerationGuard::new(&self.generation_in_progress);

        let text = match self.read_trigger_text(app, source).await {
            Ok(text) => text,
            Err(e) => {
                self.emit_generation_error(app, &e);
                return Err(e);
            }
        };
        if text.is_empty() {
            let message = "剪贴板为空，请先复制一段聊天内容再触发".to_string();
            self.emit_generation_error(app, &message);
            return Err(message);
        }

        tracing::info!("Clipboard text length: {}", text.len());

        let _ = app.emit(
            "generation-started",
            serde_json::json!({"length": text.len()}),
        );

        let config = self.config.lock().unwrap().clone();
        let system_prompt = self.prompt_composer.system_prompt();
        let task_prompt = self.prompt_composer.task_prompt(
            &text,
            &config.tone,
            &config.length,
            config.emoji_level,
            config.humor_level,
        );
        let full_prompt = format!("{}\n\n---\n\n{}", system_prompt, task_prompt);

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

        let result = self
            .call_provider(&config, &full_prompt, &schema_json, &schema_path)
            .await;

        match result {
            Ok(envelope) => {
                if let Err(errs) = self.parser.validate(&envelope) {
                    tracing::warn!("Validation warnings: {:?}", errs);
                }
                let _ = app.emit(
                    "candidates-ready",
                    serde_json::json!({
                        "candidates": &envelope.candidates,
                        "provider": &config.primary_provider,
                        "mode": "standard",
                    }),
                );
                self.window.show_popup(app);
                Ok(envelope)
            }
            Err(e) => {
                self.emit_generation_error(app, &e);
                Err(e)
            }
        }
    }

    async fn read_trigger_text(
        &self,
        app: &AppHandle,
        source: TriggerInput,
    ) -> Result<String, String> {
        match source {
            TriggerInput::Clipboard => self.clipboard.read_text(app),
            TriggerInput::Selection => self.copy_selection_to_clipboard(app).await,
        }
    }

    async fn copy_selection_to_clipboard(&self, app: &AppHandle) -> Result<String, String> {
        let previous = self.clipboard.read_text(app).unwrap_or_default();
        let marker = clipboard_probe_marker();

        self.clipboard.write_text(app, &marker)?;
        tokio::time::sleep(SELECTION_COPY_SETTLE).await;

        if let Err(e) = self.input.copy_selection() {
            let _ = self.clipboard.write_text(app, &previous);
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
                    let _ = self.clipboard.write_text(app, &previous);
                    return Err("复制到了空内容，请先选中一段文字再按快捷键".into());
                }
                tracing::info!("Copied selected text length: {}", text.len());
                return Ok(text);
            }
        }

        let _ = self.clipboard.write_text(app, &previous);
        if let Some(e) = last_clipboard_error {
            tracing::warn!("Clipboard stayed unreadable while copying selection: {e}");
        }
        Err("没有检测到选中的文本内容。请确认当前应用支持 Ctrl+C 复制文本；也可以手动复制后点击“生成回复”。".into())
    }

    fn emit_generation_error(&self, app: &AppHandle, message: &str) {
        let _ = app.emit("generation-error", serde_json::json!({"message": message}));
        self.window.show_popup(app);
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
}

pub struct OrchestratorState(pub Orchestrator);

#[derive(Clone, Copy)]
enum TriggerInput {
    Clipboard,
    Selection,
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

fn clipboard_probe_marker() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("ECHOMATE_COPY_PROBE_{}_{}", std::process::id(), nanos)
}
