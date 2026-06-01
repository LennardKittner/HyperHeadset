use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use dbus::arg::{PropMap, RefArg};
use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;
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

// Writes are intentionally not implemented: `RACE_SET_MMI_COMMON_CONFIG`
// (0x2C82) is acknowledged but only updates a volatile RAM mirror that
// reverts on (re)connect. Persistent settings would require writing the
// matching NVKEY via the dongle's USB path.

/// A HyperX headset reached over Bluetooth (BlueZ), used as a fallback backend
/// when no USB HID dongle is present.
///
/// Read-only: battery and name come from BlueZ, while voice-prompt and
/// auto-power-off are read over the Airoha vendor BLE service (RACE). Settings
/// are not writable here because RACE writes don't persist on this firmware,
/// so [`Headset::try_apply`](crate::devices::Headset::try_apply) rejects
/// changes on the Bluetooth backend.
pub struct BluetoothHeadset {
    conn: Connection,
    path: Path<'static>,
    name: Option<String>,
    battery_level: Option<u8>,
    connected: bool,
    airoha: AirohaSnapshot,
}

impl BluetoothHeadset {
    /// Locate a connected HyperX headset on the system bus and read its battery.
    /// Returns `Ok(None)` when no connected HyperX device is present.
    ///
    /// The Airoha config is not read here: probing the vendor BLE service
    /// renegotiates the RFCOMM/HFP session and transiently disrupts the battery
    /// indicator, so it is filled lazily by [`refresh`](Self::refresh) instead.
    /// The snapshot is seeded from the process-wide [`AIROHA_CACHE`] so a
    /// headset re-found after a reconnect keeps the last known values.
    pub fn find() -> Result<Option<Self>, dbus::Error> {
        let conn = Connection::new_system()?;
        let Some((path, name)) = find_connected_hyperx(&conn)? else {
            return Ok(None);
        };
        let mut headset = Self {
            conn,
            path,
            name,
            battery_level: None,
            connected: true,
            airoha: *AIROHA_CACHE.lock().unwrap(),
        };
        headset.battery_level = headset.read_battery().ok().flatten();
        Ok(Some(headset))
    }

    /// Reads `org.bluez.Battery1.Percentage`. Returns `None` if the interface
    /// is absent, or if the value is reported by the GATT Battery Service which
    /// the HyperX firmware exposes as a stub always returning `0`. The real
    /// value comes from the HFP `AT+BIEV` indicator pushed by PipeWire/oFono.
    fn read_battery(&self) -> Result<Option<u8>, dbus::Error> {
        let proxy = self.conn.with_proxy("org.bluez", &self.path, DBUS_TIMEOUT);
        let Ok(percentage) = proxy.get::<u8>("org.bluez.Battery1", "Percentage") else {
            return Ok(None);
        };
        let source = proxy
            .get::<String>("org.bluez.Battery1", "Source")
            .unwrap_or_default();
        if percentage == 0 && source == "GATT Battery Service" {
            return Ok(None);
        }
        Ok(Some(percentage))
    }

    /// Best-effort read of the Airoha vendor BLE features. Returns an empty
    /// snapshot when the headset's GATT vendor service is unreachable;
    /// callers should retry until [`AirohaSnapshot::is_empty`] is false.
    fn read_airoha_snapshot(&self) -> AirohaSnapshot {
        let mut snap = AirohaSnapshot::default();
        let Ok(client) = RaceClient::open(&self.path.to_string()) else {
            return snap;
        };
        // Voice prompt — response: [status, module_id_le16, value_le16].
        // value 0x0000 = off, non-zero (typically 0xFFFF) = on.
        if let Ok(body) = client.request(RACE_GET_MMI_COMMON_CONFIG, &MOD_VOICE_PROMPT.to_le_bytes())
        {
            if body.len() >= 5 && body[0] == 0 && u16_le(&body[1..3]) == MOD_VOICE_PROMPT {
                snap.voice_prompt_on = Some(u16_le(&body[3..5]) != 0);
            }
        }
        // Auto power off — response: [status, module_id_le16, enabled_le16, timeout_minutes_le16, …].
        if let Ok(body) =
            client.request(RACE_GET_MMI_COMMON_CONFIG, &MOD_AUTO_POWER_OFF.to_le_bytes())
        {
            if body.len() >= 7 && body[0] == 0 && u16_le(&body[1..3]) == MOD_AUTO_POWER_OFF {
                snap.auto_power_off_enabled = Some(u16_le(&body[3..5]) != 0);
                snap.auto_power_off_minutes = Some(u16_le(&body[5..7]));
            }
        }
        snap
    }

    /// Re-locate the headset and refresh the cached battery. The last good
    /// reading is kept across transient HFP disruptions. When the headset can
    /// no longer be reached, `connected` is cleared so the next
    /// `device_properties()` reflects it, and an error signals the caller to
    /// reconnect.
    ///
    /// The Airoha snapshot is read lazily and only while still empty, so a
    /// successful read is cached (both on this instance and in the process-wide
    /// [`AIROHA_CACHE`]) and we avoid repeatedly disturbing the HFP battery
    /// indicator. Because `find` seeds from that cache, the values also survive
    /// the reconnect triggered from the tray.
    pub fn refresh(&mut self) -> Result<(), DeviceError> {
        match BluetoothHeadset::find() {
            Ok(Some(fresh)) => {
                let battery_level = fresh.battery_level.or(self.battery_level);
                let airoha = self.airoha;
                *self = fresh;
                self.battery_level = battery_level;
                self.airoha = airoha;
                if self.airoha.is_empty() {
                    let snap = self.read_airoha_snapshot();
                    if !snap.is_empty() {
                        self.airoha = snap;
                        *AIROHA_CACHE.lock().unwrap() = snap;
                    }
                }
                Ok(())
            }
            _ => {
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

/// Cached snapshot of the Airoha vendor-BLE config we read over RACE.
#[derive(Debug, Default, Clone, Copy)]
pub struct AirohaSnapshot {
    pub voice_prompt_on: Option<bool>,
    pub auto_power_off_enabled: Option<bool>,
    pub auto_power_off_minutes: Option<u16>,
}

impl AirohaSnapshot {
    const EMPTY: Self = Self {
        voice_prompt_on: None,
        auto_power_off_enabled: None,
        auto_power_off_minutes: None,
    };

    /// `true` when none of the individual reads succeeded — treat as a
    /// retryable failure rather than a valid cache.
    pub fn is_empty(&self) -> bool {
        self.voice_prompt_on.is_none()
            && self.auto_power_off_enabled.is_none()
            && self.auto_power_off_minutes.is_none()
    }
}

/// Process-wide cache of the last good Airoha snapshot.
///
/// The vendor BLE/GATT link is torn down on every reconnect (see
/// [`reconnect_paired_hyperx`]) and only comes back after a physical
/// power-cycle, so a fresh [`BluetoothHeadset`] created after a reconnect can no
/// longer read the voice-prompt / auto-power-off config. Caching it here — at
/// the process level rather than per-instance — keeps those values visible
/// across reconnects instead of reverting to "unknown".
static AIROHA_CACHE: Mutex<AirohaSnapshot> = Mutex::new(AirohaSnapshot::EMPTY);

/// Force a BT reconnect on the paired HyperX headset to trigger a fresh HFP
/// BIND/BIA handshake. Wired to the tray's "Battery level: -" entry: BlueZ only
/// exposes a real percentage once the headset re-announces it over HFP
/// `AT+BIEV`, so when that source has gone stale a reconnect is the only way to
/// recover the reading.
///
/// Side effect: the firmware also tears down the BLE/GATT link until the next
/// physical power-cycle, so any cached Airoha values are transiently lost.
pub fn reconnect_paired_hyperx() -> Result<(), dbus::Error> {
    let conn = Connection::new_system()?;
    let Some((path, _)) = find_connected_hyperx(&conn)? else {
        return Err(dbus::Error::new_custom(
            "com.hyperheadset.NotConnected",
            "no connected HyperX headset to reconnect",
        ));
    };
    let dev_proxy = conn.with_proxy("org.bluez", &path, DBUS_TIMEOUT);
    let _ = dev_proxy.method_call::<(), _, _, _>("org.bluez.Device1", "Disconnect", ());
    std::thread::sleep(Duration::from_secs(2));
    dev_proxy.method_call::<(), _, _, _>("org.bluez.Device1", "Connect", ())?;
    Ok(())
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
