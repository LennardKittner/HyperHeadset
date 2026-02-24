use crate::{
    debug_println,
    devices::{ChargingStatus, Device, DeviceError, DeviceEvent, DeviceState},
};
use std::time::Duration;

const HP: u16 = 0x03F0;
pub const VENDOR_IDS: [u16; 1] = [HP];
pub const PRODUCT_IDS: [u16; 1] = [0x0aa0];

const BASE_PACKET: [u8; 64] = {
    let mut packet = [0; 64];
    packet[0] = 102;
    packet
};

const GET_CHARGING_CMD_ID: u8 = 138;
const CHARGING_RESPONSE_ID: u8 = 12;
const GET_MIC_CONNECTED_CMD_ID: u8 = 140;
const MIC_CONNECTED_RESPONSE_ID: u8 = 7;
const GET_BATTERY_CMD_ID: u8 = 137;
const BATTERY_RESPONSE_ID: u8 = 13;
const GET_AUTO_SHUTDOWN_CMD_ID: u8 = 133;
const SET_AUTO_SHUTDOWN_CMD_ID: u8 = 2;
const GET_MUTE_CMD_ID: u8 = 134;
const MUTE_RESPONSE_ID: u8 = 10;
const SET_MUTE_CMD_ID: u8 = 3;
const GET_PAIRING_CMD_ID: u8 = 129;
const GET_SIDE_TONE_ON_CMD_ID: u8 = 132;
const SIDE_TONE_RESPONSE_ID: u8 = 9;
const SET_SIDE_TONE_ON_CMD_ID: u8 = 1;
const GET_SIDE_TONE_VOLUME_CMD_ID: u8 = 136;
const SET_SIDE_TONE_VOLUME_CMD_ID: u8 = 5;
const GET_WIRELESS_STATUS_CMD_ID: u8 = 130;
const WIRELESS_STATUS_RESPONSE_ID: u8 = 11;
const GET_PLAY_BACK_MUTE_CMD_ID: u8 = 135;
const SET_PLAY_BACK_MUTE_CMD_ID: u8 = 4;
const GET_NOISE_GATE_CMD_ID: u8 = 141;
const SET_NOISE_GATE_CMD_ID: u8 = 15;

pub struct CloudIICoreWireless {
    state: DeviceState,
}

impl CloudIICoreWireless {
    pub fn new_from_state(state: DeviceState) -> Self {
        let mut state = state;
        state.connected = Some(true);
        CloudIICoreWireless { state }
    }

    pub fn new() -> Result<Self, DeviceError> {
        let mut state = DeviceState::new(&PRODUCT_IDS, &VENDOR_IDS)?;
        state.connected = Some(true);
        Ok(CloudIICoreWireless { state })
    }
}

impl Device for CloudIICoreWireless {
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
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = GET_MIC_CONNECTED_CMD_ID;
        Some(tmp)
    }

    fn get_pairing_info_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = GET_PAIRING_CMD_ID;
        Some(tmp)
    }

    fn get_product_color_packet(&self) -> Option<Vec<u8>> {
        None
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
        tmp[2] = (volume as i8).clamp(-5, 5) as u8;
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
        None
    }

    fn reset_sirk_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn get_silent_mode_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = GET_PLAY_BACK_MUTE_CMD_ID;
        Some(tmp)
    }

    fn set_silent_mode_packet(&self, silence: bool) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = SET_PLAY_BACK_MUTE_CMD_ID;
        tmp[2] = silence as u8;
        Some(tmp)
    }

    fn get_noise_gate_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = GET_NOISE_GATE_CMD_ID;
        Some(tmp)
    }

    fn set_noise_gate_packet(&self, enable: bool) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = SET_NOISE_GATE_CMD_ID;
        tmp[2] = enable as u8;
        Some(tmp)
    }

    fn get_event_from_device_response(&self, response: &[u8]) -> Option<Vec<DeviceEvent>> {
        debug_println!("Read packet: {:?}", response);
        if response[0] != 102 {
            return None;
        }
        match (response[1], response[2], response[3], response[4]) {
            (GET_CHARGING_CMD_ID, status, _, _) | (CHARGING_RESPONSE_ID, status, _, _) => {
                Some(vec![DeviceEvent::Charging(ChargingStatus::from(status))])
            }
            (GET_MIC_CONNECTED_CMD_ID, status, _, _)
            | (MIC_CONNECTED_RESPONSE_ID, status, _, _) => {
                Some(vec![DeviceEvent::MicConnected(status == 1)])
            }
            (GET_BATTERY_CMD_ID, b2, b3, level) | (BATTERY_RESPONSE_ID, b2, b3, level) => {
                if b2 != 0 || b3 != 0 {
                    Some(vec![DeviceEvent::BatterLevel(level)])
                } else {
                    None
                }
            }
            (GET_AUTO_SHUTDOWN_CMD_ID, time, _, _) | (SET_AUTO_SHUTDOWN_CMD_ID, time, _, _) => {
                Some(vec![DeviceEvent::AutomaticShutdownAfter(
                    Duration::from_secs(time as u64 * 60),
                )])
            }
            (SET_MUTE_CMD_ID, status, _, _)
            | (GET_MUTE_CMD_ID, status, _, _)
            | (MUTE_RESPONSE_ID, status, _, _) => Some(vec![DeviceEvent::Muted(status == 1)]),
            (GET_PAIRING_CMD_ID, status, _, _) => Some(vec![DeviceEvent::PairingInfo(status)]),
            (GET_SIDE_TONE_ON_CMD_ID, status, _, _)
            | (SET_SIDE_TONE_ON_CMD_ID, status, _, _)
            | (SIDE_TONE_RESPONSE_ID, status, _, _) => {
                Some(vec![DeviceEvent::SideToneOn(status == 1)])
            }
            (GET_SIDE_TONE_VOLUME_CMD_ID, status, _, _)
            | (SET_SIDE_TONE_VOLUME_CMD_ID, status, _, _) => {
                let status = if status >= 251 {
                    (status as i32 | -256i32) as u8
                } else if (0..=5).contains(&status) {
                    status
                } else {
                    0u8
                };
                Some(vec![DeviceEvent::SideToneVolume(status)])
            }
            (GET_WIRELESS_STATUS_CMD_ID, status, _, _)
            | (WIRELESS_STATUS_RESPONSE_ID, status, _, _) => {
                Some(vec![DeviceEvent::WirelessConnected(status == 1)])
            }
            (GET_PLAY_BACK_MUTE_CMD_ID, status, _, _)
            | (SET_PLAY_BACK_MUTE_CMD_ID, status, _, _) => {
                Some(vec![DeviceEvent::Silent(status == 1)])
            }
            (GET_NOISE_GATE_CMD_ID, status, _, _) | (SET_NOISE_GATE_CMD_ID, status, _, _) => {
                Some(vec![DeviceEvent::NoiseGateActive(status == 1)])
            }
            _ => {
                debug_println!("Unknown device event: {:?}", response);
                None
            }
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
