use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::mpsc;

use super::NUM_BANDS;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqPreset {
    pub name: String,
    pub bands: [f32; NUM_BANDS],
}

/// Stored in selected_profile.json — just the active preset name.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SelectedProfile {
    pub active_preset: Option<String>,
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

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hyper_headset")
}

fn presets_dir() -> PathBuf {
    config_dir().join("eq_presets")
}

fn selected_profile_path() -> PathBuf {
    config_dir().join("selected_profile.json")
}

/// Load a single preset by name. Checks user presets dir first, then builtins.
pub fn load_preset(name: &str) -> Option<EqPreset> {
    // Check user preset file first
    let path = presets_dir().join(format!("{}.json", name));
    if path.exists() {
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(preset) = serde_json::from_str::<EqPreset>(&data) {
                return Some(preset);
            }
        }
    }
    // Fall back to builtin
    builtin_presets().into_iter().find(|p| p.name == name)
}

/// Save a single preset to its own file.
pub fn save_preset(preset: &EqPreset) -> std::io::Result<()> {
    let dir = presets_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", preset.name));
    let data = serde_json::to_string_pretty(preset)?;
    std::fs::write(&path, data)
}

/// Delete a user preset file.
pub fn delete_preset(name: &str) -> std::io::Result<()> {
    let path = presets_dir().join(format!("{}.json", name));
    if path.exists() {
        std::fs::remove_file(&path)
    } else {
        Ok(())
    }
}

/// Load all user presets from individual files in the presets directory.
pub fn load_user_presets() -> Vec<EqPreset> {
    let dir = presets_dir();
    if !dir.exists() {
        return Vec::new();
    }
    let mut presets = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                if let Ok(data) = std::fs::read_to_string(&path) {
                    if let Ok(preset) = serde_json::from_str::<EqPreset>(&data) {
                        presets.push(preset);
                    }
                }
            }
        }
    }
    presets.sort_by(|a, b| a.name.cmp(&b.name));
    presets
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

pub fn load_selected_profile() -> SelectedProfile {
    let path = selected_profile_path();
    if !path.exists() {
        return SelectedProfile::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => SelectedProfile::default(),
    }
}

pub fn save_selected_profile(profile: &SelectedProfile) -> std::io::Result<()> {
    let path = selected_profile_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(profile)?;
    std::fs::write(&path, data)
}

/// Creates a file watcher on the config directory (recursive, includes eq_presets/).
/// Returns the watcher (must be kept alive) and a receiver that fires on file changes.
pub fn watch_config_dir() -> notify::Result<(notify::RecommendedWatcher, mpsc::Receiver<()>)> {
    use notify::{Event, EventKind, RecursiveMode, Watcher};

    let (tx, rx) = mpsc::channel();
    let config = config_dir();

    // Ensure the directories exist so we can watch them
    std::fs::create_dir_all(config.join("eq_presets")).ok();

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

    watcher.watch(&config, RecursiveMode::Recursive)?;

    Ok((watcher, rx))
}
