//! Long-lived EQ subsystem. Owns the config-dir watcher process-wide;
//! the active device is bound on each (re)connect via `bind_device`.
//! Per-tick methods load disk changes into the device's properties and
//! sync the active preset to the headset on connection transitions.

use crate::devices::{Device, DeviceEvent};
use crate::eq::presets;
use crate::eq::presets::ConfigWatcher;

pub struct EqSession {
    watcher: ConfigWatcher,
    was_connected: bool,
    active: bool,
}

impl EqSession {
    /// Set up the config-dir watcher. Returns `None` only when the
    /// watcher cannot be set up; device capability is checked later
    /// in `bind_device`.
    pub fn new() -> Option<Self> {
        let watcher = match ConfigWatcher::new() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Warning: failed to watch EQ config directory: {e}");
                return None;
            }
        };
        Some(Self {
            watcher,
            was_connected: false,
            active: false,
        })
    }

    /// Bind a freshly (re)connected device. Checks EQ capability, seeds
    /// the device's properties from disk, drains any watcher events
    /// queued during the disconnect, and resets the connection-tracking
    /// flag so the next `sync_if_reconnected` call pushes the active
    /// preset to the headset.
    pub fn bind_device(&mut self, device: &mut dyn Device) {
        self.active = device.get_device_state().device_properties.can_set_equalizer;
        if !self.active {
            return;
        }
        Self::load_props_from_disk(device);
        self.watcher.take_pending();
        self.was_connected = false;
    }

    /// Load EQ info from disk if the watcher has signalled a change
    /// since the last call. No-op when nothing is pending or no
    /// EQ-capable device is bound.
    pub fn load_if_config_changed(&self, device: &mut dyn Device) {
        if !self.active {
            return;
        }
        if !self.watcher.take_pending() {
            return;
        }
        Self::load_props_from_disk(device);
    }

    /// Sync the on-disk profile to the headset when it just transitioned
    /// to connected and is unsynced. Updates internal connection-state
    /// tracking. On sync failure, leaves the tracking flag false so the
    /// next call retries.
    pub fn sync_if_reconnected(&mut self, device: &mut dyn Device) {
        if !self.active {
            return;
        }
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

    /// Load the on-disk EQ info (preset list, active preset, sync flag)
    /// into the device's properties. Used by `bind_device` for initial
    /// seeding and by `load_if_config_changed` after the watcher fires.
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
