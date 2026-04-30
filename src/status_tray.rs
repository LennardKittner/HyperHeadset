use std::sync::mpsc::Sender;

use hyper_headset::devices::{DeviceEvent, DeviceProperties, DeviceState, PropertyType};
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

    pub fn update(&self, device_state: &DeviceState) {
        self.handle.update(|tray| {
            tray.device_properties = Some(device_state.device_properties.clone());
        })
    }

    pub fn clear_state(&self) {
        self.handle.update(|tray| {
            tray.device_properties = None;
        })
    }
}

pub struct StatusTray {
    device_properties: Option<DeviceProperties>,
    update_sender: Sender<DeviceEvent>,
}

impl StatusTray {
    pub fn new(update_sender: Sender<DeviceEvent>) -> Self {
        StatusTray {
            device_properties: None,
            update_sender,
        }
    }
}

impl Tray for StatusTray {
    fn id(&self) -> String {
        env!("CARGO_PKG_NAME").into()
    }

    fn icon_name(&self) -> String {
        TrayBatteryIconState::from_device_properties(self.device_properties.as_ref())
            .linux_icon_name()
            .to_string()
    }

    fn tool_tip(&self) -> ToolTip {
        let Some(device_properties) = self.device_properties.as_ref() else {
            return ToolTip {
                title: "Unknown".to_string(),
                description: NO_COMPATIBLE_DEVICE.to_string(),
                icon_name: "audio-headset-symbolic".into(),
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
                .linux_icon_name()
                .to_string(),
            icon_pixmap: Vec::new(),
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let make_exit = || StandardItem {
            label: "Quit".into(),
            icon_name: "application-exit".into(),
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
        for property in device_properties.get_properties() {
            match property {
                hyper_headset::devices::PropertyDescriptorWrapper::Int(property, []) => {
                    let Some(current_value) = property.data else {
                        continue;
                    };
                    let create_event = property.create_event;
                    menu_items.push(
                        StandardItem {
                            label: format!(
                                "{} {}{}",
                                property.prefix, current_value, property.suffix
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
                                label: format!("{}{}", val, property.suffix),
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
                                "{} {}{}",
                                property.prefix, current_value, property.suffix
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
                                "{} {}{}",
                                property.prefix, current_value, property.suffix
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
                                "{} {}{}",
                                property.prefix, current_value, property.suffix
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
