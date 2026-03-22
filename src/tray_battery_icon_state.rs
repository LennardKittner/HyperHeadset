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
    pub fn linux_icon_name(self) -> &'static str {
        match self {
            Self::NoDevice | Self::Disconnected | Self::ConnectedUnknown => "audio-headset",
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
                    match level_name {
                        "battery-caution" => "battery-caution-charging",
                        "battery-low" => "battery-low-charging",
                        "battery-medium" => "battery-medium-charging",
                        "battery-good" => "battery-good-charging",
                        _ => "battery-full-charging",
                    }
                } else {
                    level_name
                }
            }
        }
    }
}
