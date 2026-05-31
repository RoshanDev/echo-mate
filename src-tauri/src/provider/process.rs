use std::io;
use std::process::Output;
use std::time::Duration;

use anyhow::Context;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Child;
use tokio::task::JoinHandle;

pub async fn wait_with_timeout(
    mut child: Child,
    limit: Duration,
    label: &str,
) -> anyhow::Result<Output> {
    let mut stdout = Some(read_pipe(child.stdout.take()));
    let mut stderr = Some(read_pipe(child.stderr.take()));

    let status = tokio::select! {
        status = child.wait() => {
            status.with_context(|| format!("{label} process wait failed"))?
        }
        _ = tokio::time::sleep(limit) => {
            let kill_result = child.kill().await;
            stdout.take().expect("stdout reader").abort();
            stderr.take().expect("stderr reader").abort();

            match kill_result {
                Ok(()) => {
                    anyhow::bail!(
                        "{label} timed out after {}; process was terminated",
                        format_duration(limit),
                    );
                }
                Err(e) => {
                    anyhow::bail!(
                        "{label} timed out after {}; failed to terminate process: {e}",
                        format_duration(limit),
                    );
                }
            }
        }
    };

    Ok(Output {
        status,
        stdout: collect_pipe(stdout.take().expect("stdout reader"), label, "stdout").await?,
        stderr: collect_pipe(stderr.take().expect("stderr reader"), label, "stderr").await?,
    })
}

fn read_pipe<R>(mut pipe: Option<R>) -> JoinHandle<io::Result<Vec<u8>>>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut bytes = Vec::new();
        if let Some(pipe) = pipe.as_mut() {
            pipe.read_to_end(&mut bytes).await?;
        }
        Ok(bytes)
    })
}

async fn collect_pipe(
    task: JoinHandle<io::Result<Vec<u8>>>,
    label: &str,
    stream: &str,
) -> anyhow::Result<Vec<u8>> {
    task.await
        .with_context(|| format!("{label} {stream} reader task failed"))?
        .with_context(|| format!("{label} {stream} read failed"))
}

fn format_duration(duration: Duration) -> String {
    if duration.as_millis() % 1000 == 0 {
        format!("{}s", duration.as_secs())
    } else {
        format!("{}ms", duration.as_millis())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Stdio;

    #[tokio::test]
    async fn wait_with_timeout_returns_output_for_fast_process() {
        let child = test_command("printf ok", "[Console]::Out.Write('ok')")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn test process");

        let output = wait_with_timeout(child, Duration::from_secs(2), "test")
            .await
            .expect("process should finish");

        assert!(output.status.success());
        assert_eq!(output.stdout, b"ok");
    }

    #[tokio::test]
    async fn wait_with_timeout_terminates_slow_process() {
        let child = test_command("sleep 5", "Start-Sleep -Seconds 5")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn test process");

        let err = wait_with_timeout(child, Duration::from_millis(100), "test")
            .await
            .expect_err("process should time out");

        assert!(err.to_string().contains("timed out"));
        assert!(err.to_string().contains("process was terminated"));
    }

    #[cfg(windows)]
    fn test_command(_unix_script: &str, windows_script: &str) -> tokio::process::Command {
        let mut cmd = tokio::process::Command::new("powershell.exe");
        cmd.arg("-NoProfile").arg("-Command").arg(windows_script);
        cmd
    }

    #[cfg(not(windows))]
    fn test_command(unix_script: &str, _windows_script: &str) -> tokio::process::Command {
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg(unix_script);
        cmd
    }
}
