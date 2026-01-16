use hyper_headset::devices::DeviceState;
use std::sync::{Arc, Mutex};
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::Graphics::Gdi::*,
    Win32::System::LibraryLoader::*,
    Win32::UI::Shell::*,
    Win32::UI::WindowsAndMessaging::*,
};

const NO_COMPATIBLE_DEVICE: &str = "No compatible device found.\nIs the dongle plugged in?";

pub struct TrayHandler {
    window_handle: HWND,
    icon_data: Arc<Mutex<IconData>>,
    message_thread: Option<std::thread::JoinHandle<()>>,
}

// HWND is not Send by default, but it's safe to send across threads
// as long as we only use it from the thread that created the window
unsafe impl Send for TrayHandler {}

struct IconData {
    device_name: Option<String>,
    message: String,
}

static mut GLOBAL_ICON_DATA: Option<Arc<Mutex<IconData>>> = None;

const TRAY_ICON_MESSAGE: u32 = 0x0400 + 1; // WM_USER + 1

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    umsg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match umsg {
        msg if msg == TRAY_ICON_MESSAGE => {
            // Tray icon message
            match lparam.0 as u32 {
                msg if msg == WM_LBUTTONUP || msg == WM_RBUTTONUP => {
                    if let Some(ref icon_data) = GLOBAL_ICON_DATA {
                        let data = icon_data.lock().unwrap();
                        TrayHandler::show_context_menu(hwnd, &data);
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }
        msg if msg == WM_COMMAND => {
            let cmd = wparam.0 as u32;
            if cmd == 1 {
                // Exit
                Shell_NotifyIconW(NIM_DELETE, &NOTIFYICONDATAW {
                    cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                    hWnd: hwnd,
                    uID: 1,
                    ..Default::default()
                });
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        msg if msg == WM_DESTROY => {
            Shell_NotifyIconW(NIM_DELETE, &NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: 1,
                ..Default::default()
            });
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcA(hwnd, umsg, wparam, lparam),
    }
}

impl TrayHandler {
    pub fn new() -> Result<Self> {
        let icon_data = Arc::new(Mutex::new(IconData {
            device_name: None,
            message: NO_COMPATIBLE_DEVICE.to_string(),
        }));

        // Store global reference for window proc
        unsafe {
            GLOBAL_ICON_DATA = Some(icon_data.clone());
        }

        // Create a hidden window for the tray icon
        let window_handle = unsafe {
            let hinstance = HINSTANCE::default();
            let class_name = PCSTR(b"HyperHeadsetTrayWindow\0".as_ptr());

            let wc = WNDCLASSEXA {
                cbSize: std::mem::size_of::<WNDCLASSEXA>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(window_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinstance.into(),
                hIcon: HICON::default(),
                hCursor: HCURSOR::default(),
                hbrBackground: HBRUSH::default(),
                lpszMenuName: PCSTR::null(),
                lpszClassName: class_name,
                hIconSm: HICON::default(),
            };

            // Try to register the class, ignore error if already registered
            let _ = RegisterClassExA(&wc);

            let hwnd = CreateWindowExA(
                WINDOW_EX_STYLE::default(),
                class_name,
                PCSTR(b"HyperHeadset Tray\0".as_ptr()),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                None,
                hinstance,
                None,
            )?;

            // Add tray icon
            let icon_data_clone = icon_data.clone();
            let tip_text: Vec<u16> = "HyperHeadset".encode_utf16().chain(std::iter::once(0)).collect();
            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: 1,
                uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
                uCallbackMessage: TRAY_ICON_MESSAGE,
                hIcon: Self::create_icon()?,
                ..Default::default()
            };
            let max_len = (tip_text.len() - 1).min(127);
            nid.szTip[..max_len].copy_from_slice(&tip_text[..max_len]);
            nid.szTip[max_len] = 0;

            Shell_NotifyIconW(NIM_ADD, &nid);

            // Update with initial state
            let data = icon_data_clone.lock().unwrap();
            Self::update_tray_icon(hwnd, &data)?;
            drop(data);

            hwnd
        };

        // Start message loop in a separate thread
        let hwnd_for_thread = window_handle;
        let message_thread = std::thread::spawn(move || {
            unsafe {
                let mut msg = MSG::default();
                while GetMessageA(&mut msg, None, 0, 0).as_bool() {
                    TranslateMessage(&msg);
                    DispatchMessageA(&msg);
                }
            }
        });

        Ok(TrayHandler {
            window_handle,
            icon_data,
            message_thread: Some(message_thread),
        })
    }


    fn create_icon() -> Result<HICON> {
        // Create a simple icon using system icon
        unsafe {
            let hinstance = HINSTANCE::default();
            // IDI_APPLICATION = 32512, convert to PCSTR using MAKEINTRESOURCE
            let icon_id = 32512u16;
            let icon_ptr = (icon_id as usize) as *const u8;
            LoadIconA(hinstance, PCSTR(icon_ptr))
        }
    }

    fn update_tray_icon(hwnd: HWND, data: &IconData) -> Result<()> {
        let tooltip = if let Some(ref name) = data.device_name {
            format!("{}\n{}", name, data.message)
        } else {
            data.message.clone()
        };

        let tooltip_wide: Vec<u16> = tooltip.encode_utf16().chain(std::iter::once(0)).collect();

        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            uFlags: NIF_TIP,
            ..Default::default()
        };

        // Copy tooltip text (limited to 128 chars including null terminator)
        let max_len = (tooltip_wide.len() - 1).min(127);
        nid.szTip[..max_len].copy_from_slice(&tooltip_wide[..max_len]);
        nid.szTip[max_len] = 0;

        unsafe {
            Shell_NotifyIconW(NIM_MODIFY, &nid);
        }

        Ok(())
    }

    fn show_context_menu(hwnd: HWND, data: &IconData) {
        unsafe {
            let hmenu = CreatePopupMenu().unwrap_or_default();
            
            // Add status lines
            for line in data.message.lines() {
                if !line.is_empty() && !line.contains("Unknown") {
                    let line_bytes = line.as_bytes();
                    let mut line_with_null = Vec::with_capacity(line_bytes.len() + 1);
                    line_with_null.extend_from_slice(line_bytes);
                    line_with_null.push(0);
                    AppendMenuA(
                        hmenu,
                        MF_STRING,
                        0,
                        PCSTR(line_with_null.as_ptr()),
                    );
                }
            }

            // Separator
            AppendMenuA(hmenu, MF_SEPARATOR, 0, PCSTR::null());

            // Exit
            AppendMenuA(hmenu, MF_STRING, 1, PCSTR(b"Exit\0".as_ptr()));

            let mut pt = POINT::default();
            GetCursorPos(&mut pt);

            SetForegroundWindow(hwnd);
            let cmd = TrackPopupMenu(
                hmenu,
                TPM_RETURNCMD | TPM_NONOTIFY,
                pt.x,
                pt.y,
                0,
                hwnd,
                None,
            );

            DestroyMenu(hmenu);

            if cmd.as_bool() {
                PostMessageA(hwnd, WM_COMMAND, WPARAM(cmd.0 as usize), LPARAM::default());
            }
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

        {
            let mut data = self.icon_data.lock().unwrap();
            data.message = message;
            data.device_name = name.clone();
        }
        // Update icon after releasing the lock
        let data = self.icon_data.lock().unwrap();
        let _ = Self::update_tray_icon(self.window_handle, &*data);
    }

    pub fn wait_for_exit(&mut self) {
        if let Some(handle) = self.message_thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for TrayHandler {
    fn drop(&mut self) {
        unsafe {
            Shell_NotifyIconW(NIM_DELETE, &NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.window_handle,
                uID: 1,
                ..Default::default()
            });
        }
    }
}
