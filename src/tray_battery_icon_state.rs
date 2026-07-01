use hyper_headset::devices::{ChargingStatus, DeviceProperties};

#[cfg(target_os = "linux")]
use freedesktop_icons::lookup;

#[cfg(target_os = "linux")]
const HEADSET_MONOCHROME: &str = "audio-headset-symbolic";
#[cfg(target_os = "linux")]
const HEADSET: &str = "audio-headset";
#[cfg(target_os = "linux")]
const HEADSET_FALLBACK: &str = "headset";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrayBatteryIconState {
    NoDevice,
    Disconnected,
    ConnectedUnknown,
    Connected { percent: u8, charging: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg(target_os = "windows")]
pub struct WindowsIconKey {
    pub percent: u8,
    pub charging: bool,
}

impl TrayBatteryIconState {
    pub fn from_device_properties(device_properties: Option<&DeviceProperties>) -> Self {
        let Some(device_properties) = device_properties else {
            return Self::NoDevice;
        };
        if !device_properties.connected.unwrap_or(false) {
            return Self::Disconnected;
        }
        let charging = matches!(
            device_properties.charging,
            Some(ChargingStatus::Charging | ChargingStatus::FullyCharged)
        );
        let Some(percent) = device_properties.battery_level else {
            return Self::ConnectedUnknown;
        };
        Self::Connected {
            percent: percent.min(100),
            charging,
        }
    }

    #[cfg(target_os = "windows")]
    pub fn windows_icon_key(self) -> Option<WindowsIconKey> {
        match self {
            Self::Connected { percent, charging } => Some(WindowsIconKey { percent, charging }),
            _ => None,
        }
    }

    #[cfg(target_os = "linux")]
    pub fn linux_icon_name(self, monochrome: bool, theme_name: Option<&String>) -> String {
        let if_icon_exists = |name: &str, fallback: &dyn Fn() -> String| {
            if let Some(theme_name) = theme_name {
                if lookup(name)
                    .with_theme(theme_name)
                    .with_cache()
                    .find()
                    .is_some()
                {
                    name.to_string()
                } else {
                    fallback()
                }
            } else {
                if lookup(name).with_cache().find().is_some() {
                    name.to_string()
                } else {
                    fallback()
                }
            }
        };
        let default_icon = &|| if_icon_exists(HEADSET, &|| HEADSET_FALLBACK.to_string());
        match self {
            Self::NoDevice | Self::Disconnected | Self::ConnectedUnknown => {
                if monochrome {
                    if_icon_exists(HEADSET_MONOCHROME, default_icon)
                } else {
                    default_icon()
                }
            }
            Self::Connected { percent, charging } => {
                let precise_icon = format!(
                    "battery-{:0>3}{}{}",
                    (percent / 10) * 10,
                    if charging { "-charging" } else { "" },
                    if monochrome { "-symbolic" } else { "" },
                );

                let modifier = match percent {
                    0..10 => "caution",
                    10..30 => "low",
                    30..70 => "medium",
                    70..95 => "good",
                    95.. => "full",
                };

                let imprecise_icon = format!(
                    "battery-{modifier}{}{}",
                    if charging { "-charging" } else { "" },
                    if monochrome { "-symbolic" } else { "" },
                );

                if_icon_exists(&precise_icon, &|| {
                    if_icon_exists(&imprecise_icon, default_icon)
                })
            }
        }
    }
}
