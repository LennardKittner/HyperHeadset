use crate::{
    debug_println,
    devices::{ChargingStatus, Device, DeviceEvent, DeviceState},
};
use std::time::Duration;

const HP: u16 = 0x03F0;
const HYPERX: u16 = 0x0951;
pub const VENDOR_IDS: [u16; 2] = [HP, HYPERX];
pub const PRODUCT_IDS: [u16; 2] = [0x0e90, 0x1749];

const BASE_PACKET: [u8; 64] = {
    let mut packet = [0; 64];
    packet[0] = 33;
    packet[1] = 255;
    packet
};

const GET_BATTERY_CMD_ID: u8 = 5;

pub struct CloudFlightWireless {
    state: DeviceState,
}

impl CloudFlightWireless {
    pub fn new_from_state(state: DeviceState) -> Self {
        let mut state = state;
        state.device_properties.connected = Some(true);
        CloudFlightWireless { state }
    }
}

const THRESHOLDS: [u16; 20] = [
    3328, 3584, 3674, 3704, 3732, 3744, 3754, 3764, 3774, 3784, 3794, 3804, 3824, 3840, 3860, 3890,
    3910, 3940, 3960, 3970,
];

const PERCENTAGES: [u8; 20] = [
    5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55, 60, 65, 70, 75, 80, 85, 90, 95, 100,
];

impl Device for CloudFlightWireless {
    fn get_battery_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[2] = GET_BATTERY_CMD_ID;
        Some(tmp)
    }

    fn get_event_from_device_response(&self, response: &[u8]) -> Option<Vec<DeviceEvent>> {
        debug_println!("Read packet: {:?}", response);
        if response[0] != BASE_PACKET[0] || response[1] != BASE_PACKET[1] {
            return None;
        }
        match response[2] {
            GET_BATTERY_CMD_ID => {
                let upper = response[3];
                let lower = response[4];
                let mut events = Vec::new();
                if (upper == 16 && lower >= 20) || upper >= 17 {
                    events.push(DeviceEvent::Charging(ChargingStatus::Charging));
                } else {
                    let index =
                        match THRESHOLDS.binary_search(&(((upper as u16) << 8) | (lower as u16))) {
                            Ok(i) => i,
                            Err(0) => 0,
                            Err(i) => i - 1,
                        };
                    events.push(DeviceEvent::BatterLevel(PERCENTAGES[index]));
                    events.push(DeviceEvent::Charging(ChargingStatus::NotCharging));
                }
                Some(events)
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

    fn get_charging_packet(&self) -> Option<Vec<u8>> {
        None
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
}
