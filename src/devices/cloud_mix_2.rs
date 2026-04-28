use crate::{
    debug_println,
    devices::{ANCState, Device, DeviceEvent, DeviceState},
};
use std::time::Duration;

const HP: u16 = 0x03F0;
pub const VENDOR_IDS: [u16; 1] = [HP];
pub const PRODUCT_IDS: [u16; 1] = [0x0fae];

const BASE_PACKET: [u8; 64] = {
    let mut packet = [0; 64];
    packet[0] = 12;
    packet[1] = 2;
    packet[2] = 3;
    packet[3] = 1;
    packet
};

const GET_ANC_LEVEL_CMD_ID: u8 = 16;
const GET_ANC_STATE_CMD_ID: u8 = 14;
const SET_ANC_LEVEL_CMD_ID: u8 = 9;
const SET_ANC_STATE_CMD_ID: u8 = 7;
const GET_ANC_STATE_RESPONSE_CODE: u8 = 6;

const GET_BATTERY_CMD_ID: u8 = 6;
const GET_BATTERY_RESPONSE_CODE: u8 = 1;
const GET_MUTE_CMD_ID: u8 = 4;
const GET_MUTE_RESPONSE_CODE: u8 = 3;
const SET_MUTE_CMD_ID: u8 = 1;
const GET_PRODUCT_COLOR_CMD_ID: u8 = 8;
const GET_SIDE_TONE_ON_CMD_ID: u8 = 22;
const SET_SIDE_TONE_ON_CMD_ID: u8 = 13;
const GET_WIRELESS_STATUS_CMD_ID: u8 = 2;
const GET_WIRELESS_STATUS_RESPONSE_CODE: u8 = 4;

pub struct CloudMix2 {
    state: DeviceState,
}

impl CloudMix2 {
    pub fn new_from_state(state: DeviceState) -> Self {
        CloudMix2 { state }
    }
}

fn valid_eq_index(index: u8) -> bool {
    (0..4).contains(&index)
}

impl Device for CloudMix2 {
    fn get_charging_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn get_battery_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[5] = GET_BATTERY_CMD_ID;
        Some(tmp)
    }

    fn set_automatic_shut_down_packet(&self, _shutdown_after: Duration) -> Option<Vec<u8>> {
        None
    }

    fn get_automatic_shut_down_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn get_mute_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[5] = GET_MUTE_CMD_ID;
        Some(tmp)
    }

    fn set_mute_packet(&self, mute: bool) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[3] = 0;
        tmp[5] = SET_MUTE_CMD_ID;
        tmp[6] = mute as u8;
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
        tmp[5] = GET_PRODUCT_COLOR_CMD_ID;
        Some(tmp)
    }

    fn get_side_tone_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[5] = GET_SIDE_TONE_ON_CMD_ID;
        Some(tmp)
    }

    fn set_side_tone_packet(&self, side_tone_on: bool) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[3] = 0;
        tmp[5] = SET_SIDE_TONE_ON_CMD_ID;
        tmp[6] = side_tone_on as u8;
        Some(tmp)
    }

    fn get_side_tone_volume_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn set_side_tone_volume_packet(&self, _volume: u8) -> Option<Vec<u8>> {
        None
    }

    fn get_voice_prompt_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn set_voice_prompt_packet(&self, _enable: bool) -> Option<Vec<u8>> {
        None
    }

    fn get_wireless_connected_status_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[5] = GET_WIRELESS_STATUS_CMD_ID;
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

    fn get_anc_state_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[5] = GET_ANC_STATE_CMD_ID;
        Some(tmp)
    }

    fn set_anc_state_packet(&self, state: ANCState) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[3] = 0;
        tmp[5] = SET_ANC_STATE_CMD_ID;
        tmp[6] = match state {
            ANCState::On => 1,
            ANCState::Off => 0,
            ANCState::Transparent => 2,
        };
        Some(tmp)
    }

    fn get_anc_level_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[5] = GET_ANC_LEVEL_CMD_ID;
        Some(tmp)
    }
    fn set_anc_level_packet(&self, level: u8) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[3] = 0;
        tmp[5] = SET_ANC_LEVEL_CMD_ID;
        tmp[6] = level.clamp(0, 2);
        Some(tmp)
    }

    fn get_event_from_device_response(&self, response: &[u8]) -> Option<Vec<DeviceEvent>> {
        debug_println!("Read packet: {:?}", response);
        if response[0] != 13
            && response[1] == BASE_PACKET[1]
            && response[2] == BASE_PACKET[2]
            && response[3] == 0
        {
            match (response[4], response[5]) {
                (GET_BATTERY_RESPONSE_CODE, value) => Some(vec![DeviceEvent::BatterLevel(value)]),
                (GET_MUTE_RESPONSE_CODE, value) => Some(vec![DeviceEvent::Muted(value == 1)]),

                (GET_WIRELESS_STATUS_RESPONSE_CODE, value) => {
                    Some(vec![DeviceEvent::WirelessConnected(value == 1)])
                }
                (GET_ANC_STATE_RESPONSE_CODE, value) => {
                    Some(vec![DeviceEvent::ANCState(match value {
                        0 => ANCState::Off,
                        1 => ANCState::On,
                        2 => ANCState::Transparent,
                        _ => {
                            debug_println!("wrong anc state response {}", value);
                            ANCState::Off
                        }
                    })])
                }
                _ => {
                    debug_println!("Unknown device event: {:?}", response);
                    None
                }
            }
        } else if response[0] != BASE_PACKET[0]
            && response[1] == BASE_PACKET[1]
            && response[2] == BASE_PACKET[2]
            && response[3] == BASE_PACKET[3]
            && response[4] == 0
        {
            match (response[5], response[6]) {
                (2, value) => Some(vec![DeviceEvent::WirelessConnected(value == 1)]),
                (4, value) => Some(vec![DeviceEvent::Muted(value == 1)]),
                (6, value) => Some(vec![DeviceEvent::BatterLevel(value)]),
                (3, _) | (5, _) | (7, _) | (8, _) => None,
                (14, value) => Some(vec![DeviceEvent::ANCState(match value {
                    0 => ANCState::Off,
                    1 => ANCState::On,
                    2 => ANCState::Transparent,
                    _ => {
                        debug_println!("wrong anc state response {}", value);
                        ANCState::Off
                    }
                })]),
                (28, value) => todo!(), //TODO: EQ
                _ => {
                    debug_println!("Unknown device event: {:?}", response);
                    None
                }
            }
        } else {
            None
        }
    }

    fn get_device_state(&self) -> &DeviceState {
        &self.state
    }

    fn get_device_state_mut(&mut self) -> &mut DeviceState {
        &mut self.state
    }

    fn allow_passive_refresh(&mut self) -> bool {
        true
    }
}
