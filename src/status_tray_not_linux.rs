use std::{
    collections::HashMap,
    sync::{mpsc::Sender, Arc, Mutex},
};

use hyper_headset::devices::{DeviceEvent, DeviceProperties, PropertyType};
use tray_icon::{
    menu::{IconMenuItem, Menu, MenuEvent, MenuId, Submenu},
    TrayIcon, TrayIconBuilder,
};
use winit::{application::ApplicationHandler, event::StartCause, event_loop::ControlFlow};

//TODO: maybe use MenuItem instead of IconMenuItem but than I probably have to patch Muda because
//it crashes sometimes when trying to handle an image with zero size

const NO_COMPATIBLE_DEVICE: &str = "No compatible device found. Is the dongle plugged in?";
const HEADSET_NOT_CONNECTED: &str = "Headset is not connected";

fn placeholder_icon() -> tray_icon::menu::Icon {
    tray_icon::menu::Icon::from_rgba(vec![0, 0, 0, 0], 1, 1).unwrap()
}

type CallbackMap = Arc<Mutex<HashMap<MenuId, Box<dyn Fn() + Send + Sync>>>>;

pub struct TrayApp {
    pub tray_icon: Option<TrayIcon>,
    pub sender: Sender<DeviceEvent>,
    callbacks: CallbackMap,
    current_state: Option<Option<DeviceProperties>>,
    //TODO: maybe not needed anymore?
    pending_update: Option<Option<DeviceProperties>>,
}

impl ApplicationHandler<Option<DeviceProperties>> for TrayApp {
    fn new_events(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop, cause: StartCause) {
        if cause == StartCause::Init {
            self.tray_icon = Some(
                TrayIconBuilder::new()
                    .with_menu(Box::new(Menu::new()))
                    .with_title("🎧")
                    .with_tooltip(NO_COMPATIBLE_DEVICE)
                    .build()
                    .unwrap(),
            );

            self.update(None);
        }
    }

    fn user_event(
        &mut self,
        el: &winit::event_loop::ActiveEventLoop,
        device_properties: Option<DeviceProperties>,
    ) {
        // Don't call set_menu here — macOS menu is still active at this point.
        // Buffer the update and apply it once the event loop is idle.
        self.pending_update = Some(device_properties);
        el.set_control_flow(ControlFlow::Poll); // wake about_to_wait immediately
    }

    // Called once the event loop has drained all pending events — menu is closed by now
    fn about_to_wait(&mut self, el: &winit::event_loop::ActiveEventLoop) {
        if let Some(props) = self.pending_update.take() {
            self.update(props);
        }
        el.set_control_flow(ControlFlow::Wait); // go back to sleeping
    }

    fn resumed(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {}

    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        _event: winit::event::WindowEvent,
    ) {
    }
}

impl TrayApp {
    pub fn new(sender: Sender<DeviceEvent>) -> Self {
        let callbacks: CallbackMap = Arc::new(Mutex::new(HashMap::new()));

        let callbacks_clone = Arc::clone(&callbacks);

        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            if let Ok(map) = callbacks_clone.try_lock() {
                if let Some(f) = map.get(&e.id) {
                    f();
                }
            }
            // Unknown id (read-only items, stale events) → silently ignored
        }));

        Self {
            tray_icon: None,
            sender,
            callbacks,
            current_state: None,
            pending_update: None,
        }
    }

    fn update(&mut self, device_properties: Option<DeviceProperties>) {
        if let Some(current_state) = self.current_state.as_ref() {
            if current_state == &device_properties {
                return;
            }
        }
        let Some(tray) = &mut self.tray_icon else {
            return;
        };

        let quit_item = IconMenuItem::new("Quit", true, Some(placeholder_icon()), None);
        let menu = Menu::new();
        let mut new_callbacks: HashMap<MenuId, Box<dyn Fn() + Send + Sync>> = HashMap::new();

        let Some(device_properties) = device_properties else {
            let _ = tray.set_tooltip(Some(NO_COMPATIBLE_DEVICE));
            tray.set_title(Some(&format!("🎧?")));
            let status_item =
                IconMenuItem::new(NO_COMPATIBLE_DEVICE, false, Some(placeholder_icon()), None);
            menu.append(&status_item).unwrap();
            menu.append(&quit_item).unwrap();
            new_callbacks.insert(quit_item.id().clone(), Box::new(|| std::process::exit(0)));

            *self.callbacks.lock().unwrap() = new_callbacks;
            tray.set_menu(Some(Box::new(menu)));
            self.current_state = Some(device_properties);
            return;
        };

        if !device_properties.connected.unwrap_or(false) {
            let _ = tray.set_tooltip(Some(HEADSET_NOT_CONNECTED));
            tray.set_title(Some(&format!("🎧?")));
            let status_item =
                IconMenuItem::new(HEADSET_NOT_CONNECTED, false, Some(placeholder_icon()), None);
            menu.append(&status_item).unwrap();
            menu.append(&quit_item).unwrap();
            new_callbacks.insert(quit_item.id().clone(), Box::new(|| std::process::exit(0)));

            *self.callbacks.lock().unwrap() = new_callbacks;
            tray.set_menu(Some(Box::new(menu)));
            self.current_state = Some(Some(device_properties));
            return;
        }

        let _ = tray.set_tooltip(Some(
            device_properties
                .to_string_with_padding(0)
                .lines()
                .filter(|l| !l.contains("Unknown"))
                .collect::<Vec<&str>>()
                .join("\n"),
        ));

        if let Some(battery_level) = device_properties.battery_level {
            tray.set_title(Some(&format!("🎧 {battery_level}%")));
        }

        for property in device_properties.get_properties() {
            match property {
                hyper_headset::devices::PropertyDescriptorWrapper::Int(property, []) => {
                    let Some(current_value) = property.data else {
                        continue;
                    };
                    let menu_item = IconMenuItem::new(
                        format!("{} {}{}", property.prefix, current_value, property.suffix),
                        false,
                        Some(placeholder_icon()),
                        None,
                    );
                    let _ = menu.append(&menu_item);
                }
                hyper_headset::devices::PropertyDescriptorWrapper::Int(property, items) => {
                    let Some(current_value) = property.data else {
                        continue;
                    };
                    let submenu = Submenu::new(
                        format!("{} {}{}", property.prefix, current_value, property.suffix),
                        property.property_type == PropertyType::ReadWrite,
                    );

                    for item_value in items {
                        let entry = IconMenuItem::new(
                            format!("{}{}", item_value, property.suffix),
                            true,
                            Some(placeholder_icon()),
                            None,
                        );
                        submenu.append(&entry).unwrap();

                        let create_event = property.create_event;
                        let tx = self.sender.clone();
                        let entry_id = entry.id().clone();
                        new_callbacks.insert(
                            entry_id,
                            Box::new(move || {
                                if let Some(event) = (create_event)(*item_value) {
                                    let _ = tx.send(event);
                                }
                            }),
                        );
                    }

                    menu.append(&submenu).unwrap();
                }
                hyper_headset::devices::PropertyDescriptorWrapper::Bool(property) => {
                    let Some(current_value) = property.data else {
                        continue;
                    };
                    let create_event = property.create_event;
                    let update_sender = self.sender.clone();
                    let menu_item = IconMenuItem::new(
                        format!("{} {}{}", property.prefix, current_value, property.suffix),
                        property.property_type == PropertyType::ReadWrite
                            && property.data.is_some(),
                        Some(placeholder_icon()),
                        None,
                    );
                    let _ = menu.append(&menu_item);
                    let menu_itme_id = menu_item.id().clone();
                    new_callbacks.insert(
                        menu_itme_id,
                        Box::new(move || {
                            if let Some(command) = (create_event)(!current_value) {
                                let _ = update_sender.send(command);
                            }
                        }),
                    );
                }
                hyper_headset::devices::PropertyDescriptorWrapper::String(property) => {
                    let Some(current_value) = property.data else {
                        continue;
                    };
                    let menu_item = IconMenuItem::new(
                        format!("{} {}{}", property.prefix, current_value, property.suffix),
                        false,
                        Some(placeholder_icon()),
                        None,
                    );
                    let _ = menu.append(&menu_item);
                }
            }
        }

        menu.append(&quit_item).unwrap();
        new_callbacks.insert(quit_item.id().clone(), Box::new(|| std::process::exit(0)));

        *self.callbacks.lock().unwrap() = new_callbacks;
        tray.set_menu(Some(Box::new(menu)));
        self.current_state = Some(Some(device_properties));
    }
}
