pub struct ScreenCapture;

impl ScreenCapture {
    pub fn new() -> Self {
        Self
    }

    /// Starts the native region screenshot UI when the platform supports it.
    pub fn start_region_capture(&self) -> Result<bool, String> {
        start_region_capture()
    }
}

#[cfg(windows)]
fn start_region_capture() -> Result<bool, String> {
    std::process::Command::new("explorer.exe")
        .arg("ms-screenclip:")
        .spawn()
        .map_err(|e| format!("无法打开 Windows 截图工具：{e}"))?;
    Ok(true)
}

#[cfg(not(windows))]
fn start_region_capture() -> Result<bool, String> {
    Ok(false)
}
