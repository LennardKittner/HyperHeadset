use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::mpsc;

use super::NUM_BANDS;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqPreset {
    pub name: String,
    pub bands: [f32; NUM_BANDS],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqSettings {
    pub bands: [f32; NUM_BANDS],
    pub active_preset: Option<String>,
}

impl Default for EqSettings {
    fn default() -> Self {
        EqSettings {
            bands: [0.0; NUM_BANDS],
            active_preset: None,
        }
    }
}

pub fn builtin_presets() -> Vec<EqPreset> {
    vec![
        EqPreset {
            name: "Flat".into(),
            bands: [0.0; 10],
        },
        EqPreset {
            name: "Bass Boost".into(),
            bands: [6.0, 5.0, 3.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        },
        EqPreset {
            name: "Treble Boost".into(),
            bands: [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 4.0, 5.0, 6.0],
        },
        EqPreset {
            name: "V-Shape".into(),
            bands: [5.0, 4.0, 2.0, 0.0, -2.0, -2.0, 0.0, 2.0, 4.0, 5.0],
        },
        EqPreset {
            name: "Vocal".into(),
            bands: [-2.0, -1.0, 0.0, 2.0, 4.0, 4.0, 3.0, 1.0, 0.0, -1.0],
        },
    ]
}

fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hyper_headset")
}

fn presets_path() -> PathBuf {
    config_dir().join("eq_presets.json")
}

fn settings_path() -> PathBuf {
    config_dir().join("eq_settings.json")
}

pub fn load_user_presets() -> Vec<EqPreset> {
    let path = presets_path();
    if !path.exists() {
        return Vec::new();
    }
    match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

pub fn save_user_presets(presets: &[EqPreset]) -> std::io::Result<()> {
    let path = presets_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(presets)?;
    std::fs::write(&path, data)
}

/// Returns all presets: builtins + user presets.
/// User presets with matching names override builtins.
pub fn all_presets() -> Vec<EqPreset> {
    let mut presets = builtin_presets();
    let user = load_user_presets();
    for up in user {
        if let Some(existing) = presets.iter_mut().find(|p| p.name == up.name) {
            *existing = up;
        } else {
            presets.push(up);
        }
    }
    presets
}

pub fn is_builtin(name: &str) -> bool {
    builtin_presets().iter().any(|p| p.name == name)
}

pub fn load_settings() -> EqSettings {
    let path = settings_path();
    if !path.exists() {
        return EqSettings::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => EqSettings::default(),
    }
}

pub fn save_settings(settings: &EqSettings) -> std::io::Result<()> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(settings)?;
    std::fs::write(&path, data)
}

/// Creates a file watcher on the config directory.
/// Returns the watcher (must be kept alive) and a receiver that fires on preset/settings file changes.
pub fn watch_config_dir() -> notify::Result<(notify::RecommendedWatcher, mpsc::Receiver<()>)> {
    use notify::{Event, EventKind, RecursiveMode, Watcher};

    let (tx, rx) = mpsc::channel();
    let config = config_dir();

    // Ensure the directory exists so we can watch it
    std::fs::create_dir_all(&config).ok();

    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            match event.kind {
                EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                    let _ = tx.send(());
                }
                _ => {}
            }
        }
    })?;

    watcher.watch(&config, RecursiveMode::NonRecursive)?;

    Ok((watcher, rx))
}
