pub mod cloud_ii_wireless;
pub mod cloud_ii_wireless_dts;

use crate::devices::{
    cloud_ii_wireless::CloudIIWireless, cloud_ii_wireless_dts::CloudIIWirelessDTS,
};
use hidapi::{HidApi, HidDevice, HidError};
use std::{fmt::Display, time::Duration};
use thistermination::TerminationFull;

// Possible vendor IDs [HyperX, HP]
const VENDOR_IDS: [u16; 2] = [0x0951, 0x03F0];
// Possible Cloud II Wireless product IDs
const PRODUCT_IDS: [u16; 5] = [0x1718, 0x018B, 0x0D93, 0x0696, 0x0b92];

const RESPONSE_BUFFER_SIZE: usize = 256;

pub fn connect_compatible_device() -> Result<Box<dyn Device>, DeviceError> {
    let state = DeviceState::new(&PRODUCT_IDS, &VENDOR_IDS)?;
    let name = state
        .hid_device
        .get_product_string()?
        .ok_or(DeviceError::NoDeviceFound())?;
    println!("Connecting to {}", name);
    match (state.vendor_id, state.product_id) {
        (v, p)
            if cloud_ii_wireless::VENDOR_IDS.contains(&v)
                && cloud_ii_wireless::PRODUCT_IDS.contains(&p) =>
        {
            Ok(Box::new(CloudIIWireless::new_from_state(state)))
        }
        (v, p)
            if cloud_ii_wireless_dts::VENDOR_IDS.contains(&v)
                && cloud_ii_wireless_dts::PRODUCT_IDS.contains(&p) =>
        {
            Ok(Box::new(CloudIIWirelessDTS::new_from_state(state)))
        }
        (_, _) => Err(DeviceError::NoDeviceFound()),
    }
}

#[derive(Debug)]
pub struct DeviceState {
    pub hid_device: HidDevice,
    pub product_id: u16,
    pub vendor_id: u16,
    pub device_name: Option<String>,
    pub battery_level: Option<u8>,
    pub charging: Option<ChargingStatus>,
    pub muted: Option<bool>,
    pub mic_connected: Option<bool>,
    pub automatic_shutdown_after: Option<Duration>,
    pub pairing_info: Option<u8>,
    pub product_color: Option<Color>,
    pub side_tone_on: Option<bool>,
    pub side_tone_volume: Option<u8>,
    pub surround_sound: Option<bool>,
    pub voice_prompt_on: Option<bool>,
    pub connected: Option<bool>,
}

impl Display for DeviceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let unknown = "Unknown".to_string();
        write!(
            f,
            "Name:                     {}
Battery level:            {}
Charging status:          {}
Muted:                    {}
Mic connected:            {}
Automatic shutdown after: {}
Pairing info:             {}
Product color:            {}
Side tone on:             {}
Side tone volume:         {}
Surround sound:           {}
Voice prompt on:          {}
Connected:                {}",
            self.device_name.clone().unwrap_or("Unknown".to_string()),
            self.battery_level
                .map_or(unknown.clone(), |l| format!("{l}%")),
            self.charging.map_or(unknown.clone(), |c| c.to_string()),
            self.muted.map_or(unknown.clone(), |m| m.to_string()),
            self.mic_connected
                .map_or(unknown.clone(), |m| m.to_string()),
            self.automatic_shutdown_after
                .map_or(unknown.clone(), |a| format!("{} min", a.as_secs() / 60)),
            self.pairing_info.map_or(unknown.clone(), |p| p.to_string()),
            self.product_color
                .map_or(unknown.clone(), |c| c.to_string()),
            self.side_tone_on.map_or(unknown.clone(), |s| s.to_string()),
            self.side_tone_volume
                .map_or(unknown.clone(), |s| s.to_string()),
            self.surround_sound
                .map_or(unknown.clone(), |s| s.to_string()),
            self.voice_prompt_on
                .map_or(unknown.clone(), |v| v.to_string()),
            self.connected.map_or(unknown.clone(), |c| c.to_string()),
        )
    }
}

impl DeviceState {
    pub fn new(product_ids: &[u16], vendor_ids: &[u16]) -> Result<Self, DeviceError> {
        let hid_api = HidApi::new()?;
        let (hid_device, product_id, vendor_id) = hid_api
            .device_list()
            .find_map(|info| {
                if product_ids.contains(&info.product_id())
                    && vendor_ids.contains(&info.vendor_id())
                {
                    Some((
                        hid_api.open(info.vendor_id(), info.product_id()),
                        info.product_id(),
                        info.vendor_id(),
                    ))
                } else {
                    None
                }
            })
            .ok_or(DeviceError::NoDeviceFound())?;
        let hid_device = hid_device?;
        let device_name = hid_device.get_product_string()?;
        Ok(DeviceState {
            hid_device,
            product_id,
            vendor_id,
            device_name,
            charging: None,
            battery_level: None,
            muted: None,
            surround_sound: None,
            mic_connected: None,
            automatic_shutdown_after: None,
            pairing_info: None,
            product_color: None,
            side_tone_on: None,
            side_tone_volume: None,
            voice_prompt_on: None,
            connected: None,
        })
    }

    pub fn to_string_no_padding(&self) -> String {
        let unknown = "Unknown".to_string();
        format!(
            "Battery level: {}
Charging status: {}
Muted: {}
Mic connected: {}
Automatic shutdown after: {}
Pairing info: {}
Product color: {}
Side tone on: {}
Side tone volume: {}
Surround sound: {}
Voice prompt on: {}
Connected: {}",
            self.battery_level
                .map_or(unknown.clone(), |l| format!("{l}%")),
            self.charging.map_or(unknown.clone(), |c| c.to_string()),
            self.muted.map_or(unknown.clone(), |m| m.to_string()),
            self.mic_connected
                .map_or(unknown.clone(), |m| m.to_string()),
            self.automatic_shutdown_after
                .map_or(unknown.clone(), |a| format!("{} min", a.as_secs() / 60)),
            self.pairing_info.map_or(unknown.clone(), |p| p.to_string()),
            self.product_color
                .map_or(unknown.clone(), |c| c.to_string()),
            self.side_tone_on.map_or(unknown.clone(), |s| s.to_string()),
            self.side_tone_volume
                .map_or(unknown.clone(), |s| s.to_string()),
            self.surround_sound
                .map_or(unknown.clone(), |s| s.to_string()),
            self.voice_prompt_on
                .map_or(unknown.clone(), |v| v.to_string()),
            self.connected.map_or(unknown.clone(), |c| c.to_string()),
        )
    }

    fn update_self_with_event(&mut self, event: &DeviceEvent) {
        match event {
            DeviceEvent::BatterLevel(level) => self.battery_level = Some(*level),
            DeviceEvent::Charging(status) => self.charging = Some(*status),
            DeviceEvent::Muted(status) => self.muted = Some(*status),
            DeviceEvent::MicConnected(status) => self.mic_connected = Some(*status),
            DeviceEvent::AutomaticShutdownAfter(duration) => {
                self.automatic_shutdown_after = Some(*duration)
            }
            DeviceEvent::PairingInfo(info) => self.pairing_info = Some(*info),
            DeviceEvent::ProductColor(color) => self.product_color = Some(*color),
            DeviceEvent::SideToneOn(side) => self.side_tone_on = Some(*side),
            DeviceEvent::SideToneVolume(volume) => self.side_tone_volume = Some(*volume),
            DeviceEvent::SurroundSound(status) => self.surround_sound = Some(*status),
            DeviceEvent::VoicePrompt(on) => self.voice_prompt_on = Some(*on),
            DeviceEvent::WirelessConnected(connected) => self.connected = Some(*connected),
        };
    }

    pub fn clear_state(&mut self) {
        self.charging = None;
        self.battery_level = None;
        self.muted = None;
        self.surround_sound = None;
        self.mic_connected = None;
        self.automatic_shutdown_after = None;
        self.pairing_info = None;
        self.product_color = None;
        self.side_tone_on = None;
        self.side_tone_volume = None;
        self.voice_prompt_on = None;
        self.connected = None;
    }
}

#[derive(TerminationFull)]
pub enum DeviceError {
    #[termination(msg("{0:?}"))]
    HidError(#[from] HidError),
    #[termination(msg("No device found."))]
    NoDeviceFound(),
    #[termination(msg("No response. Is the headset turned on?"))]
    HeadSetOff(),
    #[termination(msg("No response."))]
    NoResponse(),
    #[termination(msg("Unknown response: {0:?} with length: {1:?}"))]
    UnknownResponse([u8; 8], usize),
}

#[derive(Debug, Copy, Clone)]
pub enum DeviceEvent {
    BatterLevel(u8),
    Muted(bool),
    MicConnected(bool),
    Charging(ChargingStatus),
    AutomaticShutdownAfter(Duration),
    PairingInfo(u8),
    ProductColor(Color),
    SideToneOn(bool),
    SideToneVolume(u8),
    VoicePrompt(bool),
    WirelessConnected(bool),
    SurroundSound(bool),
}

#[derive(Debug, Copy, Clone)]
pub enum Color {
    Red,
    UnknownColor(u8),
}

impl Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Color::Red => "Red".to_string(),
                Color::UnknownColor(n) => format!("Unknown color {}", n),
            }
        )
    }
}

impl From<u8> for Color {
    fn from(color: u8) -> Self {
        match color {
            0 => Color::Red,
            _ => Color::UnknownColor(color),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum ChargingStatus {
    NotCharging,
    Charging,
    FullyCharged,
    ChargeError,
}

impl Display for ChargingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ChargingStatus::NotCharging => "Not charging",
                ChargingStatus::Charging => "Charging",
                ChargingStatus::FullyCharged => "Fully charged",
                ChargingStatus::ChargeError => "Charging error!",
            }
        )
    }
}

impl From<u8> for ChargingStatus {
    fn from(value: u8) -> ChargingStatus {
        match value {
            0 => ChargingStatus::NotCharging,
            1 => ChargingStatus::Charging,
            2 => ChargingStatus::FullyCharged,
            _ => ChargingStatus::ChargeError,
        }
    }
}

pub trait Device {
    fn get_charging_packet(&self) -> Option<Vec<u8>>;
    fn get_battery_packet(&self) -> Option<Vec<u8>>;
    fn set_automatic_shut_down_packet(&self, shutdown_after: Duration) -> Option<Vec<u8>>;
    fn get_automatic_shut_down_packet(&self) -> Option<Vec<u8>>;
    fn get_mute_packet(&self) -> Option<Vec<u8>>;
    fn set_mute_packet(&self, mute: bool) -> Option<Vec<u8>>;
    fn get_surround_sound_packet(&self) -> Option<Vec<u8>>;
    fn set_surround_sound_packet(&self, surround_sound: bool) -> Option<Vec<u8>>;
    fn get_mic_connected_packet(&self) -> Option<Vec<u8>>;
    fn get_pairing_info_packet(&self) -> Option<Vec<u8>>;
    fn get_product_color_packet(&self) -> Option<Vec<u8>>;
    fn get_side_tone_packet(&self) -> Option<Vec<u8>>;
    fn set_side_tone_packet(&self, side_tone_on: bool) -> Option<Vec<u8>>;
    fn get_side_tone_volume_packet(&self) -> Option<Vec<u8>>;
    fn set_side_tone_volume_packet(&self, volume: u8) -> Option<Vec<u8>>;
    fn get_voice_prompt_packet(&self) -> Option<Vec<u8>>;
    fn set_voice_prompt_packet(&self, enable: bool) -> Option<Vec<u8>>;
    fn get_wireless_connected_status_packet(&self) -> Option<Vec<u8>>;
    fn get_event_from_device_response(&self, response: &[u8]) -> Option<Vec<DeviceEvent>>;
    fn get_device_state(&self) -> &DeviceState;
    fn get_device_state_mut(&mut self) -> &mut DeviceState;
    fn prepare_write(&mut self) {}
    fn wait_for_updates(&mut self, duration: Duration) -> Option<Vec<DeviceEvent>> {
        let mut buf = [0u8; RESPONSE_BUFFER_SIZE];
        let res = self
            .get_device_state()
            .hid_device
            .read_timeout(&mut buf[..], duration.as_millis() as i32)
            .ok()?;

        if res == 0 {
            return None;
        }

        self.get_event_from_device_response(&buf)
    }

    /// Refreshes the state by querying all available information
    fn active_refresh_state(&mut self) -> Result<(), DeviceError> {
        let packets = vec![
            self.get_wireless_connected_status_packet(),
            self.get_charging_packet(),
            self.get_battery_packet(),
            self.get_automatic_shut_down_packet(),
            self.get_mute_packet(),
            self.get_surround_sound_packet(),
            self.get_mic_connected_packet(),
            self.get_pairing_info_packet(),
            self.get_product_color_packet(),
            self.get_side_tone_packet(),
            self.get_side_tone_volume_packet(),
            self.get_voice_prompt_packet(),
        ];

        let mut responded = false;
        for packet in packets.into_iter().flatten() {
            self.prepare_write();
            println!("writing Packet {packet:?}");
            self.get_device_state().hid_device.write(&packet)?;
            if let Some(events) = self.wait_for_updates(Duration::from_secs(1)) {
                for event in events {
                    self.get_device_state_mut().update_self_with_event(&event);
                }
                responded = true;
            }
            if !self.get_device_state().connected.map_or(true, |c| c) {
                break;
            }
            std::thread::sleep(Duration::from_secs_f64(2.));
            println!("waiting 2s before sending the next packet ...");
        }

        if responded {
            Ok(())
        } else {
            Err(DeviceError::NoResponse())
        }
    }

    /// Refreshes the state by listening for events
    /// Only the battery level is actively queried because it is not communicated by the device on its own
    fn passive_refresh_state(&mut self) -> Result<(), DeviceError> {
        if let Some(events) = self.wait_for_updates(Duration::from_secs(1)) {
            for event in events {
                self.get_device_state_mut().update_self_with_event(&event);
            }
        }
        if let Some(batter_packet) = self.get_battery_packet() {
            self.get_device_state().hid_device.write(&batter_packet)?;
            if let Some(events) = self.wait_for_updates(Duration::from_secs(1)) {
                for event in events {
                    self.get_device_state_mut().update_self_with_event(&event);
                }
            }
        }

        Ok(())
    }
}
