use hyper_headset::devices::DeviceState;
use hyper_headset::eq::presets;
use hyper_headset::eq::TrayCommand;
use ksni::{
    menu::{RadioGroup, RadioItem, StandardItem, SubMenu},
    Handle, MenuItem, ToolTip, Tray, TrayService,
};
use std::sync::mpsc::Sender;

pub struct TrayHandler {
    handle: Handle<StatusTray>,
}

const NO_COMPATIBLE_DEVICE: &str = "No compatible device found.\nIs the dongle plugged in?\nIf you are using Linux did you add the Udev rules?";

impl TrayHandler {
    pub fn new(tray: StatusTray) -> Self {
        let tray_service = TrayService::new(tray);
        let handle = tray_service.handle();
        tray_service.spawn();
        TrayHandler { handle }
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
        self.handle.update(|tray| {
            tray.message = message;
            tray.device_name = name;
            tray.can_set_equalizer = can_set_equalizer;
        })
    }

    pub fn reload_presets(&self) {
        let all = presets::all_presets();
        let profile = presets::load_selected_profile();
        let preset_names: Vec<String> = all.iter().map(|p| p.name.clone()).collect();
        let active_preset = profile
            .active_preset
            .as_ref()
            .and_then(|name| preset_names.iter().position(|n| n == name));

        self.handle.update(|tray| {
            tray.eq_presets = preset_names;
            tray.active_eq_preset = active_preset;
        })
    }
}

pub struct StatusTray {
    device_name: Option<String>,
    message: String,
    command_tx: Sender<TrayCommand>,
    eq_presets: Vec<String>,
    active_eq_preset: Option<usize>,
    can_set_equalizer: bool,
}

impl StatusTray {
    pub fn new(command_tx: Sender<TrayCommand>) -> Self {
        let all = presets::all_presets();
        let profile = presets::load_selected_profile();
        let preset_names: Vec<String> = all.iter().map(|p| p.name.clone()).collect();
        let active_preset = profile
            .active_preset
            .as_ref()
            .and_then(|name| preset_names.iter().position(|n| n == name));

        StatusTray {
            device_name: None,
            message: NO_COMPATIBLE_DEVICE.to_string(),
            command_tx,
            eq_presets: preset_names,
            active_eq_preset: active_preset,
            can_set_equalizer: false,
        }
    }
}

impl Tray for StatusTray {
    fn id(&self) -> String {
        env!("CARGO_PKG_NAME").into()
    }
    fn icon_name(&self) -> String {
        "audio-headset".into()
    }
    fn tool_tip(&self) -> ToolTip {
        let description = self
            .message
            .lines()
            .filter(|l| !l.contains("Unknown"))
            .collect::<Vec<&str>>()
            .join("\n");
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
                    label: line.to_string(),
                    enabled: false,
                    ..Default::default()
                }
                .into()
            })
            .collect();

        if self.can_set_equalizer && !self.eq_presets.is_empty() {
            state_items.push(MenuItem::Separator);

            let radio_options: Vec<RadioItem> = self
                .eq_presets
                .iter()
                .map(|name| RadioItem {
                    label: name.clone(),
                    ..Default::default()
                })
                .collect();

            let submenu_items: Vec<MenuItem<Self>> = vec![
                RadioGroup {
                    selected: self.active_eq_preset.unwrap_or(usize::MAX),
                    select: Box::new(|this: &mut Self, index| {
                        if let Some(name) = this.eq_presets.get(index).cloned() {
                            // Persist selection immediately so other processes (TUI) see it
                            let profile = presets::SelectedProfile {
                                active_preset: Some(name.clone()),
                            };
                            let _ = presets::save_selected_profile(&profile);
                            let _ = this.command_tx.send(TrayCommand::ApplyEqPreset(name));
                            this.active_eq_preset = Some(index);
                        }
                    }),
                    options: radio_options,
                }
                .into(),
                MenuItem::Separator,
                StandardItem {
                    label: "Edit with: hyper_headset_cli --eq".into(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            ];

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
