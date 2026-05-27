use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

pub struct ClipboardManager;

impl ClipboardManager {
    pub fn new() -> Self {
        Self
    }

    /// Read current clipboard text
    pub fn read_text(&self, app: &AppHandle) -> Result<String, String> {
        let text = app.clipboard().read_text()
            .map_err(|e| format!("Clipboard read error: {}", e))?;
        Ok(text.trim().to_string())
    }

    /// Write text to clipboard
    pub fn write_text(&self, app: &AppHandle, text: &str) -> Result<(), String> {
        app.clipboard().write_text(text.to_string())
            .map_err(|e| format!("Clipboard write error: {}", e))
    }
}
