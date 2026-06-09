#[cfg(target_os = "windows")]
use enigo::{
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Settings,
};
#[cfg(target_os = "macos")]
use std::process::Command;
#[cfg(target_os = "windows")]
use std::time::Duration;

pub struct InputSimulator;

impl InputSimulator {
    pub fn new() -> Self {
        Self
    }

    /// Simulate Ctrl/Cmd+C to copy current selection
    /// NOTE: Requires Accessibility permission on macOS.
    #[cfg(target_os = "windows")]
    pub fn copy_selection(&self) -> Result<(), String> {
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| format!("Input simulation unavailable: {e}"))?;

        let modifier = copy_modifier();
        let _ = enigo.key(Key::Shift, Release);
        let _ = enigo.key(modifier, Release);
        std::thread::sleep(Duration::from_millis(25));

        enigo
            .key(modifier, Press)
            .map_err(|e| format!("Failed to press copy modifier: {e}"))?;

        let click_result = enigo
            .key(Key::Unicode('c'), Click)
            .map_err(|e| format!("Failed to press copy key: {e}"));
        let release_result = enigo
            .key(modifier, Release)
            .map_err(|e| format!("Failed to release copy modifier: {e}"));

        click_result?;
        release_result
    }

    /// Simulate Cmd+C through System Events on macOS.
    ///
    /// Enigo's macOS keyboard path can touch input-source APIs that must run on
    /// the main dispatch queue. Hotkey handling runs on a Tokio worker, so use
    /// AppleScript here to avoid crashing the process while preserving the same
    /// user-visible behavior.
    #[cfg(target_os = "macos")]
    pub fn copy_selection(&self) -> Result<(), String> {
        let output = Command::new("osascript")
            .arg("-e")
            .arg(r#"tell application "System Events" to keystroke "c" using command down"#)
            .output()
            .map_err(|e| format!("Input simulation unavailable: {e}"))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("Failed to press copy shortcut: {}", stderr.trim()))
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    pub fn copy_selection(&self) -> Result<(), String> {
        Err("Auto-copy selection is only supported on Windows and macOS".into())
    }
}

#[cfg(target_os = "windows")]
fn copy_modifier() -> Key {
    Key::Control
}
