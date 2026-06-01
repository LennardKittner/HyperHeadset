use std::sync::mpsc::Sender;

use hyper_headset::devices::{
    format_int_value, DeviceEvent, DeviceProperties, PropertyType,
};
use ksni::{
    menu::{StandardItem, SubMenu},
    Handle, MenuItem, ToolTip, Tray, TrayService,
};

use crate::tray_battery_icon_state::TrayBatteryIconState;

pub struct TrayHandler {
    handle: Handle<StatusTray>,
}

const NO_COMPATIBLE_DEVICE: &str = "No compatible device found.\nIs the dongle plugged in?\nIf you are using Linux did you\nadd the Udev rules?";
const HEADSET_NOT_CONNECTED: &str = "Headset is not connected";

impl TrayHandler {
    pub fn new(tray: StatusTray) -> Self {
        let tray_service = TrayService::new(tray);
        let handle = tray_service.handle();
        tray_service.spawn();
        TrayHandler { handle }
    }

    pub fn update(&self, properties: &DeviceProperties) {
        self.handle.update(|tray| {
            tray.device_properties = Some(properties.clone());
        })
    }

    /// Toggles whether the battery menu entry is clickable to trigger a BT
    /// reconnect. Set to `true` while displaying Bluetooth-sourced data,
    /// `false` on the HID dongle path or when no device is visible.
    pub fn set_bt_source(&self, on: bool) {
        self.handle.update(move |tray| {
            tray.bt_source = on;
        })
    }

    pub fn clear_state(&self) {
        self.handle.update(|tray| {
            tray.device_properties = None;
            tray.bt_source = false;
        })
    }
}

pub struct StatusTray {
    device_properties: Option<DeviceProperties>,
    update_sender: Sender<DeviceEvent>,
    monochrome_icons: bool,
    /// True when the displayed data comes from the Bluetooth backend. Gates the
    /// click-to-reconnect action on the battery menu entry.
    bt_source: bool,
}

impl StatusTray {
    pub fn new(update_sender: Sender<DeviceEvent>, monochrome_icons: bool) -> Self {
        StatusTray {
            device_properties: None,
            update_sender,
            monochrome_icons,
            bt_source: false,
        }
    }

    fn fallback_headset_icon(&self) -> &'static str {
        if self.monochrome_icons {
            "audio-headset-symbolic"
        } else {
            "audio-headset"
        }
    }

    fn exit_icon(&self) -> &'static str {
        if self.monochrome_icons {
            "application-exit-symbolic"
        } else {
            "application-exit"
        }
    }
}

impl Tray for StatusTray {
    fn id(&self) -> String {
        env!("CARGO_PKG_NAME").into()
    }

    fn icon_name(&self) -> String {
        TrayBatteryIconState::from_device_properties(self.device_properties.as_ref())
            .linux_icon_name(self.monochrome_icons)
            .to_string()
    }

    fn tool_tip(&self) -> ToolTip {
        let Some(device_properties) = self.device_properties.as_ref() else {
            return ToolTip {
                title: "Unknown".to_string(),
                description: NO_COMPATIBLE_DEVICE.to_string(),
                icon_name: self.fallback_headset_icon().into(),
                icon_pixmap: Vec::new(),
            };
        };
        let description = if device_properties.connected.unwrap_or(false) {
            device_properties
                .to_string_with_padding(0)
                .lines()
                .filter(|l| !l.contains("Unknown"))
                .collect::<Vec<&str>>()
                .join("\n")
        } else {
            HEADSET_NOT_CONNECTED.to_string()
        };

        ToolTip {
            title: device_properties
                .device_name
                .clone()
                .unwrap_or("Unknown".to_string()),
            description,
            icon_name: TrayBatteryIconState::from_device_properties(Some(device_properties))
                .linux_icon_name(self.monochrome_icons)
                .to_string(),
            icon_pixmap: Vec::new(),
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let exit_icon = self.exit_icon();
        let make_exit = || StandardItem {
            label: "Quit".into(),
            icon_name: exit_icon.into(),
            activate: Box::new(|_| std::process::exit(0)),
            ..Default::default()
        };
        let mut menu_items: Vec<MenuItem<Self>> = Vec::new();

        let Some(device_properties) = self.device_properties.as_ref() else {
            menu_items.push(
                StandardItem {
                    label: NO_COMPATIBLE_DEVICE.to_string(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
            menu_items.push(MenuItem::Separator);
            menu_items.push(make_exit().into());
            return menu_items;
        };

        if !device_properties.connected.unwrap_or(false) {
            menu_items.push(
                StandardItem {
                    label: HEADSET_NOT_CONNECTED.to_string(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
            menu_items.push(MenuItem::Separator);
            menu_items.push(make_exit().into());
            return menu_items;
        }
        let bt_source = self.bt_source;
        for property in device_properties.get_properties() {
            match property {
                // On the BT backend keep the battery line always visible: when
                // the value is unknown the entry becomes clickable to force a BT
                // reconnect (Device1.Disconnect/Connect) and get the HFP
                // `AT+BIEV` battery indicator flowing again. On HID we fall
                // through to the generic arm (unknown values stay hidden).
                hyper_headset::devices::PropertyDescriptorWrapper::Int(property, [])
                    if bt_source && property.name == "battery_level" =>
                {
                    let can_refresh = property.data.is_none();
                    let label = match property.data {
                        Some(v) => format!(
                            "{}: {}",
                            property.pretty_name,
                            format_int_value(v, property.suffix)
                        ),
                        None => format!("{}: -", property.pretty_name),
                    };
                    menu_items.push(
                        StandardItem {
                            label,
                            enabled: can_refresh,
                            activate: Box::new(move |_| {
                                // Thread off: D-Bus + a 2 s sleep between
                                // disconnect and reconnect.
                                std::thread::spawn(|| {
                                    if let Err(err) =
                                        hyper_headset::bluetooth::reconnect_paired_hyperx()
                                    {
                                        eprintln!("BT reconnect failed: {err}");
                                    }
                                });
                            }),
                            ..Default::default()
                        }
                        .into(),
                    );
                }
                hyper_headset::devices::PropertyDescriptorWrapper::Int(property, []) => {
                    let Some(current_value) = property.data else {
                        continue;
                    };
                    let create_event = property.create_event;
                    menu_items.push(
                        StandardItem {
                            label: format!(
                                "{}: {}",
                                property.pretty_name,
                                format_int_value(current_value, property.suffix)
                            ),
                            enabled: false,
                            activate: Box::new(move |_| {
                                let _ = (create_event)(!current_value);
                            }),
                            ..Default::default()
                        }
                        .into(),
                    );
                }
                hyper_headset::devices::PropertyDescriptorWrapper::Int(property, options) => {
                    let Some(current_value) = property.data else {
                        continue;
                    };
                    let create_event = property.create_event;
                    let sub_menu = options
                        .iter()
                        .map(|val| {
                            let update_sender = self.update_sender.clone();
                            StandardItem {
                                label: format_int_value(*val, property.suffix),
                                enabled: property.property_type == PropertyType::ReadWrite
                                    && property.data.is_some(),
                                activate: Box::new(move |_| {
                                    if let Some(command) = (create_event)(*val) {
                                        let _ = update_sender.send(command);
                                    }
                                }),
                                ..Default::default()
                            }
                            .into()
                        })
                        .collect();
                    menu_items.push(
                        SubMenu {
                            label: format!(
                                "{}: {}",
                                property.pretty_name,
                                format_int_value(current_value, property.suffix)
                            ),
                            enabled: property.property_type == PropertyType::ReadWrite
                                && property.data.is_some(),
                            submenu: sub_menu,
                            ..Default::default()
                        }
                        .into(),
                    );
                }
                hyper_headset::devices::PropertyDescriptorWrapper::Bool(property) => {
                    let Some(current_value) = property.data else {
                        continue;
                    };
                    let create_event = property.create_event;
                    let update_sender = self.update_sender.clone();
                    menu_items.push(
                        StandardItem {
                            label: format!(
                                "{}: {}{}",
                                property.pretty_name, current_value, property.suffix
                            ),
                            enabled: property.property_type == PropertyType::ReadWrite
                                && property.data.is_some(),
                            activate: Box::new(move |_| {
                                if let Some(command) = (create_event)(!current_value) {
                                    let _ = update_sender.send(command);
                                }
                            }),
                            ..Default::default()
                        }
                        .into(),
                    );
                }
                hyper_headset::devices::PropertyDescriptorWrapper::String(property) => {
                    let Some(current_value) = property.data else {
                        continue;
                    };
                    let create_event = property.create_event;
                    menu_items.push(
                        StandardItem {
                            label: format!(
                                "{}: {}{}",
                                property.pretty_name, current_value, property.suffix
                            ),
                            enabled: false,
                            activate: Box::new(move |_| {
                                let _ = (create_event)(String::new());
                            }),
                            ..Default::default()
                        }
                        .into(),
                    );
                }
            }
        }

        menu_items.push(MenuItem::Separator);
        menu_items.push(make_exit().into());
        menu_items
    }
}
