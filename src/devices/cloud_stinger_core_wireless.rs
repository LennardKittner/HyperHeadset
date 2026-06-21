use crate::{
    debug_println,
    devices::{ChargingStatus, Device, DeviceEvent, DeviceState},
};
use std::time::Duration;

const HP: u16 = 0x03F0;
const HYPERX: u16 = 0x0951;
pub const VENDOR_IDS: [u16; 2] = [HP, HYPERX];
pub const PRODUCT_IDS: [u16; 1] = [0x170B];

const CONNECTED_PACKET: [u8; 64] = {
    let mut packet = [0; 64];
    packet[0] = u8::MAX;
    packet[1] = 7;
    packet[2] = 0;
    packet[3] = 253;
    packet[4] = 4;
    packet[5] = 0;
    packet[6] = 0;
    packet[7] = 2;
    packet[8] = 130;
    packet[9] = 3;
    packet[10] = 5;
    packet[11] = 41;
    packet[12] = 15;
    packet[13] = 0;
    packet[14] = 0;
    packet[15] = 3;
    packet[16] = 104;
    packet[17] = 16;
    packet
};

const BATTERY_PACKET: [u8; 64] = {
    let mut packet = [0; 64];
    packet[0] = u8::MAX;
    packet[1] = 7;
    packet[2] = 0;
    packet[3] = 253;
    packet[4] = 4;
    packet[5] = 12;
    packet[6] = 241;
    packet[7] = 2;
    packet[8] = 1;
    packet[9] = 4;
    packet[10] = 240;
    packet[11] = 12;
    packet
};

pub struct CloudStingerCoreWireless {
    state: DeviceState,
}

impl CloudStingerCoreWireless {
    pub fn new_from_state(state: DeviceState) -> Self {
        let mut state = state;
        state.device_properties.connected = Some(true);
        CloudStingerCoreWireless { state }
    }
}

const THRESHOLDS: [i16; 11] = [
    3400, 3831, 3871, 3916, 3930, 3946, 3967, 4000, 4050, 4090, 4130,
];

const THRESHOLDS_2: [i16; 11] = [
    3517, 3651, 3700, 3738, 3763, 3785, 3838, 3905, 3954, 4042, 4121,
];

impl Device for CloudStingerCoreWireless {
    fn get_battery_packet(&self) -> Option<Vec<u8>> {
        let tmp = BATTERY_PACKET.to_vec();
        Some(tmp)
    }

    fn get_event_from_device_response(&self, response: &[u8]) -> Option<Vec<DeviceEvent>> {
        //TODO: we probably have to differentiate between connection and battery level
        debug_println!("Read packet: {:?}", response);
        let mut events = Vec::new();
        events.push(DeviceEvent::WirelessConnected(response[11] == 1));

        let value = i16::from_le_bytes([response[11], response[12]]);
        let charging_status = match response[9] {
            3 => ChargingStatus::NotCharging,
            5 => ChargingStatus::Charging,
            6 => ChargingStatus::FullyCharged,
            _ => return Some(events),
        };
        events.push(DeviceEvent::Charging(charging_status));
        if charging_status != ChargingStatus::NotCharging {
            if charging_status == ChargingStatus::FullyCharged
                || value >= *THRESHOLDS.last().expect("THRESHOLDS empty")
            {
                events.push(DeviceEvent::BatterLevel(100));
            } else {
                let index = match THRESHOLDS.binary_search(&value) {
                    Ok(i) => i.min(THRESHOLDS.len() - 2),
                    Err(i) => i.saturating_sub(1).min(THRESHOLDS.len() - 2),
                };
                let battery_level = index * 10;
                let battery_level = battery_level as u8
                    + ((value as f32 - THRESHOLDS[index] as f32)
                        / (THRESHOLDS[index + 1] as f32 - THRESHOLDS[index] as f32)
                        * 10f32) as u8;
                events.push(DeviceEvent::BatterLevel(battery_level));
            }
        } else if value >= *THRESHOLDS_2.last().expect("THRESHOLDS empty") {
            events.push(DeviceEvent::BatterLevel(100));
        } else {
            let index = match THRESHOLDS_2.binary_search(&value) {
                Ok(i) => i.min(THRESHOLDS_2.len() - 2),
                Err(i) => i.saturating_sub(1).min(THRESHOLDS_2.len() - 2),
            };
            let battery_level = index * 10;
            let battery_level = battery_level as u8
                + ((value as f32 - THRESHOLDS_2[index] as f32)
                    / (THRESHOLDS_2[index + 1] as f32 - THRESHOLDS_2[index] as f32)
                    * 10f32) as u8;
            events.push(DeviceEvent::BatterLevel(battery_level));
        }

        Some(events)
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
        let tmp = CONNECTED_PACKET.to_vec();
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
}
