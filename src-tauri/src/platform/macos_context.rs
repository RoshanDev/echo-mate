use crate::domain::MacosContextSnapshot;

pub struct MacosContextHelper;

impl MacosContextHelper {
    pub fn collect(
        helper_enabled: bool,
        accessibility_enabled: bool,
        pasteboard_text: Option<String>,
    ) -> MacosContextSnapshot {
        collect_impl(helper_enabled, accessibility_enabled, pasteboard_text)
    }
}

#[cfg(target_os = "macos")]
fn collect_impl(
    helper_enabled: bool,
    accessibility_enabled: bool,
    pasteboard_text: Option<String>,
) -> MacosContextSnapshot {
    if !helper_enabled {
        return snapshot(
            false,
            false,
            accessibility_enabled,
            "",
            "",
            "",
            pasteboard_text,
            "macOS 上下文 helper 已关闭；继续使用复制、选中文本热键和截图。",
        );
    }

    let front_app = run_osascript(
        r#"tell application "System Events" to get name of first application process whose frontmost is true"#,
    )
    .unwrap_or_default();
    let window_title = run_osascript(
        r#"tell application "System Events"
  tell first application process whose frontmost is true
    if (count of windows) > 0 then
      return name of front window
    else
      return ""
    end if
  end tell
end tell"#,
    )
    .unwrap_or_default();
    let selected_text = if accessibility_enabled {
        run_osascript(
            r#"tell application "System Events"
  tell first application process whose frontmost is true
    try
      return value of attribute "AXSelectedText" of focused UI element
    on error
      return ""
    end try
  end tell
end tell"#,
        )
        .unwrap_or_default()
    } else {
        String::new()
    };

    let status = if accessibility_enabled {
        "macOS helper 已尝试读取前台应用、窗口标题、Pasteboard 与 Accessibility 选中文本；失败时降级到热键/截图。"
    } else {
        "macOS helper 已尝试读取前台应用、窗口标题和 Pasteboard；Accessibility 关闭时不读取选中文本。"
    };
    snapshot(
        true,
        true,
        accessibility_enabled,
        &front_app,
        &window_title,
        &selected_text,
        pasteboard_text,
        status,
    )
}

#[cfg(not(target_os = "macos"))]
fn collect_impl(
    helper_enabled: bool,
    accessibility_enabled: bool,
    pasteboard_text: Option<String>,
) -> MacosContextSnapshot {
    snapshot(
        false,
        helper_enabled,
        accessibility_enabled,
        "",
        "",
        "",
        pasteboard_text,
        "当前构建环境不是 macOS，macOS 近似 helper 不运行。",
    )
}

fn snapshot(
    available: bool,
    helper_enabled: bool,
    accessibility_enabled: bool,
    front_app: &str,
    window_title: &str,
    selected_text: &str,
    pasteboard_text: Option<String>,
    status: &str,
) -> MacosContextSnapshot {
    let pasteboard_text = pasteboard_text.unwrap_or_default();
    MacosContextSnapshot {
        platform: std::env::consts::OS.to_string(),
        available,
        helper_enabled,
        accessibility_enabled,
        front_app: trim_excerpt(front_app, 80),
        window_title: trim_excerpt(window_title, 120),
        selected_text_available: !selected_text.trim().is_empty(),
        selected_text_excerpt: trim_excerpt(selected_text, 120),
        pasteboard_available: !pasteboard_text.trim().is_empty(),
        pasteboard_excerpt: trim_excerpt(&pasteboard_text, 120),
        status: status.to_string(),
        fallback_path: "权限关闭或平台能力不可用时，继续使用复制、选中文本热键和截图生成。"
            .to_string(),
    }
}

fn trim_excerpt(value: &str, limit: usize) -> String {
    value.trim().chars().take(limit).collect()
}

#[cfg(target_os = "macos")]
fn run_osascript(script: &str) -> Option<String> {
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|text| text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_macos_snapshot_is_explicitly_unavailable() {
        let snapshot = MacosContextHelper::collect(true, true, Some("clipboard".to_string()));
        if std::env::consts::OS != "macos" {
            assert!(!snapshot.available);
            assert!(snapshot.status.contains("不是 macOS"));
            assert!(snapshot.pasteboard_available);
        }
    }
}
