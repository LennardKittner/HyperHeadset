use std::{
    collections::HashMap,
    sync::{mpsc::Sender, Arc, Mutex},
};

#[cfg(target_os = "windows")]
use image::{Rgba, RgbaImage};
use hyper_headset::devices::{DeviceEvent, DeviceProperties, PropertyType};
use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu},
    TrayIcon, TrayIconBuilder,
};
use winit::{application::ApplicationHandler, event::StartCause};
#[cfg(target_os = "windows")]
use winreg::{
    enums::{RegType, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE},
    RegKey, RegValue,
};

#[cfg(target_os = "windows")]
use crate::tray_battery_icon_state::{TrayBatteryIconState, WindowsIconKey};

const NO_COMPATIBLE_DEVICE: &str = "No compatible device found. Is the dongle plugged in?";
const HEADSET_NOT_CONNECTED: &str = "Headset is not connected";
#[cfg(target_os = "windows")]
const RUN_KEY_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(target_os = "windows")]
const STARTUP_APPROVED_RUN_KEY_PATH: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
#[cfg(target_os = "windows")]
const STARTUP_VALUE_NAME: &str = "HyperHeadset";
const WINDOWS_ICON_SIZE: u32 = 16;

#[cfg(target_os = "windows")]
fn create_default_tray_icon() -> tray_icon::Icon {
    // embed a headset .ico/.png at compile time — no file needed at runtime
    let bytes = include_bytes!("../assets/headphone.png");
    let img = image::load_from_memory(bytes).unwrap().into_rgba8();
    let (w, h) = img.dimensions();
    tray_icon::Icon::from_rgba(img.into_raw(), w, h).unwrap()
}

#[cfg(target_os = "windows")]
fn draw_rect(image: &mut RgbaImage, x: i32, y: i32, width: i32, height: i32, color: Rgba<u8>) {
    for px in x.max(0)..(x + width).min(WINDOWS_ICON_SIZE as i32) {
        for py in y.max(0)..(y + height).min(WINDOWS_ICON_SIZE as i32) {
            image.put_pixel(px as u32, py as u32, color);
        }
    }
}

#[cfg(target_os = "windows")]
fn draw_digit(
    image: &mut RgbaImage,
    digit: char,
    x: i32,
    y: i32,
    scale: i32,
    color: Rgba<u8>,
) {
    let rows = match digit {
        '0' => ["111", "101", "101", "101", "111"],
        // Narrow upright '1'.
        '1' => ["01", "01", "01", "01", "01"],
        '2' => ["111", "001", "111", "100", "111"],
        '3' => ["111", "001", "111", "001", "111"],
        '4' => ["101", "101", "111", "001", "001"],
        '5' => ["111", "100", "111", "001", "111"],
        '6' => ["111", "100", "111", "101", "111"],
        '7' => ["111", "001", "010", "010", "010"],
        '8' => ["111", "101", "111", "101", "111"],
        '9' => ["111", "101", "111", "001", "111"],
        _ => ["000", "000", "000", "000", "000"],
    };

    for (row_index, row) in rows.iter().enumerate() {
        for (col_index, bit) in row.chars().enumerate() {
            if bit == '1' {
                draw_rect(
                    image,
                    x + (col_index as i32 * scale),
                    y + (row_index as i32 * scale),
                    scale,
                    scale,
                    color,
                );
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn render_windows_battery_icon_rgba(key: WindowsIconKey) -> Vec<u8> {
    let mut image = RgbaImage::from_pixel(WINDOWS_ICON_SIZE, WINDOWS_ICON_SIZE, Rgba([0, 0, 0, 0]));

    // Charging overrides battery-level color with yellow background.
    let background_color = if key.charging {
        Rgba([245, 216, 64, 255])
    } else if key.percent < 30 {
        Rgba([220, 90, 90, 255])
    } else {
        Rgba([96, 196, 106, 255])
    };
    draw_rect(
        &mut image,
        0,
        0,
        WINDOWS_ICON_SIZE as i32,
        WINDOWS_ICON_SIZE as i32,
        background_color,
    );

    // Custom compact "100" layout for 16x16:
    // keeps large text while enforcing spacing between all digits.
    if key.percent == 100 {
        let text_color = Rgba([10, 10, 10, 255]);
        let y = 3;

        // "1" (3x10)
        draw_rect(&mut image, 1, y, 1, 10, text_color);
        draw_rect(&mut image, 0, y + 9, 3, 1, text_color);

        // First "0" (5x10), 1px gap from "1"
        let z1 = 4;
        draw_rect(&mut image, z1, y, 5, 1, text_color);
        draw_rect(&mut image, z1, y + 9, 5, 1, text_color);
        draw_rect(&mut image, z1, y, 1, 10, text_color);
        draw_rect(&mut image, z1 + 4, y, 1, 10, text_color);

        // Second "0" (5x10), 1px gap from first "0"
        let z2 = 10;
        draw_rect(&mut image, z2, y, 5, 1, text_color);
        draw_rect(&mut image, z2, y + 9, 5, 1, text_color);
        draw_rect(&mut image, z2, y, 1, 10, text_color);
        draw_rect(&mut image, z2 + 4, y, 1, 10, text_color);

        return image.into_raw();
    }

    let text = key.percent.to_string();
    let mut scale = 2;
    // Borderless icon: preserve explicit outer padding + spacing between digits.
    let spacing = if text.len() >= 3 { 0 } else { 1 };
    // Allow 100 to use full icon width so it can stay at scale 2.
    let horizontal_padding = if text.len() >= 3 { 0 } else { 1 };
    let inner_left = horizontal_padding;
    let inner_right = (WINDOWS_ICON_SIZE as i32 - 1 - horizontal_padding).max(inner_left);
    let usable_width = (inner_right - inner_left + 1).max(1);

    let mut glyph_widths: Vec<i32> = text
        .chars()
        .map(|digit| if digit == '1' { 2 * scale } else { 3 * scale })
        .collect();
    let mut total_width: i32 = glyph_widths.iter().sum::<i32>()
        + spacing * (text.chars().count().saturating_sub(1) as i32);
    if total_width > usable_width && scale > 1 {
        // On 16x16 icons, enforce padding on both sides over large glyph size.
        scale = 1;
        glyph_widths = text
            .chars()
            .map(|digit| if digit == '1' { 2 * scale } else { 3 * scale })
            .collect();
        total_width = glyph_widths.iter().sum::<i32>()
            + spacing * (text.chars().count().saturating_sub(1) as i32);
    }
    let centered_start_x = inner_left + ((usable_width - total_width).max(0) / 2);
    let max_start_x = (inner_right - total_width + 1).max(inner_left);
    let start_x = centered_start_x.clamp(inner_left, max_start_x);
    let start_y = if scale == 2 { 3 } else { 5 };

    let mut x = start_x;
    for (idx, digit) in text.chars().enumerate() {
        draw_digit(&mut image, digit, x, start_y, scale, Rgba([10, 10, 10, 255]));
        x += glyph_widths[idx] + spacing;
    }

    image.into_raw()
}

type CallbackMap = Arc<Mutex<HashMap<MenuId, Box<dyn Fn() + Send + Sync>>>>;

pub struct TrayApp {
    pub tray_icon: Option<TrayIcon>,
    pub sender: Sender<DeviceEvent>,
    callbacks: CallbackMap,
    current_state: Option<Option<DeviceProperties>>,
    #[cfg(target_os = "windows")]
    icon_cache: HashMap<WindowsIconKey, Vec<u8>>,
    #[cfg(target_os = "windows")]
    current_icon_key: Option<WindowsIconKey>,
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
                        .with_icon(create_default_tray_icon())
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
            #[cfg(target_os = "windows")]
            icon_cache: HashMap::new(),
            #[cfg(target_os = "windows")]
            current_icon_key: None,
        }
    }

    #[cfg(target_os = "windows")]
    fn update_windows_icon(&mut self, device_properties: Option<&DeviceProperties>) {
        let Some(tray) = self.tray_icon.as_ref() else {
            return;
        };
        let icon_state = TrayBatteryIconState::from_device_properties(device_properties);
        let desired_key = icon_state.windows_icon_key();
        if desired_key == self.current_icon_key {
            return;
        }

        if let Some(key) = desired_key {
            let rgba = self
                .icon_cache
                .entry(key)
                .or_insert_with(|| render_windows_battery_icon_rgba(key))
                .clone();
            if let Ok(icon) = tray_icon::Icon::from_rgba(rgba, WINDOWS_ICON_SIZE, WINDOWS_ICON_SIZE)
            {
                let _ = tray.set_icon(Some(icon));
            }
        } else {
            let _ = tray.set_icon(Some(create_default_tray_icon()));
        }

        self.current_icon_key = desired_key;
    }

    fn update(&mut self, device_properties: Option<DeviceProperties>) {
        if let Some(current_state) = self.current_state.as_ref() {
            if current_state == &device_properties {
                return;
            }
        }

        #[cfg(target_os = "windows")]
        self.update_windows_icon(device_properties.as_ref());

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
                append_startup_toggle(&menu, &mut new_callbacks);
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
                append_startup_toggle(&menu, &mut new_callbacks);
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
                hyper_headset::devices::PropertyDescriptorWrapper::SelectEQ {
                    descriptor,
                    options,
                    active_preset,
                    synced,
                } => {
                    let Some(ref current_value) = descriptor.data else {
                        continue;
                    };

                    if options.is_empty() {
                        let menu_item = MenuItem::new(
                            format!(
                                "{} {}{}",
                                descriptor.prefix, current_value, descriptor.suffix
                            ),
                            false,
                            None,
                        );
                        let _ = menu.append(&menu_item);
                        continue;
                    }

                    let submenu = Submenu::new(
                        format!("{} {}", descriptor.prefix, current_value),
                        true,
                    );

                    let applying_name = if !synced {
                        active_preset.as_deref()
                    } else {
                        None
                    };

                    for option_name in &options {
                        let is_active = active_preset
                            .as_ref()
                            .map(|a| a == option_name)
                            .unwrap_or(false);
                        let label = if applying_name == Some(option_name.as_str()) {
                            format!("{} (applying...)", option_name)
                        } else {
                            option_name.clone()
                        };
                        let entry = CheckMenuItem::new(
                            &label,
                            true,
                            is_active,
                            None,
                        );
                        let tx = self.sender.clone();
                        let create_event = descriptor.create_event;
                        let name = option_name.clone();
                        let entry_id = entry.id().clone();
                        new_callbacks.insert(
                            entry_id,
                            Box::new(move || {
                                if let Some(event) = (create_event)(name.clone()) {
                                    let _ = tx.send(event);
                                }
                            }),
                        );
                        let _ = submenu.append(&entry);
                    }

                    let _ = menu.append(&submenu);
                }
            }
        }

        menu.append(&PredefinedMenuItem::separator()).unwrap();

        #[cfg(target_os = "windows")]
        {
            append_startup_toggle(&menu, &mut new_callbacks);
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
fn append_startup_toggle(
    menu: &Menu,
    callbacks: &mut HashMap<MenuId, Box<dyn Fn() + Send + Sync>>,
) {
    let startup_enabled = is_start_with_windows_enabled();
    let startup_item = CheckMenuItem::new("Start with Windows", true, startup_enabled, None);
    let _ = menu.append(&startup_item);
    callbacks.insert(
        startup_item.id().clone(),
        Box::new(|| {
            let currently_enabled = is_start_with_windows_enabled();
            if let Err(error) = set_start_with_windows_enabled(!currently_enabled) {
                eprintln!("Failed to update startup setting: {error}");
            }
        }),
    );
}

#[cfg(target_os = "windows")]
fn startup_command_line() -> std::io::Result<String> {
    let exe_path = std::env::current_exe()?;
    Ok(format!("\"{}\"", exe_path.display()))
}

#[cfg(target_os = "windows")]
fn open_run_key_with_access(access: u32) -> std::io::Result<RegKey> {
    RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY_PATH, access)
}

#[cfg(target_os = "windows")]
fn open_or_create_run_key_with_access(access: u32) -> std::io::Result<RegKey> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (run_key, _) = hkcu.create_subkey_with_flags(RUN_KEY_PATH, access)?;
    Ok(run_key)
}

#[cfg(target_os = "windows")]
fn open_startup_approved_key_with_access(access: u32) -> std::io::Result<RegKey> {
    RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(STARTUP_APPROVED_RUN_KEY_PATH, access)
}

#[cfg(target_os = "windows")]
fn open_or_create_startup_approved_key_with_access(access: u32) -> std::io::Result<RegKey> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey_with_flags(STARTUP_APPROVED_RUN_KEY_PATH, access)?;
    Ok(key)
}

#[cfg(target_os = "windows")]
fn startup_approved_state() -> Option<bool> {
    let Ok(key) = open_startup_approved_key_with_access(KEY_READ) else {
        return None;
    };
    let Ok(value) = key.get_raw_value(STARTUP_VALUE_NAME) else {
        return None;
    };
    match value.bytes.first().copied() {
        Some(0x02) => Some(true),
        Some(0x03) => Some(false),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
fn set_startup_approved_state(enabled: bool) -> std::io::Result<()> {
    let key = open_or_create_startup_approved_key_with_access(KEY_SET_VALUE)?;
    // 0x02 => enabled, 0x03 => disabled (same convention used by Startup Apps)
    let state = if enabled { 0x02u8 } else { 0x03u8 };
    key.set_raw_value(
        STARTUP_VALUE_NAME,
        &RegValue {
            vtype: RegType::REG_BINARY,
            bytes: vec![state, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0].into(),
        },
    )?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn is_start_with_windows_enabled() -> bool {
    let Ok(run_key) = open_run_key_with_access(KEY_READ) else {
        return false;
    };
    if run_key.get_value::<String, _>(STARTUP_VALUE_NAME).is_err() {
        return false;
    }

    startup_approved_state().unwrap_or(true)
}

#[cfg(target_os = "windows")]
fn set_start_with_windows_enabled(enabled: bool) -> std::io::Result<()> {
    let run_key = open_or_create_run_key_with_access(KEY_SET_VALUE)?;
    if enabled {
        run_key.set_value(STARTUP_VALUE_NAME, &startup_command_line()?)?;
        set_startup_approved_state(true)?;
    } else {
        // Keep the Run entry so Windows Startup Apps can manage the toggle too.
        if run_key.get_value::<String, _>(STARTUP_VALUE_NAME).is_err() {
            run_key.set_value(STARTUP_VALUE_NAME, &startup_command_line()?)?;
        }
        set_startup_approved_state(false)?;
    }
    Ok(())
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
