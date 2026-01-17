use crate::{
    debug_println,
    devices::{Device, DeviceError, DeviceEvent, DeviceState},
};
use std::time::Duration;

const HP: u16 = 0x03F0;
pub const VENDOR_IDS: [u16; 1] = [HP];
pub const PRODUCT_IDS: [u16; 1] = [0x06BE];

// Cloud III S uses a different protocol than Cloud III
// Header 0x05 for mic control, 20-byte packets
const PACKET_SIZE: usize = 20;
const MIC_HEADER: u8 = 0x05;

// Mic control commands (byte 1 after header)
// Pattern: (cmd & 0x02) == 0 means ON
const MIC_ON_CMD: u8 = 0x00;
const MIC_OFF_CMD: u8 = 0x02;

// Auto-shutdown control (via SET_REPORT, report ID 0x0c)
// Packet structure: 0c 02 03 00 00 4a XX 00... (64 bytes total)
// XX values: 00=disabled, 02=10min, 04=20min, 07=30min
const AUTO_SHUTDOWN_REPORT_ID: u8 = 0x0c;
const AUTO_SHUTDOWN_CMD: [u8; 5] = [0x02, 0x03, 0x00, 0x00, 0x4a];
const AUTO_SHUTDOWN_PACKET_SIZE: usize = 64;

// Equalizer control (via SET_REPORT, report ID 0x0c)
// Packet structure: 0c 02 03 00 00 5f [band] [value_hi] [value_lo] 00... (64 bytes total)
// band: 0-9 (32Hz, 64Hz, 125Hz, 250Hz, 500Hz, 1kHz, 2kHz, 4kHz, 8kHz, 16kHz)
// value: signed 16-bit big-endian, dB * 100 (e.g., +6.0dB = 600, -3.5dB = -350)
const EQ_REPORT_ID: u8 = 0x0c;
const EQ_CMD: [u8; 5] = [0x02, 0x03, 0x00, 0x00, 0x5f];
const EQ_PACKET_SIZE: usize = 64;

// Button report header (incoming from headset)
const CONSUMER_CONTROL_HEADER: u8 = 0x0f;
// Consumer control button values
const _VOL_UP: u8 = 0x01;
const _VOL_DOWN: u8 = 0x02;
const _PLAY_PAUSE: u8 = 0x08;

fn make_mic_packet(mute: bool) -> Vec<u8> {
    let mut packet = vec![0u8; PACKET_SIZE];
    packet[0] = MIC_HEADER;
    packet[1] = if mute { MIC_OFF_CMD } else { MIC_ON_CMD };
    packet
}

fn make_auto_shutdown_packet(minutes: u64) -> Vec<u8> {
    let mut packet = vec![0u8; AUTO_SHUTDOWN_PACKET_SIZE];
    packet[0] = AUTO_SHUTDOWN_REPORT_ID;
    packet[1..6].copy_from_slice(&AUTO_SHUTDOWN_CMD);
    // Value is 16-bit big-endian seconds
    let seconds = (minutes * 60) as u16;
    packet[6] = (seconds >> 8) as u8;   // High byte
    packet[7] = (seconds & 0xFF) as u8; // Low byte
    packet
}

fn make_equalizer_band_packet(band_index: u8, db_value: f32) -> Vec<u8> {
    let mut packet = vec![0u8; EQ_PACKET_SIZE];
    packet[0] = EQ_REPORT_ID;
    packet[1..6].copy_from_slice(&EQ_CMD);
    packet[6] = band_index;
    // Convert dB to device units (dB * 100), clamp to ±12dB range
    let value_int = (db_value * 100.0).clamp(-1200.0, 1200.0) as i16;
    let value_bytes = value_int.to_be_bytes();
    packet[7] = value_bytes[0]; // High byte
    packet[8] = value_bytes[1]; // Low byte
    packet
}

pub struct CloudIIISWireless {
    state: DeviceState,
}

impl CloudIIISWireless {
    pub fn new_from_state(state: DeviceState) -> Self {
        CloudIIISWireless { state }
    }

    pub fn new() -> Result<Self, DeviceError> {
        let state = DeviceState::new(&PRODUCT_IDS, &VENDOR_IDS)?;
        Ok(CloudIIISWireless { state })
    }
}

impl Device for CloudIIISWireless {
    // Cloud III S: Battery query not discovered yet
    fn get_charging_packet(&self) -> Option<Vec<u8>> {
        None
    }

    // Cloud III S: Battery query not discovered yet
    fn get_battery_packet(&self) -> Option<Vec<u8>> {
        None
    }

    // Cloud III S: Auto shutdown via SET_REPORT (report ID 0x0c)
    fn set_automatic_shut_down_packet(&self, shutdown_after: Duration) -> Option<Vec<u8>> {
        let minutes = shutdown_after.as_secs() / 60;
        Some(make_auto_shutdown_packet(minutes))
    }

    fn get_automatic_shut_down_packet(&self) -> Option<Vec<u8>> {
        None
    }

    // Cloud III S: Cannot query mic state (no response received)
    // Physical mic button doesn't emit state packets over HID
    fn get_mute_packet(&self) -> Option<Vec<u8>> {
        None
    }

    // Cloud III S: Mic control - CONFIRMED WORKING
    fn set_mute_packet(&self, mute: bool) -> Option<Vec<u8>> {
        Some(make_mic_packet(mute))
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

    // Cloud III S: Sidetone not discovered yet
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

    // Cloud III S: Equalizer control - CONFIRMED WORKING
    fn set_equalizer_band_packet(&self, band_index: u8, db_value: f32) -> Option<Vec<u8>> {
        if band_index > 9 {
            return None;
        }
        Some(make_equalizer_band_packet(band_index, db_value))
    }

    fn get_event_from_device_response(&self, response: &[u8]) -> Option<Vec<DeviceEvent>> {
        debug_println!("Read packet: {response:?}");

        match response[0] {
            MIC_HEADER => {
                // Mic state response
                // Pattern: (byte[1] & 0x02) == 0 means mic is ON (not muted)
                let muted = (response[1] & 0x02) != 0;
                Some(vec![DeviceEvent::Muted(muted)])
            }
            CONSUMER_CONTROL_HEADER => {
                // Button press events - we log but don't need to store state
                debug_println!(
                    "Consumer control event: 0x{:02x}",
                    response.get(1).unwrap_or(&0)
                );
                None
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

    /// Cloud III S has limited status reporting over HID.
    /// We can SET mic mute but cannot query device state.
    /// Override to prevent "No response" errors.
    fn active_refresh_state(&mut self) -> Result<(), DeviceError> {
        // Cloud III S doesn't respond to status queries.
        // Just mark as connected since we successfully opened the device.
        self.state.connected = Some(true);

        // Listen briefly for any incoming events (button presses, etc.)
        let events = self.wait_for_updates(Duration::from_millis(100));
        if let Some(events) = events {
            for event in events {
                self.state.update_self_with_event(&event);
            }
        }

        Ok(())
    }
}
