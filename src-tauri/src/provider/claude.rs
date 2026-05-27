use crate::domain::{CandidateEnvelope, Candidate};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

pub struct ClaudeProvider {
    binary: String,
    timeout: Duration,
    workspace: PathBuf,
}

impl ClaudeProvider {
    pub fn new() -> Self {
        Self {
            binary: "claude".to_string(),
            timeout: Duration::from_secs(45),
            workspace: std::env::temp_dir().join("echomate-claude"),
        }
    }

    pub fn with_binary(mut self, bin: &str) -> Self {
        self.binary = bin.to_string();
        self
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout = Duration::from_secs(secs);
        self
    }

    /// Call Claude CLI to generate candidate replies
    pub async fn generate(&self, prompt: &str, schema: &serde_json::Value) -> anyhow::Result<CandidateEnvelope> {
        // Ensure workspace exists
        tokio::fs::create_dir_all(&self.workspace).await?;

        let schema_str = serde_json::to_string(schema)?;

        let mut child = Command::new(&self.binary)
            .arg("-p")
            .arg("--output-format").arg("json")
            .arg("--json-schema").arg(&schema_str)
            .arg("--tools").arg("")
            .arg("--no-session-persistence")
            .arg("--max-turns").arg("2")
            .arg(prompt)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", std::env::var("HOME").unwrap_or_default())
            .current_dir(&self.workspace)
            .spawn()?;

        // Close stdin immediately (no stdin input)
        if let Some(stdin) = child.stdin.take() {
            drop(stdin);
        }

        let result = timeout(self.timeout, child.wait_with_output()).await;
        let output = match result {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => anyhow::bail!("Claude process error: {}", e),
            Err(_) => anyhow::bail!("Claude timed out after {:?}", self.timeout),
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Claude failed (exit {}): {}", output.status, stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_claude_output(&stdout)
    }
}

fn parse_claude_output(stdout: &str) -> anyhow::Result<CandidateEnvelope> {
    // Try direct parse first
    if let Ok(env) = serde_json::from_str::<CandidateEnvelope>(stdout) {
        return Ok(env);
    }

    // Try wrapped: {"structured_output": {...}}
    if let Ok(wrapped) = serde_json::from_str::<serde_json::Value>(stdout) {
        if let Some(inner) = wrapped.get("structured_output") {
            if let Ok(env) = serde_json::from_value::<CandidateEnvelope>(inner.clone()) {
                return Ok(env);
            }
        }
        // Try "content" or "result" keys
        for key in &["content", "result", "output"] {
            if let Some(inner) = wrapped.get(key) {
                if let Ok(env) = serde_json::from_value::<CandidateEnvelope>(inner.clone()) {
                    return Ok(env);
                }
            }
        }
        // Try top-level candidates array directly
        if let Some(candidates) = wrapped.get("candidates") {
            if let Ok(list) = serde_json::from_value::<Vec<Candidate>>(candidates.clone()) {
                return Ok(CandidateEnvelope { candidates: list });
            }
        }
    }

    anyhow::bail!("Failed to parse Claude output as CandidateEnvelope");
}
