#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(any(target_os = "linux", target_os = "windows"))]
use clap::{Arg, Command};
#[cfg(any(target_os = "linux", target_os = "windows"))]
use std::time::Duration;

#[cfg(target_os = "linux")]
mod status_tray;
#[cfg(target_os = "windows")]
mod status_tray_windows;

#[cfg(any(target_os = "linux", target_os = "windows"))]
use hyper_headset::devices::connect_compatible_device;

#[cfg(target_os = "linux")]
use status_tray::{StatusTray, TrayHandler};
#[cfg(target_os = "windows")]
use status_tray_windows::TrayHandler;

#[cfg(target_os = "linux")]
fn main() {
    let matches = Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about("A tray application for monitoring HyperX headsets.")
        .arg(
            Arg::new("refresh_interval")
                .long("refresh_interval")
                .required(false)
                .help("Set the refresh interval (in seconds)")
                .value_parser(clap::value_parser!(u64)),
        )
        .get_matches();
    let refresh_interval = *matches.get_one::<u64>("refresh_interval").unwrap_or(&3);
    let refresh_interval = Duration::from_secs(refresh_interval);
    let tray_handler = TrayHandler::new(status_tray::StatusTray::new());
    loop {
        let mut device = loop {
            match connect_compatible_device() {
                Ok(d) => break d,
                Err(e) => eprintln!("Connecting failed with error: {e}"),
            }
            std::thread::sleep(Duration::from_secs(1));
        };

        // Run loop
        let mut run_counter = 0;
        loop {
            std::thread::sleep(refresh_interval);
            // with the default refresh_interval the state is only actively queried every 3min
            // querying the device to frequently can lead to instability
            match if run_counter % 30 == 0 {
                device.active_refresh_state()
            } else {
                device.passive_refresh_state()
            } {
                Ok(()) => (),
                Err(error) => {
                    eprintln!("{error}");
                    tray_handler.update(device.get_device_state());
                    break; // try to reconnect
                }
            };
            tray_handler.update(device.get_device_state());
            run_counter += 1;
        }
    }
}

#[cfg(target_os = "windows")]
fn main() {
    let matches = Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about("A tray application for monitoring HyperX headsets.")
        .arg(
            Arg::new("refresh_interval")
                .long("refresh_interval")
                .required(false)
                .help("Set the refresh interval (in seconds)")
                .value_parser(clap::value_parser!(u64)),
        )
        .get_matches();
    let refresh_interval = *matches.get_one::<u64>("refresh_interval").unwrap_or(&3);
    let refresh_interval = Duration::from_secs(refresh_interval);
    
    let tray_handler = match TrayHandler::new() {
        Ok(handler) => handler,
        Err(e) => {
            // Try to show error in a message box since there's no console
            unsafe {
                use windows::{
                    core::*,
                    Win32::UI::WindowsAndMessaging::*,
                };
                let error_msg = HSTRING::from(format!("Failed to create tray handler: {e}"));
                MessageBoxW(None, &error_msg, w!("HyperHeadset Error"), MB_OK | MB_ICONERROR);
            }
            return;
        }
    };

    // Try to connect to device immediately and update tray
    let tray_handler_arc = std::sync::Arc::new(std::sync::Mutex::new(tray_handler));
    {
        match connect_compatible_device() {
            Ok(mut device) => {
                // Try to get initial state
                let _ = device.active_refresh_state();
                // Update tray with initial device state
                if let Ok(handler) = tray_handler_arc.lock() {
                    handler.update(device.get_device_state());
                }
            }
            Err(_) => {
                // Device not found initially, will retry in background thread
                // Tray already shows "No compatible device found"
            }
        }
    }

    // Run device monitoring in a separate thread
    let tray_handler_monitor = tray_handler_arc.clone();
    
    let monitor_thread = std::thread::spawn(move || {
        loop {
            let mut device = loop {
                match connect_compatible_device() {
                    Ok(d) => break d,
                    Err(_e) => {
                        // Connection failed, tray already shows error message
                        // Wait before retrying
                    }
                }
                std::thread::sleep(Duration::from_secs(1));
            };

            // Run loop
            let mut run_counter = 0;
            loop {
                std::thread::sleep(refresh_interval);
                // with the default refresh_interval the state is only actively queried every 3min
                // querying the device to frequently can lead to instability
                match if run_counter % 30 == 0 {
                    device.active_refresh_state()
                } else {
                    device.passive_refresh_state()
                } {
                    Ok(()) => (),
                    Err(error) => {
                        eprintln!("{error}");
                        if let Ok(handler) = tray_handler_monitor.lock() {
                            handler.update(device.get_device_state());
                        }
                        break; // try to reconnect
                    }
                };
                if let Ok(handler) = tray_handler_monitor.lock() {
                    handler.update(device.get_device_state());
                }
                run_counter += 1;
            }
        }
    });

    // Wait for the message loop thread to exit (when user clicks Exit)
    if let Ok(mut handler) = tray_handler_arc.lock() {
        handler.wait_for_exit();
    }
    
    // Wait for monitor thread to finish (it should exit when message loop exits)
    let _ = monitor_thread.join();
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn main() {
    eprintln!(
        "HyperHeadset tray app is currently only supported on Linux and Windows."
    );
}
