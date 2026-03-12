use crate::{
    debug_println,
    devices::{Device, DeviceEvent, DeviceState},
};
use std::time::Duration;

const HP: u16 = 0x03F0;
pub const VENDOR_IDS: [u16; 1] = [HP];
pub const PRODUCT_IDS: [u16; 1] = [0x03C0];

const BATTER_PACKET: [u8; 16] = {
    let mut packet = [0; 16];
    (packet[0], packet[1], packet[2]) = (0x06, 0xF0, 0x09);
    packet
};

pub struct CloudJetWireless {
    state: DeviceState,
}

impl CloudJetWireless {
    pub fn new_from_state(state: DeviceState) -> Self {
        let mut state = state;
        state.connected = Some(true);
        CloudJetWireless { state }
    }
}

impl Device for CloudJetWireless {
    fn get_charging_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn get_battery_packet(&self) -> Option<Vec<u8>> {
        Some(BATTER_PACKET.to_vec())
    }

    fn set_automatic_shut_down_packet(&self, _shutdown_after: Duration) -> Option<Vec<u8>> {
        None
    }

    fn get_automatic_shut_down_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn get_mute_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn set_mute_packet(&self, _mute: bool) -> Option<Vec<u8>> {
        None
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
        None
    }

    fn get_side_tone_packet(&self) -> Option<Vec<u8>> {
        None
    }

    fn set_side_tone_packet(&self, _side_tone_on: bool) -> Option<Vec<u8>> {
        None
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
        match response {
            [11, 218, 1, level, 0, 7, 1, _level2, 136, 0, 1, 14, 0, 0, 105, 2] => {
                Some(vec![DeviceEvent::BatterLevel(*level)])
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
