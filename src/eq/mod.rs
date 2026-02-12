pub mod presets;

#[cfg(feature = "eq-editor")]
pub mod editor;

/// EQ band frequencies with labels and descriptions.
/// (frequency_hz, short_label, description)
pub const EQ_FREQUENCIES: [(u32, &str, &str); 10] = [
    (32, "32 Hz", "Sub-bass"),
    (64, "64 Hz", "Bass"),
    (125, "125 Hz", "Low-mid"),
    (250, "250 Hz", "Mid"),
    (500, "500 Hz", "Mid"),
    (1000, "1 kHz", "Upper-mid"),
    (2000, "2 kHz", "Presence"),
    (4000, "4 kHz", "Presence"),
    (8000, "8 kHz", "Brilliance"),
    (16000, "16 kHz", "Air"),
];

pub const NUM_BANDS: usize = 10;
pub const DB_MIN: f32 = -12.0;
pub const DB_MAX: f32 = 12.0;

/// Commands sent from the tray to the main loop.
pub enum TrayCommand {
    ApplyEqPreset(String),
}
