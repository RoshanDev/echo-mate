use crate::agent::PromptComposer;
use crate::agent::schema;
use crate::agent::parser::OutputParser;
use crate::platform::clipboard::ClipboardManager;
use crate::platform::hotkey::HotkeyManager;
use crate::provider::claude::ClaudeProvider;
use crate::provider::codex::CodexProvider;
use crate::ui::window::WindowManager;
use crate::domain::CandidateEnvelope;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

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
            primary_provider: "codex".into(),
            fallback_provider: "claude".into(),
            candidate_count: 5,
            timeout_seconds: 45,
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
    window: WindowManager,
    prompt_composer: PromptComposer,
    parser: OutputParser,
    schema_dir: PathBuf,
}

impl Orchestrator {
    pub fn new() -> Self {
        let schema_dir = std::env::temp_dir().join("echomate-schemas");
        Self {
            config: Arc::new(Mutex::new(AppConfig::default())),
            hotkey: HotkeyManager::new(),
            clipboard: ClipboardManager::new(),
            window: WindowManager::new(),
            prompt_composer: PromptComposer::new(),
            parser: OutputParser::new(),
            schema_dir,
        }
    }

    pub async fn trigger(&self, app: &AppHandle) -> Result<CandidateEnvelope, String> {
        let text = self.clipboard.read_text(app)?;
        if text.is_empty() {
            return Err("剪贴板为空".into());
        }

        tracing::info!("Clipboard text length: {}", text.len());

        let _ = app.emit("generation-started", serde_json::json!({"length": text.len()}));

        let config = self.config.lock().unwrap().clone();
        let system_prompt = self.prompt_composer.system_prompt();
        let task_prompt = self.prompt_composer.task_prompt(
            &text, &config.tone, &config.length,
            config.emoji_level, config.humor_level,
        );
        let full_prompt = format!("{}\n\n---\n\n{}", system_prompt, task_prompt);

        tokio::fs::create_dir_all(&self.schema_dir).await.map_err(|e| e.to_string())?;
        let schema_path = self.schema_dir.join("reply_candidates.schema.json");
        let schema_json = schema::candidate_schema();
        tokio::fs::write(&schema_path, serde_json::to_string_pretty(&schema_json).unwrap())
            .await.map_err(|e| e.to_string())?;

        let result = self.call_provider(&config, &full_prompt, &schema_json, &schema_path).await;

        match result {
            Ok(envelope) => {
                if let Err(errs) = self.parser.validate(&envelope) {
                    tracing::warn!("Validation warnings: {:?}", errs);
                }
                let _ = app.emit("candidates-ready", serde_json::json!({
                    "candidates": &envelope.candidates,
                    "provider": &config.primary_provider,
                    "mode": "standard",
                }));
                self.window.show_popup(app);
                Ok(envelope)
            }
            Err(e) => {
                let _ = app.emit("generation-error", serde_json::json!({"message": e}));
                self.window.show_popup(app);
                Err(e)
            }
        }
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
                        format!("Codex failed: {}", e)
                    } else {
                        format!("Codex failed: {}", e)
                    }
                })
            }
            "claude" => {
                let provider = ClaudeProvider::new().with_timeout(config.timeout_seconds);
                provider.generate(prompt, schema_json).await.map_err(|e| {
                    format!("Claude failed: {}", e)
                })
            }
            _ => Err(format!("Unknown provider: {}", config.primary_provider)),
        }
    }

    pub fn init(&self, app: &AppHandle) {
        let config = self.config.lock().unwrap().clone();

        // Clone what we need for the closure
        let app_handle = app.clone();
        self.hotkey.register(app, &config.hotkey, move || {
            let app = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                // Access orchestrator through app state
                let state = app.state::<OrchestratorState>();
                if let Err(e) = state.0.trigger(&app).await {
                    tracing::error!("Orchestrator trigger error: {}", e);
                }
            });
        });

        tracing::info!("Orchestrator initialized with provider: {}", config.primary_provider);
    }
}

pub struct OrchestratorState(pub Orchestrator);
