//! Glue between the EQ preset module and a running `Device` instance:
//! disk-load on connect, file-watcher on the config dir, auto-sync after
//! reconnect. Used by both the Linux and non-Linux tray main loops.

use crate::devices::{Device, DeviceEvent};
use crate::eq::presets;
use std::sync::mpsc;

pub type WatcherPair = (notify::RecommendedWatcher, mpsc::Receiver<()>);

/// Seed device EQ properties from disk and start watching the config dir.
/// Returns `None` when the device does not support EQ; otherwise returns the
/// watcher pair, which the caller must keep alive for events to flow.
pub fn init_device_eq_state(device: &mut dyn Device) -> Option<WatcherPair> {
    if !device.get_device_state().device_properties.can_set_equalizer {
        return None;
    }
    seed_eq_props_from_disk(device);

    match presets::watch_config_dir() {
        Ok(pair) => Some(pair),
        Err(e) => {
            eprintln!("Warning: failed to watch EQ config directory: {e}");
            None
        }
    }
}

/// Re-read the on-disk EQ state (preset list, active preset, sync flag) into
/// the device's properties. Call when the config-dir watcher fires so the tray
/// reflects external edits to `selected_profile.json` or the `eq_presets/`
/// directory. Note: per-band edits to the *content* of an existing preset are
/// not detected — only changes to the set of preset names and the
/// `selected_profile.json` contents propagate.
pub fn refresh_eq_state_from_disk(device: &mut dyn Device) {
    seed_eq_props_from_disk(device);
}

fn seed_eq_props_from_disk(device: &mut dyn Device) {
    let profile = presets::load_selected_profile();
    let preset_names: Vec<String> = presets::all_presets().into_iter().map(|p| p.name).collect();
    let props = &mut device.get_device_state_mut().device_properties;
    props.active_eq_preset = profile.active_preset;
    props.eq_synced = Some(profile.synced);
    props.eq_preset_options = preset_names;
}

/// Drain any pending watcher events. Returns true when at least one event was
/// consumed, signalling the caller should refresh the on-screen preset list.
pub fn drain_watcher(rx: &mpsc::Receiver<()>) -> bool {
    if rx.try_recv().is_err() {
        return false;
    }
    while rx.try_recv().is_ok() {}
    true
}

/// If the headset just transitioned to connected and the on-disk profile is
/// unsynced, push it to the device. Returns the new connected state so the
/// caller can update its tracking variable.
pub fn maybe_sync_on_reconnect(device: &mut dyn Device, was_connected: bool) -> bool {
    let is_connected = device.get_device_state().device_properties.connected == Some(true);
    if is_connected
        && !was_connected
        && device.get_device_state().device_properties.can_set_equalizer
    {
        let profile = presets::load_selected_profile();
        if !profile.synced {
            if let Some(ref name) = profile.active_preset {
                println!("Syncing EQ preset '{}' to headset...", name);
                let _ = device.try_apply(DeviceEvent::EqualizerPreset(name.clone()));
            }
        }
    }
    is_connected
}
