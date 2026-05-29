use crate::domain::CandidateEnvelope;
use crate::provider::wsl;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
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

    pub async fn generate(&self, prompt: &str, schema: &serde_json::Value) -> anyhow::Result<CandidateEnvelope> {
        tracing::info!("ClaudeProvider::generate called, binary={}, timeout={:?}", self.binary, self.timeout);

        tokio::fs::create_dir_all(&self.workspace).await?;
        let schema_str = serde_json::to_string(schema)?;

        let mut cmd = wsl::wsl_command(&self.binary);
        cmd.arg("-p")
            .arg("--output-format").arg("json")
            .arg("--no-session-persistence")
            .arg("--max-turns").arg("10");

        if wsl::is_windows() {
            // Write schema to Windows temp dir, read via /mnt/c/ inside WSL2.
            // This avoids both UNC path issues AND wsl.exe JSON quote mangling.
            let schema_file = self.workspace.join("schema.json");
            std::fs::write(&schema_file, &schema_str)?;
            let wsl_schema_path = wsl::to_wsl_path(&schema_file);

            let wsl_binary = wsl::wsl_binary_path(&self.binary);
            let shell_cmd = format!(
                r#"{} -p --output-format json --json-schema "$(cat {})" --no-session-persistence --max-turns 10 "$@""#,
                wsl_binary, wsl_schema_path.display()
            );
            tracing::info!("Shell: {}", shell_cmd);
            cmd = wsl::new_wsl_command();
            cmd.arg("bash").arg("-c").arg(&shell_cmd);
            cmd.arg("--");
            cmd.arg(prompt);
        } else {
            cmd.arg("--json-schema").arg(&schema_str);
            cmd.arg(prompt);
            cmd.current_dir(&self.workspace);
        }

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = cmd.spawn()?;

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

        let debug_dir = std::env::temp_dir().join("echomate-claude");
        let _ = std::fs::create_dir_all(&debug_dir);
        let _ = std::fs::write(debug_dir.join("last-claude-output.json"), stdout.as_bytes());

        parse_json_output(&stdout)
    }
}

fn parse_json_output(stdout: &str) -> anyhow::Result<CandidateEnvelope> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        anyhow::bail!("Claude returned empty output");
    }
    tracing::info!("Claude raw output (first 500 chars): {}", &trimmed[..std::cmp::min(500, trimmed.len())]);

    let wrapped: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|e| anyhow::anyhow!("Claude output not valid JSON: {}. Raw: {}", e, &trimmed[..std::cmp::min(300, trimmed.len())]))?;

    if let Some(so) = wrapped.get("structured_output") {
        if !so.is_null() {
            if let Ok(env) = serde_json::from_value::<CandidateEnvelope>(so.clone()) {
                tracing::info!("Parsed candidates from structured_output");
                return Ok(env);
            }
        }
    }

    if let Some(result_val) = wrapped.get("result") {
        if !result_val.is_null() {
            if let Ok(env) = serde_json::from_value::<CandidateEnvelope>(result_val.clone()) {
                tracing::info!("Parsed candidates from result field (object)");
                return Ok(env);
            }
            if let Some(s) = result_val.as_str() {
                if let Ok(env) = serde_json::from_str::<CandidateEnvelope>(s) {
                    tracing::info!("Parsed candidates from result field (string)");
                    return Ok(env);
                }
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
        "Failed to parse CandidateEnvelope. Keys: {:?}. result(first 300): {:.300}",
        wrapped.as_object().map(|o| o.keys().collect::<Vec<_>>()).unwrap_or_default(),
        wrapped.get("result").and_then(|v| v.as_str()).unwrap_or(""),
    )
}
