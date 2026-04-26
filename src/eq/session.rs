//! Per-connection EQ session: bridges the static EQ preset module to a live
//! `Device` instance. Owns the config-dir watcher and the connection-state
//! tracking; exposes initial seeding plus per-tick load and sync under a
//! single namespace.

use crate::devices::{Device, DeviceEvent};
use crate::eq::presets;
use std::sync::mpsc;

pub struct EqSession {
    _watcher: notify::RecommendedWatcher,
    rx: mpsc::Receiver<()>,
    was_connected: bool,
}

impl EqSession {
    /// Set up an EQ session for a connected device: seed info from disk, start
    /// the config-dir watcher. Returns `None` when the device does not support
    /// EQ or the watcher cannot be installed.
    pub fn new(device: &mut dyn Device) -> Option<Self> {
        if !device.get_device_state().device_properties.can_set_equalizer {
            return None;
        }
        Self::load_props_from_disk(device);

        let (watcher, rx) = match presets::watch_config_dir() {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("Warning: failed to watch EQ config directory: {e}");
                return None;
            }
        };
        let was_connected = device.get_device_state().device_properties.connected == Some(true);
        Some(Self {
            _watcher: watcher,
            rx,
            was_connected,
        })
    }

    /// Load EQ info from disk if the watcher has signalled a change since
    /// the last call. No-op when nothing is pending.
    pub fn load_if_config_changed(&self, device: &mut dyn Device) {
        if self.rx.try_recv().is_err() {
            return;
        }
        while self.rx.try_recv().is_ok() {}
        Self::load_props_from_disk(device);
    }

    /// Sync the on-disk profile to the headset when it just transitioned to
    /// connected and is unsynced. Updates internal connection-state tracking.
    /// On sync failure, leaves the tracking flag false so the next call retries.
    pub fn sync_if_reconnected(&mut self, device: &mut dyn Device) {
        let is_connected = device.get_device_state().device_properties.connected == Some(true);
        let just_reconnected = is_connected && !self.was_connected;
        let synced_ok = if just_reconnected {
            self.try_sync_active_preset(device)
        } else {
            true
        };
        self.was_connected = is_connected && synced_ok;
    }

    fn try_sync_active_preset(&self, device: &mut dyn Device) -> bool {
        let profile = presets::load_selected_profile();
        let Some(ref name) = profile.active_preset else {
            return true;
        };
        if profile.synced {
            return true;
        }
        println!("Syncing EQ preset '{}' to headset...", name);
        match device.try_apply(DeviceEvent::EqualizerPreset(name.clone())) {
            Ok(()) => true,
            Err(e) => {
                eprintln!("Failed to sync EQ preset '{name}' on reconnect: {e}");
                false
            }
        }
    }

    /// Load the on-disk EQ info (preset list, active preset, sync flag) into
    /// the device's properties. Used by `new` for initial seeding and by
    /// `load_if_config_changed` after the watcher fires.
    fn load_props_from_disk(device: &mut dyn Device) {
        let profile = presets::load_selected_profile();
        let preset_names: Vec<String> =
            presets::all_presets().into_iter().map(|p| p.name).collect();
        let props = &mut device.get_device_state_mut().device_properties;
        props.active_eq_preset = profile.active_preset;
        props.eq_synced = Some(profile.synced);
        props.eq_preset_options = preset_names;
    }
}
