use crate::domain::CandidateEnvelope;
use crate::provider::process::wait_with_timeout;
use crate::provider::wsl;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

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

    pub async fn generate(
        &self,
        prompt: &str,
        schema: &serde_json::Value,
    ) -> anyhow::Result<CandidateEnvelope> {
        tracing::info!(
            "ClaudeProvider::generate called, binary={}, timeout={:?}",
            self.binary,
            self.timeout
        );

        tokio::fs::create_dir_all(&self.workspace).await?;
        let schema_str = serde_json::to_string(schema)?;

        let mut cmd = wsl::wsl_command(&self.binary);
        cmd.arg("-p")
            .arg("--output-format")
            .arg("json")
            .arg("--no-session-persistence")
            .arg("--max-turns")
            .arg("10");

        if wsl::is_windows() {
            // Write schema to Windows temp dir, read via /mnt/c/ inside WSL2.
            // This avoids both UNC path issues AND wsl.exe JSON quote mangling.
            let schema_file = self.workspace.join("schema.json");
            std::fs::write(&schema_file, &schema_str)?;
            let wsl_schema_path = wsl::to_wsl_path(&schema_file);

            let wsl_binary = wsl::wsl_binary_path(&self.binary);
            let shell_cmd = format!(
                r#"{} -p --output-format json --json-schema "$(cat {})" --no-session-persistence --max-turns 10 "$@""#,
                wsl_binary,
                wsl_schema_path.display()
            );
            tracing::info!("Shell: {}", shell_cmd);
            cmd = wsl::new_wsl_command();
            cmd.arg("-e").arg("bash").arg("-c").arg(&shell_cmd);
            cmd.arg("--");
            cmd.arg(prompt);
        } else {
            cmd.arg("--json-schema").arg(&schema_str);
            cmd.arg(prompt);
            cmd.current_dir(&self.workspace);
        }

        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let child = cmd
            .spawn()
            .map_err(|e| anyhow::anyhow!("Claude could not start: {}", e))?;

        tracing::info!(
            "Claude process spawned, waiting with timeout {:?}...",
            self.timeout
        );

        let output = wait_with_timeout(child, self.timeout, "Claude").await?;
        tracing::info!("Claude process exited with status: {}", output.status);

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
    tracing::info!(
        "Claude raw output (first 500 chars): {}",
        &trimmed[..std::cmp::min(500, trimmed.len())]
    );

    let wrapped: serde_json::Value = serde_json::from_str(trimmed).map_err(|e| {
        anyhow::anyhow!(
            "Claude output not valid JSON: {}. Raw: {}",
            e,
            &trimmed[..std::cmp::min(300, trimmed.len())]
        )
    })?;

    if let Some((env, source)) = parse_candidate_envelope(&wrapped) {
        tracing::info!("Parsed candidates from {source}");
        return Ok(env);
    }

    anyhow::bail!(
        "Failed to parse CandidateEnvelope. Top-level: {}. Keys: {:?}. result(first 300): {:.300}",
        value_kind(&wrapped),
        object_keys(&wrapped),
        wrapped.get("result").and_then(|v| v.as_str()).unwrap_or(""),
    )
}

fn parse_candidate_envelope(
    value: &serde_json::Value,
) -> Option<(CandidateEnvelope, &'static str)> {
    if let Ok(env) = serde_json::from_value::<CandidateEnvelope>(value.clone()) {
        return Some((env, "direct object"));
    }

    if let Some(so) = value
        .get("structured_output")
        .filter(|item| !item.is_null())
    {
        if let Some((env, _)) = parse_candidate_envelope(so) {
            return Some((env, "structured_output"));
        }
    }

    if let Some(result) = value.get("result").filter(|item| !item.is_null()) {
        if let Some((env, _)) = parse_candidate_envelope(result) {
            return Some((env, "result field"));
        }
        if let Some(text) = result.as_str() {
            if let Some(env) = parse_candidate_envelope_from_text(text) {
                return Some((env, "result text"));
            }
        }
    }

    if let Some(content) = value
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_array())
    {
        for item in content.iter().rev() {
            if item.get("name").and_then(|name| name.as_str()) == Some("StructuredOutput") {
                if let Some(input) = item.get("input") {
                    if let Some((env, _)) = parse_candidate_envelope(input) {
                        return Some((env, "StructuredOutput tool input"));
                    }
                }
            }
            if let Some(text) = item.get("text").and_then(|text| text.as_str()) {
                if let Some(env) = parse_candidate_envelope_from_text(text) {
                    return Some((env, "assistant text"));
                }
            }
        }
    }

    if let Some(items) = value.as_array() {
        for item in items.iter().rev() {
            if let Some((env, source)) = parse_candidate_envelope(item) {
                return Some((env, source));
            }
        }
    }

    None
}

fn parse_candidate_envelope_from_text(text: &str) -> Option<CandidateEnvelope> {
    if let Ok(env) = serde_json::from_str::<CandidateEnvelope>(text) {
        return Some(env);
    }
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    serde_json::from_str::<CandidateEnvelope>(&text[start..=end]).ok()
}

fn value_kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

fn object_keys(value: &serde_json::Value) -> Vec<&String> {
    value
        .as_object()
        .map(|object| object.keys().collect::<Vec<_>>())
        .unwrap_or_default()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    #[tokio::test]
    async fn provider_e2e_parses_fake_claude_json_output() {
        let dir = test_dir("claude-ok");
        let cli = write_fake_cli(
            &dir,
            "claude-ok",
            r#"#!/usr/bin/env bash
printf '%s\n' '{"structured_output":{"candidates":[{"text":"好的，我马上看","style_tags":["warm"],"risk_flags":["none"],"reason":"direct"}]}}'
"#,
        );

        let schema = serde_json::json!({"type": "object"});
        let envelope = ClaudeProvider::new()
            .with_binary(cli.to_str().expect("utf-8 path"))
            .with_timeout(2)
            .generate("帮我回复", &schema)
            .await
            .expect("fake Claude should return candidates");

        assert_eq!(envelope.candidates.len(), 1);
        assert_eq!(envelope.candidates[0].text, "好的，我马上看");

        let _ = fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn provider_e2e_times_out_fake_claude_without_hanging() {
        let dir = test_dir("claude-timeout");
        let cli = write_fake_cli(
            &dir,
            "claude-timeout",
            r#"#!/usr/bin/env bash
sleep 5
printf '%s\n' '{"structured_output":{"candidates":[]}}'
"#,
        );

        let schema = serde_json::json!({"type": "object"});
        let started = Instant::now();
        let err = ClaudeProvider::new()
            .with_binary(cli.to_str().expect("utf-8 path"))
            .with_timeout(1)
            .generate("帮我回复", &schema)
            .await
            .expect_err("fake Claude should time out");

        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(err.to_string().contains("timed out"));
        assert!(err.to_string().contains("process was terminated"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn parses_claude_code_json_event_stream_structured_output() {
        let raw = serde_json::json!([
            {
                "type": "system",
                "subtype": "init",
                "session_id": "test"
            },
            {
                "type": "assistant",
                "message": {
                    "content": [
                        {
                            "type": "tool_use",
                            "name": "StructuredOutput",
                            "input": {
                                "candidates": [
                                    {
                                        "text": "最近有吃到什么好吃的吗，推荐一下～",
                                        "style_tags": ["稳妥"],
                                        "risk_flags": ["none"],
                                        "reason": "低压开启话题"
                                    }
                                ],
                                "action_card": {
                                    "action_type": "light_follow_up",
                                    "reason": "自然延续",
                                    "confidence": 0.88
                                },
                                "memory_candidates": [],
                                "reminder_candidates": [],
                                "context_summary": {
                                    "source_kind": "manual",
                                    "source_ref": "主动找齐齐开启话题",
                                    "summary": "主动找话题"
                                }
                            }
                        }
                    ]
                }
            },
            {
                "type": "result",
                "subtype": "success",
                "result": "human readable markdown",
                "structured_output": {
                    "candidates": [
                        {
                            "text": "今天过得怎么样，有没有好好吃饭～",
                            "style_tags": ["温柔"],
                            "risk_flags": ["none"],
                            "reason": "轻关心"
                        }
                    ],
                    "action_card": {
                        "action_type": "light_follow_up",
                        "reason": "自然延续",
                        "confidence": 0.88
                    },
                    "memory_candidates": [],
                    "reminder_candidates": [],
                    "context_summary": {
                        "source_kind": "manual",
                        "source_ref": "主动找齐齐开启话题",
                        "summary": "主动找话题"
                    }
                }
            }
        ]);
        let envelope = parse_json_output(&raw.to_string()).expect("event stream should parse");

        assert_eq!(envelope.candidates.len(), 1);
        assert_eq!(
            envelope.candidates[0].text,
            "今天过得怎么样，有没有好好吃饭～"
        );
        assert_eq!(envelope.action_card.action_type, "light_follow_up");
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
