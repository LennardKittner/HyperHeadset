use crate::devices::{ChargingStatus, Color, Device, DeviceError, DeviceEvent, DeviceState};
use std::time::Duration;

const HP: u16 = 0x03F0;
const HYPERX: u16 = 0x0951;
const VENDOR_IDS: [u16; 2] = [HP, HYPERX];
// Possible Cloud II Wireless product IDs
const PRODUCT_IDS: [u16; 4] = [0x1718, 0x018B, 0x0D93, 0x0696];

const BASE_PACKET_HP: [u8; 20] = {
    let mut packet = [0; 20];
    (packet[0], packet[1], packet[2]) = (0x06, 0xff, 0xbb);
    packet
};

const BASE_PACKET_HYPERX: [u8; 62] = {
    let mut packet = [0; 62];
    packet[0] = 0x06;
    packet[2] = 0x02;
    packet[4] = 0x9A;
    packet[7] = 0x68;
    packet[8] = 0x4A;
    packet[9] = 0x8E;
    packet[10] = 0x0A;
    packet[14] = 0xBB;
    packet[15] = 0x01;
    packet
};

#[allow(dead_code)]
const BASE_PACKET2: [u8; 20] = {
    let mut packet = [0; 20];
    (packet[0], packet[1]) = (33, 187);
    packet
};

const GET_CHARGING_CMD_ID: u8 = 3;
const GET_MIC_CONNECTED_CMD_ID: u8 = 8;
const GET_BATTERY_CMD_ID: u8 = 2;
const GET_AUTO_SHUTDOWN_CMD_ID: u8 = 7;
const SET_AUTO_SHUTDOWN_CMD_ID: u8 = 34;
const GET_MUTE_CMD_ID: u8 = 5;
const SET_MUTE_CMD_ID: u8 = 32;
const GET_PAIRING_CMD_ID: u8 = 9;
const GET_PRODUCT_COLOR_CMD_ID: u8 = 14;
const GET_SIDE_TONE_ON_CMD_ID: u8 = 6;
const SET_SIDE_TONE_ON_CMD_ID: u8 = 33;
const GET_SIDE_TONE_VOLUME_CMD_ID: u8 = 11;
const SET_SIDE_TONE_VOLUME_CMD_ID: u8 = 35;
const GET_VOICE_PROMPT_CMD_ID: u8 = 9;
#[allow(dead_code)]
const SET_VOICE_PROMPT_CMD_ID: u8 = 19;
const GET_WIRELESS_STATUS_CMD_ID: u8 = 1;

pub struct CloudIIWirelessDTS {
    state: DeviceState,
    base_packet: &'static [u8],
}

impl CloudIIWirelessDTS {
    pub fn new_from_state(state: DeviceState) -> Self {
        let base_packet = if state.vendor_id == HP {
            BASE_PACKET_HP.as_ref()
        } else {
            BASE_PACKET_HYPERX.as_ref()
        };
        CloudIIWirelessDTS { state, base_packet }
    }

    pub fn new() -> Result<Self, DeviceError> {
        let state = DeviceState::new(&PRODUCT_IDS, &VENDOR_IDS)?;
        let base_packet = if state.vendor_id == HP {
            BASE_PACKET_HP.as_ref()
        } else {
            BASE_PACKET_HYPERX.as_ref()
        };
        Ok(CloudIIWirelessDTS { state, base_packet })
    }
}

impl Device for CloudIIWirelessDTS {
    fn get_charging_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = self.base_packet.to_vec();
        tmp[3] = GET_CHARGING_CMD_ID;
        Some(tmp)
    }

    fn get_battery_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = self.base_packet.to_vec();
        tmp[3] = GET_BATTERY_CMD_ID;
        Some(tmp)
    }

    fn set_automatic_shut_down_packet(&self, shutdown_after: Duration) -> Option<Vec<u8>> {
        let mut tmp = self.base_packet.to_vec();
        tmp[3] = SET_AUTO_SHUTDOWN_CMD_ID;
        tmp[4] = (shutdown_after.as_secs() / 60) as u8;
        Some(tmp)
    }

    fn get_automatic_shut_down_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = self.base_packet.to_vec();
        tmp[3] = GET_AUTO_SHUTDOWN_CMD_ID;
        Some(tmp)
    }

    fn get_mute_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = self.base_packet.to_vec();
        tmp[3] = GET_MUTE_CMD_ID;
        Some(tmp)
    }

    fn set_mute_packet(&self, mute: bool) -> Option<Vec<u8>> {
        let mut tmp = self.base_packet.to_vec();
        tmp[3] = SET_MUTE_CMD_ID;
        tmp[4] = mute as u8;
        Some(tmp)
    }

    fn get_mic_connected_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = self.base_packet.to_vec();
        tmp[3] = GET_MIC_CONNECTED_CMD_ID;
        Some(tmp)
    }

    fn get_pairing_info_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = self.base_packet.to_vec();
        tmp[3] = GET_PAIRING_CMD_ID;
        Some(tmp)
    }

    fn get_product_color_packet(&self) -> Option<Vec<u8>> {
        // let mut tmp = BASE_PACKET2.to_vec();
        // tmp[2] = GET_PRODUCT_COLOR_CMD_ID;
        // Some(tmp)
        // Doesn't work
        None
    }

    fn get_side_tone_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = self.base_packet.to_vec();
        tmp[3] = GET_SIDE_TONE_ON_CMD_ID;
        Some(tmp)
    }

    fn set_side_tone_packet(&self, side_tone_on: bool) -> Option<Vec<u8>> {
        let mut tmp = self.base_packet.to_vec();
        tmp[3] = SET_SIDE_TONE_ON_CMD_ID;
        tmp[4] = side_tone_on as u8;
        Some(tmp)
    }

    fn get_side_tone_volume_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = self.base_packet.to_vec();
        tmp[3] = GET_SIDE_TONE_VOLUME_CMD_ID;
        Some(tmp)
    }

    fn set_side_tone_volume_packet(&self, volume: u8) -> Option<Vec<u8>> {
        let mut tmp = self.base_packet.to_vec();
        tmp[3] = SET_SIDE_TONE_VOLUME_CMD_ID;
        tmp[4] = volume;
        Some(tmp)
    }

    fn get_voice_prompt_packet(&self) -> Option<Vec<u8>> {
        // let mut tmp = BASE_PACKET2.to_vec();
        // tmp[2] = GET_VOICE_PROMPT_CMD_ID;
        // Some(tmp)
        // Doesn't work
        None
    }

    fn set_voice_prompt_packet(&self, _enable: bool) -> Option<Vec<u8>> {
        // let mut tmp = BASE_PACKET2.to_vec();
        // tmp[2] = SET_VOICE_PROMPT_CMD_ID;
        // Some(tmp)
        // Doesn't work
        None
    }

    fn get_wireless_connected_status_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = self.base_packet.to_vec();
        tmp[3] = GET_WIRELESS_STATUS_CMD_ID;
        Some(tmp)
    }

    fn get_event_from_device_response(&self, response: &[u8]) -> Option<DeviceEvent> {
        if response.len() < 7 {
            return None;
        }
        match (response[2], response[3], response[4], response[7]) {
            (_, GET_CHARGING_CMD_ID, status, _) => {
                Some(DeviceEvent::Charging(ChargingStatus::from(status)))
            }
            (_, GET_MIC_CONNECTED_CMD_ID, status, _) => {
                Some(DeviceEvent::MicConnected(status == 1))
            }
            (_, GET_BATTERY_CMD_ID, _, level) => Some(DeviceEvent::BatterLevel(level)),
            (_, GET_AUTO_SHUTDOWN_CMD_ID, time, _) => Some(DeviceEvent::AutomaticShutdownAfter(
                Duration::from_secs(time as u64 * 60),
            )),
            (_, SET_MUTE_CMD_ID, status, _) | (_, GET_MUTE_CMD_ID, status, _) => {
                Some(DeviceEvent::Muted(status == 1))
            }
            (_, GET_PAIRING_CMD_ID, status, _) => Some(DeviceEvent::PairingInfo(status)),
            (_, GET_SIDE_TONE_ON_CMD_ID, status, _) => Some(DeviceEvent::SideToneOn(status == 1)),
            (_, GET_SIDE_TONE_VOLUME_CMD_ID, status, _) => {
                Some(DeviceEvent::SideToneVolume(status))
            }
            (_, GET_WIRELESS_STATUS_CMD_ID, status, _) => {
                Some(DeviceEvent::WirelessConnected(status == 1 || status == 4))
            }
            (GET_VOICE_PROMPT_CMD_ID, status, _, _) => Some(DeviceEvent::VoicePrompt(status == 1)),
            (GET_PRODUCT_COLOR_CMD_ID, status, _, _) => {
                Some(DeviceEvent::ProductColor(Color::from(status)))
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
}
