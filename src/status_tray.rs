use std::sync::mpsc::Sender;

use hyper_headset::devices::{DeviceEvent, DeviceProperties, DeviceState, PropertyType};
use ksni::{
    menu::{StandardItem, SubMenu},
    Handle, MenuItem, ToolTip, Tray, TrayService,
};

use crate::tray_battery_icon_state::TrayBatteryIconState;

#[cfg(feature = "eq-support")]
use hyper_headset::eq::presets;
#[cfg(feature = "eq-support")]
use hyper_headset::eq::TrayCommand;
#[cfg(feature = "eq-support")]
use ksni::menu::{RadioGroup, RadioItem};

/// Escape underscores for ksni labels (single `_` is an accelerator prefix).
#[cfg(feature = "eq-support")]
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
        let profile = presets::load_selected_profile();
        let preset_names: Vec<String> = all.iter().map(|p| p.name.clone()).collect();
        let synced = profile.synced;
        let active_name = profile.active_preset;

        let active_index = active_name
            .as_ref()
            .and_then(|name| preset_names.iter().position(|n| n == name));

        self.handle.update(|tray| {
            tray.eq_presets = preset_names;
            tray.active_eq_preset = active_index;
            tray.eq_synced = synced;
            tray.active_preset_name = active_name;
        })
    }
}

pub struct StatusTray {
    device_properties: Option<DeviceProperties>,
    update_sender: Sender<DeviceEvent>,
    #[cfg(feature = "eq-support")]
    command_tx: Sender<TrayCommand>,
    #[cfg(feature = "eq-support")]
    eq_presets: Vec<String>,
    #[cfg(feature = "eq-support")]
    active_eq_preset: Option<usize>,
    #[cfg(feature = "eq-support")]
    eq_synced: bool,
    #[cfg(feature = "eq-support")]
    active_preset_name: Option<String>,
}

impl StatusTray {
    #[cfg(feature = "eq-support")]
    pub fn new(update_sender: Sender<DeviceEvent>, command_tx: Sender<TrayCommand>) -> Self {
        let all = presets::all_presets();
        let profile = presets::load_selected_profile();
        let preset_names: Vec<String> = all.iter().map(|p| p.name.clone()).collect();

        let eq_synced = profile.synced;
        let active_preset_name = profile.active_preset;
        let active_preset = active_preset_name
            .as_ref()
            .and_then(|name| preset_names.iter().position(|n| n == name));

        StatusTray {
            device_properties: None,
            update_sender,
            command_tx,
            eq_presets: preset_names,
            active_eq_preset: active_preset,
            eq_synced,
            active_preset_name,
        }
    }

    #[cfg(not(feature = "eq-support"))]
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
        #[allow(unused_mut)]
        let mut description = if device_properties.connected.unwrap_or(false) {
            device_properties
                .to_string_with_padding(0)
                .lines()
                .filter(|l| !l.contains("Unknown"))
                .collect::<Vec<&str>>()
                .join("\n")
        } else {
            HEADSET_NOT_CONNECTED.to_string()
        };

        // Show EQ info in tooltip only when connected and EQ is supported
        #[cfg(feature = "eq-support")]
        if device_properties.connected.unwrap_or(false) && device_properties.can_set_equalizer {
            if let Some(ref name) = self.active_preset_name {
                if self.eq_synced {
                    description.push_str(&format!("\nEQ: {}", name));
                } else {
                    description.push_str(&format!("\nEQ: {} (not synced)", name));
                }
            }
        }

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

        // EQ preset submenu — only when headset supports EQ and is connected
        #[cfg(feature = "eq-support")]
        if device_properties.can_set_equalizer && !self.eq_presets.is_empty() {
            menu_items.push(MenuItem::Separator);

            let applying_name = if !self.eq_synced {
                self.active_preset_name.as_deref()
            } else {
                None
            };
            let radio_options: Vec<RadioItem> = self
                .eq_presets
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

            let mut submenu_items: Vec<MenuItem<Self>> = vec![
                RadioGroup {
                    selected: self.active_eq_preset.unwrap_or(usize::MAX),
                    select: Box::new(|this: &mut Self, index| {
                        if let Some(name) = this.eq_presets.get(index).cloned() {
                            let profile = presets::SelectedProfile {
                                active_preset: Some(name.clone()),
                                synced: false,
                            };
                            let _ = presets::save_selected_profile(&profile);
                            let _ = this.command_tx.send(TrayCommand::ApplyEqPreset(name.clone()));
                            this.active_eq_preset = Some(index);
                            this.active_preset_name = Some(name);
                            this.eq_synced = false;
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
                    label: "EQ Preset".into(),
                    submenu: submenu_items,
                    ..Default::default()
                }
                .into(),
            );
        }

        #[cfg(not(feature = "eq-support"))]
        if device_properties.can_set_equalizer {
            menu_items.push(MenuItem::Separator);
            menu_items.push(
                StandardItem {
                    label: "EQ presets available — rebuild with --features eq-support".into(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
        }

        menu_items.push(MenuItem::Separator);
        menu_items.push(make_exit().into());
        menu_items
    }
}
