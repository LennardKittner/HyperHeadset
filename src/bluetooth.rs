use std::collections::HashMap;
use std::time::Duration;

use dbus::arg::{PropMap, RefArg};
use dbus::blocking::Connection;
use dbus::Path;

use crate::airoha_race::RaceClient;
use crate::devices::{DeviceError, DeviceProperties};

const HYPERX_NAME_HINT: &str = "HyperX";
const DBUS_TIMEOUT: Duration = Duration::from_millis(2000);

/// MMI_COMMON_CONFIG MODULE_IDs we read on the Airoha vendor BLE service.
const MOD_AUTO_POWER_OFF: u16 = 1;
const MOD_VOICE_PROMPT: u16 = 6;
const RACE_GET_MMI_COMMON_CONFIG: u16 = 0x2C83;
/// RACE_GET_BATTERY (`0x0CD6` RaceGetBattery in the Airoha SDK). Takes a `role`
/// byte; the `0x5B` response is a status-only ack and the level arrives in a
/// `0x5D` indication. (The AirReps table's `0x0C02` is unimplemented on this
/// firmware — no response.)
const RACE_GET_BATTERY: u16 = 0x0CD6;
/// Battery `role` argument. Confirmed on Cloud III S: role 0 = the headset
/// (role 1 errors — no second battery on this non-TWS device).
const BATTERY_ROLE: u8 = 0x00;

// Writes are intentionally not implemented: `RACE_SET_MMI_COMMON_CONFIG`
// (0x2C82) is acknowledged but only updates a volatile RAM mirror that
// reverts on (re)connect. Persistent settings would require writing the
// matching NVKEY via the dongle's USB path.

/// A HyperX headset reached over Bluetooth (BlueZ), used as a fallback backend
/// when no USB HID dongle is present.
///
/// Read-only: name comes from BlueZ, while battery, voice-prompt and
/// auto-power-off are all read over the Airoha vendor BLE service (RACE) on a
/// single long-lived session. Settings are not writable because RACE writes
/// don't persist on this firmware, so
/// [`Headset::try_apply`](crate::devices::Headset::try_apply) rejects changes on
/// the Bluetooth backend.
pub struct BluetoothHeadset {
    path: Path<'static>,
    name: Option<String>,
    battery_level: Option<u8>,
    connected: bool,
    airoha: AirohaSnapshot,
    /// Long-lived RACE session. Held open so battery polls reuse one subscribe
    /// instead of cycling StartNotify/StopNotify (which eventually locks up the
    /// vendor service). Boxed to keep `BluetoothHeadset` (and the `Headset`
    /// enum) small.
    race: Option<Box<RaceClient>>,
}

impl BluetoothHeadset {
    /// Locate a connected HyperX headset on the system bus and open its RACE
    /// session (reading the initial battery + Airoha config). Returns
    /// `Ok(None)` when no connected HyperX device is present.
    pub fn find() -> Result<Option<Self>, dbus::Error> {
        let conn = Connection::new_system()?;
        let Some((path, name)) = find_connected_hyperx(&conn)? else {
            return Ok(None);
        };
        let mut headset = Self {
            path,
            name,
            battery_level: None,
            connected: true,
            airoha: AirohaSnapshot::default(),
            race: None,
        };
        headset.open_race_session();
        Ok(Some(headset))
    }

    /// Open the long-lived RACE session and take the first battery + Airoha
    /// reads over it. On failure `race` stays `None` and the next `refresh`
    /// retries.
    fn open_race_session(&mut self) {
        let Ok(client) = RaceClient::open(&self.path.to_string()) else {
            return;
        };
        self.battery_level = read_race_battery(&client);
        if self.airoha.is_empty() {
            let snap = read_airoha_via(&client);
            if !snap.is_empty() {
                self.airoha = snap;
            }
        }
        self.race = Some(Box::new(client));
    }

    /// Poll the battery over the held RACE session, reusing the single
    /// subscribe. If the session is gone (first run after a failure) it is
    /// re-established via `find`. A failed battery read tears the session down
    /// so the next cycle opens a fresh subscribe instead of polling a dead one.
    /// The Airoha config is read lazily and only while still empty.
    pub fn refresh(&mut self) -> Result<(), DeviceError> {
        if self.race.is_none() {
            let airoha = self.airoha; // keep last known config across the re-subscribe
            match BluetoothHeadset::find() {
                Ok(Some(fresh)) => {
                    *self = fresh;
                    if self.airoha.is_empty() {
                        self.airoha = airoha;
                    }
                }
                _ => {
                    self.connected = false;
                    return Err(DeviceError::NoDeviceFound());
                }
            }
        }
        let Some(client) = self.race.as_ref() else {
            self.connected = false;
            return Err(DeviceError::NoDeviceFound());
        };
        match read_race_battery(client) {
            Some(level) => {
                self.battery_level = Some(level);
                if self.airoha.is_empty() {
                    let snap = read_airoha_via(client);
                    if !snap.is_empty() {
                        self.airoha = snap;
                    }
                }
                Ok(())
            }
            None => {
                self.race = None;
                self.connected = false;
                Err(DeviceError::NoDeviceFound())
            }
        }
    }

    /// Build a `DeviceProperties` snapshot from the Bluetooth headset. Battery,
    /// name, connection state and any cached Airoha values are populated; the
    /// rest stays `None` so the UI only shows what we actually know.
    pub fn device_properties(&self) -> DeviceProperties {
        let mut props = DeviceProperties::new(0, 0, self.name.clone());
        props.battery_level = self.battery_level;
        props.connected = Some(self.connected);
        props.voice_prompt_on = self.airoha.voice_prompt_on;
        if let Some(minutes) = self.airoha.auto_power_off_minutes {
            let effective_secs = if self.airoha.auto_power_off_enabled == Some(false) {
                0
            } else {
                u64::from(minutes) * 60
            };
            props.automatic_shutdown_after = Some(Duration::from_secs(effective_secs));
        }
        props
    }
}

/// Read voice-prompt and auto-power-off over an already-open RACE session.
fn read_airoha_via(client: &RaceClient) -> AirohaSnapshot {
    let mut snap = AirohaSnapshot::default();
    // Voice prompt — response: [status, module_id_le16, value_le16].
    // value 0x0000 = off, non-zero (typically 0xFFFF) = on.
    if let Ok(body) = client.request(RACE_GET_MMI_COMMON_CONFIG, &MOD_VOICE_PROMPT.to_le_bytes()) {
        if body.len() >= 5 && body[0] == 0 && u16_le(&body[1..3]) == MOD_VOICE_PROMPT {
            snap.voice_prompt_on = Some(u16_le(&body[3..5]) != 0);
        }
    }
    // Auto power off — response: [status, module_id_le16, enabled_le16, timeout_minutes_le16, …].
    if let Ok(body) = client.request(
        RACE_GET_MMI_COMMON_CONFIG,
        &MOD_AUTO_POWER_OFF.to_le_bytes(),
    ) {
        if body.len() >= 7 && body[0] == 0 && u16_le(&body[1..3]) == MOD_AUTO_POWER_OFF {
            snap.auto_power_off_enabled = Some(u16_le(&body[3..5]) != 0);
            snap.auto_power_off_minutes = Some(u16_le(&body[5..7]));
        }
    }
    snap
}

/// Read the battery percentage over an already-open RACE session.
///
/// `0x0CD6` acks with a status-only `0x5B`; the data lands in a `0x5D`
/// indication with body `[status, role, level]`. Confirmed on Cloud III S:
/// role 0 → `00 00 53` (`0x53` = 83%). An invalid role (or any error) yields no
/// indication, so `request_indication` times out and we return `None`.
fn read_race_battery(client: &RaceClient) -> Option<u8> {
    let body = client
        .request_indication(RACE_GET_BATTERY, &[BATTERY_ROLE])
        .ok()?;
    if body.len() < 3 || body[0] != 0 {
        return None;
    }
    let level = body[2];
    (level <= 100).then_some(level)
}

/// Cached snapshot of the Airoha vendor-BLE config we read over RACE.
#[derive(Debug, Default, Clone, Copy)]
pub struct AirohaSnapshot {
    pub voice_prompt_on: Option<bool>,
    pub auto_power_off_enabled: Option<bool>,
    pub auto_power_off_minutes: Option<u16>,
}

impl AirohaSnapshot {
    /// `true` when none of the individual reads succeeded — treat as a
    /// retryable failure rather than a valid cache.
    pub fn is_empty(&self) -> bool {
        self.voice_prompt_on.is_none()
            && self.auto_power_off_enabled.is_none()
            && self.auto_power_off_minutes.is_none()
    }
}

/// Scan BlueZ for a connected device whose name advertises HyperX, returning
/// its object path and reported name.
fn find_connected_hyperx(
    conn: &Connection,
) -> Result<Option<(Path<'static>, Option<String>)>, dbus::Error> {
    let proxy = conn.with_proxy("org.bluez", "/", DBUS_TIMEOUT);
    let (objects,): (HashMap<Path<'static>, HashMap<String, PropMap>>,) = proxy.method_call(
        "org.freedesktop.DBus.ObjectManager",
        "GetManagedObjects",
        (),
    )?;
    for (path, ifaces) in objects {
        let Some(dev) = ifaces.get("org.bluez.Device1") else {
            continue;
        };
        let connected = dev
            .get("Connected")
            .and_then(|v| v.0.as_any().downcast_ref::<bool>().copied())
            .unwrap_or(false);
        if !connected {
            continue;
        }
        let name = dev
            .get("Name")
            .or_else(|| dev.get("Alias"))
            .and_then(|v| v.0.as_str().map(str::to_string));
        if name
            .as_deref()
            .is_some_and(|n| n.contains(HYPERX_NAME_HINT))
        {
            return Ok(Some((path, name)));
        }
    }
    Ok(None)
}

fn u16_le(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}
