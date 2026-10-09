use std::path::PathBuf;
use std::time::Duration;

#[cfg(feature = "eq-support")]
use serde::{Deserialize, Serialize};

use crate::devices::{DeviceEvent, Headset};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "eq-support", derive(Serialize, Deserialize))]
pub struct DeviceSettings {
    #[cfg_attr(
        feature = "eq-support",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub side_tone: Option<bool>,
    #[cfg_attr(
        feature = "eq-support",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub voice_prompt: Option<bool>,
    #[cfg_attr(
        feature = "eq-support",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub automatic_shutdown_minutes: Option<u64>,
    #[cfg_attr(
        feature = "eq-support",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub last_battery_level: Option<u8>,
    #[cfg_attr(
        feature = "eq-support",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub muted: Option<bool>,
}

pub fn settings_path() -> PathBuf {
    #[cfg(feature = "eq-support")]
    {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("hyper_headset")
            .join("headset_settings.json")
    }
    #[cfg(not(feature = "eq-support"))]
    {
        PathBuf::from(".")
    }
}

pub fn load_device_settings() -> DeviceSettings {
    #[cfg(feature = "eq-support")]
    {
        let path = settings_path();
        if !path.exists() {
            return DeviceSettings::default();
        }
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default()
    }
    #[cfg(not(feature = "eq-support"))]
    {
        DeviceSettings::default()
    }
}

pub fn save_device_settings(settings: &DeviceSettings) -> std::io::Result<()> {
    #[cfg(feature = "eq-support")]
    {
        let path = settings_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(settings)?;
        std::fs::write(path, data)
    }
    #[cfg(not(feature = "eq-support"))]
    {
        let _ = settings;
        Ok(())
    }
}

pub fn update_setting(update: impl FnOnce(&mut DeviceSettings)) {
    let mut s = load_device_settings();
    update(&mut s);
    let _ = save_device_settings(&s);
}

/// For headsets that do not support reading state over their wireless connection
/// (such as the Cloud III S 2.4 GHz USB dongle), sync saved user preferences
/// (voice prompt, auto-shutdown) to the headset upon connect.
///
/// NOTE: We deliberately do NOT sync sidetone here because sending a sidetone
/// command causes the headset firmware to audibly announce "side tone activated"
/// or "side tone deactivated" every time the application starts or reconnects.
/// Instead, the last known sidetone state is restored into `DeviceProperties`
/// in `init_capabilities()`.
pub fn sync_headset_settings_if_needed(device: &mut Headset) {
    let settings = load_device_settings();
    if let Some(dev) = device.hid_mut() {
        if dev.get_voice_prompt_packet().is_none() && dev.can_set_voice_prompt() {
            if let Some(vp) = settings.voice_prompt {
                let _ = dev.try_apply(DeviceEvent::VoicePrompt(vp));
            }
        }
        if dev.get_automatic_shut_down_packet().is_none() && dev.can_set_automatic_shutdown() {
            if let Some(mins) = settings.automatic_shutdown_minutes {
                let _ = dev.try_apply(DeviceEvent::AutomaticShutdownAfter(Duration::from_secs(
                    mins * 60,
                )));
            }
        }
    }
}
