use crate::{
    debug_println,
    devices::{Color, Device, DeviceEvent, DeviceState},
    HP,
};

pub const VENDOR_IDS: [u16; 1] = [HP];
pub const PRODUCT_IDS: [u16; 1] = [0x089d];

const BASE_PACKET: [u8; 62] = {
    let mut packet = [0; 62];
    packet[0] = 32;
    packet
};

const GET_PRODUCT_COLOR_CMD_ID: u8 = 4;
const GET_SIDE_TONE_ON_CMD_ID: u8 = 6;
const SET_SIDE_TONE_ON_CMD_ID: u8 = 134;
const GET_SIDE_TONE_VOLUME_CMD_ID: u8 = 5;
const SET_SIDE_TONE_VOLUME_CMD_ID: u8 = 133;
const PERSIST_SIDE_TONE: u8 = 135;

pub struct CloudIII {
    state: DeviceState,
}

impl CloudIII {
    pub fn new_from_state(state: DeviceState) -> Self {
        CloudIII { state }
    }
}

impl Device for CloudIII {
    fn get_response_buffer(&self) -> Vec<u8> {
        BASE_PACKET.to_vec()
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

    fn persist_settings_to_the_device_packet(&self, enabled: bool) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[1] = PERSIST_SIDE_TONE;
        tmp[2] = if enabled { 2 } else { 1 };
        Some(tmp)
    }

    fn get_event_from_device_response(&self, response: &[u8]) -> Option<Vec<DeviceEvent>> {
        debug_println!("Read packet: {response:?}");
        if response[0] != 102 {
            return None;
        }
        match (response[1], response[2]) {
            (GET_PRODUCT_COLOR_CMD_ID, color) => {
                Some(vec![DeviceEvent::ProductColor(Color::from(color))])
            }
            (SET_SIDE_TONE_VOLUME_CMD_ID, volume) | (GET_SIDE_TONE_VOLUME_CMD_ID, volume) => {
                Some(vec![DeviceEvent::SideToneVolume(volume)])
            }
            (SET_SIDE_TONE_ON_CMD_ID, enabled) | (GET_SIDE_TONE_ON_CMD_ID, enabled) => {
                Some(vec![DeviceEvent::SideToneOn(enabled == 1)])
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
