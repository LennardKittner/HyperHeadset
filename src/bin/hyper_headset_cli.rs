use std::time::Duration;

use clap::{Arg, Command};
use hyper_headset::devices::connect_compatible_device;

fn main() {
    let matches = Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about("A CLI application for monitoring and managing HyperX headsets.")
        .arg(
            Arg::new("automatic_shutdown")
                .long("automatic_shutdown")
                .required(false)
                .help(
                    "Set the delay in minutes after which the headset will automatically shutdown.\n0 will disable automatic shutdown.",
                )
                .value_parser(clap::value_parser!(u8)),
        )
        .arg(
            Arg::new("mute")
                .long("mute")
                .required(false)
                .help("Mute or unmute the headset.")
                .value_parser(clap::value_parser!(bool)),
        )
        .arg(
            Arg::new("enable_side_tone")
                .long("enable_side_tone")
                .required(false)
                .help("Enable or disable side tone.")
                .value_parser(clap::value_parser!(bool)),
        )
        .arg(
            Arg::new("side_tone_volume")
                .long("side_tone_volume")
                .required(false)
                .help("Set the side tone volume.")
                .value_parser(clap::value_parser!(u8)),
        )
        .arg(
            Arg::new("enable_voice_prompt")
                .long("enable_voice_prompt")
                .required(false)
                .help("Enable voice prompt. This may not be supported on your device.")
                .value_parser(clap::value_parser!(bool)),
        )
        .arg(
            Arg::new("surround_sound")
                .long("surround_sound")
                .required(false)
                .help("Enables surround sound. This may be on by default and cannot be changed on your device.")
                .value_parser(clap::value_parser!(bool)),
        )
        .arg(
            Arg::new("mute_playback")
                .long("mute_playback")
                .required(false)
                .help("Mute or unmute playback.")
                .value_parser(clap::value_parser!(bool)),
        )
        .get_matches();

    let mut device = match connect_compatible_device() {
        Ok(device) => device,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    println!("State before doing anything");
    if let Err(error) = device.active_refresh_state() {
        eprintln!("{error}");
        std::process::exit(1);
    };
    println!("{}", device.get_device_state());

    if let Some(delay) = matches.get_one::<u8>("automatic_shutdown") {
        println!("entering automatic_shutdown packet");
        let delay = *delay as u64;
        if let Some(packet) =
            device.set_automatic_shut_down_packet(Duration::from_secs(delay * 60u64))
        {
            println!("sending automatic_shutdown packet");
            device.prepare_write();
            if let Err(err) = device.get_device_state().hid_device.write(&packet) {
                println!("Failed to set automatic shutdown with error: {:?}", err)
            }
            if let Some(events) = device.wait_for_updates(Duration::from_secs(1)) {
                println!("{:?}", events);
            }
        } else {
            println!("Automatic shutdown can't be enabled on this device")
        }
    } else {
        println!("not sending automatic_shutdown packet");
    }

    if let Some(mute) = matches.get_one::<bool>("mute") {
        if let Some(packet) = device.set_mute_packet(*mute) {
            device.prepare_write();
            if let Err(err) = device.get_device_state().hid_device.write(&packet) {
                println!("Failed to mute with error: {:?}", err)
            }
        } else {
            println!("Can't mute this device")
        }
    }

    if let Some(enable) = matches.get_one::<bool>("enable_side_tone") {
        println!("entering enable_side_tone packet");
        if let Some(packet) = device.set_side_tone_packet(*enable) {
            println!("sending enable_side_tone packet");
            device.prepare_write();
            if let Err(err) = device.get_device_state().hid_device.write(&packet) {
                println!("Failed to enable side tone with error: {:?}", err)
            }
            std::thread::sleep(Duration::from_millis(50));
            if let Some(events) = device.wait_for_updates(Duration::from_secs(1)) {
                println!("{:?}", events);
            }
        } else {
            println!("Can't enable side tone on this device")
        }
    } else {
        println!("not sending enable_side_tone packet");
    }

    if let Some(volume) = matches.get_one::<u8>("side_tone_volume") {
        if let Some(packet) = device.set_side_tone_volume_packet(*volume) {
            device.prepare_write();
            if let Err(err) = device.get_device_state().hid_device.write(&packet) {
                println!("Failed to set side tone volume with error: {:?}", err)
            }
        } else {
            println!("Can't set side tone volume on this device")
        }
    }

    if let Some(enable) = matches.get_one::<bool>("enable_voice_prompt") {
        if let Some(packet) = device.set_voice_prompt_packet(*enable) {
            device.prepare_write();
            if let Err(err) = device.get_device_state().hid_device.write(&packet) {
                println!("Failed to enable voice prompt with error: {:?}", err)
            }
        } else {
            println!("Can't enable voice prompt on this device")
        }
    }

    if let Some(surround_sound) = matches.get_one::<bool>("surround_sound") {
        if let Some(packet) = device.set_surround_sound_packet(*surround_sound) {
            device.prepare_write();
            if let Err(err) = device.get_device_state().hid_device.write(&packet) {
                println!("Failed to set surround sound with error: {:?}", err)
            }
        } else {
            println!("Can't change surround sound on this device")
        }
    }

    if let Some(mute_playback) = matches.get_one::<bool>("mute_playback") {
        if let Some(packet) = device.set_silent_mode_packet(*mute_playback) {
            device.prepare_write();
            if let Err(err) = device.get_device_state().hid_device.write(&packet) {
                println!("Failed to mute playback with error: {:?}", err)
            }
        } else {
            println!("Can't mute playback on this device")
        }
    }

    std::thread::sleep(Duration::from_secs_f64(0.5));

    println!("State after potentially setting some stuff");
    if let Err(error) = device.active_refresh_state() {
        eprintln!("{error}");
        std::process::exit(1);
    };
    println!("{}", device.get_device_state());
}
