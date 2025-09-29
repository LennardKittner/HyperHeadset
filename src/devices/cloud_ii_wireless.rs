use crate::devices::{ChargingStatus, Device, DeviceError, DeviceEvent, DeviceState};
use std::{time::Duration, u8};

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
        let mut tmp_state = state;
        tmp_state.connected = Some(true);
        CloudIIWireless { state: tmp_state }
    }

    pub fn new() -> Result<Self, DeviceError> {
        let mut state = DeviceState::new(&PRODUCT_IDS, &VENDOR_IDS)?;
        state.connected = Some(true);
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
        let mut tmp = [0u8; 62];
        tmp[0] = 6;
        tmp[2] = 0;
        tmp[4] = u8::MAX;
        tmp[7] = 104;
        tmp[8] = 74;
        tmp[9] = 142;
        Some(tmp.to_vec())
    }

    fn set_surround_sound_packet(&self, _surround_sound: bool) -> Option<Vec<u8>> {
        None
    }

    fn get_event_from_device_response(&self, response: &[u8]) -> Option<Vec<DeviceEvent>> {
        if response.len() < 7 {
            return None;
        }
        println!("Received packet: {:?}", response);
        match (
            response[0],
            response[3],
            response[4],
            response[7],
            response[12],
            response[14],
        ) {
            (_, GET_BATTERY_CMD_ID, _, level, _, _) => {
                println!("Battery Level {level}");
                Some(vec![DeviceEvent::BatterLevel(level)])
            }
            (11, GET_CHARGING_CMD_ID, status, _, _, _) => {
                println!("Charging {status} {:?}", ChargingStatus::from(status));
                Some(vec![DeviceEvent::Charging(ChargingStatus::from(status))])
            }
            (_, GET_AUTO_SHUTDOWN_CMD_ID, shutdown, _, _, _) => {
                println!("Shutdown time {shutdown}");
                Some(vec![DeviceEvent::AutomaticShutdownAfter(
                    Duration::from_secs(shutdown as u64 * 60),
                )])
            }
            (_, GET_MUTE_CMD_ID, muted, _, surround, other) => {
                println!("More info {} {}", surround, other);
                Some(vec![
                    DeviceEvent::SideToneOn((other & 16) != 0),
                    DeviceEvent::Muted((muted & 4) != 0),
                    DeviceEvent::SurroundSound((surround & 2) != 0),
                ])
            }
            (10, surround, _, _, _, _) => {
                println!("Surround sound {}", surround);
                Some(vec![DeviceEvent::SurroundSound((surround & 3) == 0)])
            }
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
        println!("Setting input report");
        let mut input_report_buffer = [0u8; 64];
        input_report_buffer[0] = 6;
        self.state
            .hid_device
            .get_input_report(&mut input_report_buffer)
            .unwrap();
    }

    fn execute_headset_specific_functionality(&mut self) -> Result<(), DeviceError> {
        println!("Writing special sequence");
        let mut packet = [0u8; 62];
        packet[0] = 6;
        packet[2] = 2;
        packet[4] = 154;
        packet[7] = 104;
        packet[8] = 74;
        packet[9] = 142;
        packet[10] = 10;
        packet[14] = 187;
        packet[15] = 1;
        self.prepare_write();
        println!("Writing {:?}", packet);
        self.state.hid_device.write(&packet)?;
        std::thread::sleep(Duration::from_millis(200));
        if let Some(events) = self.wait_for_updates(Duration::from_secs(1)) {
            println!("{:?}", events);
            for event in events {
                self.get_device_state_mut().update_self_with_event(&event);
            }
        }
        let mut packet = [0u8; 62];
        packet[0] = 6;
        packet[2] = 0;
        packet[4] = u8::MAX;
        packet[7] = 104;
        packet[8] = 74;
        packet[9] = 142;
        self.prepare_write();
        println!("Writing {:?}", packet);
        self.state.hid_device.write(&packet)?;
        std::thread::sleep(Duration::from_millis(200));
        if let Some(events) = self.wait_for_updates(Duration::from_secs(1)) {
            println!("{:?}", events);
            for event in events {
                self.get_device_state_mut().update_self_with_event(&event);
            }
        }
        let mut packet = [0u8; 62];
        packet[0] = 6;
        packet[2] = 2;
        packet[4] = 154;
        packet[7] = 104;
        packet[8] = 74;
        packet[9] = 142;
        packet[10] = 10;
        packet[14] = 187;
        packet[15] = 17;
        self.prepare_write();
        println!("Writing {:?}", packet);
        self.state.hid_device.write(&packet)?;
        std::thread::sleep(Duration::from_millis(200));
        if let Some(events) = self.wait_for_updates(Duration::from_secs(1)) {
            println!("{:?}", events);
            for event in events {
                self.get_device_state_mut().update_self_with_event(&event);
            }
        }
        let mut packet = [0u8; 62];
        packet[0] = 6;
        packet[2] = 2;
        packet[4] = 154;
        packet[7] = 104;
        packet[8] = 74;
        packet[9] = 142;
        packet[10] = 10;
        packet[14] = 187;
        packet[15] = 29;
        self.prepare_write();
        println!("Writing {:?}", packet);
        self.state.hid_device.write(&packet)?;
        std::thread::sleep(Duration::from_millis(200));
        if let Some(events) = self.wait_for_updates(Duration::from_secs(1)) {
            println!("{:?}", events);
            for event in events {
                self.get_device_state_mut().update_self_with_event(&event);
            }
        }
        let mut packet = [0u8; 62];
        packet[0] = 6;
        packet[2] = 2;
        packet[4] = 154;
        packet[7] = 104;
        packet[8] = 74;
        packet[9] = 142;
        packet[10] = 10;
        packet[14] = 187;
        packet[15] = 9;
        self.prepare_write();
        println!("Writing {:?}", packet);
        self.state.hid_device.write(&packet)?;
        std::thread::sleep(Duration::from_millis(200));
        if let Some(events) = self.wait_for_updates(Duration::from_secs(1)) {
            println!("{:?}", events);
            for event in events {
                self.get_device_state_mut().update_self_with_event(&event);
            }
        }

        Ok(())
    }
}
