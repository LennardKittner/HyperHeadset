/// State snapshot sent to the popup for rendering.
#[derive(Clone, Debug)]
pub struct PopupState {
    pub presets: Vec<String>,
    pub active_preset: Option<String>,
    pub synced: bool,
    pub is_connected: bool,
}

/// Commands from the tray/main thread to the popup window.
pub enum PopupCommand {
    /// Show the popup at the given screen coordinates (or toggle if already visible).
    Show { x: i32, y: i32, state: PopupState },
    /// Hide the popup (e.g. on headset disconnect).
    Hide,
    /// Update the popup's state without changing visibility.
    UpdateState(PopupState),
}

/// Trait for controlling the EQ popup from any thread.
/// The implementation must be Send + Sync so it can live inside StatusTray (ksni thread).
pub trait EqPopupController: Send + Sync {
    fn send(&self, cmd: PopupCommand);
}
