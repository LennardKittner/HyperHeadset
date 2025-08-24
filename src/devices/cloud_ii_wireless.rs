use crate::devices::{ChargingStatus, Device, DeviceError, DeviceEvent, DeviceState};
use std::time::Duration;

const HYPERX: u16 = 0x0951;
pub const VENDOR_IDS: [u16; 1] = [HYPERX];
// Possible Cloud II Wireless product IDs
pub const PRODUCT_IDS: [u16; 3] = [0x1718, 0x018B, 0x0b92];

const BASE_PACKET: [u8; 62] = {
    let mut tmp = [0u8; 62];
    tmp[0] = 0x06;
    tmp[1] = 0x00;
    tmp[2] = 0x02;
    tmp[3] = 0x00;
    tmp[4] = 0x9A;
    tmp[5] = 0x00;
    tmp[6] = 0x00;
    tmp[7] = 0x68;
    tmp[8] = 0x4A;
    tmp[9] = 0x8E;
    tmp[10] = 0x0A;
    tmp[11] = 0x00;
    tmp[12] = 0x00;
    tmp[13] = 0x00;
    tmp[14] = 0xBB;
    tmp[15] = 0x01;
    tmp
};

// I am unsure about all the other command ids

const GET_CHARGING_CMD_ID: u8 = 3;
// const GET_MIC_CONNECTED_CMD_ID: u8 = 8;
const GET_BATTERY_CMD_ID: u8 = 2;
const GET_AUTO_SHUTDOWN_CMD_ID: u8 = 26;
const SET_AUTO_SHUTDOWN_CMD_ID: u8 = 24;
// includes also some other information such as side tone and surround sound
const GET_MUTE_CMD_ID: u8 = 1;
// const SET_MUTE_CMD_ID: u8 = 32;
// const GET_PAIRING_CMD_ID: u8 = 9;
// const GET_PRODUCT_COLOR_CMD_ID: u8 = 14;
// const GET_SIDE_TONE_ON_CMD_ID: u8 = 6;
const SET_SIDE_TONE_ON_CMD_ID: u8 = 25;
// const GET_SIDE_TONE_VOLUME_CMD_ID: u8 = 11;
// const SET_SIDE_TONE_VOLUME_CMD_ID: u8 = 35;
// const GET_VOICE_PROMPT_CMD_ID: u8 = 9;
// const SET_VOICE_PROMPT_CMD_ID: u8 = 19;
// const GET_WIRELESS_STATUS_CMD_ID: u8 = 1;

pub struct CloudIIWireless {
    state: DeviceState,
}

impl CloudIIWireless {
    pub fn new_from_state(state: DeviceState) -> Self {
        CloudIIWireless { state }
    }

    pub fn new() -> Result<Self, DeviceError> {
        let state = DeviceState::new(&PRODUCT_IDS, &VENDOR_IDS)?;
        Ok(CloudIIWireless { state })
    }
}

impl Device for CloudIIWireless {
    fn get_charging_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[15] = GET_CHARGING_CMD_ID;
        Some(tmp)
    }

    fn get_battery_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[15] = GET_BATTERY_CMD_ID;
        Some(tmp)
    }

    fn set_automatic_shut_down_packet(&self, shutdown_after: Duration) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[15] = SET_AUTO_SHUTDOWN_CMD_ID;
        tmp[16] = (shutdown_after.as_secs() / 60) as u8;
        Some(tmp)
    }

    fn get_automatic_shut_down_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[15] = GET_AUTO_SHUTDOWN_CMD_ID;
        Some(tmp)
    }

    fn get_mute_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[15] = GET_MUTE_CMD_ID;
        Some(tmp)
    }

    fn set_mute_packet(&self, _mute: bool) -> Option<Vec<u8>> {
        None
    }

    fn get_mic_connected_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn get_pairing_info_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn get_product_color_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn get_side_tone_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn set_side_tone_packet(&self, side_tone_on: bool) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[15] = SET_SIDE_TONE_ON_CMD_ID;
        tmp[16] = side_tone_on as u8;
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
        None
    }

    fn get_surround_sound_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn set_surround_sound_packet(&self, _surround_sound: bool) -> Option<Vec<u8>> {
        None
    }

    fn get_event_from_device_response(&self, response: &[u8]) -> Option<Vec<DeviceEvent>> {
        if response.len() < 7 {
            return None;
        }
        println!("Received packet: {:?}", response);
        match (response[3], response[7], response[12], response[14]) {
            (GET_BATTERY_CMD_ID, level, _, _) => Some(vec![DeviceEvent::BatterLevel(level)]),
            (GET_CHARGING_CMD_ID, status, _, _) => {
                Some(vec![DeviceEvent::Charging(ChargingStatus::from(status))])
            }
            (GET_AUTO_SHUTDOWN_CMD_ID, shutdown, _, _) => {
                Some(vec![DeviceEvent::AutomaticShutdownAfter(
                    Duration::from_secs(shutdown as u64 * 60),
                )])
            }
            (GET_MUTE_CMD_ID, _, surround, other) => Some(vec![
                DeviceEvent::SideToneOn((other & 16) != 0),
                DeviceEvent::Muted((other & 2) != 0),
                DeviceEvent::SurroundSound((surround & 2) != 0),
            ]),
            _ => {
                println!("Unknown device event: {:?}", response);
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

    fn prepare_write(&mut self) {
        let mut input_report_buffer = [0u8; 64];
        input_report_buffer[0] = 6;
        self.state
            .hid_device
            .get_input_report(&mut input_report_buffer)
            .unwrap();
    }
}
