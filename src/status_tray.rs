use std::sync::mpsc::Sender;
use std::sync::Arc;

use hyper_headset::devices::DeviceState;
use hyper_headset::eq::popup::{EqPopupController, PopupCommand, PopupState};
use hyper_headset::eq::presets;
use hyper_headset::eq::TrayCommand;
use ksni::{
    menu::{RadioGroup, RadioItem, StandardItem, SubMenu},
    Handle, MenuItem, ToolTip, Tray, TrayService,
};

/// Escape underscores for ksni labels (single `_` is an accelerator prefix).
fn escape_label(s: &str) -> String {
    s.replace('_', "__")
}

pub struct TrayHandler {
    handle: Handle<StatusTray>,
    popup_controller: Option<Arc<dyn EqPopupController>>,
}

const NO_COMPATIBLE_DEVICE: &str = "No compatible device found.\nIs the dongle plugged in?\nIf you are using Linux did you add the Udev rules?";

impl TrayHandler {
    pub fn new(tray: StatusTray, popup_controller: Option<Arc<dyn EqPopupController>>) -> Self {
        let tray_service = TrayService::new(tray);
        let handle = tray_service.handle();
        tray_service.spawn();
        TrayHandler {
            handle,
            popup_controller,
        }
    }

    pub fn update(&self, device_state: &DeviceState) {
        let (message, name) = match device_state.connected {
            None => (NO_COMPATIBLE_DEVICE.to_string(), None),
            Some(false) => (
                "Headset is not connected".to_string(),
                device_state.device_name.clone(),
            ),
            Some(true) => (
                device_state.to_string_with_padding(0),
                device_state.device_name.clone(),
            ),
        };
        let can_set_equalizer = device_state.can_set_equalizer;
        let is_connected = device_state.connected == Some(true);
        self.handle.update(|tray| {
            tray.message = message;
            tray.device_name = name;
            tray.can_set_equalizer = can_set_equalizer;
            tray.is_connected = is_connected;
        })
    }

    pub fn reload_presets(&self) {
        let all = presets::all_presets();
        let profile = presets::load_selected_profile();
        let preset_names: Vec<String> = all.iter().map(|p| p.name.clone()).collect();
        let synced = profile.synced;
        let active_name = profile.active_preset;

        let active_index = active_name
            .as_ref()
            .and_then(|name| preset_names.iter().position(|n| n == name));

        // Update popup if present
        if let Some(ref popup) = self.popup_controller {
            popup.send(PopupCommand::UpdateState(PopupState {
                presets: preset_names.clone(),
                active_preset: active_name.clone(),
                synced,
                is_connected: true, // reload_presets is only called when device is available
            }));
        }

        self.handle.update(|tray| {
            tray.eq_presets = preset_names;
            tray.active_eq_preset = active_index;
            tray.eq_synced = synced;
            tray.active_preset_name = active_name;
        })
    }

    /// Hide the popup (e.g. on device disconnect or reconnect loop).
    pub fn hide_popup(&self) {
        if let Some(ref popup) = self.popup_controller {
            popup.send(PopupCommand::Hide);
        }
    }
}

pub struct StatusTray {
    device_name: Option<String>,
    message: String,
    command_tx: Sender<TrayCommand>,
    eq_presets: Vec<String>,
    active_eq_preset: Option<usize>,
    can_set_equalizer: bool,
    is_connected: bool,
    eq_synced: bool,
    active_preset_name: Option<String>,
    popup_controller: Option<Arc<dyn EqPopupController>>,
}

impl StatusTray {
    pub fn new(
        command_tx: Sender<TrayCommand>,
        popup_controller: Option<Arc<dyn EqPopupController>>,
    ) -> Self {
        let all = presets::all_presets();
        let profile = presets::load_selected_profile();
        let preset_names: Vec<String> = all.iter().map(|p| p.name.clone()).collect();

        let eq_synced = profile.synced;
        let active_preset_name = profile.active_preset;
        let active_preset = active_preset_name
            .as_ref()
            .and_then(|name| preset_names.iter().position(|n| n == name));

        StatusTray {
            device_name: None,
            message: NO_COMPATIBLE_DEVICE.to_string(),
            command_tx,
            eq_presets: preset_names,
            active_eq_preset: active_preset,
            can_set_equalizer: false,
            is_connected: false,
            eq_synced,
            active_preset_name,
            popup_controller,
        }
    }
}

impl Tray for StatusTray {
    fn activate(&mut self, x: i32, y: i32) {
        if let Some(ref popup) = self.popup_controller {
            if self.can_set_equalizer {
                popup.send(PopupCommand::Show {
                    x,
                    y,
                    state: PopupState {
                        presets: self.eq_presets.clone(),
                        active_preset: self.active_preset_name.clone(),
                        synced: self.eq_synced,
                        is_connected: self.is_connected,
                    },
                });
            }
        }
    }
    fn id(&self) -> String {
        env!("CARGO_PKG_NAME").into()
    }
    fn icon_name(&self) -> String {
        "audio-headset".into()
    }
    fn tool_tip(&self) -> ToolTip {
        let mut description = self
            .message
            .lines()
            .filter(|l| !l.contains("Unknown"))
            .collect::<Vec<&str>>()
            .join("\n");
        if let Some(ref name) = self.active_preset_name {
            if self.eq_synced {
                description.push_str(&format!("\nEQ: {}", name));
            } else {
                description.push_str(&format!("\nEQ: {} (not synced)", name));
            }
        }
        ToolTip {
            title: self.device_name.clone().unwrap_or("Unknown".to_string()),
            description,
            icon_name: "audio-headset".into(),
            icon_pixmap: Vec::new(),
        }
    }
    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut state_items: Vec<MenuItem<Self>> = self
            .message
            .lines()
            .map(|line| {
                StandardItem {
                    label: escape_label(line),
                    enabled: false,
                    ..Default::default()
                }
                .into()
            })
            .collect();

        if self.can_set_equalizer && !self.eq_presets.is_empty() {
            state_items.push(MenuItem::Separator);

            let is_connected = self.is_connected;
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
                        enabled: is_connected,
                        ..Default::default()
                    }
                })
                .collect();

            let mut submenu_items: Vec<MenuItem<Self>> = vec![
                RadioGroup {
                    selected: self.active_eq_preset.unwrap_or(usize::MAX),
                    select: Box::new(|this: &mut Self, index| {
                        if let Some(name) = this.eq_presets.get(index).cloned() {
                            // Save with synced=false — main loop confirms sync
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

            state_items.push(
                SubMenu {
                    label: "EQ Preset".into(),
                    submenu: submenu_items,
                    ..Default::default()
                }
                .into(),
            );
        }

        let exit = StandardItem {
            label: "Exit".into(),
            icon_name: "application-exit".into(),
            activate: Box::new(|_| std::process::exit(0)),
            ..Default::default()
        };
        state_items.push(exit.into());
        state_items
    }
}
