use std::collections::HashMap;
use std::time::Duration;

use dbus::arg::{PropMap, RefArg};
use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;
use dbus::blocking::Connection;
use dbus::Path;
use hyper_headset::devices::DeviceProperties;

use crate::airoha_race::RaceClient;

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

pub struct BluetoothHeadset {
    conn: Connection,
    path: Path<'static>,
    name: Option<String>,
}

impl BluetoothHeadset {
    pub fn find() -> Result<Option<Self>, dbus::Error> {
        let conn = Connection::new_system()?;
        let Some((path, name)) = find_connected_hyperx(&conn)? else {
            return Ok(None);
        };
        Ok(Some(Self { conn, path, name }))
    }

    /// Reads `org.bluez.Battery1.Percentage`. Returns `None` when the value
    /// comes from the GATT Battery Service stub (which the HyperX firmware
    /// hardwires to `0`); a real reading is only available via the HFP
    /// `+BIEV` indicator pushed by PipeWire/oFono.
    pub fn battery(&self) -> Result<Option<u8>, dbus::Error> {
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
    pub fn read_airoha_snapshot(&self) -> AirohaSnapshot {
        let mut snap = AirohaSnapshot::default();
        let Ok(client) = RaceClient::open(&self.path.to_string()) else {
            return snap;
        };
        // Voice prompt — response: [status, module_id_le16, value_le16].
        // value 0x0000 = off, non-zero (typically 0xFFFF) = on.
        if let Ok(body) = client.request(
            RACE_GET_MMI_COMMON_CONFIG,
            &MOD_VOICE_PROMPT.to_le_bytes(),
        ) {
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
}

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

/// Build a `DeviceProperties` snapshot from a Bluetooth-connected headset.
/// Only battery, name and any cached Airoha values are populated; everything
/// else stays `None` so the tray UI only shows what we actually know.
pub fn to_device_properties(
    bt: &BluetoothHeadset,
    battery_level: Option<u8>,
    airoha: Option<&AirohaSnapshot>,
) -> DeviceProperties {
    let mut props = DeviceProperties::new(0, 0, bt.name.clone());
    props.battery_level = battery_level;
    props.connected = Some(true);
    if let Some(a) = airoha {
        props.voice_prompt_on = a.voice_prompt_on;
        if let Some(minutes) = a.auto_power_off_minutes {
            let effective_secs = if a.auto_power_off_enabled == Some(false) {
                0
            } else {
                u64::from(minutes) * 60
            };
            props.automatic_shutdown_after = Some(Duration::from_secs(effective_secs));
        }
    }
    props
}

/// Force a BT reconnect on the paired HyperX headset to trigger a fresh HFP
/// BIND/BIA handshake. Used by the tray's "Battery level: -" entry when the
/// HFP battery source has gone stale. Side effect: the BLE/GATT link is also
/// torn down by the firmware until the next physical power-cycle, so Airoha
/// values are transiently lost.
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
