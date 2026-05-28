use crate::domain::CandidateEnvelope;
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
            timeout: Duration::from_secs(120),
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
        tracing::info!("ClaudeProvider::generate called, binary={}, timeout={:?}", self.binary, self.timeout);
        tracing::debug!("Prompt (first 300 chars): {}", &prompt[..std::cmp::min(300, prompt.len())]);

        // Ensure workspace exists
        tokio::fs::create_dir_all(&self.workspace).await?;

        let schema_str = serde_json::to_string(schema)?;
        tracing::debug!("Schema length: {} bytes", schema_str.len());

        let mut child = Command::new(&self.binary)
            .arg("-p")
            .arg("--output-format").arg("json")
            .arg("--json-schema").arg(&schema_str)
            .arg("--no-session-persistence")
            .arg("--max-turns").arg("10")
            .arg(prompt)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .current_dir(&self.workspace)
            .spawn()?;

        // Close stdin immediately (no stdin input)
        if let Some(stdin) = child.stdin.take() {
            drop(stdin);
        }

        tracing::info!("Claude process spawned, waiting with timeout {:?}...", self.timeout);

        let result = timeout(self.timeout, child.wait_with_output()).await;
        let output = match result {
            Ok(Ok(out)) => {
                tracing::info!("Claude process exited with status: {}", out.status);
                out
            }
            Ok(Err(e)) => {
                tracing::error!("Claude process error: {}", e);
                anyhow::bail!("Claude process error: {}", e)
            }
            Err(_) => {
                tracing::error!("Claude timed out after {:?}", self.timeout);
                anyhow::bail!("Claude timed out after {:?}", self.timeout)
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!("Claude failed (exit {}): {}", output.status, stderr);
            anyhow::bail!("Claude failed (exit {}): {}", output.status, stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        tracing::info!("Claude stdout length: {} bytes", stdout.len());

        // Dump raw output for debugging
        let debug_path = std::env::var("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(".echomate")
            .join("logs")
            .join("last-claude-output.json");
        let _ = std::fs::write(&debug_path, stdout.as_bytes());

        parse_json_output(&stdout)
    }
}

/// Parse --output-format json output from Claude CLI.
/// The result is a single JSON object with "structured_output" and "result" fields.
fn parse_json_output(stdout: &str) -> anyhow::Result<CandidateEnvelope> {
    let trimmed = stdout.trim();
    tracing::info!("Claude raw output (first 500 chars): {}", &trimmed[..std::cmp::min(500, trimmed.len())]);

    let wrapped: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|e| anyhow::anyhow!("Claude output not valid JSON: {}. Raw: {}", e, &trimmed[..std::cmp::min(300, trimmed.len())]))?;

    // Primary: "structured_output" from --json-schema
    if let Some(so) = wrapped.get("structured_output") {
        if !so.is_null() {
            if let Ok(env) = serde_json::from_value::<CandidateEnvelope>(so.clone()) {
                tracing::info!("Parsed candidates from structured_output");
                return Ok(env);
            }
            tracing::warn!("structured_output present but failed to parse as CandidateEnvelope: {}", so);
        }
    }

    // Fallback: "result" field (may contain JSON string)
    if let Some(result_val) = wrapped.get("result") {
        if !result_val.is_null() {
            if let Ok(env) = serde_json::from_value::<CandidateEnvelope>(result_val.clone()) {
                tracing::info!("Parsed candidates from result field (object)");
                return Ok(env);
            }
            if let Some(s) = result_val.as_str() {
                // Try direct parse
                if let Ok(env) = serde_json::from_str::<CandidateEnvelope>(s) {
                    tracing::info!("Parsed candidates from result field (string)");
                    return Ok(env);
                }
                // Try finding JSON within the text
                if let Some(start) = s.find('{') {
                    if let Some(end) = s.rfind('}') {
                        if let Ok(env) = serde_json::from_str::<CandidateEnvelope>(&s[start..=end]) {
                            tracing::info!("Parsed candidates from JSON embedded in result text");
                            return Ok(env);
                        }
                    }
                }
            }
        }
    }

    anyhow::bail!(
        "Failed to parse CandidateEnvelope. Keys: {:?}. structured_output: {}. result(first 300): {:.300}",
        wrapped.as_object().map(|o| o.keys().collect::<Vec<_>>()).unwrap_or_default(),
        wrapped.get("structured_output").map(|v| v.to_string()).unwrap_or_else(|| "null".into()),
        wrapped.get("result").and_then(|v| v.as_str()).unwrap_or(""),
    )
}
