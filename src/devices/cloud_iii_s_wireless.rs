use crate::{
    debug_println,
    devices::{ChargingStatus, Color, Device, DeviceEvent, DeviceState},
};
use std::time::Duration;

#[cfg(target_os = "linux")]
use crate::devices::{
    DeviceError, DeviceProperties, HidTransport, LibusbTransport,
};
#[cfg(target_os = "linux")]
use rusb::UsbContext;

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

// Battery packet
const BASE_PACKET: [u8; 64] = {
    let mut packet = [0u8; 64];
    packet[0] = 0x0C;
    packet[1] = 0x02;
    packet[2] = 0x03;
    packet[3] = 0x01;
    packet[4] = 0x00;
    packet
};
const RESPONSE_ID: u8 = 0x0C;
const NOTIFICATION_ID: u8 = 0x0D;

const BATTERY_COMMAND_ID: u8 = 0x06;
const DONGLE_CONNECTED_COMMAND_ID: u8 = 0x02;
const COLOR_COMMAND_ID: u8 = 0x4D;
const CHARGE_STATE_COMMAND_ID: u8 = 0x48;
const GET_MIC_MUTE_COMMAND_ID: u8 = 0x04;
const GET_SIDE_TONE_COMMAND_ID: u8 = 0x16;
const GET_AUTO_POWER_OFF_COMMAND_ID: u8 = 0x4B;
const GET_VOICE_PROMPT_COMMAND_ID: u8 = 0x14;

// Button report header (incoming from headset)
const CONSUMER_CONTROL_HEADER: u8 = 0x0f;
// Consumer control button values
const VOL_UP: u8 = 0x01;
const VOL_DOWN: u8 = 0x02;
const PLAY_PAUSE: u8 = 0x08;

// HID interface exposed by the dongle. The dongle has only one HID interface
// (`bInterfaceNumber = 3`) with a single INTERRUPT IN endpoint at `0x84`; all
// writes are SET_REPORT class control transfers.
#[cfg(target_os = "linux")]
const HID_INTERFACE: u8 = 3;
#[cfg(target_os = "linux")]
const HID_EP_IN: u8 = 0x84;

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
    packet[6] = (seconds >> 8) as u8; // High byte
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

fn parse_automatic_shutdown_payload(high: u8, low: u8) -> Duration {
    let num = (high as u64) * 256 + low as u64;
    Duration::from_secs(num)
}

fn parse_response(response: &[u8]) -> Option<Vec<DeviceEvent>> {
    if response[6] == 0xFF {
        return None;
    }
    match response[5] {
        DONGLE_CONNECTED_COMMAND_ID => Some(vec![DeviceEvent::WirelessConnected(response[6] == 2)]),
        GET_MIC_MUTE_COMMAND_ID => Some(vec![DeviceEvent::Muted(response[6] == 1)]),
        BATTERY_COMMAND_ID => Some(vec![DeviceEvent::BatterLevel(response[6])]),
        GET_VOICE_PROMPT_COMMAND_ID => Some(vec![DeviceEvent::VoicePrompt(response[6] == 1)]),
        GET_SIDE_TONE_COMMAND_ID => Some(vec![DeviceEvent::SideToneOn(response[6] == 1)]),
        CHARGE_STATE_COMMAND_ID => Some(vec![DeviceEvent::Charging(ChargingStatus::from(
            response[6],
        ))]),
        GET_AUTO_POWER_OFF_COMMAND_ID => Some(vec![DeviceEvent::AutomaticShutdownAfter(
            parse_automatic_shutdown_payload(response[6], response[7]),
        )]),
        COLOR_COMMAND_ID => Some(vec![DeviceEvent::ProductColor(Color::from(response[6]))]),
        3 | 5 => None,
        _ => {
            debug_println!("Unknown response {:?}", response);
            None
        }
    }
}

fn parse_notification(response: &[u8]) -> Option<Vec<DeviceEvent>> {
    match response[4] {
        1 => Some(vec![DeviceEvent::BatterLevel(response[5])]),
        3 => Some(vec![DeviceEvent::Muted(response[5] == 1)]),
        5 => Some(vec![DeviceEvent::SideToneOn(response[5] == 1)]),
        10 => Some(vec![DeviceEvent::Charging(ChargingStatus::from(
            response[5],
        ))]),
        12 => Some(vec![DeviceEvent::WirelessConnected(response[5] == 1)]),
        _ => None,
    }
}

pub struct CloudIIISWireless {
    state: DeviceState,
}

impl CloudIIISWireless {
    pub fn new_from_state(state: DeviceState) -> Self {
        CloudIIISWireless { state }
    }
}

impl Device for CloudIIISWireless {
    // Use the default `write_hid_report` from the `Device` trait, which calls
    // `hid_device.write()`. The Cloud III S HID interface has no OUT endpoint, so writes
    // become `SET_REPORT` control transfers with report type **Output**, which is what the
    // Ngenuity application uses (and what the device firmware listens for on Report ID 0x0c).
    // The previous override forced `send_feature_report` (type **Feature**), which the device
    // silently ignores — the symptom was that battery / connection / side tone queries never
    // produced a response.

    fn get_charging_packet(&self) -> Option<Vec<u8>> {
        let mut packet = BASE_PACKET.to_vec();
        packet[5] = CHARGE_STATE_COMMAND_ID;
        Some(packet)
    }

    fn get_battery_packet(&self) -> Option<Vec<u8>> {
        let mut packet = BASE_PACKET.to_vec();
        packet[5] = BATTERY_COMMAND_ID;
        Some(packet)
    }

    // Cloud III S: Auto shutdown via SET_REPORT (report ID 0x0c)
    fn set_automatic_shut_down_packet(&self, shutdown_after: Duration) -> Option<Vec<u8>> {
        let minutes = shutdown_after.as_secs() / 60;
        Some(make_auto_shutdown_packet(minutes))
    }

    fn get_automatic_shut_down_packet(&self) -> Option<Vec<u8>> {
        let mut packet = BASE_PACKET.to_vec();
        packet[5] = GET_AUTO_POWER_OFF_COMMAND_ID;
        Some(packet)
    }

    fn get_mute_packet(&self) -> Option<Vec<u8>> {
        let mut packet = BASE_PACKET.to_vec();
        packet[5] = GET_MIC_MUTE_COMMAND_ID;
        Some(packet)
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
        let mut packet = BASE_PACKET.to_vec();
        packet[5] = COLOR_COMMAND_ID;
        Some(packet)
    }

    fn get_side_tone_packet(&self) -> Option<Vec<u8>> {
        let mut packet = BASE_PACKET.to_vec();
        packet[5] = GET_SIDE_TONE_COMMAND_ID;
        Some(packet)
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
        let mut packet = BASE_PACKET.to_vec();
        packet[5] = GET_VOICE_PROMPT_COMMAND_ID;
        Some(packet)
    }

    fn set_voice_prompt_packet(&self, _enable: bool) -> Option<Vec<u8>> {
        None
    }

    fn get_wireless_connected_status_packet(&self) -> Option<Vec<u8>> {
        let mut packet = BASE_PACKET.to_vec();
        packet[5] = DONGLE_CONNECTED_COMMAND_ID;
        Some(packet)
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
            RESPONSE_ID => parse_response(response),
            NOTIFICATION_ID => parse_notification(response),
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

/// Open the dongle via libusb instead of hidapi/hidraw.
///
/// On Linux, once the kernel hidraw driver has touched this dongle's HID
/// interface, RF-forwarded query responses (battery, charge, side tone, etc.)
/// silently stop reaching user space. Going via libusb — detach the kernel
/// driver, claim the interface exclusively, drive raw SET_REPORT control
/// transfers + INT IN reads ourselves — sidesteps the issue and gives reliable
/// bidirectional traffic.
///
/// Linux-only: the firmware quirk has only been observed on Linux's hidraw
/// stack, and the kernel-driver-detach pattern is meaningful only there.
#[cfg(target_os = "linux")]
pub fn open_via_libusb() -> Result<Option<DeviceState>, DeviceError> {
    let ctx = match rusb::Context::new() {
        Ok(c) => c,
        Err(_e) => {
            debug_println!("libusb Context::new failed: {_e}");
            return Ok(None);
        }
    };
    let devices = match ctx.devices() {
        Ok(d) => d,
        Err(_e) => {
            debug_println!("libusb devices() failed: {_e}");
            return Ok(None);
        }
    };
    let want_vid = VENDOR_IDS[0];
    let want_pid = PRODUCT_IDS[0];
    let dev = devices.iter().find(|d| {
        d.device_descriptor()
            .map(|desc| desc.vendor_id() == want_vid && desc.product_id() == want_pid)
            .unwrap_or(false)
    });
    let Some(dev) = dev else {
        return Ok(None);
    };
    let handle = match dev.open() {
        Ok(h) => h,
        Err(_e) => {
            debug_println!("libusb open failed: {_e}");
            return Ok(None);
        }
    };

    if handle
        .kernel_driver_active(HID_INTERFACE)
        .unwrap_or(false)
    {
        if let Err(_e) = handle.detach_kernel_driver(HID_INTERFACE) {
            debug_println!("detach_kernel_driver: {_e}");
        }
    }
    if let Err(_e) = handle.claim_interface(HID_INTERFACE) {
        debug_println!("claim_interface: {_e}");
        let _ = handle.attach_kernel_driver(HID_INTERFACE);
        return Ok(None);
    }

    let product_string = handle
        .read_product_string_ascii(
            &dev.device_descriptor()
                .map_err(|_| DeviceError::NoDeviceFound())?,
        )
        .ok();

    // Forward Consumer Control reports (vol-up / vol-down / play-pause) back
    // to the OS via synthesized media key presses. With the kernel HID driver
    // detached, the input subsystem no longer sees these events on its own.
    //
    // Caveat: on Wayland this depends on the compositor accepting input
    // emulation through libei (`xdg-desktop-portal-*`). On compositors that
    // don't, the synthetic events are silently dropped and the headset's
    // volume / play-pause buttons stay inert while this program runs. Works
    // out of the box on X11.
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    let mut enigo = match Enigo::new(&Settings::default()) {
        Ok(e) => Some(e),
        Err(e) => {
            eprintln!("Cloud III S: input forwarding disabled (enigo init: {e})");
            None
        }
    };
    let packet_hook = Box::new(move |buf: &[u8]| {
        if buf.len() >= 2 && buf[0] == CONSUMER_CONTROL_HEADER {
            let key = match buf[1] {
                VOL_UP => Some(Key::VolumeUp),
                VOL_DOWN => Some(Key::VolumeDown),
                PLAY_PAUSE => Some(Key::MediaPlayPause),
                _ => None,
            };
            if let (Some(k), Some(e)) = (key, enigo.as_mut()) {
                let _ = e.key(k, Direction::Click);
            }
        }
    });

    let transport =
        LibusbTransport::open(handle, HID_INTERFACE, HID_EP_IN, product_string.clone(), packet_hook);
    Ok(Some(DeviceState {
        transport: HidTransport::Libusb(transport),
        device_properties: DeviceProperties::new(want_pid, want_vid, product_string),
    }))
}
