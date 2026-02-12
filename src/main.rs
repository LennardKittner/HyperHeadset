use clap::{Arg, Command};
use std::time::Duration;

mod status_tray;
use hyper_headset::devices::connect_compatible_device;
use hyper_headset::eq::presets;
use hyper_headset::eq::TrayCommand;
use status_tray::{StatusTray, TrayHandler};

/// Apply the named EQ preset to the device and update sync state.
/// Saves synced=true on success, synced=false on failure, then reloads tray.
fn apply_and_sync(
    device: &mut Box<dyn hyper_headset::devices::Device>,
    name: &str,
    tray_handler: &TrayHandler,
) {
    // WORKAROUND(firmware-no-response): Probe connected status fresh before applying.
    // TODO: Remove this probe once firmware NAKs writes when headset is off.
    device.probe_connected_status();
    tray_handler.update(device.get_device_state());
    if device.get_device_state().connected != Some(true) {
        eprintln!("Headset not connected, EQ preset '{}' queued for sync.", name);
        let profile = presets::SelectedProfile {
            active_preset: Some(name.to_string()),
            synced: false,
        };
        let _ = presets::save_selected_profile(&profile);
        tray_handler.reload_presets();
        return;
    }

    let success = if let Some(preset) = presets::load_preset(name) {
        let pairs: Vec<(u8, f32)> = preset
            .bands
            .iter()
            .enumerate()
            .map(|(i, &db)| (i as u8, db))
            .collect();
        if let Some(packets) = device.set_equalizer_bands_packets(&pairs) {
            let mut ok = true;
            for packet in packets {
                device.prepare_write();
                if let Err(err) = device.get_device_state().hid_devices[0].write(&packet) {
                    eprintln!("Failed to apply EQ preset '{}': {:?}", name, err);
                    ok = false;
                    break;
                }
                std::thread::sleep(Duration::from_millis(3));
            }
            ok
        } else {
            eprintln!("Device does not support EQ");
            false
        }
    } else {
        eprintln!("EQ preset '{}' not found", name);
        false
    };

    if success {
        println!("EQ preset '{}' applied.", name);
    }

    let profile = presets::SelectedProfile {
        active_preset: Some(name.to_string()),
        synced: success,
    };
    let _ = presets::save_selected_profile(&profile);
    tray_handler.reload_presets();
}

fn main() {
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
                .value_parser(clap::value_parser!(u64)),
        )
        .get_matches();
    let refresh_interval = *matches.get_one::<u64>("refresh-interval").unwrap_or(&3);
    let refresh_interval = Duration::from_secs(refresh_interval);

    // Channel for tray → main loop commands
    let (command_tx, command_rx) = std::sync::mpsc::channel::<TrayCommand>();

    let tray_handler = TrayHandler::new(StatusTray::new(command_tx));

    // File watcher for preset/settings changes
    let (_watcher, watcher_rx) = match presets::watch_config_dir() {
        Ok(pair) => (Some(pair.0), Some(pair.1)),
        Err(e) => {
            eprintln!("Warning: failed to watch config directory: {e}");
            (None, None)
        }
    };

    loop {
        let mut device = loop {
            match connect_compatible_device() {
                Ok(d) => break d,
                Err(e) => eprintln!("Connecting failed with error: {e}"),
            }
            std::thread::sleep(Duration::from_secs(1));
        };

        if device.get_device_state().can_set_equalizer {
            tray_handler.reload_presets();
            // Auto-sync is handled by the transition detection in the inner loop
            // (was_connected: false → true triggers sync when headset comes online)
        }

        // Run loop
        let mut run_counter = 0;
        let mut was_connected = device.get_device_state().connected == Some(true);
        loop {
            // Process tray commands
            while let Ok(cmd) = command_rx.try_recv() {
                match cmd {
                    TrayCommand::ApplyEqPreset(name) => {
                        apply_and_sync(&mut device, &name, &tray_handler);
                    }
                }
            }

            // Check for preset file changes
            if let Some(ref wrx) = watcher_rx {
                if wrx.try_recv().is_ok() {
                    // Drain any additional events
                    while wrx.try_recv().is_ok() {}
                    tray_handler.reload_presets();
                }
            }

            std::thread::sleep(refresh_interval);
            // with the default refresh_interval the state is only actively queried every 3min
            // querying the device too frequently can lead to instability
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

            // WORKAROUND(firmware-no-response): passive_refresh may not reliably detect
            // reconnection. Actively probe connected status when headset is not connected.
            // TODO: Remove once passive_refresh reliably detects reconnects.
            if !was_connected && device.get_device_state().can_set_equalizer {
                device.probe_connected_status();
                tray_handler.update(device.get_device_state());
            }

            // Sync unsynced profile when headset transitions to connected
            let is_connected = device.get_device_state().connected == Some(true);
            if is_connected && !was_connected && device.get_device_state().can_set_equalizer {
                let profile = presets::load_selected_profile();
                if !profile.synced {
                    if let Some(ref name) = profile.active_preset {
                        println!("Syncing EQ preset '{}' to headset...", name);
                        apply_and_sync(&mut device, name, &tray_handler);
                    }
                }
            }
            was_connected = is_connected;

            run_counter += 1;
        }
    }
}
