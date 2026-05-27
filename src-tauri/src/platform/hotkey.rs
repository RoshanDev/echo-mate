use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use std::sync::Mutex;

const DEFAULT_HOTKEY: &str = "CmdOrCtrl+Shift+Space";

pub struct HotkeyManager {
    registered: Mutex<Option<Shortcut>>,
}

impl HotkeyManager {
    pub fn new() -> Self {
        Self { registered: Mutex::new(None) }
    }

    pub fn register<F>(&self, app: &AppHandle, hotkey_str: &str, on_trigger: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        let shortcut: Shortcut = hotkey_str.parse().unwrap_or_else(|_| {
            tracing::warn!("Invalid hotkey '{}', using default", hotkey_str);
            DEFAULT_HOTKEY.parse().unwrap()
        });

        app.global_shortcut().on_shortcut(shortcut.clone(), move |_app, _sc, event| {
            if event.state == ShortcutState::Pressed {
                tracing::info!("Hotkey triggered: {:?}", _sc);
                on_trigger();
            }
        }).ok();

        let mut registered = self.registered.lock().unwrap();
        if let Some(old) = registered.take() {
            app.global_shortcut().unregister(old).ok();
        }
        *registered = Some(shortcut);
        tracing::info!("Hotkey registered: {}", hotkey_str);
    }

    pub fn unregister_all(&self, app: &AppHandle) {
        let mut registered = self.registered.lock().unwrap();
        if let Some(sc) = registered.take() {
            app.global_shortcut().unregister(sc).ok();
        }
    }
}
