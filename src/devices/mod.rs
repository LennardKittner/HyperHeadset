pub mod cloud_ii_wireless;
pub mod cloud_ii_wireless_dts;
pub mod cloud_iii_s_wireless;
pub mod cloud_iii_wireless;

use crate::{
    debug_println,
    devices::{
        cloud_ii_wireless::CloudIIWireless, cloud_ii_wireless_dts::CloudIIWirelessDTS,
        cloud_iii_s_wireless::CloudIIISWireless, cloud_iii_wireless::CloudIIIWireless,
    },
};
use hidapi::{HidApi, HidDevice, HidError};
use std::{fmt::Display, time::Duration};
use thistermination::TerminationFull;

// Possible vendor IDs [HyperX, HP]
const VENDOR_IDS: [u16; 2] = [0x0951, 0x03F0];
// All supported product IDs
const PRODUCT_IDS: [u16; 9] = [0x1718, 0x018B, 0x0D93, 0x0696, 0x0b92, 0x05B7, 0x16EA, 0x0c9d, 0x06BE];

const RESPONSE_BUFFER_SIZE: usize = 256;
const RESPONSE_DELAY: Duration = Duration::from_millis(50);

pub fn connect_compatible_device() -> Result<Box<dyn Device>, DeviceError> {
    let state = DeviceState::new(&PRODUCT_IDS, &VENDOR_IDS)?;
    let name = state
        .hid_device
        .get_product_string()?
        .ok_or(DeviceError::NoDeviceFound())?;
    println!("Connecting to {}", name);
    let mut device: Box<dyn Device> = match (state.vendor_id, state.product_id) {
        (v, p)
            if cloud_ii_wireless::VENDOR_IDS.contains(&v)
                && cloud_ii_wireless::PRODUCT_IDS.contains(&p) =>
        {
            Box::new(CloudIIWireless::new_from_state(state))
        }
        (v, p)
            if cloud_ii_wireless_dts::VENDOR_IDS.contains(&v)
                && cloud_ii_wireless_dts::PRODUCT_IDS.contains(&p) =>
        {
            Box::new(CloudIIWirelessDTS::new_from_state(state))
        }
        (v, p)
            if cloud_iii_s_wireless::VENDOR_IDS.contains(&v)
                && cloud_iii_s_wireless::PRODUCT_IDS.contains(&p) =>
        {
            Box::new(CloudIIISWireless::new_from_state(state))
        }
        (v, p)
            if cloud_iii_wireless::VENDOR_IDS.contains(&v)
                && cloud_iii_wireless::PRODUCT_IDS.contains(&p) =>
        {
            Box::new(CloudIIIWireless::new_from_state(state))
        }
        (_, _) => return Err(DeviceError::NoDeviceFound()),
    };

    // Initialize capability flags
    device.init_capabilities();

    Ok(device)
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
    pub silent: Option<bool>,
    // Capability flags - set once during device initialization
    pub can_set_mute: bool,
    pub can_set_surround_sound: bool,
    pub can_set_side_tone: bool,
    pub can_set_automatic_shutdown: bool,
    pub can_set_side_tone_volume: bool,
    pub can_set_voice_prompt: bool,
    pub can_set_silent_mode: bool,
    pub can_set_equalizer: bool,
}

impl Display for DeviceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string_with_readonly_info(25))
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
            silent: None,
            // Capability flags - will be set by init_capabilities()
            can_set_mute: false,
            can_set_surround_sound: false,
            can_set_side_tone: false,
            can_set_automatic_shutdown: false,
            can_set_side_tone_volume: false,
            can_set_voice_prompt: false,
            can_set_silent_mode: false,
            can_set_equalizer: false,
        })
    }

    fn get_display_data(&self) -> Vec<(&str, Option<String>, &str, bool)> {
        vec![
            (
                "Battery level:",
                self.battery_level.map(|l| l.to_string()),
                "%",
                false,
            ),
            (
                "Charging status:",
                self.charging.map(|c| c.to_string()),
                "",
                false,
            ),
            (
                "Muted:",
                self.muted.map(|c| c.to_string()),
                "",
                !self.can_set_mute,
            ),
            (
                "Mic connected:",
                self.mic_connected.map(|c| c.to_string()),
                "",
                false,
            ),
            (
                "Automatic shutdown after:",
                self.automatic_shutdown_after
                    .map(|c| (c.as_secs() / 60).to_string()),
                "min",
                !self.can_set_automatic_shutdown,
            ),
            (
                "Pairing info:",
                self.pairing_info.map(|c| c.to_string()),
                "",
                false,
            ),
            (
                "Product color:",
                self.product_color.map(|c| c.to_string()),
                "",
                false,
            ),
            (
                "Side tone:",
                self.side_tone_on.map(|c| c.to_string()),
                "",
                !self.can_set_side_tone,
            ),
            (
                "Side tone volume:",
                self.side_tone_volume.map(|c| c.to_string()),
                "",
                !self.can_set_side_tone_volume,
            ),
            (
                "Surround sound:",
                self.surround_sound.map(|c| c.to_string()),
                "",
                !self.can_set_surround_sound,
            ),
            (
                "Voice prompt:",
                self.voice_prompt_on.map(|c| c.to_string()),
                "",
                !self.can_set_voice_prompt,
            ),
            (
                "Connected:",
                self.connected.map(|c| c.to_string()),
                "",
                false,
            ),
            (
                "Playback muted:",
                self.silent.map(|c| c.to_string()),
                "",
                !self.can_set_silent_mode,
            ),
        ]
    }

    pub fn to_string_with_padding(&self, padding: usize) -> String {
        self.get_display_data()
            .iter()
            .filter_map(|(prefix, data, suffix, _)| {
                data.as_ref()
                    .map(|data| format!("{:<padding$} {}{}", prefix, data, suffix))
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    pub fn to_string_with_readonly_info(&self, padding: usize) -> String {
        self.get_display_data()
            .iter()
            .filter_map(|(prefix, data, suffix, readonly)| {
                if let Some(data) = data {
                    let readonly_marker = if *readonly { " (read-only)" } else { "" };
                    Some(format!(
                        "{:<padding$} {}{}{}",
                        prefix, data, suffix, readonly_marker
                    ))
                } else {
                    None
                }
            })
            .collect::<Vec<String>>()
            .join("\n")
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
            DeviceEvent::Silent(silent) => self.silent = Some(*silent),
            DeviceEvent::RequireSIRKReset(reset) => {
                println!("requested SIRK reset {reset}")
            }
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
        self.silent = None;
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
    Silent(bool),
    RequireSIRKReset(bool),
}

#[derive(Debug, Copy, Clone)]
pub enum Color {
    BlackBlack,
    BlackRed,
    UnknownColor(u8),
}

impl Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Color::BlackBlack => "Black".to_string(),
                Color::BlackRed => "Red".to_string(),
                Color::UnknownColor(n) => format!("Unknown color {}", n),
            }
        )
    }
}

impl From<u8> for Color {
    fn from(color: u8) -> Self {
        match color {
            0 => Color::BlackBlack,
            2 => Color::BlackRed,
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
    fn get_sirk_packet(&self) -> Option<Vec<u8>>;
    fn reset_sirk_packet(&self) -> Option<Vec<u8>>;
    fn get_silent_mode_packet(&self) -> Option<Vec<u8>>;
    fn set_silent_mode_packet(&self, silence: bool) -> Option<Vec<u8>>;
    /// Set equalizer band (0-9) to dB value (-12.0 to +12.0)
    /// Bands: 0=32Hz, 1=64Hz, 2=125Hz, 3=250Hz, 4=500Hz, 5=1kHz, 6=2kHz, 7=4kHz, 8=8kHz, 9=16kHz
    fn set_equalizer_band_packet(&self, _band_index: u8, _db_value: f32) -> Option<Vec<u8>> {
        None
    }
    fn get_event_from_device_response(&self, response: &[u8]) -> Option<Vec<DeviceEvent>>;
    fn get_device_state(&self) -> &DeviceState;
    fn get_device_state_mut(&mut self) -> &mut DeviceState;
    fn prepare_write(&mut self) {}
    /// whether the app should periodically listen for packets from the headsets
    fn allow_passive_refresh(&mut self) -> bool;

    // Helper methods to check if features are writable
    fn can_set_mute(&self) -> bool {
        self.set_mute_packet(false).is_some()
    }
    fn can_set_surround_sound(&self) -> bool {
        self.set_surround_sound_packet(false).is_some()
    }
    fn can_set_side_tone(&self) -> bool {
        self.set_side_tone_packet(false).is_some()
    }
    fn can_set_automatic_shutdown(&self) -> bool {
        self.set_automatic_shut_down_packet(Duration::from_secs(0))
            .is_some()
    }
    fn can_set_side_tone_volume(&self) -> bool {
        self.set_side_tone_volume_packet(0).is_some()
    }
    fn can_set_voice_prompt(&self) -> bool {
        self.set_voice_prompt_packet(false).is_some()
    }
    fn can_set_silent_mode(&self) -> bool {
        self.set_silent_mode_packet(false).is_some()
    }
    fn can_set_equalizer(&self) -> bool {
        self.set_equalizer_band_packet(0, 0.0).is_some()
    }

    // Initialize capability flags in device state
    fn init_capabilities(&mut self) {
        // Collect capabilities first to avoid borrowing conflicts
        let can_set_mute = self.can_set_mute();
        let can_set_surround_sound = self.can_set_surround_sound();
        let can_set_side_tone = self.can_set_side_tone();
        let can_set_automatic_shutdown = self.can_set_automatic_shutdown();
        let can_set_side_tone_volume = self.can_set_side_tone_volume();
        let can_set_voice_prompt = self.can_set_voice_prompt();
        let can_set_silent_mode = self.can_set_silent_mode();
        let can_set_equalizer = self.can_set_equalizer();

        // Now set them in device state
        let state = self.get_device_state_mut();
        state.can_set_mute = can_set_mute;
        state.can_set_surround_sound = can_set_surround_sound;
        state.can_set_side_tone = can_set_side_tone;
        state.can_set_automatic_shutdown = can_set_automatic_shutdown;
        state.can_set_side_tone_volume = can_set_side_tone_volume;
        state.can_set_voice_prompt = can_set_voice_prompt;
        state.can_set_silent_mode = can_set_silent_mode;
        state.can_set_equalizer = can_set_equalizer;
    }

    fn execute_headset_specific_functionality(&mut self) -> Result<(), DeviceError> {
        Ok(())
    }
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
            self.get_sirk_packet(),
            self.get_silent_mode_packet(),
        ];

        self.execute_headset_specific_functionality()?;

        let mut responded = false;
        for packet in packets.into_iter().flatten() {
            self.prepare_write();
            debug_println!("Write packet: {packet:?}");
            self.get_device_state().hid_device.write(&packet)?;
            std::thread::sleep(RESPONSE_DELAY);
            if let Some(events) = self.wait_for_updates(Duration::from_secs(1)) {
                for event in events {
                    self.get_device_state_mut().update_self_with_event(&event);
                }
                responded = true;
            }
            if !matches!(self.get_device_state().connected, Some(true)) {
                break;
            }
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
        if self.allow_passive_refresh() {
            if let Some(events) = self.wait_for_updates(Duration::from_secs(1)) {
                for event in events {
                    self.get_device_state_mut().update_self_with_event(&event);
                }
            }
        }
        if let Some(batter_packet) = self.get_battery_packet() {
            self.prepare_write();
            self.get_device_state().hid_device.write(&batter_packet)?;
            std::thread::sleep(RESPONSE_DELAY);
            if let Some(events) = self.wait_for_updates(Duration::from_secs(1)) {
                for event in events {
                    self.get_device_state_mut().update_self_with_event(&event);
                }
            }
        }

        Ok(())
    }
}
