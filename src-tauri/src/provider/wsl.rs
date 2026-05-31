//! WSL bridge — on Windows, wrap CLI calls through `wsl.exe` so that
//! `claude` and `codex` installed inside WSL2 can be invoked natively.

use std::path::{Path, PathBuf};
use tokio::process::Command;

const WSL_EXE: &str = r"C:\Windows\System32\wsl.exe";

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn is_windows() -> bool {
    std::env::consts::OS == "windows"
}

/// Full WSL2 path for a binary name.
pub fn wsl_binary_path(binary: &str) -> String {
    match binary {
        "claude" => "/home/roshan/.local/bin/claude".to_string(),
        "codex" => "/home/roshan/.bun/bin/codex".to_string(),
        other => other.to_string(),
    }
}

/// Create a new Command starting with `wsl.exe`.
pub fn new_wsl_command() -> Command {
    let mut cmd = Command::new(WSL_EXE);
    hide_console_window(&mut cmd);
    cmd
}

/// Build a Command that runs the given binary.
/// On Windows, routes through `wsl.exe -e <wsl-path>`.
pub fn wsl_command(binary: &str) -> Command {
    wsl_command_for_platform(binary, is_windows())
}

fn wsl_command_for_platform(binary: &str, windows: bool) -> Command {
    if windows {
        direct_wsl_command(binary)
    } else {
        Command::new(binary)
    }
}

fn direct_wsl_command(binary: &str) -> Command {
    let mut cmd = new_wsl_command();
    cmd.arg("-e");
    cmd.arg(wsl_binary_path(binary));
    cmd
}

#[cfg(windows)]
fn hide_console_window(cmd: &mut Command) {
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console_window(_cmd: &mut Command) {}

/// Convert a Windows path like `C:\Users\foo\bar` to a WSL path like
/// `/mnt/c/Users/foo/bar`. On non-Windows this is a no-op.
pub fn to_wsl_path(path: &Path) -> PathBuf {
    if !is_windows() {
        return path.to_path_buf();
    }
    let s = path.to_string_lossy();
    if s.len() < 2 {
        return path.to_path_buf();
    }
    let bytes = s.as_bytes();
    if bytes[1] == b':' {
        let drive = (bytes[0] as char).to_lowercase().to_string();
        let rest = &s[2..].replace('\\', "/");
        PathBuf::from(format!("/mnt/{}{}", drive, rest))
    } else {
        path.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_command_routes_binary_through_wsl_exec() {
        let cmd = wsl_command_for_platform("claude", true);
        let args = cmd
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(cmd.as_std().get_program().to_string_lossy(), WSL_EXE);
        assert_eq!(args, vec!["-e", "/home/roshan/.local/bin/claude"]);
    }

    #[test]
    fn non_windows_command_runs_binary_directly() {
        let cmd = wsl_command_for_platform("claude", false);
        let args = cmd.as_std().get_args().collect::<Vec<_>>();

        assert_eq!(cmd.as_std().get_program().to_string_lossy(), "claude");
        assert!(args.is_empty());
    }
}
