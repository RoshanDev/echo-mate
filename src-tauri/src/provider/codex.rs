use crate::domain::{CandidateEnvelope, Candidate};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

pub struct CodexProvider {
    binary: String,
    timeout: Duration,
    workspace: PathBuf,
}

impl CodexProvider {
    pub fn new() -> Self {
        Self {
            binary: "codex".to_string(),
            timeout: Duration::from_secs(45),
            workspace: std::env::temp_dir().join("echomate-codex"),
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

    /// Call Codex CLI to generate candidate replies
    pub async fn generate(&self, prompt: &str, schema_path: &PathBuf) -> anyhow::Result<CandidateEnvelope> {
        // Ensure workspace exists
        tokio::fs::create_dir_all(&self.workspace).await?;

        let output_file = self.workspace.join("final.json");

        let mut child = Command::new(&self.binary)
            .arg("exec")
            .arg("--sandbox").arg("read-only")
            .arg("--ephemeral")
            .arg("--ignore-user-config")
            .arg("--ignore-rules")
            .arg("--skip-git-repo-check")
            .arg("--cd").arg(&self.workspace)
            .arg("--json")
            .arg("--output-schema").arg(schema_path)
            .arg("--output-last-message").arg(&output_file)
            .arg("-")  // read stdin
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", std::env::var("HOME").unwrap_or_default())
            .current_dir(&self.workspace)
            .spawn()?;

        // Write prompt to stdin
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes()).await?;
            stdin.shutdown().await?;
        }

        let result = timeout(self.timeout, child.wait_with_output()).await;
        let output = match result {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => anyhow::bail!("Codex process error: {}", e),
            Err(_) => anyhow::bail!("Codex timed out after {:?}", self.timeout),
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Codex failed (exit {}): {}", output.status, stderr);
        }

        // Try to read from output-last-message file first
        if output_file.exists() {
            let content = tokio::fs::read_to_string(&output_file).await?;
            return parse_codex_output(&content);
        }

        // Fallback: parse stdout
        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_codex_output(&stdout)
    }
}

fn parse_codex_output(raw: &str) -> anyhow::Result<CandidateEnvelope> {
    // Try direct parse first
    if let Ok(env) = serde_json::from_str::<CandidateEnvelope>(raw) {
        return Ok(env);
    }

    // Try wrapped values
    if let Ok(wrapped) = serde_json::from_str::<serde_json::Value>(raw) {
        if let Some(candidates) = wrapped.get("candidates") {
            if let Ok(list) = serde_json::from_value::<Vec<Candidate>>(candidates.clone()) {
                return Ok(CandidateEnvelope { candidates: list });
            }
        }
    }

    anyhow::bail!("Failed to parse Codex output as CandidateEnvelope");
}
