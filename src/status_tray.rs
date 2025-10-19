use hyper_headset::devices::DeviceState;
use ksni::{menu::StandardItem, Handle, MenuItem, ToolTip, Tray, TrayService};

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
        self.handle.update(|tray| {
            tray.message = message;
            tray.device_name = name;
        })
    }
}

pub struct StatusTray {
    device_name: Option<String>,
    message: String,
}

impl StatusTray {
    pub fn new() -> Self {
        StatusTray {
            device_name: None,
            message: NO_COMPATIBLE_DEVICE.to_string(),
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
            .clone()
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
