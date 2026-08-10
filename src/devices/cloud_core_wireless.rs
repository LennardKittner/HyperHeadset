use crate::{
    debug_println,
    devices::{ChargingStatus, Device, DeviceEvent, DeviceState},
};
use std::time::Duration;

const HP: u16 = 0x03F0;
const HYPERX: u16 = 0x0951;
pub const VENDOR_IDS: [u16; 2] = [HP, HYPERX];
pub const PRODUCT_IDS: [u16; 2] = [0x173F, 0x1740];

const BATTERY_PACKET: [u8; 64] = {
    let mut buf = [0; 64];
    buf[0] = 0xFF;
    buf[1] = 0x07;
    buf[2] = 0x00;
    buf[3] = 0xFD;
    buf[4] = 0x04;
    buf[5] = 0x0C;
    buf[6] = 0xF1;
    buf[7] = 0x02;
    buf[8] = 0x01;
    buf[9] = 0x04;
    buf[10] = 0xF0;
    buf[11] = 0x0C;
    buf
};

pub struct CloudCoreWireless {
    state: DeviceState,
}

impl CloudCoreWireless {
    pub fn new_from_state(state: DeviceState) -> Self {
        let mut state = state;
        state.device_properties.connected = Some(true);
        CloudCoreWireless { state }
    }
}

//TODO: use real THRESHOLDS
const THRESHOLDS: [u16; 20] = [
    3328, 3584, 3674, 3704, 3732, 3744, 3754, 3764, 3774, 3784, 3794, 3804, 3824, 3840, 3860, 3890,
    3910, 3940, 3960, 3970,
];

const PERCENTAGES: [u8; 20] = [
    5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55, 60, 65, 70, 75, 80, 85, 90, 95, 100,
];

impl Device for CloudCoreWireless {
    fn get_battery_packet(&self) -> Option<Vec<u8>> {
        Some(BATTERY_PACKET.to_vec())
    }
    fn get_response_buffer(&self) -> Vec<u8> {
        let mut buf = vec![0x00; 64];
        buf[0] = 0xFF;
        buf
    }

    fn wait_for_updates(&mut self, duration: Duration) -> Option<Vec<DeviceEvent>> {
        let mut buf1 = self.get_response_buffer();
        let mut events = Vec::new();

        let res1 = match self
            .get_device_state()
            .hid_device
            .read_timeout(&mut buf1[..], duration.as_millis() as i32)
        {
            Ok(n) => n,
            Err(e) => {
                println!("read_timeout error: {e:?}");
                0
            }
        };
        if res1 != 0 {
            if let Some(mut e) = self.get_event_from_device_response(&buf1) {
                events.append(&mut e);
            }
        }

        let mut buf2 = self.get_response_buffer();
        let res2 = match self
            .get_device_state()
            .hid_device
            .get_feature_report(&mut buf2[..])
        {
            Ok(n) => n,
            Err(e) => {
                println!("get_feature_report error: {e:?}");
                0
            }
        };
        if res2 != 0 {
            if let Some(mut e) = self.get_event_from_device_response(&buf2) {
                events.append(&mut e);
            }
        }
        println!("read timeout: {buf1:?}");
        println!("get feature report: {buf2:?}");
        if res1 + res2 == 0 {
            None
        } else {
            Some(events)
        }
    }

    fn get_event_from_device_response(&self, response: &[u8]) -> Option<Vec<DeviceEvent>> {
        println!("Read packet: {:?}", response);
        if response[0] == 0xFF && response[1] == 0x12 {
            let lower = response[11] as u32;
            let upper = response[12] as u32;
            let mut events = Vec::new();
            let index = match THRESHOLDS.binary_search(&(((upper as u16) << 8) | (lower as u16))) {
                Ok(i) => i,
                Err(0) => 0,
                Err(i) => i - 1,
            };
            events.push(DeviceEvent::BatterLevel(PERCENTAGES[index]));
            events.push(DeviceEvent::Charging(if response[9] == 0x05 {
                ChargingStatus::Charging
            } else {
                ChargingStatus::NotCharging
            }));
            Some(events)
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
