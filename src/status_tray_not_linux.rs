use std::{
    collections::HashMap,
    sync::{mpsc::Sender, Arc, Mutex},
};

use hyper_headset::devices::{DeviceEvent, DeviceProperties, PropertyType};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu},
    TrayIcon, TrayIconBuilder,
};
use winit::{application::ApplicationHandler, event::StartCause};

const NO_COMPATIBLE_DEVICE: &str = "No compatible device found. Is the dongle plugged in?";
const HEADSET_NOT_CONNECTED: &str = "Headset is not connected";

#[cfg(target_os = "windows")]
fn create_tray_icon() -> tray_icon::Icon {
    // embed a headset .ico/.png at compile time — no file needed at runtime
    let bytes = include_bytes!("../assets/headphone.png");
    let img = image::load_from_memory(bytes).unwrap().into_rgba8();
    let (w, h) = img.dimensions();
    tray_icon::Icon::from_rgba(img.into_raw(), w, h).unwrap()
}

type CallbackMap = Arc<Mutex<HashMap<MenuId, Box<dyn Fn() + Send + Sync>>>>;

pub struct TrayApp {
    pub tray_icon: Option<TrayIcon>,
    pub sender: Sender<DeviceEvent>,
    callbacks: CallbackMap,
    current_state: Option<Option<DeviceProperties>>,
}

impl ApplicationHandler<Option<DeviceProperties>> for TrayApp {
    fn new_events(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop, cause: StartCause) {
        if cause == StartCause::Init {
            #[cfg(target_os = "windows")]
            unsafe {
                enable_dark_context_menus();
            }

            #[cfg(target_os = "windows")]
            {
                self.tray_icon = Some(
                    TrayIconBuilder::new()
                        .with_menu(Box::new(Menu::new()))
                        .with_icon(create_tray_icon())
                        .with_tooltip(NO_COMPATIBLE_DEVICE)
                        .with_menu_on_left_click(true)
                        .build()
                        .unwrap(),
                );
            }
            #[cfg(target_os = "macos")]
            {
                self.tray_icon = Some(
                    TrayIconBuilder::new()
                        .with_menu(Box::new(Menu::new()))
                        .with_title("🎧")
                        .with_tooltip(NO_COMPATIBLE_DEVICE)
                        .with_menu_on_left_click(true)
                        .build()
                        .unwrap(),
                );
            }

            self.update(None);
        }
    }

    fn user_event(
        &mut self,
        _el: &winit::event_loop::ActiveEventLoop,
        device_properties: Option<DeviceProperties>,
    ) {
        self.update(device_properties);
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

        #[cfg(target_os = "windows")]
        let quit_item = MenuItem::new("Quit", true, None);

        let menu = Menu::new();
        let mut new_callbacks: HashMap<MenuId, Box<dyn Fn() + Send + Sync>> = HashMap::new();

        let Some(device_properties) = device_properties else {
            let _ = tray.set_tooltip(Some(NO_COMPATIBLE_DEVICE));
            #[cfg(target_os = "macos")]
            tray.set_title(Some(&format!("🎧?")));
            let status_item = MenuItem::new(NO_COMPATIBLE_DEVICE, false, None);
            menu.append(&status_item).unwrap();
            menu.append(&PredefinedMenuItem::separator()).unwrap();

            #[cfg(target_os = "windows")]
            {
                menu.append(&quit_item).unwrap();
                new_callbacks.insert(quit_item.id().clone(), Box::new(|| std::process::exit(0)));
            }

            #[cfg(target_os = "macos")]
            menu.append(&PredefinedMenuItem::quit(Some("Quit")))
                .unwrap();

            *self.callbacks.lock().unwrap() = new_callbacks;
            tray.set_menu(Some(Box::new(menu)));
            self.current_state = Some(device_properties);
            return;
        };

        if !device_properties.connected.unwrap_or(false) {
            let _ = tray.set_tooltip(Some(HEADSET_NOT_CONNECTED));
            #[cfg(target_os = "macos")]
            tray.set_title(Some(&format!("🎧?")));
            let status_item = MenuItem::new(HEADSET_NOT_CONNECTED, false, None);
            menu.append(&status_item).unwrap();
            menu.append(&PredefinedMenuItem::separator()).unwrap();

            #[cfg(target_os = "windows")]
            {
                menu.append(&quit_item).unwrap();
                new_callbacks.insert(quit_item.id().clone(), Box::new(|| std::process::exit(0)));
            }

            #[cfg(target_os = "macos")]
            menu.append(&PredefinedMenuItem::quit(Some("Quit")))
                .unwrap();

            *self.callbacks.lock().unwrap() = new_callbacks;
            tray.set_menu(Some(Box::new(menu)));
            self.current_state = Some(Some(device_properties));
            return;
        }

        #[cfg(target_os = "macos")]
        let _ = tray.set_tooltip(Some(
            device_properties
                .to_string_with_padding(0)
                .lines()
                .filter(|l| !l.contains("Unknown"))
                .collect::<Vec<&str>>()
                .join("\n"),
        ));

        #[cfg(target_os = "windows")]
        let _ = tray.set_tooltip(Some(
            device_properties
                .to_string_with_padding(0)
                .lines()
                .take(2)
                .filter(|l| !l.contains("Unknown"))
                .collect::<Vec<&str>>()
                .join("\n"),
        ));

        #[cfg(target_os = "macos")]
        if let Some(battery_level) = device_properties.battery_level {
            tray.set_title(Some(&format!("🎧 {battery_level}%")));
        }

        for property in device_properties.get_properties() {
            match property {
                hyper_headset::devices::PropertyDescriptorWrapper::Int(property, []) => {
                    let Some(current_value) = property.data else {
                        continue;
                    };
                    let menu_item = MenuItem::new(
                        format!("{} {}{}", property.prefix, current_value, property.suffix),
                        false,
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
                        let entry =
                            MenuItem::new(format!("{}{}", item_value, property.suffix), true, None);
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
                    let menu_item = MenuItem::new(
                        format!("{} {}{}", property.prefix, current_value, property.suffix),
                        property.property_type == PropertyType::ReadWrite
                            && property.data.is_some(),
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
                    let menu_item = MenuItem::new(
                        format!("{} {}{}", property.prefix, current_value, property.suffix),
                        false,
                        None,
                    );
                    let _ = menu.append(&menu_item);
                }
            }
        }

        menu.append(&PredefinedMenuItem::separator()).unwrap();

        #[cfg(target_os = "windows")]
        {
            menu.append(&quit_item).unwrap();
            new_callbacks.insert(quit_item.id().clone(), Box::new(|| std::process::exit(0)));
        }

        #[cfg(target_os = "macos")]
        menu.append(&PredefinedMenuItem::quit(Some("Quit")))
            .unwrap();

        *self.callbacks.lock().unwrap() = new_callbacks;
        tray.set_menu(Some(Box::new(menu)));
        self.current_state = Some(Some(device_properties));
    }
}

#[cfg(target_os = "windows")]
/// Dark magic to set dark mode
unsafe fn enable_dark_context_menus() {
    use windows::core::PCSTR;
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

    let uxtheme = LoadLibraryW(windows::core::w!("uxtheme.dll")).unwrap();

    // SetPreferredAppMode is ordinal 135 (undocumented, no name export)
    type SetPreferredAppMode = unsafe extern "system" fn(i32) -> i32;
    if let Some(func) = GetProcAddress(uxtheme, PCSTR(135 as *const u8)) {
        let set_mode: SetPreferredAppMode = std::mem::transmute(func);
        set_mode(1); // 1 = AllowDark (follows system theme)
    }

    // FlushMenuThemes is ordinal 136 — applies the change immediately
    type FlushMenuThemes = unsafe extern "system" fn();
    if let Some(func) = GetProcAddress(uxtheme, PCSTR(136 as *const u8)) {
        let flush: FlushMenuThemes = std::mem::transmute(func);
        flush();
    }
}
