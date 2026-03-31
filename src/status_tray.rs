use std::sync::mpsc::Sender;

use hyper_headset::devices::{DeviceEvent, DeviceProperties, DeviceState, PropertyType};
use ksni::{
    menu::{RadioGroup, RadioItem, StandardItem, SubMenu},
    Handle, MenuItem, ToolTip, Tray, TrayService,
};

use crate::tray_battery_icon_state::TrayBatteryIconState;

#[cfg(feature = "eq-support")]
use hyper_headset::eq::presets;

/// Escape underscores for ksni labels (single `_` is an accelerator prefix).
fn escape_label(s: &str) -> String {
    s.replace('_', "__")
}

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
        let device_properties = device_state.device_properties.clone();
        self.handle.update(|tray| {
            tray.device_properties = Some(device_properties);
        })
    }

    pub fn clear_state(&self) {
        self.handle.update(|tray| {
            tray.device_properties = None;
        })
    }

    #[cfg(feature = "eq-support")]
    pub fn reload_presets(&self) {
        let all = presets::all_presets();
        let preset_names: Vec<String> = all.iter().map(|p| p.name.clone()).collect();
        self.handle.update(|tray| {
            if let Some(ref mut props) = tray.device_properties {
                props.eq_preset_options = preset_names;
            }
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
                icon_name: "audio-headset".into(),
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
                hyper_headset::devices::PropertyDescriptorWrapper::Select {
                    descriptor,
                    options,
                } => {
                    if options.is_empty() {
                        // No options available — show as read-only label if data exists
                        if let Some(ref current_value) = descriptor.data {
                            menu_items.push(
                                StandardItem {
                                    label: format!(
                                        "{} {}{}",
                                        descriptor.prefix, current_value, descriptor.suffix
                                    ),
                                    enabled: false,
                                    ..Default::default()
                                }
                                .into(),
                            );
                        }
                        continue;
                    }

                    menu_items.push(MenuItem::Separator);

                    let active_name = device_properties.active_eq_preset.as_deref();
                    let eq_synced = device_properties.eq_synced.unwrap_or(false);
                    let active_index = active_name
                        .and_then(|name| options.iter().position(|n| n == name));

                    let applying_name = if !eq_synced { active_name } else { None };
                    let radio_options: Vec<RadioItem> = options
                        .iter()
                        .map(|name| {
                            let label = if applying_name == Some(name.as_str()) {
                                escape_label(&format!("{} (applying...)", name))
                            } else {
                                escape_label(name)
                            };
                            RadioItem {
                                label,
                                enabled: true,
                                ..Default::default()
                            }
                        })
                        .collect();

                    let options_clone = options.clone();
                    let mut submenu_items: Vec<MenuItem<Self>> = vec![
                        RadioGroup {
                            selected: active_index.unwrap_or(usize::MAX),
                            select: Box::new(move |this: &mut Self, index| {
                                if let Some(name) = options_clone.get(index).cloned() {
                                    let _ = this.update_sender.send(DeviceEvent::EqualizerPreset(name));
                                }
                            }),
                            options: radio_options,
                        }
                        .into(),
                    ];

                    submenu_items.push(MenuItem::Separator);
                    submenu_items.push(
                        StandardItem {
                            label: "Edit with: hyper__headset__cli --eq".into(),
                            enabled: false,
                            ..Default::default()
                        }
                        .into(),
                    );

                    menu_items.push(
                        SubMenu {
                            label: format!("{} {}", descriptor.prefix, descriptor.data.as_deref().unwrap_or("None")),
                            submenu: submenu_items,
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
