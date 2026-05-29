//! WSL bridge — on Windows, wrap CLI calls through `wsl.exe` so that
//! `claude` and `codex` installed inside WSL2 can be invoked natively.

use std::path::{Path, PathBuf};
use tokio::process::Command;

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
    Command::new(r"C:\Windows\System32\wsl.exe")
}

/// Build a Command that runs the given binary.
/// On Windows, routes through `wsl.exe -e <wsl-path>`.
pub fn wsl_command(binary: &str) -> Command {
    if is_windows() {
        let mut cmd = new_wsl_command();
        cmd.arg("-e");
        cmd.arg(wsl_binary_path(binary));
        cmd
    } else {
        Command::new(binary)
    }
}

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
