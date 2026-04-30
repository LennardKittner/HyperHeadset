pub mod cloud_alpha_wireless;
pub mod cloud_ii_core_wireless;
pub mod cloud_ii_wireless;
pub mod cloud_ii_wireless_dts;
pub mod cloud_iii_s_wireless;
pub mod cloud_iii_wireless;

use crate::{
    debug_println,
    devices::{
        cloud_alpha_wireless::CloudAlphaWireless, cloud_ii_core_wireless::CloudIICoreWireless,
        cloud_ii_wireless::CloudIIWireless, cloud_ii_wireless_dts::CloudIIWirelessDTS,
        cloud_iii_s_wireless::CloudIIISWireless, cloud_iii_wireless::CloudIIIWireless,
    },
};
use hidapi::{HidApi, HidDevice, HidError};
use rusb::UsbContext;
use std::{
    collections::HashSet,
    fmt::{Debug, Display},
    time::Duration,
};
use thistermination::TerminationFull;

const PASSIVE_REFRESH_TIME_OUT: Duration = Duration::from_secs(2);

/// HID interface exposed by the Cloud III S Wireless dongle. The dongle has
/// only one HID interface (`bInterfaceNumber = 3`) with a single INTERRUPT IN
/// endpoint at `0x84`; all writes are SET_REPORT class control transfers.
const CLOUD_III_S_HID_INTERFACE: u8 = 3;
const CLOUD_III_S_HID_EP_IN: u8 = 0x84;

fn open_cloud_iii_s_via_libusb() -> Result<Option<DeviceState>, DeviceError> {
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
    let want_vid = cloud_iii_s_wireless::VENDOR_IDS[0];
    let want_pid = cloud_iii_s_wireless::PRODUCT_IDS[0];
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
            debug_println!("Cloud III S: libusb open failed: {_e}");
            return Ok(None);
        }
    };

    // Best-effort detach the kernel HID driver so we can claim the interface
    // exclusively. On non-Linux this is a no-op.
    if handle
        .kernel_driver_active(CLOUD_III_S_HID_INTERFACE)
        .unwrap_or(false)
    {
        if let Err(_e) = handle.detach_kernel_driver(CLOUD_III_S_HID_INTERFACE) {
            debug_println!("Cloud III S: detach_kernel_driver: {_e}");
        }
    }
    if let Err(_e) = handle.claim_interface(CLOUD_III_S_HID_INTERFACE) {
        debug_println!("Cloud III S: claim_interface: {_e}");
        let _ = handle.attach_kernel_driver(CLOUD_III_S_HID_INTERFACE);
        return Ok(None);
    }

    let product_string = handle
        .read_product_string_ascii(&dev.device_descriptor().map_err(|_| DeviceError::NoDeviceFound())?)
        .ok();

    let handle = std::sync::Arc::new(handle);
    let rx = std::sync::Arc::new((
        std::sync::Mutex::new(std::collections::VecDeque::new()),
        std::sync::Condvar::new(),
    ));
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    // Background reader: continuously poll EP IN with a short timeout. Push every
    // received packet into the shared queue so callers see responses even when the
    // surrounding flow does a `write → sleep → read` (active_refresh_state).
    let reader = {
        let handle = std::sync::Arc::clone(&handle);
        let rx = std::sync::Arc::clone(&rx);
        let stop = std::sync::Arc::clone(&stop);
        std::thread::spawn(move || {
            // Forwarding Consumer Control events (Report ID 0x0f, vol-up / vol-down /
            // play-pause buttons on the headset) back to the OS. With the kernel HID
            // driver detached, the input subsystem no longer sees them, so we
            // synthesize the corresponding media key presses with `enigo`.
            //
            // Caveat: on Linux/Wayland this depends on the compositor accepting input
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
            let mut buf = [0u8; 64];
            while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                match handle.read_interrupt(
                    CLOUD_III_S_HID_EP_IN,
                    &mut buf,
                    Duration::from_millis(100),
                ) {
                    Ok(n) if n > 0 => {
                        if n >= 2 && buf[0] == 0x0f {
                            let key = match buf[1] {
                                0x01 => Some(Key::VolumeUp),
                                0x02 => Some(Key::VolumeDown),
                                0x08 => Some(Key::MediaPlayPause),
                                _ => None,
                            };
                            if let (Some(k), Some(e)) = (key, enigo.as_mut()) {
                                let _ = e.key(k, Direction::Click);
                            }
                        }
                        let (lock, cvar) = &*rx;
                        let mut q = lock.lock().unwrap();
                        q.push_back(buf[..n].to_vec());
                        cvar.notify_one();
                    }
                    Ok(_) => {}
                    Err(rusb::Error::Timeout) => {}
                    Err(rusb::Error::NoDevice) | Err(rusb::Error::Pipe) => break,
                    Err(_) => {}
                }
            }
        })
    };

    let transport = LibusbTransport {
        handle,
        interface: CLOUD_III_S_HID_INTERFACE,
        ep_in: CLOUD_III_S_HID_EP_IN,
        product_string: product_string.clone(),
        rx,
        stop,
        reader: Some(reader),
    };
    Ok(Some(DeviceState {
        transport: HidTransport::Libusb(transport),
        device_properties: DeviceProperties::new(want_pid, want_vid, product_string),
    }))
}

type DeviceFactory = fn(DeviceState) -> Box<dyn Device>;

struct DeviceEntry {
    vendor_ids: &'static [u16],
    product_ids: &'static [u16],
    factory: DeviceFactory,
}

const DEVICE_REGISTER: &[DeviceEntry] = &[
    DeviceEntry {
        vendor_ids: &cloud_ii_wireless::VENDOR_IDS,
        product_ids: &cloud_ii_wireless::PRODUCT_IDS,
        factory: |s| Box::new(CloudIIWireless::new_from_state(s)),
    },
    DeviceEntry {
        vendor_ids: &cloud_ii_wireless_dts::VENDOR_IDS,
        product_ids: &cloud_ii_wireless_dts::PRODUCT_IDS,
        factory: |s| Box::new(CloudIIWirelessDTS::new_from_state(s)),
    },
    DeviceEntry {
        vendor_ids: &cloud_iii_s_wireless::VENDOR_IDS,
        product_ids: &cloud_iii_s_wireless::PRODUCT_IDS,
        factory: |s| Box::new(CloudIIISWireless::new_from_state(s)),
    },
    DeviceEntry {
        vendor_ids: &cloud_iii_wireless::VENDOR_IDS,
        product_ids: &cloud_iii_wireless::PRODUCT_IDS,
        factory: |s| Box::new(CloudIIIWireless::new_from_state(s)),
    },
    DeviceEntry {
        vendor_ids: &cloud_alpha_wireless::VENDOR_IDS,
        product_ids: &cloud_alpha_wireless::PRODUCT_IDS,
        factory: |s| Box::new(CloudAlphaWireless::new_from_state(s)),
    },
    DeviceEntry {
        vendor_ids: &cloud_ii_core_wireless::VENDOR_IDS,
        product_ids: &cloud_ii_core_wireless::PRODUCT_IDS,
        factory: |s| Box::new(CloudIICoreWireless::new_from_state(s)),
    },
];

const RESPONSE_BUFFER_SIZE: usize = 256;
pub const RESPONSE_DELAY: Duration = Duration::from_millis(50);

pub fn connect_compatible_device() -> Result<Box<dyn Device>, DeviceError> {
    let all_product_ids: Vec<u16> = DEVICE_REGISTER
        .iter()
        .flat_map(|e| e.product_ids.iter().copied())
        .collect();
    let all_vendor_ids: Vec<u16> = DEVICE_REGISTER
        .iter()
        .flat_map(|e| e.vendor_ids.iter().copied())
        .collect();
    let states = DeviceState::new(&all_product_ids, &all_vendor_ids)?;
    debug_println!("Found device selecting handler");

    // On Linux and MacOS we can just take the first
    // #[cfg(not(target_os = "windows"))]
    // {
    //     let state = states
    //         .into_iter()
    //         .next()
    //         .ok_or(DeviceError::NoDeviceFound())?;
    //     println!(
    //         "Connecting to {}",
    //         state
    //             .device_properties
    //             .device_name
    //             .clone()
    //             .unwrap_or("???".to_string())
    //     );
    //     let entry = DEVICE_REGISTER
    //         .iter()
    //         .find(|e| {
    //             e.vendor_ids.contains(&state.device_properties.vendor_id)
    //                 && e.product_ids.contains(&state.device_properties.product_id)
    //         })
    //         .ok_or(DeviceError::NoDeviceFound())?;
    //
    //     let mut device = (entry.factory)(state);
    //     device.init_capabilities();
    //     Ok(device)
    // }
    // On Windows we have to check which interface can be used
    // #[cfg(target_os = "windows")]
    {
        let mut device = None;
        for state in states {
            println!(
                "Try to connecting to {}",
                state
                    .device_properties
                    .device_name
                    .clone()
                    .unwrap_or("???".to_string())
            );
            let entry = DEVICE_REGISTER
                .iter()
                .find(|e| {
                    e.vendor_ids.contains(&state.device_properties.vendor_id)
                        && e.product_ids.contains(&state.device_properties.product_id)
                })
                .ok_or(DeviceError::NoDeviceFound())?;

            let mut test_device = (entry.factory)(state);
            test_device.init_capabilities();

            // Use the wireless-connected query as the probe: it is the cheapest one
            // and the one most consistently answered by the dongle from its local cache
            // (no RF round-trip to the headset required). Some devices (e.g. Cloud III S
            // Wireless) won't answer the battery query when the headset isn't actively
            // using the audio link, so probing on battery would falsely fail.
            let probe_packet = test_device
                .get_query_packets()
                .into_iter()
                .next()
                .expect("Why is there a device without packets ???");

            test_device.prepare_write();
            if let Err(_e) = test_device.write_hid_report(&probe_packet) {
                debug_println!("Failed to open: {_e:?}");
                continue;
            } else {
                std::thread::sleep(RESPONSE_DELAY);

                if let Some(events) = test_device.wait_for_updates(Duration::from_secs(1)) {
                    for _event in events {
                        debug_println!("got response {_event:?}");
                    }
                } else {
                    continue;
                }

                device = Some(test_device);
                break;
            }
        }
        device.ok_or(DeviceError::NoDeviceFound())
    }
}

#[derive(Debug)]
pub struct DeviceState {
    pub transport: HidTransport,
    pub device_properties: DeviceProperties,
}

/// HID transport abstraction. Most devices use `Hidapi` (the `hidapi-rs` library
/// which talks to `/dev/hidrawN` on Linux). The `Libusb` variant is for devices
/// that need direct USB access — specifically the Cloud III S Wireless on
/// firmware ≥ 0x4118, where the kernel hidraw stack causes RF-forwarded query
/// responses to be lost. With `Libusb` we detach the kernel driver, claim the
/// HID interface exclusively, and issue raw SET_REPORT class control transfers
/// + INT IN reads ourselves.
pub enum HidTransport {
    Hidapi(HidDevice),
    Libusb(LibusbTransport),
}

pub struct LibusbTransport {
    handle: std::sync::Arc<rusb::DeviceHandle<rusb::Context>>,
    interface: u8,
    ep_in: u8,
    product_string: Option<String>,
    rx: std::sync::Arc<(std::sync::Mutex<std::collections::VecDeque<Vec<u8>>>, std::sync::Condvar)>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    reader: Option<std::thread::JoinHandle<()>>,
}

impl Debug for HidTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HidTransport::Hidapi(_) => f.write_str("HidTransport::Hidapi(..)"),
            HidTransport::Libusb(t) => f
                .debug_struct("HidTransport::Libusb")
                .field("interface", &t.interface)
                .field("ep_in", &format_args!("0x{:02x}", t.ep_in))
                .finish(),
        }
    }
}

const HID_REQ_GET_REPORT: u8 = 0x01;
const HID_REQ_SET_REPORT: u8 = 0x09;
const HID_REPORT_TYPE_INPUT: u16 = 0x01;
const HID_REPORT_TYPE_OUTPUT: u16 = 0x02;
const HID_REPORT_TYPE_FEATURE: u16 = 0x03;

impl HidTransport {
    pub fn write(&self, data: &[u8]) -> Result<usize, HidError> {
        match self {
            HidTransport::Hidapi(d) => d.write(data),
            HidTransport::Libusb(t) => t.set_report(HID_REPORT_TYPE_OUTPUT, data),
        }
    }

    pub fn read_timeout(&self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, HidError> {
        match self {
            HidTransport::Hidapi(d) => d.read_timeout(buf, timeout_ms),
            HidTransport::Libusb(t) => t.read_interrupt(buf, timeout_ms),
        }
    }

    pub fn send_feature_report(&self, data: &[u8]) -> Result<(), HidError> {
        match self {
            HidTransport::Hidapi(d) => d.send_feature_report(data),
            HidTransport::Libusb(t) => t.set_report(HID_REPORT_TYPE_FEATURE, data).map(|_| ()),
        }
    }

    pub fn get_input_report(&self, buf: &mut [u8]) -> Result<usize, HidError> {
        match self {
            HidTransport::Hidapi(d) => d.get_input_report(buf),
            HidTransport::Libusb(t) => t.get_report(HID_REPORT_TYPE_INPUT, buf),
        }
    }

    pub fn product_string(&self) -> Option<String> {
        match self {
            HidTransport::Hidapi(d) => d.get_product_string().ok().flatten(),
            HidTransport::Libusb(t) => t.product_string.clone(),
        }
    }
}

fn rusb_to_hid(err: rusb::Error) -> HidError {
    HidError::HidApiError {
        message: format!("libusb: {err}"),
    }
}

impl LibusbTransport {
    fn set_report(&self, report_type: u16, data: &[u8]) -> Result<usize, HidError> {
        if data.is_empty() {
            return Err(HidError::InvalidZeroSizeData);
        }
        let report_id = data[0];
        let request_type = rusb::request_type(
            rusb::Direction::Out,
            rusb::RequestType::Class,
            rusb::Recipient::Interface,
        );
        let wvalue = (report_type << 8) | (report_id as u16);
        self.handle
            .write_control(
                request_type,
                HID_REQ_SET_REPORT,
                wvalue,
                self.interface as u16,
                data,
                Duration::from_millis(500),
            )
            .map_err(rusb_to_hid)
    }

    fn get_report(&self, report_type: u16, buf: &mut [u8]) -> Result<usize, HidError> {
        if buf.is_empty() {
            return Err(HidError::InvalidZeroSizeData);
        }
        let report_id = buf[0];
        let request_type = rusb::request_type(
            rusb::Direction::In,
            rusb::RequestType::Class,
            rusb::Recipient::Interface,
        );
        let wvalue = (report_type << 8) | (report_id as u16);
        self.handle
            .read_control(
                request_type,
                HID_REQ_GET_REPORT,
                wvalue,
                self.interface as u16,
                buf,
                Duration::from_millis(500),
            )
            .map_err(rusb_to_hid)
    }

    fn read_interrupt(&self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, HidError> {
        // Pop from the background-reader queue. This decouples USB polling from
        // user calls so a `write` followed by a `sleep` followed by a `read` still
        // captures any response that arrived during the sleep.
        let timeout = if timeout_ms < 0 {
            Duration::from_secs(60 * 60)
        } else {
            Duration::from_millis(timeout_ms as u64)
        };
        let (lock, cvar) = &*self.rx;
        let deadline = std::time::Instant::now() + timeout;
        let mut q = lock.lock().unwrap();
        while q.is_empty() {
            let now = std::time::Instant::now();
            if now >= deadline {
                return Ok(0);
            }
            let (g, _) = cvar.wait_timeout(q, deadline - now).unwrap();
            q = g;
        }
        let pkt = q.pop_front().unwrap();
        let n = pkt.len().min(buf.len());
        buf[..n].copy_from_slice(&pkt[..n]);
        Ok(n)
    }
}

impl Drop for LibusbTransport {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(t) = self.reader.take() {
            let _ = t.join();
        }
        let _ = self.handle.release_interface(self.interface);
        let _ = self.handle.attach_kernel_driver(self.interface);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceProperties {
    pub product_id: u16,
    pub vendor_id: u16,
    pub device_name: Option<String>,
    pub battery_level: Option<u8>,
    pub charging: Option<ChargingStatus>,
    pub muted: Option<bool>,
    pub mic_connected: Option<bool>,
    pub automatic_shutdown_after: Option<Duration>,
    pub pairing_info: Option<u8>,
    pub product_color: Option<Color>,
    pub side_tone_on: Option<bool>,
    pub side_tone_volume: Option<u8>,
    pub surround_sound: Option<bool>,
    pub voice_prompt_on: Option<bool>,
    pub connected: Option<bool>,
    pub silent: Option<bool>,
    pub noise_gate_active: Option<bool>,
    // Capability flags - set once during device initialization
    pub can_set_mute: bool,
    pub can_set_surround_sound: bool,
    pub can_set_side_tone: bool,
    pub can_set_automatic_shutdown: bool,
    pub can_set_side_tone_volume: bool,
    pub can_set_voice_prompt: bool,
    pub can_set_silent_mode: bool,
    pub can_set_equalizer: bool,
    pub can_set_noise_gate: bool,
}

impl Display for DeviceProperties {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string_with_readonly_info(25))
    }
}

impl DeviceState {
    pub fn new(product_ids: &[u16], vendor_ids: &[u16]) -> Result<Vec<Self>, DeviceError> {
        // Cloud III S Wireless: open via libusb FIRST, before any hidapi/hidraw
        // enumeration. The kernel hidraw driver, once it has touched the dongle's
        // HID interface, leaves the INT IN URB queue in a state where responses
        // never reach a subsequent libusb claim. Doing libusb first (and keeping
        // it claimed) sidesteps the race.
        let cloud_iii_s_state = if vendor_ids.contains(&cloud_iii_s_wireless::VENDOR_IDS[0])
            && product_ids.contains(&cloud_iii_s_wireless::PRODUCT_IDS[0])
        {
            open_cloud_iii_s_via_libusb()?
        } else {
            None
        };

        let hid_api = HidApi::new()?;
        let mut potential_devices = HashSet::new();
        let mut error = Ok(());
        debug_println!(
            "Devices: {:?}",
            hid_api
                .device_list()
                .by_ref()
                .map(|d| { (d.vendor_id(), d.product_id(), d.product_string()) })
                .collect::<Vec<(u16, u16, Option<&str>)>>()
        );
        let device_candidates: Vec<(HidDevice, u16, u16)> = hid_api
            .device_list()
            .filter_map(|info| {
                // Skip Cloud III S: opened via libusb separately below. Letting
                // hidapi-hidraw open `/dev/hidraw*` here would race with our
                // detach_kernel_driver/claim_interface call.
                if info.vendor_id() == cloud_iii_s_wireless::VENDOR_IDS[0]
                    && info.product_id() == cloud_iii_s_wireless::PRODUCT_IDS[0]
                {
                    return None;
                }
                if product_ids.contains(&info.product_id())
                    && vendor_ids.contains(&info.vendor_id())
                {
                    debug_println!(
                        "Selecting: {:x}:{:x} {:?}",
                        info.vendor_id(),
                        info.product_id(),
                        info.product_string()
                    );
                    match info.open_device(&hid_api) {
                        Ok(device) => Some((device, info.product_id(), info.vendor_id())),
                        Err(e) => {
                            debug_println!(
                                "Failed to open: {:x}:{:x} {:?}: {:?}",
                                info.vendor_id(),
                                info.product_id(),
                                info.product_string(),
                                e
                            );
                            error = Err(e);
                            None
                        }
                    }
                } else {
                    if let Some(name) = info.product_string() {
                        if name.contains("HyperX") {
                            potential_devices.insert((
                                info.vendor_id(),
                                info.product_id(),
                                info.product_string(),
                            ));
                        }
                    }
                    None
                }
            })
            .collect();

        if device_candidates.is_empty() && cloud_iii_s_state.is_none() {
            if !potential_devices.is_empty() {
                let names = potential_devices
                    .iter()
                    .map(|e| {
                        format!(
                            "    vendorID: 0x{:04X} productID: 0x{:04X} name: {}",
                            e.0,
                            e.1,
                            e.2.unwrap_or("Unknown")
                        )
                    })
                    .collect::<Vec<String>>()
                    .join(",\n");
                //TODO: show as message in tray app
                println!(
                    "Found the following HyperX device{}: [\n{}\n]\nHowever, either {} not supported or the product ID is not yet known.",
                    if potential_devices.len() > 1 { "s" } else { "" }, names, if potential_devices.len() > 1 { "they are" } else { "it is" }
                );
            }
            error?;
            return Err(DeviceError::NoDeviceFound());
        }

        let mut states: Vec<DeviceState> = device_candidates
            .into_iter()
            .map(|(hid_device, product_id, vendor_id)| {
                let device_name = hid_device.get_product_string().ok().flatten();
                DeviceState {
                    transport: HidTransport::Hidapi(hid_device),
                    device_properties: DeviceProperties::new(product_id, vendor_id, device_name),
                }
            })
            .collect();
        if let Some(state) = cloud_iii_s_state {
            states.insert(0, state);
        }
        Ok(states)
    }

    fn update_self_with_event(&mut self, event: &DeviceEvent) {
        match event {
            DeviceEvent::BatterLevel(level) => self.device_properties.battery_level = Some(*level),
            DeviceEvent::Charging(status) => self.device_properties.charging = Some(*status),
            DeviceEvent::Muted(status) => self.device_properties.muted = Some(*status),
            DeviceEvent::MicConnected(status) => {
                self.device_properties.mic_connected = Some(*status)
            }
            DeviceEvent::AutomaticShutdownAfter(duration) => {
                self.device_properties.automatic_shutdown_after = Some(*duration)
            }
            DeviceEvent::PairingInfo(info) => self.device_properties.pairing_info = Some(*info),
            DeviceEvent::ProductColor(color) => self.device_properties.product_color = Some(*color),
            DeviceEvent::SideToneOn(side) => self.device_properties.side_tone_on = Some(*side),
            DeviceEvent::SideToneVolume(volume) => {
                self.device_properties.side_tone_volume = Some(*volume)
            }
            DeviceEvent::SurroundSound(status) => {
                self.device_properties.surround_sound = Some(*status)
            }
            DeviceEvent::VoicePrompt(on) => self.device_properties.voice_prompt_on = Some(*on),
            DeviceEvent::WirelessConnected(connected) => {
                self.device_properties.connected = Some(*connected)
            }
            DeviceEvent::Silent(silent) => self.device_properties.silent = Some(*silent),
            DeviceEvent::RequireSIRKReset(_reset) => {
                debug_println!("requested SIRK reset {_reset}");
            }
            DeviceEvent::NoiseGateActive(on) => {
                self.device_properties.noise_gate_active = Some(*on)
            }
        };
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum PropertyType {
    ReadOnly,
    AlwaysReadOnly,
    ReadWrite,
}

#[derive(Debug)]
pub enum PropertyDescriptorWrapper {
    Int(PropertyDescriptor<u8>, &'static [u8]),
    Bool(PropertyDescriptor<bool>),
    String(PropertyDescriptor<String>),
}

pub struct PropertyDescriptor<T: 'static> {
    pub prefix: &'static str,
    pub data: Option<T>,
    pub suffix: &'static str,
    pub property_type: PropertyType,
    pub create_event: &'static (dyn Fn(T) -> Option<DeviceEvent> + Send + Sync),
}

impl<T: Debug> Debug for PropertyDescriptor<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PropertyDescriptor")
            .field("prefix", &self.prefix)
            .field("data", &self.data)
            .field("suffix", &self.suffix)
            .field("property_type", &self.property_type)
            .finish()
    }
}

impl DeviceProperties {
    pub fn new(product_id: u16, vendor_id: u16, device_name: Option<String>) -> DeviceProperties {
        DeviceProperties {
            product_id,
            vendor_id,
            device_name,
            battery_level: None,
            charging: None,
            muted: None,
            mic_connected: None,
            automatic_shutdown_after: None,
            pairing_info: None,
            product_color: None,
            side_tone_on: None,
            side_tone_volume: None,
            surround_sound: None,
            voice_prompt_on: None,
            connected: None,
            silent: None,
            noise_gate_active: None,
            can_set_mute: false,
            can_set_surround_sound: false,
            can_set_side_tone: false,
            can_set_automatic_shutdown: false,
            can_set_side_tone_volume: false,
            can_set_voice_prompt: false,
            can_set_silent_mode: false,
            can_set_equalizer: false,
            can_set_noise_gate: false,
        }
    }

    pub fn get_properties(&self) -> Vec<PropertyDescriptorWrapper> {
        vec![
            PropertyDescriptorWrapper::String(PropertyDescriptor {
                prefix: "Charging status:",
                data: self.charging.map(|c| c.to_string()),
                suffix: "",
                property_type: PropertyType::AlwaysReadOnly,
                create_event: &|_| None,
            }),
            PropertyDescriptorWrapper::Int(
                PropertyDescriptor {
                    prefix: "Battery level:",
                    data: self.battery_level,
                    suffix: "%",
                    property_type: PropertyType::AlwaysReadOnly,
                    create_event: &|_| None,
                },
                &[],
            ),
            PropertyDescriptorWrapper::Bool(PropertyDescriptor {
                prefix: "Muted:",
                data: self.muted,
                suffix: "",
                property_type: if self.can_set_mute {
                    PropertyType::ReadWrite
                } else {
                    PropertyType::ReadOnly
                },
                create_event: &move |mute| Some(DeviceEvent::Muted(mute)),
            }),
            PropertyDescriptorWrapper::Bool(PropertyDescriptor {
                prefix: "Mic connected:",
                data: self.mic_connected,
                suffix: "",
                property_type: PropertyType::AlwaysReadOnly,
                create_event: &|_| None,
            }),
            PropertyDescriptorWrapper::Int(
                PropertyDescriptor {
                    prefix: "Automatic shutdown after:",
                    data: self
                        .automatic_shutdown_after
                        .map(|t| (t.as_secs() / 60) as u8),
                    suffix: "min",
                    property_type: if self.can_set_mute {
                        PropertyType::ReadWrite
                    } else {
                        PropertyType::ReadOnly
                    },
                    create_event: &|t| {
                        Some(DeviceEvent::AutomaticShutdownAfter(Duration::from_secs(
                            t as u64 * 60,
                        )))
                    },
                },
                &[0, 5, 10, 15, 20, 30, 40, 60],
            ),
            PropertyDescriptorWrapper::Int(
                PropertyDescriptor {
                    prefix: "Pairing info:",
                    data: self.pairing_info,
                    suffix: "",
                    property_type: PropertyType::AlwaysReadOnly,
                    create_event: &|_| None,
                },
                &[],
            ),
            PropertyDescriptorWrapper::String(PropertyDescriptor {
                prefix: "Product color:",
                data: self.product_color.map(|c| c.to_string()),
                suffix: "",
                property_type: PropertyType::AlwaysReadOnly,
                create_event: &|_| None,
            }),
            PropertyDescriptorWrapper::Bool(PropertyDescriptor {
                prefix: "Side tone:",
                data: self.side_tone_on,
                suffix: "",
                property_type: if self.can_set_side_tone {
                    PropertyType::ReadWrite
                } else {
                    PropertyType::ReadOnly
                },
                create_event: &move |enable| Some(DeviceEvent::SideToneOn(enable)),
            }),
            PropertyDescriptorWrapper::Int(
                PropertyDescriptor {
                    prefix: "Side tone volume:",
                    data: self.side_tone_volume,
                    suffix: "",
                    property_type: if self.can_set_side_tone_volume {
                        PropertyType::ReadWrite
                    } else {
                        PropertyType::ReadOnly
                    },
                    create_event: &|v| Some(DeviceEvent::SideToneVolume(v)),
                },
                &[0, 25, 50, 75, 100, 125, 150, 175, 200, 225, 250],
            ),
            PropertyDescriptorWrapper::Bool(PropertyDescriptor {
                prefix: "Surround sound:",
                data: self.surround_sound,
                suffix: "",
                property_type: if self.can_set_surround_sound {
                    PropertyType::ReadWrite
                } else {
                    PropertyType::ReadOnly
                },
                create_event: &move |enable| Some(DeviceEvent::SurroundSound(enable)),
            }),
            PropertyDescriptorWrapper::Bool(PropertyDescriptor {
                prefix: "Voice prompt:",
                data: self.voice_prompt_on,
                suffix: "",
                property_type: if self.can_set_voice_prompt {
                    PropertyType::ReadWrite
                } else {
                    PropertyType::ReadOnly
                },
                create_event: &move |enable| Some(DeviceEvent::VoicePrompt(enable)),
            }),
            PropertyDescriptorWrapper::Bool(PropertyDescriptor {
                prefix: "Playback muted:",
                data: self.silent,
                suffix: "",
                property_type: if self.can_set_silent_mode {
                    PropertyType::ReadWrite
                } else {
                    PropertyType::ReadOnly
                },
                create_event: &move |enable| Some(DeviceEvent::Silent(enable)),
            }),
            PropertyDescriptorWrapper::Bool(PropertyDescriptor {
                prefix: "Noise gate active:",
                data: self.noise_gate_active,
                suffix: "",
                property_type: if self.can_set_noise_gate {
                    PropertyType::ReadWrite
                } else {
                    PropertyType::ReadOnly
                },
                create_event: &move |enable| Some(DeviceEvent::NoiseGateActive(enable)),
            }),
            PropertyDescriptorWrapper::Bool(PropertyDescriptor {
                prefix: "Connected:",
                data: self.connected,
                suffix: "",
                property_type: PropertyType::AlwaysReadOnly,
                create_event: &|_| None,
            }),
        ]
    }

    pub fn to_string_with_padding(&self, padding: usize) -> String {
        self.get_properties()
            .iter()
            .filter_map(|prop| {
                let (prefix, data, suffix) = match prop {
                    PropertyDescriptorWrapper::Int(property_descriptor, _) => (
                        property_descriptor.prefix,
                        &property_descriptor.data.map(|v| v.to_string()),
                        property_descriptor.suffix,
                    ),
                    PropertyDescriptorWrapper::Bool(property_descriptor) => (
                        property_descriptor.prefix,
                        &property_descriptor.data.map(|v| v.to_string()),
                        property_descriptor.suffix,
                    ),
                    PropertyDescriptorWrapper::String(property_descriptor) => (
                        property_descriptor.prefix,
                        &property_descriptor.data,
                        property_descriptor.suffix,
                    ),
                };
                data.as_ref()
                    .map(|data| format!("{:<padding$} {}{}", prefix, data, suffix))
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    pub fn to_string_with_readonly_info(&self, padding: usize) -> String {
        self.get_properties()
            .iter()
            .filter_map(|prop| {
                let (prefix, data, suffix, property_type) = match prop {
                    PropertyDescriptorWrapper::Int(property_descriptor, _) => (
                        property_descriptor.prefix,
                        &property_descriptor.data.map(|v| v.to_string()),
                        property_descriptor.suffix,
                        property_descriptor.property_type,
                    ),
                    PropertyDescriptorWrapper::Bool(property_descriptor) => (
                        property_descriptor.prefix,
                        &property_descriptor.data.map(|v| v.to_string()),
                        property_descriptor.suffix,
                        property_descriptor.property_type,
                    ),
                    PropertyDescriptorWrapper::String(property_descriptor) => (
                        property_descriptor.prefix,
                        &property_descriptor.data,
                        property_descriptor.suffix,
                        property_descriptor.property_type,
                    ),
                };

                data.as_ref().map(|data| {
                    let readonly_marker = if property_type == PropertyType::ReadOnly {
                        " (read-only)"
                    } else {
                        ""
                    };
                    format!("{:<padding$} {}{}{}", prefix, data, suffix, readonly_marker)
                })
            })
            .collect::<Vec<String>>()
            .join("\n")
    }
}

#[derive(TerminationFull)]
pub enum DeviceError {
    #[termination(msg("{0:?}"))]
    HidError(#[from] HidError),
    #[termination(msg("No device found."))]
    NoDeviceFound(),
    #[termination(msg("No response. Is the headset turned on?"))]
    HeadSetOff(),
    #[termination(msg("No response."))]
    NoResponse(),
    #[termination(msg("Unknown response: {0:?} with length: {1:?}"))]
    UnknownResponse([u8; 8], usize),
}

#[derive(Debug, Copy, Clone)]
pub enum DeviceEvent {
    BatterLevel(u8),
    Muted(bool),
    MicConnected(bool),
    Charging(ChargingStatus),
    AutomaticShutdownAfter(Duration),
    PairingInfo(u8),
    ProductColor(Color),
    SideToneOn(bool),
    SideToneVolume(u8),
    VoicePrompt(bool),
    WirelessConnected(bool),
    SurroundSound(bool),
    Silent(bool),
    RequireSIRKReset(bool),
    NoiseGateActive(bool),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Color {
    BlackBlack,
    WhiteWhite,
    BlackRed,
    UnknownColor(u8),
}

impl Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Color::BlackBlack => "Black".to_string(),
                Color::WhiteWhite => "White".to_string(),
                Color::BlackRed => "Red".to_string(),
                Color::UnknownColor(n) => format!("Unknown color {}", n),
            }
        )
    }
}

impl From<u8> for Color {
    fn from(color: u8) -> Self {
        match color {
            0 => Color::BlackBlack,
            1 => Color::WhiteWhite,
            2 => Color::BlackRed,
            _ => Color::UnknownColor(color),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ChargingStatus {
    NotCharging,
    Charging,
    FullyCharged,
    ChargeError,
}

impl Display for ChargingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ChargingStatus::NotCharging => "Not charging",
                ChargingStatus::Charging => "Charging",
                ChargingStatus::FullyCharged => "Fully charged",
                ChargingStatus::ChargeError => "Charging error!",
            }
        )
    }
}

impl From<u8> for ChargingStatus {
    fn from(value: u8) -> ChargingStatus {
        match value {
            0 => ChargingStatus::NotCharging,
            1 => ChargingStatus::Charging,
            2 => ChargingStatus::FullyCharged,
            _ => ChargingStatus::ChargeError,
        }
    }
}

pub trait Device {
    /// Write a HID report to the device.
    ///
    /// On Windows, some HyperX dongles expose commands as **Feature reports** only.
    /// In that case, `hidapi::HidDevice::write()` fails with:
    /// `WriteFile: (0x00000001) Incorrect function.`
    ///
    /// Linux/macOS hidraw paths often accept the same bytes via output reports, so this can look
    /// "Windows-exclusive". We transparently fall back to `send_feature_report` when we detect
    /// this specific failure.
    /// Adapted from PR #20 by @navrozashvili
    /// Source: https://github.com/LennardKittner/HyperHeadset/pull/20
    fn write_hid_report(&mut self, packet: &[u8]) -> Result<(), HidError> {
        match self.get_device_state_mut().transport.write(packet) {
            Ok(_) => Ok(()),
            Err(write_err) => {
                #[cfg(target_os = "windows")]
                {
                    if let HidError::HidApiError { message } = &write_err {
                        // Windows HID stack returns ERROR_INVALID_FUNCTION (0x1) when the device
                        // doesn't support output reports / interrupt OUT.
                        if message.contains("Incorrect function")
                            || message.contains("(0x00000001)")
                        {
                            // If the feature report also fails, prefer returning the original
                            // write() error since that's what callers attempted.
                            if let Err(_feature_err) = self
                                .get_device_state_mut()
                                .transport
                                .send_feature_report(packet)
                            {
                                return Err(write_err);
                            }
                            return Ok(());
                        }
                    }
                }
                Err(write_err)
            }
        }
    }

    fn get_response_buffer(&self) -> Vec<u8> {
        [0u8; RESPONSE_BUFFER_SIZE].to_vec()
    }
    fn get_charging_packet(&self) -> Option<Vec<u8>>;
    fn get_battery_packet(&self) -> Option<Vec<u8>>;
    fn set_automatic_shut_down_packet(&self, shutdown_after: Duration) -> Option<Vec<u8>>;
    fn get_automatic_shut_down_packet(&self) -> Option<Vec<u8>>;
    fn get_mute_packet(&self) -> Option<Vec<u8>>;
    fn set_mute_packet(&self, mute: bool) -> Option<Vec<u8>>;
    fn get_surround_sound_packet(&self) -> Option<Vec<u8>>;
    fn set_surround_sound_packet(&self, surround_sound: bool) -> Option<Vec<u8>>;
    fn get_mic_connected_packet(&self) -> Option<Vec<u8>>;
    fn get_pairing_info_packet(&self) -> Option<Vec<u8>>;
    fn get_product_color_packet(&self) -> Option<Vec<u8>>;
    fn get_side_tone_packet(&self) -> Option<Vec<u8>>;
    fn set_side_tone_packet(&self, side_tone_on: bool) -> Option<Vec<u8>>;
    fn get_side_tone_volume_packet(&self) -> Option<Vec<u8>>;
    fn set_side_tone_volume_packet(&self, volume: u8) -> Option<Vec<u8>>;
    fn get_voice_prompt_packet(&self) -> Option<Vec<u8>>;
    fn set_voice_prompt_packet(&self, enable: bool) -> Option<Vec<u8>>;
    fn get_wireless_connected_status_packet(&self) -> Option<Vec<u8>>;
    fn get_sirk_packet(&self) -> Option<Vec<u8>>;
    fn reset_sirk_packet(&self) -> Option<Vec<u8>>;
    fn get_silent_mode_packet(&self) -> Option<Vec<u8>>;
    fn set_silent_mode_packet(&self, silence: bool) -> Option<Vec<u8>>;
    /// Set equalizer band (0-9) to dB value (-12.0 to +12.0)
    /// Bands: 0=32Hz, 1=64Hz, 2=125Hz, 3=250Hz, 4=500Hz, 5=1kHz, 6=2kHz, 7=4kHz, 8=8kHz, 9=16kHz
    fn set_equalizer_band_packet(&self, _band_index: u8, _db_value: f32) -> Option<Vec<u8>> {
        None
    }
    fn get_noise_gate_packet(&self) -> Option<Vec<u8>> {
        None
    }
    fn set_noise_gate_packet(&self, _enable: bool) -> Option<Vec<u8>> {
        None
    }
    fn get_event_from_device_response(&self, response: &[u8]) -> Option<Vec<DeviceEvent>>;
    fn get_device_state(&self) -> &DeviceState;
    fn get_device_state_mut(&mut self) -> &mut DeviceState;
    fn prepare_write(&mut self) {}
    /// whether the app should periodically listen for packets from the headsets
    fn allow_passive_refresh(&mut self) -> bool;

    // Helper methods to check if features are writable
    fn can_set_mute(&self) -> bool {
        self.set_mute_packet(false).is_some()
    }
    fn can_set_surround_sound(&self) -> bool {
        self.set_surround_sound_packet(false).is_some()
    }
    fn can_set_side_tone(&self) -> bool {
        self.set_side_tone_packet(false).is_some()
    }
    fn can_set_automatic_shutdown(&self) -> bool {
        self.set_automatic_shut_down_packet(Duration::from_secs(0))
            .is_some()
    }
    fn can_set_side_tone_volume(&self) -> bool {
        self.set_side_tone_volume_packet(0).is_some()
    }
    fn can_set_voice_prompt(&self) -> bool {
        self.set_voice_prompt_packet(false).is_some()
    }
    fn can_set_silent_mode(&self) -> bool {
        self.set_silent_mode_packet(false).is_some()
    }
    fn can_set_equalizer(&self) -> bool {
        self.set_equalizer_band_packet(0, 0.0).is_some()
    }
    fn can_set_noise_gate(&self) -> bool {
        self.set_noise_gate_packet(true).is_some()
    }

    // Initialize capability flags in device state
    fn init_capabilities(&mut self) {
        // Collect capabilities first to avoid borrowing conflicts
        let can_set_mute = self.can_set_mute();
        let can_set_surround_sound = self.can_set_surround_sound();
        let can_set_side_tone = self.can_set_side_tone();
        let can_set_automatic_shutdown = self.can_set_automatic_shutdown();
        let can_set_side_tone_volume = self.can_set_side_tone_volume();
        let can_set_voice_prompt = self.can_set_voice_prompt();
        let can_set_silent_mode = self.can_set_silent_mode();
        let can_set_equalizer = self.can_set_equalizer();
        let can_set_noise_gate = self.can_set_noise_gate();

        // Now set them in device state
        let state = self.get_device_state_mut();
        state.device_properties.can_set_mute = can_set_mute;
        state.device_properties.can_set_surround_sound = can_set_surround_sound;
        state.device_properties.can_set_side_tone = can_set_side_tone;
        state.device_properties.can_set_automatic_shutdown = can_set_automatic_shutdown;
        state.device_properties.can_set_side_tone_volume = can_set_side_tone_volume;
        state.device_properties.can_set_voice_prompt = can_set_voice_prompt;
        state.device_properties.can_set_silent_mode = can_set_silent_mode;
        state.device_properties.can_set_equalizer = can_set_equalizer;
        state.device_properties.can_set_noise_gate = can_set_noise_gate;
    }

    fn execute_headset_specific_functionality(&mut self) -> Result<(), DeviceError> {
        Ok(())
    }

    fn wait_for_updates(&mut self, duration: Duration) -> Option<Vec<DeviceEvent>> {
        let mut buf = self.get_response_buffer();
        let res = self
            .get_device_state()
            .transport
            .read_timeout(&mut buf[..], duration.as_millis() as i32)
            .ok()?;

        if res == 0 {
            return None;
        }

        self.get_event_from_device_response(&buf)
    }

    fn get_query_packets(&self) -> Vec<Vec<u8>> {
        vec![
            self.get_wireless_connected_status_packet(),
            self.get_charging_packet(),
            self.get_battery_packet(),
            self.get_automatic_shut_down_packet(),
            self.get_mute_packet(),
            self.get_surround_sound_packet(),
            self.get_mic_connected_packet(),
            self.get_pairing_info_packet(),
            self.get_product_color_packet(),
            self.get_side_tone_packet(),
            self.get_side_tone_volume_packet(),
            self.get_voice_prompt_packet(),
            self.get_sirk_packet(),
            self.get_silent_mode_packet(),
            self.get_noise_gate_packet(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    /// Refreshes the state by querying all available information
    fn active_refresh_state(&mut self) -> Result<(), DeviceError> {
        let packets = self.get_query_packets();
        self.execute_headset_specific_functionality()?;

        let mut responded = false;
        for packet in packets.into_iter() {
            self.prepare_write();
            debug_println!("Write packet: {packet:?}");
            self.write_hid_report(&packet)?;
            std::thread::sleep(RESPONSE_DELAY);
            if let Some(events) = self.wait_for_updates(Duration::from_secs(1)) {
                for event in events {
                    self.get_device_state_mut().update_self_with_event(&event);
                }
                responded = true;
            }
            if !matches!(
                self.get_device_state().device_properties.connected,
                Some(true)
            ) {
                break;
            }
        }

        if responded {
            Ok(())
        } else {
            Err(DeviceError::NoResponse())
        }
    }

    /// Refreshes the state by listening for events
    /// Only the battery level is actively queried because it is not communicated by the device on its own
    fn passive_refresh_state(&mut self) -> Result<(), DeviceError> {
        let mut request_active_refresh = false;
        if self.allow_passive_refresh() {
            if let Some(events) = self.wait_for_updates(PASSIVE_REFRESH_TIME_OUT) {
                for event in events {
                    // Some headsets send this if they just turned on so we should refresh the
                    // state
                    if matches!(event, DeviceEvent::WirelessConnected(true)) {
                        request_active_refresh = true;
                    }
                    self.get_device_state_mut().update_self_with_event(&event);
                }
            }
        }
        if let Some(batter_packet) = self.get_battery_packet() {
            self.prepare_write();
            self.write_hid_report(&batter_packet)?;
            std::thread::sleep(RESPONSE_DELAY);
            if let Some(events) = self.wait_for_updates(Duration::from_secs(1)) {
                for event in events {
                    // Some headsets send this if they just turned on so we should refresh the
                    // state
                    if matches!(event, DeviceEvent::WirelessConnected(true)) {
                        request_active_refresh = true;
                    }
                    self.get_device_state_mut().update_self_with_event(&event);
                }
            }
        }
        if request_active_refresh {
            self.active_refresh_state()?;
        }

        Ok(())
    }

    fn try_apply(&mut self, command: DeviceEvent) -> Result<(), String> {
        match command {
            DeviceEvent::AutomaticShutdownAfter(delay) => {
                if let Some(packet) = self.set_automatic_shut_down_packet(delay) {
                    self.prepare_write();
                    if let Err(err) = self.write_hid_report(&packet) {
                        Err(format!(
                            "Failed to set automatic shutdown with error: {:?}",
                            err
                        ))?;
                    }
                } else {
                    Err("ERROR: Automatic shutdown is not supported on this device".to_string())?;
                }
            }
            DeviceEvent::Muted(mute) => {
                if let Some(packet) = self.set_mute_packet(mute) {
                    self.prepare_write();
                    if let Err(err) = self.write_hid_report(&packet) {
                        Err(format!("Failed to mute with error: {:?}", err))?;
                    }
                } else {
                    Err("ERROR: Microphone mute control is not supported on this device (hardware button only)")?;
                }
            }
            DeviceEvent::SideToneOn(enable) => {
                if let Some(packet) = self.set_side_tone_packet(enable) {
                    self.prepare_write();
                    if let Err(err) = self.write_hid_report(&packet) {
                        Err(format!("Failed to enable side tone with error: {:?}", err))?;
                    }
                } else {
                    Err("ERROR: Side tone control is not supported on this device".to_string())?;
                }
            }
            DeviceEvent::SideToneVolume(volume) => {
                if let Some(packet) = self.set_side_tone_volume_packet(volume) {
                    self.prepare_write();
                    if let Err(err) = self.write_hid_report(&packet) {
                        Err(format!(
                            "Failed to set side tone volume with error: {:?}",
                            err
                        ))?;
                    }
                } else {
                    Err(
                        "ERROR: Side tone volume control is not supported on this device"
                            .to_string(),
                    )?;
                }
            }
            DeviceEvent::VoicePrompt(enable) => {
                if let Some(packet) = self.set_voice_prompt_packet(enable) {
                    self.prepare_write();
                    if let Err(err) = self.write_hid_report(&packet) {
                        Err(format!(
                            "Failed to enable voice prompt with error: {:?}",
                            err
                        ))?;
                    }
                } else {
                    Err("ERROR: Voice prompt control is not supported on this device")?;
                }
            }
            DeviceEvent::SurroundSound(surround_sound) => {
                if let Some(packet) = self.set_surround_sound_packet(surround_sound) {
                    self.prepare_write();
                    if let Err(err) = self.write_hid_report(&packet) {
                        Err(format!(
                            "Failed to set surround sound with error: {:?}",
                            err
                        ))?;
                    }
                } else {
                    Err("ERROR: Surround sound control is not supported on this device")?;
                }
            }
            DeviceEvent::Silent(mute_playback) => {
                if let Some(packet) = self.set_silent_mode_packet(mute_playback) {
                    self.prepare_write();
                    if let Err(err) = self.write_hid_report(&packet) {
                        Err(format!("Failed to mute playback with error: {:?}", err))?;
                    }
                } else {
                    Err("ERROR: Playback mute control is not supported on this device")?;
                }
            }
            DeviceEvent::NoiseGateActive(activate) => {
                if let Some(packet) = self.set_noise_gate_packet(activate) {
                    self.prepare_write();
                    if let Err(err) = self.write_hid_report(&packet) {
                        Err(format!(
                            "Failed to activate noise gate with error: {:?}",
                            err
                        ))?;
                    }
                } else {
                    Err("ERROR: Activating noise gate is not supported on this device")?;
                }
            }
            _ => (),
        }
        Ok(())
    }

    fn clear_state(&mut self) {
        let product_id = self.get_device_state().device_properties.product_id;
        let vendor_id = self.get_device_state().device_properties.vendor_id;
        let device_name = self
            .get_device_state()
            .device_properties
            .device_name
            .clone();
        self.get_device_state_mut().device_properties =
            DeviceProperties::new(product_id, vendor_id, device_name)
    }
}
