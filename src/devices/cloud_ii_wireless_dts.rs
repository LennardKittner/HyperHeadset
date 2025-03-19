use crate::devices::{ChargingStatus, Color, Device, DeviceError, DeviceEvent, DeviceState};
use std::time::Duration;

// Possible vendor IDs [HP]
const VENDOR_IDS: [u16; 1] = [0x03F0];
// Possible Cloud II Wireless product IDs
const PRODUCT_IDS: [u16; 4] = [0x1718, 0x018B, 0x0D93, 0x0696];

const BASE_PACKET: [u8; 20] = {
    let mut packet = [0; 20];
    (packet[0], packet[1], packet[2]) = (0x06, 0xff, 0xbb);
    packet
};

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
const SET_VOICE_PROMPT_CMD_ID: u8 = 19;
const GET_WIRELESS_STATUS_CMD_ID: u8 = 1;

pub struct CloudIIWirelessDTS {
    state: DeviceState,
}

impl CloudIIWirelessDTS {
    pub fn new_from_state(state: DeviceState) -> Self {
        CloudIIWirelessDTS { state }
    }

    pub fn new() -> Result<Self, DeviceError> {
        let state = DeviceState::new(&PRODUCT_IDS, &VENDOR_IDS)?;
        Ok(CloudIIWirelessDTS { state })
    }
}

impl Device for CloudIIWirelessDTS {
    fn get_charging_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[3] = GET_CHARGING_CMD_ID;
        Some(tmp)
    }

    fn get_battery_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[3] = GET_BATTERY_CMD_ID;
        Some(tmp)
    }

    fn set_automatic_shut_down_packet(&self, shutdown_after: Duration) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[3] = SET_AUTO_SHUTDOWN_CMD_ID;
        tmp[4] = (shutdown_after.as_secs() / 60) as u8;
        Some(tmp)
    }

    fn get_automatic_shut_down_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[3] = GET_AUTO_SHUTDOWN_CMD_ID;
        Some(tmp)
    }

    fn get_mute_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[3] = GET_MUTE_CMD_ID;
        Some(tmp)
    }

    fn set_mute_packet(&self, mute: bool) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[3] = SET_MUTE_CMD_ID;
        tmp[4] = mute as u8;
        Some(tmp)
    }

    fn get_mic_connected_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[3] = GET_MIC_CONNECTED_CMD_ID;
        Some(tmp)
    }

    fn get_pairing_info_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
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
        let mut tmp = BASE_PACKET.to_vec();
        tmp[3] = GET_SIDE_TONE_ON_CMD_ID;
        Some(tmp)
    }

    fn set_side_tone_packet(&self, side_tone_on: bool) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[3] = SET_SIDE_TONE_ON_CMD_ID;
        tmp[4] = side_tone_on as u8;
        Some(tmp)
    }

    fn get_side_tone_volume_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[3] = GET_SIDE_TONE_VOLUME_CMD_ID;
        Some(tmp)
    }

    fn set_side_tone_volume_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[3] = SET_SIDE_TONE_VOLUME_CMD_ID;
        Some(tmp)
    }

    fn get_voice_prompt_packet(&self) -> Option<Vec<u8>> {
        // let mut tmp = BASE_PACKET2.to_vec();
        // tmp[2] = GET_VOICE_PROMPT_CMD_ID;
        // Some(tmp)
        // Doesn't work
        None
    }

    fn set_voice_prompt_packet(&self, enable: bool) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET2.to_vec();
        tmp[2] = SET_VOICE_PROMPT_CMD_ID;
        Some(tmp)
    }

    fn get_wireless_connected_status_packet(&self) -> Option<Vec<u8>> {
        let mut tmp = BASE_PACKET.to_vec();
        tmp[3] = GET_WIRELESS_STATUS_CMD_ID;
        Some(tmp)
    }

    fn get_event_from_device_response(&self, response: &[u8]) -> Option<DeviceEvent> {
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
            (_, GET_MUTE_CMD_ID, status, _) => Some(DeviceEvent::Muted(status == 1)),
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
            _ => None,
        }
    }

    fn get_device_state(&mut self) -> &mut DeviceState {
        &mut self.state
    }
}

