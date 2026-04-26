#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
mod status_tray;

#[cfg(not(target_os = "linux"))]
mod status_tray_not_linux;

mod tray_battery_icon_state;

use cfg_if::cfg_if;

#[cfg(feature = "eq-support")]
use hyper_headset::eq::runtime as eq_runtime;

#[cfg(not(feature = "eq-support"))]
fn warn_eq_unavailable_once(can_set_equalizer: bool) {
    if can_set_equalizer {
        static EQ_WARNING: std::sync::Once = std::sync::Once::new();
        EQ_WARNING.call_once(|| {
            eprintln!(
                "This headset supports EQ presets. Rebuild with --features eq-support to enable."
            )
        });
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    use std::sync::mpsc;

    use hyper_headset::devices::{DeviceEvent, DeviceProperties};
    use winit::event_loop::{ControlFlow, EventLoop, EventLoopProxy};

    use crate::status_tray_not_linux::TrayApp;

    let event_loop: EventLoop<Option<DeviceProperties>> =
        EventLoop::with_user_event().build().unwrap();
    let proxy: EventLoopProxy<Option<DeviceProperties>> = event_loop.create_proxy();
    event_loop.set_control_flow(ControlFlow::Wait);

    let (tx, rx) = mpsc::channel::<DeviceEvent>();

    std::thread::spawn(move || {
        use std::time::Duration;

        use clap::{Arg, Command};
        use enigo::{Direction, Enigo, Key, Keyboard, Settings};

        use hyper_headset::devices::connect_compatible_device;

        let matches = Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about("A tray application for monitoring HyperX headsets.")
        .arg(
            Arg::new("refresh-interval")
                .long("refresh-interval")
                .alias("refresh_interval")
                .required(false)
                .help("Set the refresh interval (in seconds)")
                .default_value("3")
                .value_parser(clap::value_parser!(u64)),
        )
        .arg(
            Arg::new("press-mute-key")
                .long("press-mute-key")
                .alias("press_mute_key")
                .required(false)
                .help("The app will simulate pressing the microphone mute key whoever the headsets is muted or unmuted.")
                .default_value("true")
                .value_parser(clap::value_parser!(bool)),
        )
        .get_matches();

        let press_mute_key = *matches.get_one::<bool>("press-mute-key").unwrap_or(&true);
        let mut enigo = if press_mute_key {
            match Enigo::new(&Settings::default()) {
                Ok(enigo) => Some(enigo),
                Err(e) => {
                    eprintln!("Virtual mute key failed to initialize: {e}");
                    None
                }
            }
        } else {
            None
        };
        let refresh_interval = *matches.get_one::<u64>("refresh-interval").unwrap_or(&3);
        let refresh_interval = Duration::from_secs(refresh_interval);

        loop {
            let mut device = loop {
                match connect_compatible_device() {
                    Ok(d) => break d,
                    Err(e) => {
                        let _ = proxy.send_event(None);
                        eprintln!("Connecting failed with error: {e}")
                    }
                }
                std::thread::sleep(Duration::from_secs(1));
            };

            cfg_if! {
                if #[cfg(feature = "eq-support")] {
                    let (_watcher, watcher_rx) = match eq_runtime::init_device_eq_state(&mut *device) {
                        Some((w, rx)) => (Some(w), Some(rx)),
                        None => (None, None),
                    };
                } else {
                    warn_eq_unavailable_once(
                        device.get_device_state().device_properties.can_set_equalizer,
                    );
                }
            }

            // Run loop
            let mut run_counter = 0;
            #[cfg(feature = "eq-support")]
            let mut was_connected =
                device.get_device_state().device_properties.connected == Some(true);
            loop {
                let mute_state = device.get_device_state().device_properties.muted;
                match if run_counter % 30 == 0 {
                    device.active_refresh_state()
                } else {
                    device.passive_refresh_state()
                } {
                    Ok(()) => (),
                    Err(error) => {
                        eprintln!("{error}");
                        let _ = proxy
                            .send_event(Some(device.get_device_state().device_properties.clone()));
                        break; // try to reconnect
                    }
                };
                if mute_state.is_some()
                    && mute_state != device.get_device_state().device_properties.muted
                {
                    if let Some(enigo) = &mut enigo {
                        if let Err(e) = enigo.key(Key::F20, Direction::Click) {
                            eprintln!("Failed to press key on mute: {e}");
                        }
                    }
                }

                // with the default refresh_interval the state is only actively queried every 3min
                // querying the device to frequently can lead to instability
                let first = rx.recv_timeout(refresh_interval);
                for command in first.into_iter().chain(rx.try_iter()) {
                    let _ = device.try_apply(command);
                    std::thread::sleep(hyper_headset::devices::RESPONSE_DELAY);
                    let _ = device.active_refresh_state();
                }

                #[cfg(feature = "eq-support")]
                if let Some(ref wrx) = watcher_rx {
                    if eq_runtime::drain_watcher(wrx) {
                        eq_runtime::refresh_eq_state_from_disk(&mut *device);
                        let _ = proxy.send_event(Some(
                            device.get_device_state().device_properties.clone(),
                        ));
                    }
                }

                #[cfg(feature = "eq-support")]
                {
                    was_connected =
                        eq_runtime::maybe_sync_on_reconnect(&mut *device, was_connected);
                }

                let _ = proxy.send_event(Some(device.get_device_state().device_properties.clone()));
                run_counter += 1;
            }
        }
    });

    event_loop.run_app(&mut TrayApp::new(tx)).unwrap();
}

#[cfg(target_os = "linux")]
fn main() {
    use clap::{Arg, Command};
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    use std::sync::mpsc;
    use std::time::Duration;

    use hyper_headset::devices::connect_compatible_device;
    use status_tray::{StatusTray, TrayHandler};

    use hyper_headset::act_as_askpass_handler;
    use hyper_headset::prompt_user_for_udev_rule;

    if let Ok(name) = std::env::current_exe() {
        if let Some(name) = name.to_str() {
            if let Ok(askpass) = std::env::var("SUDO_ASKPASS") {
                if name == askpass {
                    act_as_askpass_handler();
                }
            }
        }
    }
    prompt_user_for_udev_rule();
    let matches = Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about("A tray application for monitoring HyperX headsets.")
        .arg(
            Arg::new("refresh-interval")
                .long("refresh-interval")
                .alias("refresh_interval")
                .required(false)
                .help("Set the refresh interval (in seconds)")
                .default_value("3")
                .value_parser(clap::value_parser!(u64)),
        )
        .arg(
            Arg::new("press-mute-key")
                .long("press-mute-key")
                .alias("press_mute_key")
                .required(false)
                .help("The app will simulate pressing the microphone mute key whoever the headsets is muted or unmuted.")
                .default_value("true")
                .value_parser(clap::value_parser!(bool)),
        )
        .get_matches();

    let press_mute_key = *matches.get_one::<bool>("press-mute-key").unwrap_or(&true);
    let mut enigo = if press_mute_key {
        match Enigo::new(&Settings::default()) {
            Ok(enigo) => Some(enigo),
            Err(e) => {
                eprintln!("Virtual mute key failed to initialize: {e}");
                None
            }
        }
    } else {
        None
    };
    let refresh_interval = *matches.get_one::<u64>("refresh-interval").unwrap_or(&3);
    let refresh_interval = Duration::from_secs(refresh_interval);

    let (tx, rx) = mpsc::channel();
    let tray_handler = TrayHandler::new(StatusTray::new(tx));

    loop {
        let mut device = loop {
            match connect_compatible_device() {
                Ok(d) => break d,
                Err(e) => {
                    tray_handler.clear_state();
                    eprintln!("Connecting failed with error: {e}")
                }
            }
            std::thread::sleep(Duration::from_secs(1));
        };

        cfg_if! {
            if #[cfg(feature = "eq-support")] {
                let (_watcher, watcher_rx) = match eq_runtime::init_device_eq_state(&mut *device) {
                    Some((w, rx)) => (Some(w), Some(rx)),
                    None => (None, None),
                };
            } else {
                warn_eq_unavailable_once(
                    device.get_device_state().device_properties.can_set_equalizer,
                );
            }
        }

        // Run loop
        let mut run_counter = 0;
        #[cfg(feature = "eq-support")]
        let mut was_connected = device.get_device_state().device_properties.connected == Some(true);
        loop {
            let mute_state = device.get_device_state().device_properties.muted;
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
            if mute_state.is_some()
                && mute_state != device.get_device_state().device_properties.muted
            {
                if let Some(enigo) = &mut enigo {
                    if let Err(e) = enigo.key(Key::MicMute, Direction::Click) {
                        eprintln!("Failed to press key on mute: {e}");
                    }
                }
            }

            // Process tray device commands
            // with the default refresh_interval the state is only actively queried every 3min
            // querying the device too frequently can lead to instability
            let first = rx.recv_timeout(refresh_interval);
            for command in first.into_iter().chain(rx.try_iter()) {
                let _ = device.try_apply(command);
                std::thread::sleep(hyper_headset::devices::RESPONSE_DELAY);
                let _ = device.active_refresh_state();
            }

            // Check for preset file changes
            #[cfg(feature = "eq-support")]
            if let Some(ref wrx) = watcher_rx {
                if eq_runtime::drain_watcher(wrx) {
                    eq_runtime::refresh_eq_state_from_disk(&mut *device);
                }
            }

            // Sync unsynced profile when headset transitions to connected
            #[cfg(feature = "eq-support")]
            {
                was_connected = eq_runtime::maybe_sync_on_reconnect(&mut *device, was_connected);
            }

            tray_handler.update(device.get_device_state());

            run_counter += 1;
        }
    }
}
