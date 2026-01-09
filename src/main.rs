#[cfg(target_os = "linux")]
use clap::{Arg, Command};
#[cfg(target_os = "linux")]
use std::time::Duration;

#[cfg(target_os = "linux")]
mod status_tray;
#[cfg(target_os = "linux")]
use hyper_headset::devices::connect_compatible_device;
#[cfg(target_os = "linux")]
use status_tray::{StatusTray, TrayHandler};

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
    let tray_handler = TrayHandler::new(StatusTray::new());
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

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!(
        "HyperHeadset tray app is currently only supported on Linux."
    );
}
