use hyper_headset::devices::{ChargingStatus, DeviceProperties};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrayBatteryIconState {
    NoDevice,
    Disconnected,
    ConnectedUnknown,
    Connected {
        percent: u8,
        charging: bool,
    },
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
    pub fn linux_icon_name(self, monochrome: bool) -> &'static str {
        match self {
            Self::NoDevice | Self::Disconnected | Self::ConnectedUnknown => {
                if monochrome {
                    "audio-headset-symbolic"
                } else {
                    "audio-headset"
                }
            }
            Self::Connected { percent, charging } => {
                let level_name = if percent <= 10 {
                    "battery-caution"
                } else if percent <= 30 {
                    "battery-low"
                } else if percent <= 60 {
                    "battery-medium"
                } else if percent <= 85 {
                    "battery-good"
                } else {
                    "battery-full"
                };
                if charging {
                    match (level_name, monochrome) {
                        ("battery-caution", false) => "battery-caution-charging",
                        ("battery-low", false) => "battery-low-charging",
                        ("battery-medium", false) => "battery-medium-charging",
                        ("battery-good", false) => "battery-good-charging",
                        (_, false) => "battery-full-charging",
                        ("battery-caution", true) => "battery-caution-charging-symbolic",
                        ("battery-low", true) => "battery-low-charging-symbolic",
                        ("battery-medium", true) => "battery-medium-charging-symbolic",
                        ("battery-good", true) => "battery-good-charging-symbolic",
                        (_, true) => "battery-full-charging-symbolic",
                    }
                } else {
                    match (level_name, monochrome) {
                        ("battery-caution", true) => "battery-caution-symbolic",
                        ("battery-low", true) => "battery-low-symbolic",
                        ("battery-medium", true) => "battery-medium-symbolic",
                        ("battery-good", true) => "battery-good-symbolic",
                        (name, false) => name,
                        (_, true) => "battery-full-symbolic",
                    }
                }
            }
        }
    }
}
