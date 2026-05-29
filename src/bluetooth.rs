use std::collections::HashMap;
use std::time::Duration;

use dbus::arg::{PropMap, RefArg};
use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;
use dbus::blocking::Connection;
use dbus::Path;

use crate::devices::{DeviceError, DeviceProperties};

const HYPERX_NAME_HINT: &str = "HyperX";
const DBUS_TIMEOUT: Duration = Duration::from_millis(2000);

/// A HyperX headset reached over Bluetooth (BlueZ), used as a fallback backend
/// when no USB HID dongle is present.
///
/// Read-only: only the battery level and name are exposed.  The headset's
/// Airoha RACE config (voice prompt, auto power off, …) is reachable over BLE
/// GATT but writes don't persist on this firmware, so we deliberately don't
/// surface settings here.
pub struct BluetoothHeadset {
    conn: Connection,
    path: Path<'static>,
    name: Option<String>,
    battery_level: Option<u8>,
}

impl BluetoothHeadset {
    /// Locate a connected HyperX headset on the system bus and read its battery.
    /// Returns `Ok(None)` when no connected HyperX device is present.
    pub fn find() -> Result<Option<Self>, dbus::Error> {
        let conn = Connection::new_system()?;
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
                let mut headset = Self {
                    conn,
                    path,
                    name,
                    battery_level: None,
                };
                headset.battery_level = headset.read_battery().ok().flatten();
                return Ok(Some(headset));
            }
        }
        Ok(None)
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

    /// Re-locate the headset and refresh the cached battery. The last good
    /// reading is kept across transient HFP disruptions. An error (including
    /// the headset no longer being connected) signals the caller to reconnect.
    pub fn refresh(&mut self) -> Result<(), DeviceError> {
        match BluetoothHeadset::find() {
            Ok(Some(fresh)) => {
                let battery_level = fresh.battery_level.or(self.battery_level);
                *self = fresh;
                self.battery_level = battery_level;
                Ok(())
            }
            _ => Err(DeviceError::NoDeviceFound()),
        }
    }

    /// Build a `DeviceProperties` snapshot from the Bluetooth headset. Only
    /// battery, name and connection state are populated; the rest stays `None`
    /// so the UI only shows what we actually know.
    pub fn device_properties(&self) -> DeviceProperties {
        let mut props = DeviceProperties::new(0, 0, self.name.clone());
        props.battery_level = self.battery_level;
        props.connected = Some(true);
        props
    }
}
