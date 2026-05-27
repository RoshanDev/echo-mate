use tauri::{AppHandle, Manager};

pub struct WindowManager;

impl WindowManager {
    pub fn new() -> Self {
        Self
    }

    pub fn show_popup(&self, app: &AppHandle) {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }

    pub fn hide_popup(&self, app: &AppHandle) {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }
    }

    pub fn toggle_popup(&self, app: &AppHandle) {
        if let Some(window) = app.get_webview_window("main") {
            if window.is_visible().unwrap_or(false) {
                let _ = window.hide();
            } else {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    }

    pub fn is_visible(&self, app: &AppHandle) -> bool {
        app.get_webview_window("main")
            .map(|w| w.is_visible().unwrap_or(false))
            .unwrap_or(false)
    }
}
