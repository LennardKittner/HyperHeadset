use crate::{
    debug_println,
    devices::{ChargingStatus, Device, DeviceError, DeviceEvent, DeviceState},
};
use std::time::Duration;

const HYPERX: u16 = 0x0951;
pub const VENDOR_IDS: [u16; 1] = [HYPERX];
// Possible Cloud II Wireless product IDs (and Cloud Flight S)
pub const PRODUCT_IDS: [u16; 5] = [0x1718, 0x018B, 0x0b92, 0x16EA, 0x16EB];

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

const GET_CHARGING_CMD_ID: u8 = 3;
const GET_BATTERY_CMD_ID: u8 = 2;
const GET_AUTO_SHUTDOWN_CMD_ID: u8 = 26;
const SET_AUTO_SHUTDOWN_CMD_ID: u8 = 24;
// includes also some other information such as side tone and surround sound
const GET_MUTE_CMD_ID: u8 = 1;
const MUTE_RESPONSE_ID: u8 = 8;
const FIRMWARE_VERSION_RESPONSE_ID: u8 = 17;
const CONNECTION_STATUS_RESPONSE_ID: u8 = 1;
const SET_SIDE_TONE_ON_CMD_ID: u8 = 25;

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
        if response.len() < 7 {
            return None;
        }

        // Most responses are Report ID 11 (0x0B) with structure: [11, 0, 187, cmd_id, ...]
        // Some responses are Report ID 10 (0x0A) for DSP/surround status
        match response[0] {
            11 if response[2] == 187 => {
                // Standard response format: [11, 0, 187, cmd_id, data...]
                match response[3] {
                    CONNECTION_STATUS_RESPONSE_ID => {
                        let status = response[4];
                        let connected = status == 1 || status == 4;
                        if status == 2 {
                            debug_println!("Pairing mode");
                        }
                        Some(vec![DeviceEvent::WirelessConnected(connected)])
                    }
                    GET_BATTERY_CMD_ID => {
                        // Battery level is at byte 7, not byte 4
                        let level = response[7];
                        Some(vec![DeviceEvent::BatterLevel(level)])
                    }
                    GET_CHARGING_CMD_ID => {
                        let status = response[4];
                        Some(vec![DeviceEvent::Charging(ChargingStatus::from(status))])
                    }
                    MUTE_RESPONSE_ID => {
                        let muted = response[4] == 1;
                        Some(vec![DeviceEvent::Muted(muted)])
                    }
                    FIRMWARE_VERSION_RESPONSE_ID => {
                        debug_println!(
                            "Firmware version: {}.{}.{}.{}",
                            response[4],
                            response[5],
                            response[6],
                            response[7]
                        );
                        None
                    }
                    SET_SIDE_TONE_ON_CMD_ID => {
                        // Response format: [11, 0, 187, 25, status, ...]
                        // where status: 1 = enabled, 0 = disabled
                        let enabled = response[4] == 1;
                        Some(vec![DeviceEvent::SideToneOn(enabled)])
                    }
                    GET_AUTO_SHUTDOWN_CMD_ID => {
                        let minutes = response[4];
                        Some(vec![DeviceEvent::AutomaticShutdownAfter(
                            Duration::from_secs(minutes as u64 * 60),
                        )])
                    }
                    4 => {
                        // Command 4: Charge limit or battery management
                        // This may be sent asynchronously when charging state changes
                        debug_println!(
                            "Charge limit/battery management response (cmd 4): data={:?}",
                            &response[4..8]
                        );
                        None
                    }
                    9 | 29 => {
                        // Commands 9 and 29 are seen during initialization but purpose unclear
                        debug_println!("Initialization response (cmd {})", response[3]);
                        None
                    }
                    _ => {
                        debug_println!("Unknown command response: cmd_id={}", response[3]);
                        None
                    }
                }
            }
            10 => {
                // DSP/Surround sound status response: [10, 0, dsp_status, ...]
                let dsp_status = response[2];
                let surround_enabled = (dsp_status & 2) == 2;
                Some(vec![DeviceEvent::SurroundSound(surround_enabled)])
            }
            _ => {
                debug_println!("Unknown response format: report_id={}", response[0]);
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
        // Attempt to read input report before writing
        // This may not work for all devices (e.g., Cloud Flight S),
        // so we ignore the error
        let mut input_report_buffer = [0u8; 64];
        input_report_buffer[0] = 6;
        let _ = self
            .state
            .hid_device
            .get_input_report(&mut input_report_buffer);
    }

    fn allow_passive_refresh(&mut self) -> bool {
        false
    }

    fn execute_headset_specific_functionality(&mut self) -> Result<(), DeviceError> {
        //TODO: I think this unmutes the headset
        // println!("Writing special sequence");
        // let mut packet = [0u8; 62];
        // packet[0] = 6;
        // packet[2] = 2;
        // packet[4] = 154;
        // packet[7] = 104;
        // packet[8] = 74;
        // packet[9] = 142;
        // packet[10] = 10;
        // packet[14] = 187;
        // packet[15] = 1;
        // self.prepare_write();
        // println!("Writing {:?}", packet);
        // self.state.hid_device.write(&packet)?;
        // std::thread::sleep(Duration::from_millis(200));
        // if let Some(events) = self.wait_for_updates(Duration::from_secs(1)) {
        //     println!("{:?}", events);
        //     for event in events {
        //         self.get_device_state_mut().update_self_with_event(&event);
        //     }
        // }
        // let mut packet = [0u8; 62];
        // packet[0] = 6;
        // packet[2] = 0;
        // packet[4] = u8::MAX;
        // packet[7] = 104;
        // packet[8] = 74;
        // packet[9] = 142;
        // self.prepare_write();
        // println!("Writing {:?}", packet);
        // self.state.hid_device.write(&packet)?;
        // std::thread::sleep(Duration::from_millis(200));
        // if let Some(events) = self.wait_for_updates(Duration::from_secs(1)) {
        //     println!("{:?}", events);
        //     for event in events {
        //         self.get_device_state_mut().update_self_with_event(&event);
        //     }
        // }
        // let mut packet = [0u8; 62];
        // packet[0] = 6;
        // packet[2] = 2;
        // packet[4] = 154;
        // packet[7] = 104;
        // packet[8] = 74;
        // packet[9] = 142;
        // packet[10] = 10;
        // packet[14] = 187;
        // packet[15] = 17;
        // self.prepare_write();
        // println!("Writing {:?}", packet);
        // self.state.hid_device.write(&packet)?;
        // std::thread::sleep(Duration::from_millis(200));
        // if let Some(events) = self.wait_for_updates(Duration::from_secs(1)) {
        //     println!("{:?}", events);
        //     for event in events {
        //         self.get_device_state_mut().update_self_with_event(&event);
        //     }
        // }
        // let mut packet = [0u8; 62];
        // packet[0] = 6;
        // packet[2] = 2;
        // packet[4] = 154;
        // packet[7] = 104;
        // packet[8] = 74;
        // packet[9] = 142;
        // packet[10] = 10;
        // packet[14] = 187;
        // packet[15] = 29;
        // self.prepare_write();
        // println!("Writing {:?}", packet);
        // self.state.hid_device.write(&packet)?;
        // std::thread::sleep(Duration::from_millis(200));
        // if let Some(events) = self.wait_for_updates(Duration::from_secs(1)) {
        //     println!("{:?}", events);
        //     for event in events {
        //         self.get_device_state_mut().update_self_with_event(&event);
        //     }
        // }
        // let mut packet = [0u8; 62];
        // packet[0] = 6;
        // packet[2] = 2;
        // packet[4] = 154;
        // packet[7] = 104;
        // packet[8] = 74;
        // packet[9] = 142;
        // packet[10] = 10;
        // packet[14] = 187;
        // packet[15] = 9;
        // self.prepare_write();
        // println!("Writing {:?}", packet);
        // self.state.hid_device.write(&packet)?;
        // std::thread::sleep(Duration::from_millis(200));
        // if let Some(events) = self.wait_for_updates(Duration::from_secs(1)) {
        //     println!("{:?}", events);
        //     for event in events {
        //         self.get_device_state_mut().update_self_with_event(&event);
        //     }
        // }

        Ok(())
    }
}
