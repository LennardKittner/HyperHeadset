use crate::{
    debug_println,
    devices::{ChargingStatus, Color, Device, DeviceError, DeviceEvent, DeviceState},
};
use std::{time::Duration, vec};

const HP: u16 = 0x03F0;
pub const VENDOR_IDS: [u16; 1] = [HP];
pub const PRODUCT_IDS: [u16; 1] = [0x05B7];

const BASE_PACKET: [u8; 62] = {
    let mut packet = [0; 62];
    packet[0] = 102;
    packet
};

// sirk probably stands for Set Identity Resolving Key
const RESET_SIRK_CMD_ID: u8 = 30;
const GET_SIRK_CMD_ID: u8 = 131;
const GET_SILENT_MODE_CMD_ID: u8 = 135;
const SET_SILENT_MODE_CMD_ID: u8 = 4;
const GET_CHARGING_CMD_ID: u8 = 138;
const CHARGING_RESPONSE_ID: u8 = 12;
const GET_BATTERY_CMD_ID: u8 = 137;
const BATTERY_RESPONSE_ID: u8 = 13;
const GET_AUTO_SHUTDOWN_CMD_ID: u8 = 133;
const SET_AUTO_SHUTDOWN_CMD_ID: u8 = 2;
const GET_MUTE_CMD_ID: u8 = 134;
const MUTE_RESPONSE_ID: u8 = 10;
const SET_MUTE_CMD_ID: u8 = 3;
const GET_PRODUCT_COLOR_CMD_ID: u8 = 143;
const GET_SIDE_TONE_ON_CMD_ID: u8 = 132;
const SET_SIDE_TONE_ON_CMD_ID: u8 = 1;
const GET_SIDE_TONE_VOLUME_CMD_ID: u8 = 136;
const SET_SIDE_TONE_VOLUME_CMD_ID: u8 = 5;

// OR GetDongleStatus
const GET_WIRELESS_STATUS_CMD_ID: u8 = 130;
const WIRELESS_STATUS_RESPONSE_ID: u8 = 11;

pub struct CloudIIIWireless {
    state: DeviceState,
}

impl CloudIIIWireless {
    pub fn new_from_state(state: DeviceState) -> Self {
        CloudIIIWireless { state }
    }

    pub fn new() -> Result<Self, DeviceError> {
        let state = DeviceState::new(&PRODUCT_IDS, &VENDOR_IDS)?;
        Ok(CloudIIIWireless { state })
    }
}

impl Device for CloudIIIWireless {
    fn get_charging_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = GET_CHARGING_CMD_ID;
        Some(tmp)
    }

    fn get_battery_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = GET_BATTERY_CMD_ID;
        Some(tmp)
    }

    fn set_automatic_shut_down_packet(&self, shutdown_after: Duration) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = SET_AUTO_SHUTDOWN_CMD_ID;
        tmp[2] = (shutdown_after.as_secs() / 60) as u8;
        Some(tmp)
    }

    fn get_automatic_shut_down_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = GET_AUTO_SHUTDOWN_CMD_ID;
        Some(tmp)
    }

    fn get_mute_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = GET_MUTE_CMD_ID;
        Some(tmp)
    }

    fn set_mute_packet(&self, mute: bool) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = SET_MUTE_CMD_ID;
        tmp[2] = mute as u8;
        Some(tmp)
    }

    fn get_surround_sound_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn set_surround_sound_packet(&self, _surround_sound: bool) -> Option<Vec<u8>> {
        None
    }

    fn get_mic_connected_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn get_pairing_info_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn get_product_color_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = GET_PRODUCT_COLOR_CMD_ID;
        Some(tmp)
    }

    fn get_side_tone_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = GET_SIDE_TONE_ON_CMD_ID;
        Some(tmp)
    }

    fn set_side_tone_packet(&self, side_tone_on: bool) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = SET_SIDE_TONE_ON_CMD_ID;
        tmp[2] = side_tone_on as u8;
        Some(tmp)
    }

    fn get_side_tone_volume_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = GET_SIDE_TONE_VOLUME_CMD_ID;
        Some(tmp)
    }

    fn set_side_tone_volume_packet(&self, volume: u8) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = SET_SIDE_TONE_VOLUME_CMD_ID;
        tmp[2] = volume;
        Some(tmp)
    }

    fn get_voice_prompt_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn set_voice_prompt_packet(&self, _enable: bool) -> Option<Vec<u8>> {
        None
    }

    fn get_wireless_connected_status_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = GET_WIRELESS_STATUS_CMD_ID;
        Some(tmp)
    }

    fn get_sirk_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = GET_SIRK_CMD_ID;
        Some(tmp)
    }

    fn reset_sirk_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = RESET_SIRK_CMD_ID;
        Some(tmp)
    }

    fn get_silent_mode_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = GET_SILENT_MODE_CMD_ID;
        Some(tmp)
    }

    fn set_silent_mode_packet(&self, silence: bool) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = SET_SILENT_MODE_CMD_ID;
        tmp[2] = silence as u8;
        Some(tmp)
    }

    fn get_event_from_device_response(&self, response: &[u8]) -> Option<Vec<DeviceEvent>> {
        debug_println!("Read packet: {response:?}");
        if response[0] != 102 {
            return None;
        }
        match (response[1], response[2], response[3], response[4]) {
            (GET_MUTE_CMD_ID, mute, ..) | (MUTE_RESPONSE_ID, mute, ..) => {
                Some(vec![DeviceEvent::Muted(mute == 1)])
            }
            (GET_WIRELESS_STATUS_CMD_ID, connected, ..)
            | (WIRELESS_STATUS_RESPONSE_ID, connected, ..) => {
                Some(vec![DeviceEvent::WirelessConnected(connected == 1)])
            }
            (GET_CHARGING_CMD_ID, charging, ..) | (CHARGING_RESPONSE_ID, charging, ..) => {
                Some(vec![DeviceEvent::Charging(ChargingStatus::from(charging))])
            }
            (GET_BATTERY_CMD_ID, state1, state2, level)
            | (BATTERY_RESPONSE_ID, state1, state2, level) => {
                if state1 != 0 || state2 != 0 {
                    Some(vec![DeviceEvent::BatterLevel(level)])
                } else {
                    None
                }
            }
            (GET_AUTO_SHUTDOWN_CMD_ID, off_after, ..) => {
                Some(vec![DeviceEvent::AutomaticShutdownAfter(
                    Duration::from_secs(off_after as u64 * 60),
                )])
            }
            (GET_PRODUCT_COLOR_CMD_ID, color, ..) => {
                Some(vec![DeviceEvent::ProductColor(Color::from(color))])
            }
            (GET_SILENT_MODE_CMD_ID, silent, ..) => Some(vec![DeviceEvent::Silent(silent == 1)]),
            (GET_SIRK_CMD_ID, ..) => {
                let mut flag = false;
                for item in response.iter().take(18).skip(2) {
                    if item != &0u8 {
                        flag = true;
                        break;
                    }
                }
                Some(vec![DeviceEvent::RequireSIRKReset(flag)])
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
