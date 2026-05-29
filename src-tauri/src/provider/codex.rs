use crate::domain::{CandidateEnvelope, Candidate};
use crate::provider::wsl;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;

pub struct CodexProvider {
    binary: String,
    timeout: Duration,
    workspace: PathBuf,
    wsl_workspace: PathBuf,
}

impl CodexProvider {
    pub fn new() -> Self {
        let workspace = std::env::temp_dir().join("echomate-codex");
        let wsl_workspace = wsl::to_wsl_path(&workspace);
        Self {
            binary: "codex".to_string(),
            timeout: Duration::from_secs(45),
            workspace,
            wsl_workspace,
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

    pub async fn generate(&self, prompt: &str, schema_path: &PathBuf) -> anyhow::Result<CandidateEnvelope> {
        tokio::fs::create_dir_all(&self.workspace).await?;

        let output_file = self.workspace.join("final.json");

        // Use WSL paths when running through wsl.exe
        let (cwd, schema, out_file) = if wsl::is_windows() {
            (
                self.wsl_workspace.clone(),
                wsl::to_wsl_path(schema_path),
                wsl::to_wsl_path(&output_file),
            )
        } else {
            (self.workspace.clone(), schema_path.clone(), output_file.clone())
        };

        let mut cmd = wsl::wsl_command(&self.binary);
        cmd.arg("exec")
            .arg("--sandbox").arg("read-only")
            .arg("--ephemeral")
            .arg("--ignore-user-config")
            .arg("--ignore-rules")
            .arg("--skip-git-repo-check")
            .arg("--cd").arg(&cwd)
            .arg("--json")
            .arg("--output-schema").arg(&schema)
            .arg("--output-last-message").arg(&out_file)
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        if !wsl::is_windows() {
            cmd.current_dir(&cwd);
        }

        if !wsl::is_windows() {
            cmd.env_clear()
                .env("PATH", std::env::var("PATH").unwrap_or_default())
                .env("HOME", std::env::var("HOME").unwrap_or_default());
        }
        // On Windows, don't touch env — wsl.exe sets up the WSL environment automatically

        let mut child = cmd.spawn()?;

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

        // Try output-last-message file (WSL path on Windows, native path on Linux)
        let read_path = if wsl::is_windows() { &out_file } else { &output_file };
        if read_path.exists() {
            let content = tokio::fs::read_to_string(read_path).await?;
            return parse_codex_output(&content);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_codex_output(&stdout)
    }
}

fn parse_codex_output(raw: &str) -> anyhow::Result<CandidateEnvelope> {
    if let Ok(env) = serde_json::from_str::<CandidateEnvelope>(raw) {
        return Ok(env);
    }
    if let Ok(wrapped) = serde_json::from_str::<serde_json::Value>(raw) {
        if let Some(candidates) = wrapped.get("candidates") {
            if let Ok(list) = serde_json::from_value::<Vec<Candidate>>(candidates.clone()) {
                return Ok(CandidateEnvelope { candidates: list });
            }
        }
    }
    anyhow::bail!("Failed to parse Codex output as CandidateEnvelope");
}
