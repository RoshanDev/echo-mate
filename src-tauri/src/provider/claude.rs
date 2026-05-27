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
            .arg("--no-session-persistence")
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
    let trimmed = stdout.trim();
    tracing::info!("Claude raw output (first 500 chars): {}", &trimmed[..std::cmp::min(500, trimmed.len())]);

    // Claude Code --output-format json returns an ARRAY of events
    // The last event typically has "structured_output" and/or "result" with the schema output
    if let Ok(events) = serde_json::from_str::<Vec<serde_json::Value>>(trimmed) {
        // Scan all events (reverse order) for structured output
        for event in events.iter().rev() {
            // Primary: "structured_output" from --json-schema
            if let Some(so) = event.get("structured_output") {
                if !so.is_null() {
                    if let Ok(env) = serde_json::from_value::<CandidateEnvelope>(so.clone()) {
                        return Ok(env);
                    }
                }
            }
            // Fallback: "result" field (can be string or object)
            if let Some(result_val) = event.get("result") {
                if !result_val.is_null() {
                    if let Ok(env) = serde_json::from_value::<CandidateEnvelope>(result_val.clone()) {
                        return Ok(env);
                    }
                    if let Some(s) = result_val.as_str() {
                        if let Ok(env) = serde_json::from_str::<CandidateEnvelope>(s) {
                            return Ok(env);
                        }
                    }
                }
            }
            // Also try "message"."content" blocks
            if let Some(content) = event.get("message").and_then(|m| m.get("content")) {
                if let Some(blocks) = content.as_array() {
                    for block in blocks {
                        if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                            if let Ok(env) = serde_json::from_str::<CandidateEnvelope>(text) {
                                return Ok(env);
                            }
                        }
                    }
                }
            }
        }
        // Build detailed error with last event info
        let is_error = events.last().and_then(|e| e.get("is_error").and_then(|v| v.as_bool())).unwrap_or(false);
        let so_str = events.last().and_then(|e| e.get("structured_output")).map(|v| v.to_string()).unwrap_or_default();
        let result_str = events.last().and_then(|e| e.get("result")).map(|v| v.to_string()).unwrap_or_default();
        return Err(anyhow::anyhow!(
            "Failed to extract candidates from Claude output ({} events, is_error={}). structured_output(first 300)={:.300}, result(first 300)={:.300}",
            events.len(), is_error, so_str, result_str
        ));
    }

    // Fallback: try parsing as a single object
    let wrapped: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|e| anyhow::anyhow!("Claude output not valid JSON: {}. Raw: {}", e, &trimmed[..std::cmp::min(300, trimmed.len())]))?;

    // Try direct CandidateEnvelope
    if let Ok(env) = serde_json::from_value::<CandidateEnvelope>(wrapped.clone()) {
        return Ok(env);
    }

    // Try nested keys
    if let Some(result) = wrapped.get("result") {
        if let Ok(env) = serde_json::from_value::<CandidateEnvelope>(result.clone()) {
            return Ok(env);
        }
        if let Some(s) = result.as_str() {
            if let Ok(env) = serde_json::from_str::<CandidateEnvelope>(s) {
                return Ok(env);
            }
        }
    }
    if let Some(candidates) = wrapped.get("candidates") {
        if let Ok(list) = serde_json::from_value::<Vec<Candidate>>(candidates.clone()) {
            return Ok(CandidateEnvelope { candidates: list });
        }
    }
    for key in &["structured_output", "content", "output"] {
        if let Some(inner) = wrapped.get(key) {
            if let Ok(env) = serde_json::from_value::<CandidateEnvelope>(inner.clone()) {
                return Ok(env);
            }
            if let Some(s) = inner.as_str() {
                if let Ok(env) = serde_json::from_str::<CandidateEnvelope>(s) {
                    return Ok(env);
                }
            }
        }
    }

    anyhow::bail!("Failed to parse Claude output. Top-level keys: {:?}. Raw (first 500): {}",
        wrapped.as_object().map(|o| o.keys().collect::<Vec<_>>()).unwrap_or_default(),
        &trimmed[..std::cmp::min(500, trimmed.len())]);
}
