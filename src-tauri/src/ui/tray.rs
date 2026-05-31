use crate::agent::orchestrator::OrchestratorState;
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let _tray = TrayIconBuilder::new()
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let has_notified_reminder = app
                        .try_state::<OrchestratorState>()
                        .and_then(|state| state.0.latest_notified_reminder().ok())
                        .flatten()
                        .is_some();
                    if has_notified_reminder {
                        let _ = window.eval("window.location.href = 'index.html#reminders'");
                    }
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    tracing::info!("System tray icon set up");
    Ok(())
}
