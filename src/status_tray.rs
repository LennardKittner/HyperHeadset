use std::sync::mpsc::Sender;

use hyper_headset::devices::{format_int_value, DeviceEvent, DeviceProperties, PropertyType};
use ksni::{
    menu::{StandardItem, SubMenu},
    Handle, MenuItem, ToolTip, Tray, TrayService,
};

use crate::tray_battery_icon_state::TrayBatteryIconState;

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

    pub fn update(&self, properties: &DeviceProperties) {
        let device_properties = properties.clone();
        self.handle.update(|tray| {
            #[cfg(feature = "eq-support")]
            if let Some((_, ref to)) = tray.pending_eq_transition {
                // Clear pending transition once the target preset is confirmed active and synced.
                if device_properties.active_eq_preset.as_deref() == Some(to.as_str())
                    && device_properties.eq_synced == Some(true)
                {
                    tray.pending_eq_transition = None;
                }
            }
            tray.device_properties = Some(device_properties);
        })
    }

    pub fn clear_state(&self) {
        self.handle.update(|tray| {
            #[cfg(feature = "eq-support")]
            { tray.pending_eq_transition = None; }
            tray.device_properties = None;
        })
    }

}

pub struct StatusTray {
    theme_name: Option<String>,
    device_properties: Option<DeviceProperties>,
    update_sender: Sender<DeviceEvent>,
    monochrome_icons: bool,
    /// Tracks an in-flight EQ preset switch so the menu gives instant visual feedback
    /// before the main loop confirms the HID writes finished. Cleared once synced.
    #[cfg(feature = "eq-support")]
    pending_eq_transition: Option<(String, String)>, // (from, to)
}

impl StatusTray {
    pub fn new(update_sender: Sender<DeviceEvent>, monochrome_icons: bool) -> Self {
        let theme_name = linicon::get_system_theme();
        StatusTray {
            theme_name,
            device_properties: None,
            update_sender,
            monochrome_icons,
            #[cfg(feature = "eq-support")]
            pending_eq_transition: None,
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
            .linux_icon_name(self.monochrome_icons, self.theme_name.as_ref())
            .to_string()
    }

    fn tool_tip(&self) -> ToolTip {
        let Some(device_properties) = self.device_properties.as_ref() else {
            return ToolTip {
                title: "Unknown".to_string(),
                description: NO_COMPATIBLE_DEVICE.to_string(),
                icon_name: TrayBatteryIconState::NoDevice
                    .linux_icon_name(self.monochrome_icons, self.theme_name.as_ref()),
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
                .linux_icon_name(self.monochrome_icons, self.theme_name.as_ref())
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
                #[cfg(feature = "eq-support")]
                hyper_headset::devices::PropertyDescriptorWrapper::SelectEQ {
                    descriptor,
                    options,
                    active_preset,
                    synced,
                } => {
                    if options.is_empty() {
                        // No options available — show as read-only label if data exists
                        if let Some(ref current_value) = descriptor.data {
                            menu_items.push(
                                StandardItem {
                                    label: escape_label(&format!(
                                        "{}: {}{}",
                                        descriptor.pretty_name, current_value, descriptor.suffix
                                    )),
                                    enabled: false,
                                    ..Default::default()
                                }
                                .into(),
                            );
                        }
                        continue;
                    }

                    menu_items.push(MenuItem::Separator);

                    let active_name = active_preset.as_deref();
                    let active_index = active_name
                        .and_then(|name| options.iter().position(|n| n == name));

                    let applying_name = if !synced { active_name } else { None };

                    // Immediate visual feedback: pending_eq_transition is set on click before
                    // the main loop confirms the HID writes, so the spinner and departure marker
                    // appear instantly. At most one of each is shown (latest wins).
                    let pending_target = self
                        .pending_eq_transition
                        .as_ref()
                        .map(|(_, to)| to.as_str());
                    let pending_depart = self
                        .pending_eq_transition
                        .as_ref()
                        .map(|(from, _)| from.as_str());

                    let current_active = active_name.map(str::to_owned);

                    // Use StandardItem (not RadioGroup) so that clicking a preset closes the
                    // menu — KDE Plasma doesn't re-render radio toggle-state while a submenu
                    // is open, causing stale checked circles to accumulate across selections.
                    let mut submenu_items: Vec<MenuItem<StatusTray>> = options
                        .iter()
                        .enumerate()
                        .map(|(idx, name)| {
                            let label = if pending_target == Some(name.as_str()) {
                                // Spinner: user just selected this, HID writes in progress.
                                format!("↻ {}", escape_label(name))
                            } else if pending_depart == Some(name.as_str()) {
                                // Departure marker: this was active, switching away from it.
                                format!("· {}", escape_label(name))
                            } else if applying_name == Some(name.as_str()) {
                                escape_label(&format!("{} (applying...)", name))
                            } else if Some(idx) == active_index {
                                format!("✓ {}", escape_label(name))
                            } else {
                                format!("  {}", escape_label(name))
                            };
                            let name_clone = name.clone();
                            let current_active_clone = current_active.clone();
                            StandardItem {
                                label,
                                enabled: true,
                                activate: Box::new(move |this: &mut StatusTray| {
                                    // Set immediately so the next menu() call shows feedback
                                    // before the main loop has time to update device_properties.
                                    this.pending_eq_transition = Some((
                                        current_active_clone.clone().unwrap_or_default(),
                                        name_clone.clone(),
                                    ));
                                    let _ = this.update_sender.send(
                                        DeviceEvent::EqualizerPreset(name_clone.clone()),
                                    );
                                }),
                                ..Default::default()
                            }
                            .into()
                        })
                        .collect();

                    #[cfg(feature = "eq-editor")]
                    {
                        submenu_items.push(MenuItem::Separator);
                        submenu_items.push(
                            StandardItem {
                                label: escape_label("Edit with: hyper_headset_cli --eq"),
                                enabled: true,
                                activate: Box::new(|_| {
                                    hyper_headset::launch_eq_editor();
                                }),
                                ..Default::default()
                            }
                            .into(),
                        );
                    }

                    menu_items.push(
                        SubMenu {
                            label: escape_label(&format!("{}: {}", descriptor.pretty_name, descriptor.data.as_deref().unwrap_or("Unknown"))),
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
