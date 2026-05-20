use std::collections::HashMap;
use std::time::Duration;

use dbus::arg::{PropMap, RefArg};
use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;
use dbus::blocking::Connection;
use dbus::Path;
use hyper_headset::devices::DeviceProperties;

const HYPERX_NAME_HINT: &str = "HyperX";
const DBUS_TIMEOUT: Duration = Duration::from_millis(2000);

pub struct BluetoothHeadset {
    conn: Connection,
    path: Path<'static>,
    name: Option<String>,
}

impl BluetoothHeadset {
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
                return Ok(Some(Self { conn, path, name }));
            }
        }
        Ok(None)
    }

    /// Reads `org.bluez.Battery1.Percentage`. Returns `None` if the interface
    /// is absent, or if the value is reported by the GATT Battery Service which
    /// the HyperX firmware exposes as a stub always returning `0`. The real
    /// value comes from the HFP `AT+BIEV` indicator pushed by PipeWire/oFono.
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
}

/// Build a `DeviceProperties` snapshot from a Bluetooth-connected headset.
/// Only battery and name are populated; the rest stays `None` so the tray UI
/// only shows what we actually know.
pub fn to_device_properties(bt: &BluetoothHeadset, battery_level: Option<u8>) -> DeviceProperties {
    let mut props = DeviceProperties::new(0, 0, bt.name.clone());
    props.battery_level = battery_level;
    props.connected = Some(true);
    props
}
