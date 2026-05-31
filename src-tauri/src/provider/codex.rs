use crate::domain::{Candidate, CandidateEnvelope};
use crate::provider::process::wait_with_timeout;
use crate::provider::wsl;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

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

    pub async fn generate(
        &self,
        prompt: &str,
        schema_path: &PathBuf,
    ) -> anyhow::Result<CandidateEnvelope> {
        self.generate_with_images(prompt, schema_path, &[]).await
    }

    pub async fn generate_with_images(
        &self,
        prompt: &str,
        schema_path: &PathBuf,
        image_paths: &[PathBuf],
    ) -> anyhow::Result<CandidateEnvelope> {
        tokio::fs::create_dir_all(&self.workspace).await?;

        let output_file = self.workspace.join("final.json");
        let _ = tokio::fs::remove_file(&output_file).await;

        // Use WSL paths when running through wsl.exe
        let (cwd, schema, out_file) = if wsl::is_windows() {
            (
                self.wsl_workspace.clone(),
                wsl::to_wsl_path(schema_path),
                wsl::to_wsl_path(&output_file),
            )
        } else {
            (
                self.workspace.clone(),
                schema_path.clone(),
                output_file.clone(),
            )
        };
        let images = image_paths
            .iter()
            .map(|path| {
                if wsl::is_windows() {
                    wsl::to_wsl_path(path)
                } else {
                    path.clone()
                }
            })
            .collect::<Vec<_>>();

        let mut cmd = wsl::codex_command(&self.binary);
        cmd.arg("exec")
            .arg("--sandbox")
            .arg("read-only")
            .arg("--ephemeral")
            .arg("--ignore-user-config")
            .arg("--ignore-rules")
            .arg("--skip-git-repo-check")
            .arg("--cd")
            .arg(&cwd)
            .arg("--json")
            .arg("--output-schema")
            .arg(&schema)
            .arg("--output-last-message")
            .arg(&out_file);

        for image in &images {
            cmd.arg("--image").arg(image);
        }

        cmd.arg("-")
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

        let mut child = cmd
            .spawn()
            .map_err(|e| anyhow::anyhow!("Codex could not start: {}", e))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes()).await?;
            stdin.shutdown().await?;
        }

        let output = wait_with_timeout(child, self.timeout, "Codex").await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Codex failed (exit {}): {}", output.status, stderr);
        }

        // The agent writes through the WSL path on Windows, but this process
        // must read the host path after the child exits.
        if output_file.exists() {
            let content = tokio::fs::read_to_string(&output_file).await?;
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
                return Ok(CandidateEnvelope::from_candidates(list));
            }
        }
    }
    anyhow::bail!("Failed to parse Codex output as CandidateEnvelope");
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[tokio::test]
    async fn provider_e2e_passes_image_argument_and_parses_output_file() {
        let dir = test_dir("codex-image");
        let cli = write_fake_cli(
            &dir,
            "codex-image",
            r#"#!/usr/bin/env bash
out=""
saw_image="0"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --output-last-message)
      shift
      out="$1"
      ;;
    --image)
      shift
      test -f "$1" || exit 3
      saw_image="1"
      ;;
  esac
  shift || true
done
cat >/dev/null
if [[ "$saw_image" != "1" ]]; then
  echo "missing image argument" >&2
  exit 2
fi
printf '%s\n' '{"candidates":[{"text":"我看到了，等下回你","style_tags":["safe"],"risk_flags":["none"],"reason":"context"}]}' > "$out"
"#,
        );
        let schema = dir.join("schema.json");
        let image = dir.join("chat.png");
        fs::write(&schema, r#"{"type":"object"}"#).expect("write schema");
        fs::write(&image, b"fake image").expect("write image");

        let envelope = CodexProvider::new()
            .with_binary(cli.to_str().expect("utf-8 path"))
            .with_timeout(2)
            .generate_with_images("根据截图回复", &schema, &[image])
            .await
            .expect("fake Codex should return candidates");

        assert_eq!(envelope.candidates.len(), 1);
        assert_eq!(envelope.candidates[0].text, "我看到了，等下回你");

        let _ = fs::remove_dir_all(dir);
    }

    fn test_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("echomate-{name}-{nanos}"));
        fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    fn write_fake_cli(dir: &PathBuf, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, body).expect("write fake cli");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("chmod fake cli");
        path
    }
}
