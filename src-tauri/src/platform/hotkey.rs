use std::sync::Mutex;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

const DEFAULT_HOTKEY: &str = "CmdOrCtrl+Shift+Space";

pub struct HotkeyManager {
    registered: Mutex<Option<Shortcut>>,
}

impl HotkeyManager {
    pub fn new() -> Self {
        Self {
            registered: Mutex::new(None),
        }
    }

    pub fn register<F>(&self, app: &AppHandle, hotkey_str: &str, on_trigger: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        let shortcut: Shortcut = hotkey_str.parse().unwrap_or_else(|e| {
            tracing::warn!("Invalid hotkey '{}': {e}, using default", hotkey_str);
            DEFAULT_HOTKEY.parse().unwrap()
        });

        // Unregister old shortcut FIRST to avoid id conflicts
        {
            let mut registered = self.registered.lock().unwrap();
            if let Some(old) = registered.take() {
                if let Err(e) = app.global_shortcut().unregister(old) {
                    tracing::warn!("Failed to unregister old hotkey: {e}");
                }
            }
        }

        // Register the new shortcut
        let hotkey_for_log = hotkey_str.to_string();
        match app
            .global_shortcut()
            .on_shortcut(shortcut, move |_app, _sc, event| {
                if event.state == ShortcutState::Released {
                    tracing::info!("Hotkey released, triggering: {hotkey_for_log}");
                    on_trigger();
                }
            }) {
            Ok(()) => {
                tracing::info!("Hotkey registered: {}", hotkey_str);
            }
            Err(e) => {
                tracing::error!("Failed to register hotkey '{}': {}", hotkey_str, e);
                return;
            }
        }

        // Store the registered shortcut for later unregistration
        if let Ok(current) = hotkey_str.parse() {
            let mut registered = self.registered.lock().unwrap();
            *registered = Some(current);
        }
    }

    pub fn unregister_all(&self, app: &AppHandle) {
        let mut registered = self.registered.lock().unwrap();
        if let Some(sc) = registered.take() {
            if let Err(e) = app.global_shortcut().unregister(sc) {
                tracing::warn!("Failed to unregister hotkey: {e}");
            }
        }
    }
}
