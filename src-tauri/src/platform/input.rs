// Input simulation — reserved for future use (auto Ctrl/Cmd+C)
// MVP uses clipboard-first: user manually copies, we read clipboard.

/// Placeholder for input simulation (requires macOS Accessibility permission)
pub struct InputSimulator;

impl InputSimulator {
    pub fn new() -> Self {
        Self
    }

    /// Simulate Ctrl/Cmd+C to copy current selection
    /// NOTE: Requires Accessibility permission on macOS.
    /// NOT used in MVP — user manually copies before pressing hotkey.
    #[allow(dead_code)]
    pub fn copy_selection(&self) -> Result<(), String> {
        Err("Input simulation not enabled in MVP".into())
    }
}
