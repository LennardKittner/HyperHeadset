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

    /// Returns the first name that resolves to an actual icon file in the active theme.
    #[cfg(target_os = "linux")]
    fn first_existing_icon(names: &[String], theme_name: Option<&String>) -> Option<String> {
        names
            .iter()
            .find(|name| {
                match theme_name {
                    Some(theme_name) => lookup(name).with_theme(theme_name).with_cache().find(),
                    None => lookup(name).with_cache().find(),
                }
                .is_some()
            })
            .cloned()
    }

    #[cfg(target_os = "linux")]
    pub fn linux_icon_name(self, monochrome: bool, theme_name: Option<&String>) -> String {
        // Themes are inconsistent about which variants they ship, so always try both the
        // symbolic and the full-color name, ordered by the user's preference.
        let with_variants = |base: &str| {
            let symbolic = format!("{base}-symbolic");
            if monochrome {
                [symbolic, base.to_string()]
            } else {
                [base.to_string(), symbolic]
            }
        };

        let headset_icon = || {
            let candidates: Vec<String> = if monochrome {
                vec![HEADSET_MONOCHROME.to_string(), HEADSET.to_string()]
            } else {
                vec![HEADSET.to_string(), HEADSET_MONOCHROME.to_string()]
            };
            Self::first_existing_icon(&candidates, theme_name)
                .unwrap_or_else(|| HEADSET_FALLBACK.to_string())
        };

        match self {
            Self::NoDevice | Self::Disconnected | Self::ConnectedUnknown => headset_icon(),
            Self::Connected { percent, charging } => {
                let charge = if charging { "-charging" } else { "" };
                let level = (percent / 10) * 10;

                let modifier = match percent {
                    0..10 => "caution",
                    10..30 => "low",
                    30..70 => "medium",
                    70..95 => "good",
                    95.. => "full",
                };

                // Most precise first
                let candidates: Vec<String> = [
                    format!("battery-level-{level}{charge}"),
                    format!("battery-{level:0>3}{charge}"),
                    format!("battery-{modifier}{charge}"),
                ]
                .iter()
                .flat_map(|base| with_variants(base))
                .collect();

                Self::first_existing_icon(&candidates, theme_name).unwrap_or_else(headset_icon)
            }
        }
    }
}
