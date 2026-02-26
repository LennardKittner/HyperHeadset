use clap::{Arg, Command};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::time::Duration;

mod status_tray;
use hyper_headset::devices::connect_compatible_device;
use status_tray::{StatusTray, TrayHandler};

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
                .default_value("3")
                .value_parser(clap::value_parser!(u64)),
        )
        .arg(
            Arg::new("press_mute_key")
                .long("press_mute_key")
                .required(false)
                .help("The app will simulate pressing the microphone mute key whoever the headsets is muted or unmuted.")
                .default_value("true")
                .value_parser(clap::value_parser!(bool)),
        )
        .get_matches();

    let mut enigo = Enigo::new(&Settings::default()).unwrap();
    let refresh_interval = *matches.get_one::<u64>("refresh_interval").unwrap_or(&3);
    let press_mute_key = *matches.get_one::<bool>("press_mute_key").unwrap_or(&true);
    let refresh_interval = Duration::from_secs(refresh_interval);
    let tray_handler = TrayHandler::new(StatusTray::new());
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

        // Run loop
        let mut run_counter = 0;
        loop {
            std::thread::sleep(refresh_interval);
            // with the default refresh_interval the state is only actively queried every 3min
            // querying the device to frequently can lead to instability

            let mute_state = device.get_device_state().muted.clone();
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
            if mute_state.is_some() && mute_state != device.get_device_state().muted {
                //TODO: macOS and windows have to use another key
                if press_mute_key {
                    enigo.key(Key::MicMute, Direction::Click).unwrap();
                }
            }
            tray_handler.update(device.get_device_state());
            run_counter += 1;
        }
    }
}
