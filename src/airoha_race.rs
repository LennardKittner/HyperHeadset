//! Minimal Airoha RACE client over BlueZ GATT (Linux only).
//!
//! Used to read configuration items exposed by the headset's Airoha vendor
//! BLE service (write to TX, listen for notifications on RX). Each `request`
//! is one round-trip; callers should batch reads into a single `RaceClient`
//! to minimize how many times we subscribe — every notify-subscribe causes
//! the firmware to renegotiate its RFCOMM/HFP session, which temporarily
//! disrupts the HFP `AT+BIEV` battery indicator (see [[bluetooth.rs]]).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dbus::arg::{PropMap, RefArg, Variant};
use dbus::blocking::stdintf::org_freedesktop_dbus::PropertiesPropertiesChanged;
use dbus::blocking::Connection;
use dbus::message::MatchRule;
use dbus::Path;

const TX_UUID: &str = "43484152-2dab-3241-6972-6f6861424c45";
const RX_UUID: &str = "43484152-2dab-3141-6972-6f6861424c45";

const HEAD_NORMAL: u8 = 0x05;
const TYPE_REQUEST: u8 = 0x5A;
const TYPE_RESPONSE: u8 = 0x5B;

const DBUS_TIMEOUT: Duration = Duration::from_millis(2000);
/// Per-request wait window for a matching notification.
const REQUEST_TIMEOUT: Duration = Duration::from_millis(800);
/// Time given on connect to absorb the unsolicited "session ready" indication.
const HELLO_DRAIN: Duration = Duration::from_millis(600);

/// Build a `0x05 0x5A …` RACE request packet.
pub fn build_packet(opcode: u16, payload: &[u8]) -> Vec<u8> {
    let length = 2u16 + payload.len() as u16;
    let mut packet = Vec::with_capacity(6 + payload.len());
    packet.push(HEAD_NORMAL);
    packet.push(TYPE_REQUEST);
    packet.extend_from_slice(&length.to_le_bytes());
    packet.extend_from_slice(&opcode.to_le_bytes());
    packet.extend_from_slice(payload);
    packet
}

/// Parse a response frame: returns `(echoed_opcode, payload_after_header)`.
fn parse_packet(bytes: &[u8]) -> Option<(u16, Vec<u8>)> {
    if bytes.len() < 6 || bytes[0] != HEAD_NORMAL || bytes[1] != TYPE_RESPONSE {
        return None;
    }
    let length = u16::from_le_bytes([bytes[2], bytes[3]]) as usize;
    let opcode = u16::from_le_bytes([bytes[4], bytes[5]]);
    // length includes the 2-byte cmd_id; payload follows at offset 6
    let payload_len = length.saturating_sub(2);
    let end = (6 + payload_len).min(bytes.len());
    Some((opcode, bytes[6..end].to_vec()))
}

pub struct RaceClient {
    conn: Connection,
    tx_path: Path<'static>,
    rx_path: Path<'static>,
    inbox: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl RaceClient {
    /// Open a session: locate TX/RX chars under the BlueZ device path,
    /// subscribe to RX notifications, drain the spontaneous hello indication.
    pub fn open(device_path: &str) -> Result<Self, dbus::Error> {
        let conn = Connection::new_system()?;
        let (tx_path, rx_path) = find_char_paths(&conn, device_path)?;

        let inbox: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
        let inbox_clone = inbox.clone();
        let rule = MatchRule::new_signal("org.freedesktop.DBus.Properties", "PropertiesChanged")
            .with_path(rx_path.clone());
        conn.add_match(rule, move |args: PropertiesPropertiesChanged, _, _| {
            if let Some(Variant(value)) = args.changed_properties.get("Value") {
                if let Some(bytes) = extract_byte_array(value.as_ref()) {
                    inbox_clone.lock().unwrap().push(bytes);
                }
            }
            true
        })?;

        // StartNotify on RX
        let rx_proxy = conn.with_proxy("org.bluez", &rx_path, DBUS_TIMEOUT);
        rx_proxy.method_call::<(), _, _, _>(
            "org.bluez.GattCharacteristic1",
            "StartNotify",
            (),
        )?;

        let client = Self {
            conn,
            tx_path,
            rx_path,
            inbox,
        };
        // Drain the unsolicited hello frame
        let start = Instant::now();
        while start.elapsed() < HELLO_DRAIN {
            client.conn.process(Duration::from_millis(50))?;
        }
        client.inbox.lock().unwrap().clear();
        Ok(client)
    }

    /// Send a RACE request, wait for the matching response (echoed opcode).
    pub fn request(&self, opcode: u16, payload: &[u8]) -> Result<Vec<u8>, dbus::Error> {
        let packet = build_packet(opcode, payload);
        let tx_proxy = self.conn.with_proxy("org.bluez", &self.tx_path, DBUS_TIMEOUT);
        tx_proxy.method_call::<(), _, _, _>(
            "org.bluez.GattCharacteristic1",
            "WriteValue",
            (packet, PropMap::new()),
        )?;
        let start = Instant::now();
        while start.elapsed() < REQUEST_TIMEOUT {
            self.conn.process(Duration::from_millis(50))?;
            let mut buf = self.inbox.lock().unwrap();
            if let Some(idx) = buf
                .iter()
                .position(|f| parse_packet(f).is_some_and(|(echo, _)| echo == opcode))
            {
                let frame = buf.remove(idx);
                drop(buf);
                let (_, body) = parse_packet(&frame).unwrap();
                return Ok(body);
            }
        }
        Err(dbus::Error::new_custom(
            "com.hyperheadset.RaceTimeout",
            &format!("no response for opcode 0x{opcode:04x}"),
        ))
    }
}

impl Drop for RaceClient {
    fn drop(&mut self) {
        let rx = self
            .conn
            .with_proxy("org.bluez", &self.rx_path, Duration::from_millis(500));
        let _ =
            rx.method_call::<(), _, _, _>("org.bluez.GattCharacteristic1", "StopNotify", ());
    }
}

fn find_char_paths(
    conn: &Connection,
    device_path: &str,
) -> Result<(Path<'static>, Path<'static>), dbus::Error> {
    let proxy = conn.with_proxy("org.bluez", "/", DBUS_TIMEOUT);
    let (objects,): (HashMap<Path<'static>, HashMap<String, PropMap>>,) = proxy.method_call(
        "org.freedesktop.DBus.ObjectManager",
        "GetManagedObjects",
        (),
    )?;
    let mut tx = None;
    let mut rx = None;
    for (path, ifaces) in objects {
        if !path.to_string().starts_with(device_path) {
            continue;
        }
        let Some(ch) = ifaces.get("org.bluez.GattCharacteristic1") else {
            continue;
        };
        let Some(uuid) = ch.get("UUID").and_then(|v| v.0.as_str()) else {
            continue;
        };
        let uuid_lc = uuid.to_lowercase();
        if uuid_lc == TX_UUID {
            tx = Some(path.clone());
        } else if uuid_lc == RX_UUID {
            rx = Some(path);
        }
    }
    match (tx, rx) {
        (Some(t), Some(r)) => Ok((t, r)),
        _ => Err(dbus::Error::new_custom(
            "com.hyperheadset.AirohaCharsNotFound",
            "Airoha vendor TX/RX characteristics not found under device",
        )),
    }
}

fn extract_byte_array(v: &dyn RefArg) -> Option<Vec<u8>> {
    // BlueZ exposes `Value` as `ay` — a Variant of byte array.
    let mut iter = v.as_iter()?;
    let mut out = Vec::new();
    for item in iter.by_ref() {
        if let Some(b) = item.as_u64() {
            out.push(b as u8);
        } else {
            return None;
        }
    }
    Some(out)
}
