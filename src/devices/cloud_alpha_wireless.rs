use crate::{
    debug_println,
    devices::{ChargingStatus, Color, Device, DeviceError, DeviceEvent, DeviceState},
};
use std::time::Duration;

const HP: u16 = 0x03F0;
pub const VENDOR_IDS: [u16; 1] = [HP];
// Possible Cloud Alpha Wireless product IDs
pub const PRODUCT_IDS: [u16; 3] = [0x1743, 0x1765, 0x098D];

const BASE_PACKET: [u8; 64] = {
    let mut packet = [0; 64];
    packet[0] = 33;
    packet[1] = 187;
    packet
};

const RESPONSE_BUFFER_SIZE: usize = 256;

const GET_CHARGING_CMD_ID: u8 = 12;
const GET_CHARGING_RESPONSE_CODE: u8 = 38;
const GET_MIC_CONNECTED_CMD_ID: u8 = 8;
const GET_BATTERY_CMD_ID: u8 = 11;
const GET_BATTERY_RESPONSE_CODE: u8 = 37;
const GET_AUTO_SHUTDOWN_CMD_ID: u8 = 7;
const SET_AUTO_SHUTDOWN_CMD_ID: u8 = 18;
const GET_MUTE_CMD_ID: u8 = 10;
const GET_MUTE_RESPONSE_CODE: u8 = 35;
const SET_MUTE_CMD_ID: u8 = 21;
const GET_PAIRING_CMD_ID: u8 = 4;
const GET_PRODUCT_COLOR_CMD_ID: u8 = 14;
const GET_SIDE_TONE_ON_CMD_ID: u8 = 5;
const GET_SIDE_TONE_ON_RESPONSE_CODE: u8 = 34;
const SET_SIDE_TONE_ON_CMD_ID: u8 = 16;
const GET_SIDE_TONE_VOLUME_CMD_ID: u8 = 6;
const SET_SIDE_TONE_VOLUME_CMD_ID: u8 = 17;
const GET_VOICE_PROMPT_CMD_ID: u8 = 9;
const SET_VOICE_PROMPT_CMD_ID: u8 = 19;
const GET_WIRELESS_STATUS_CMD_ID: u8 = 3;
const GET_WIRELESS_STATUS_RESPONSE_CODE: u8 = 36;

pub struct CloudAlphaWireless {
    state: DeviceState,
}

impl CloudAlphaWireless {
    pub fn new_from_state(state: DeviceState) -> Self {
        CloudAlphaWireless { state }
    }

    pub fn new() -> Result<Self, DeviceError> {
        let state = DeviceState::new(&PRODUCT_IDS, &VENDOR_IDS)?;
        Ok(CloudAlphaWireless { state })
    }
}

impl Device for CloudAlphaWireless {
    fn get_response_buffer(&self) -> Vec<u8> {
        let mut tmp = [0u8; RESPONSE_BUFFER_SIZE].to_vec();
        tmp[0] = 33;
        tmp
    }

    fn get_charging_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[2] = GET_CHARGING_CMD_ID;
        Some(tmp)
    }

    fn get_battery_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[2] = GET_BATTERY_CMD_ID;
        Some(tmp)
    }

    fn set_automatic_shut_down_packet(&self, shutdown_after: Duration) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[2] = SET_AUTO_SHUTDOWN_CMD_ID;
        tmp[3] = (shutdown_after.as_secs() / 60) as u8;
        Some(tmp)
    }

    fn get_automatic_shut_down_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[2] = GET_AUTO_SHUTDOWN_CMD_ID;
        Some(tmp)
    }

    fn get_mute_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[2] = GET_MUTE_CMD_ID;
        Some(tmp)
    }

    fn set_mute_packet(&self, mute: bool) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[2] = SET_MUTE_CMD_ID;
        tmp[3] = mute as u8;
        Some(tmp)
    }

    fn get_surround_sound_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn set_surround_sound_packet(&self, _surround_sound: bool) -> Option<Vec<u8>> {
        None
    }

    fn get_mic_connected_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[2] = GET_MIC_CONNECTED_CMD_ID;
        Some(tmp)
    }

    fn get_pairing_info_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[2] = GET_PAIRING_CMD_ID; // Or 13
        Some(tmp)
    }

    fn get_product_color_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[2] = GET_PRODUCT_COLOR_CMD_ID;
        Some(tmp)
    }

    fn get_side_tone_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[2] = GET_SIDE_TONE_ON_CMD_ID;
        Some(tmp)
    }

    fn set_side_tone_packet(&self, side_tone_on: bool) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[2] = SET_SIDE_TONE_ON_CMD_ID;
        tmp[3] = side_tone_on as u8;
        Some(tmp)
    }

    fn get_side_tone_volume_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[2] = GET_SIDE_TONE_VOLUME_CMD_ID;
        Some(tmp)
    }

    fn set_side_tone_volume_packet(&self, volume: u8) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[2] = SET_SIDE_TONE_VOLUME_CMD_ID;
        tmp[3] = volume; // correct?
        Some(tmp)
    }

    fn get_voice_prompt_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[2] = GET_VOICE_PROMPT_CMD_ID;
        Some(tmp)
    }

    fn set_voice_prompt_packet(&self, enable: bool) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[2] = SET_VOICE_PROMPT_CMD_ID;
        tmp[3] = enable as u8;
        Some(tmp)
    }

    fn get_wireless_connected_status_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[2] = GET_WIRELESS_STATUS_CMD_ID;
        Some(tmp)
    }

    fn get_sirk_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn reset_sirk_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn get_silent_mode_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn set_silent_mode_packet(&self, _silence: bool) -> Option<Vec<u8>> {
        None
    }

    fn get_event_from_device_response(&self, response: &[u8]) -> Option<Vec<DeviceEvent>> {
        debug_println!("Read packet: {:?}", response);
        if response[0] != BASE_PACKET[0] || response[1] != BASE_PACKET[1] {
            return None;
        }
        match response[2] {
            GET_CHARGING_RESPONSE_CODE | GET_CHARGING_CMD_ID => Some(vec![DeviceEvent::Charging(
                ChargingStatus::from(response[3]),
            )]),
            GET_MIC_CONNECTED_CMD_ID => Some(vec![DeviceEvent::MicConnected(response[3] == 1)]),
            GET_BATTERY_RESPONSE_CODE | GET_BATTERY_CMD_ID => {
                Some(vec![DeviceEvent::BatterLevel(response[3])])
            }
            SET_AUTO_SHUTDOWN_CMD_ID | GET_AUTO_SHUTDOWN_CMD_ID => {
                Some(vec![DeviceEvent::AutomaticShutdownAfter(
                    Duration::from_secs(response[3] as u64 * 60),
                )])
            }
            SET_MUTE_CMD_ID | GET_MUTE_RESPONSE_CODE | GET_MUTE_CMD_ID => {
                Some(vec![DeviceEvent::Muted(response[3] == 1)])
            }
            GET_PAIRING_CMD_ID => Some(vec![DeviceEvent::PairingInfo(response[3])]),
            SET_SIDE_TONE_ON_CMD_ID | GET_SIDE_TONE_ON_RESPONSE_CODE | GET_SIDE_TONE_ON_CMD_ID => {
                Some(vec![DeviceEvent::SideToneOn(response[3] == 1)])
            }
            SET_SIDE_TONE_VOLUME_CMD_ID | GET_SIDE_TONE_VOLUME_CMD_ID => {
                Some(vec![DeviceEvent::SideToneVolume(response[3])])
            } //Correct?
            GET_WIRELESS_STATUS_RESPONSE_CODE | GET_WIRELESS_STATUS_CMD_ID => {
                Some(vec![DeviceEvent::WirelessConnected(response[3] == 2)])
            }
            SET_VOICE_PROMPT_CMD_ID | GET_VOICE_PROMPT_CMD_ID => {
                Some(vec![DeviceEvent::VoicePrompt(response[3] == 1)])
            }
            GET_PRODUCT_COLOR_CMD_ID => {
                Some(vec![DeviceEvent::ProductColor(Color::from(response[3]))])
            }
            _ => {
                debug_println!("Unknown device event: {:?}", response);
                None
            }
        }
    }

    fn allow_passive_refresh(&mut self) -> bool {
        true
    }

    fn get_device_state(&self) -> &DeviceState {
        &self.state
    }

    fn get_device_state_mut(&mut self) -> &mut DeviceState {
        &mut self.state
    }
}
