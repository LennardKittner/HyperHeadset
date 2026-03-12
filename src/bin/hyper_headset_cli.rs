use std::{
    fs,
    io::{self},
    time::Duration,
};

use clap::{Arg, Command};
use hyper_headset::{debug_println, devices::connect_compatible_device};
use std::process::Command as CommandShell;

const SHOW_ALL_OPTIONS: bool = false;

const UDEV_RULE_PATH_SYSTEM: &str = "/etc/udev/rules.d/99-HyperHeadset.rules";
const UDEV_RULE_PATH_USER: &str = "/usr/lib/udev/rules.d/99-HyperHeadset.rules";
const UDEV_RULES: &str = include_str!("./../../99-HyperHeadset.rules");

#[derive(Debug)]
pub enum RuleState {
    RuleExists(bool),
    RuleMatch(bool),
}

pub fn check_rule(path: &str, rules: &str) -> RuleState {
    let mut rule_state;

    if !fs::exists(path).unwrap_or(false) {
        rule_state = RuleState::RuleExists(false);
    } else {
        rule_state = RuleState::RuleExists(true);
        if let Ok(content) = fs::read_to_string(path) {
            if content.trim() != rules.trim() {
                rule_state = RuleState::RuleMatch(false);
            } else {
                rule_state = RuleState::RuleMatch(true);
            }
        }
    }
    rule_state
}

pub fn update_rule(path: &str, rules: &str) {
    let status = CommandShell::new("sudo")
        .arg("sh")
        .arg("-c")
        .arg(format!(
            "echo {} > {}",
            shell_escape::escape(rules.into()),
            path
        ))
        .status();

    match status {
        Ok(exit_status) if exit_status.success() => {
            println!("created rule at {path}.\nYou may need to replug your headset for the udev rules to take effect.");
        }
        Ok(e) => {
            println!("Failed to create rule at {path}: {}", e);
            println!("Your headset may not be recognized without the correct udev rules.");
        }
        Err(e) => {
            println!("Failed to create rule at {path}: {}", e);
            println!("Your headset may not be recognized without the correct udev rules.");
        }
    }
}

fn main() {
    let user_rule_state = check_rule(UDEV_RULE_PATH_USER, UDEV_RULES);
    let system_rule_state = check_rule(UDEV_RULE_PATH_SYSTEM, UDEV_RULES);

    if users::get_current_uid() != 0 {
        debug_println!("user rule: {user_rule_state:?}, system rule: {system_rule_state:?}");
        match (user_rule_state, system_rule_state) {
            (RuleState::RuleMatch(true), _) => (),
            (_, RuleState::RuleMatch(true)) => (),

            (RuleState::RuleMatch(false), _) | (RuleState::RuleExists(true), _) => {
                print!(
                    "Udev rules at {UDEV_RULE_PATH_USER} do not have the expected value. Do you want to recreate them? (y/N): "
                );
                io::Write::flush(&mut io::stdout()).unwrap();
                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();
                if matches!(input.trim(), "y" | "Y") {
                    update_rule(UDEV_RULE_PATH_USER, UDEV_RULES);
                } else {
                    println!("Your headset may not be recognized without the correct udev rules.");
                }
            }
            (RuleState::RuleExists(false), RuleState::RuleMatch(false))
            | (RuleState::RuleExists(false), RuleState::RuleExists(true)) => {
                print!(
                    "Udev rules at {UDEV_RULE_PATH_SYSTEM} do not have the expected value. Do you want to recreate them? (y/N): "
                );
                io::Write::flush(&mut io::stdout()).unwrap();
                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();
                if matches!(input.trim(), "y" | "Y") {
                    update_rule(UDEV_RULE_PATH_SYSTEM, UDEV_RULES);
                } else {
                    println!("Your headset may not be recognized without the correct udev rules.");
                }
            }

            (RuleState::RuleExists(false), RuleState::RuleExists(false)) => {
                print!("No udev rules found. Do you want to create {UDEV_RULE_PATH_USER}? (y/N): ");
                io::Write::flush(&mut io::stdout()).unwrap();
                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();
                if matches!(input.trim(), "y" | "Y") {
                    update_rule(UDEV_RULE_PATH_USER, UDEV_RULES);
                } else {
                    println!(
                        "Without udev rules your headset can only be accessed when running as root."
                    );
                }
            }
        }
    }
    let mut device = match connect_compatible_device() {
        Ok(device) => device,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    let matches = Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about("A CLI application for monitoring and managing HyperX headsets.")
        .after_help("Help only lists commands supported by this headset.")
        .arg(
            Arg::new("automatic_shutdown")
                .long("automatic_shutdown")
                .required(false)
                .help(
                    "Set the delay in minutes after which the headset will automatically shutdown.\n0 will disable automatic shutdown.",
                )
                    .hide(!SHOW_ALL_OPTIONS
                    && !device.can_set_automatic_shutdown())
                .value_parser(clap::value_parser!(u8)),
        )
        .arg(
            Arg::new("mute")
                .long("mute")
                .required(false)
                .help("Mute or unmute the headset.")
                .hide(!SHOW_ALL_OPTIONS
                    && !device.can_set_mute())
                .value_parser(clap::value_parser!(bool)),
        )
        .arg(
            Arg::new("enable_side_tone")
                .long("enable_side_tone")
                .required(false)
                .help("Enable or disable side tone.")
                .hide(!SHOW_ALL_OPTIONS
                    && !device.can_set_side_tone())
                .value_parser(clap::value_parser!(bool)),
        )
        .arg(
            Arg::new("side_tone_volume")
                .long("side_tone_volume")
                .required(false)
                .help("Set the side tone volume.")
                .hide(!SHOW_ALL_OPTIONS
                    && !device.can_set_side_tone_volume())
                .value_parser(clap::value_parser!(u8)),
        )
        .arg(
            Arg::new("enable_voice_prompt")
                .long("enable_voice_prompt")
                .required(false)
                .help("Enable voice prompt. This may not be supported on your device.")
                .hide(!SHOW_ALL_OPTIONS
                    && !device.can_set_voice_prompt())
                .value_parser(clap::value_parser!(bool)),
        )
        .arg(
            Arg::new("surround_sound")
                .long("surround_sound")
                .required(false)
                .help("Enables surround sound. This may be on by default and cannot be changed on your device.")
                .hide(!SHOW_ALL_OPTIONS
                    && !device.can_set_surround_sound())
                .value_parser(clap::value_parser!(bool)),
        )
        .arg(
            Arg::new("mute_playback")
                .long("mute_playback")
                .required(false)
                .help("Mute or unmute playback.")
                .hide(!SHOW_ALL_OPTIONS
                    && !device.can_set_silent_mode())
                .value_parser(clap::value_parser!(bool)),
        )
        .arg(
            Arg::new("activate_noise_gate")
                .long("activate_noise_gate")
                .required(false)
                .help("Activates noise gate.")
                .hide(!SHOW_ALL_OPTIONS
                    && !device.can_set_silent_mode())
                .value_parser(clap::value_parser!(bool)),
        )
        .get_matches();

    if let Some(delay) = matches.get_one::<u8>("automatic_shutdown") {
        let delay = *delay as u64;
        if let Some(packet) =
            device.set_automatic_shut_down_packet(Duration::from_secs(delay * 60u64))
        {
            device.prepare_write();
            if let Err(err) = device.get_device_state().hid_device.write(&packet) {
                eprintln!("Failed to set automatic shutdown with error: {:?}", err);
                std::process::exit(1);
            }
        } else {
            eprintln!("ERROR: Automatic shutdown is not supported on this device");
            std::process::exit(1);
        }
    }

    if let Some(mute) = matches.get_one::<bool>("mute") {
        if let Some(packet) = device.set_mute_packet(*mute) {
            device.prepare_write();
            if let Err(err) = device.get_device_state().hid_device.write(&packet) {
                eprintln!("Failed to mute with error: {:?}", err);
                std::process::exit(1);
            }
        } else {
            eprintln!("ERROR: Microphone mute control is not supported on this device (hardware button only)");
            std::process::exit(1);
        }
    }

    if let Some(enable) = matches.get_one::<bool>("enable_side_tone") {
        if let Some(packet) = device.set_side_tone_packet(*enable) {
            device.prepare_write();
            if let Err(err) = device.get_device_state().hid_device.write(&packet) {
                eprintln!("Failed to enable side tone with error: {:?}", err);
                std::process::exit(1);
            }
        } else {
            eprintln!("ERROR: Side tone control is not supported on this device");
            std::process::exit(1);
        }
    }

    if let Some(volume) = matches.get_one::<u8>("side_tone_volume") {
        if let Some(packet) = device.set_side_tone_volume_packet(*volume) {
            device.prepare_write();
            if let Err(err) = device.get_device_state().hid_device.write(&packet) {
                eprintln!("Failed to set side tone volume with error: {:?}", err);
                std::process::exit(1);
            }
        } else {
            eprintln!("ERROR: Side tone volume control is not supported on this device");
            std::process::exit(1);
        }
    }

    if let Some(enable) = matches.get_one::<bool>("enable_voice_prompt") {
        if let Some(packet) = device.set_voice_prompt_packet(*enable) {
            device.prepare_write();
            if let Err(err) = device.get_device_state().hid_device.write(&packet) {
                eprintln!("Failed to enable voice prompt with error: {:?}", err);
                std::process::exit(1);
            }
        } else {
            eprintln!("ERROR: Voice prompt control is not supported on this device");
            std::process::exit(1);
        }
    }

    if let Some(surround_sound) = matches.get_one::<bool>("surround_sound") {
        if let Some(packet) = device.set_surround_sound_packet(*surround_sound) {
            device.prepare_write();
            if let Err(err) = device.get_device_state().hid_device.write(&packet) {
                eprintln!("Failed to set surround sound with error: {:?}", err);
                std::process::exit(1);
            }
        } else {
            eprintln!("ERROR: Surround sound control is not supported on this device");
            eprintln!("       Use the physical headset button or Windows audio settings to toggle surround sound.");
            std::process::exit(1);
        }
    }

    if let Some(mute_playback) = matches.get_one::<bool>("mute_playback") {
        if let Some(packet) = device.set_silent_mode_packet(*mute_playback) {
            device.prepare_write();
            if let Err(err) = device.get_device_state().hid_device.write(&packet) {
                eprintln!("Failed to mute playback with error: {:?}", err);
                std::process::exit(1);
            }
        } else {
            eprintln!("ERROR: Playback mute control is not supported on this device");
            std::process::exit(1);
        }
    }

    if let Some(activate) = matches.get_one::<bool>("activate_noise_gate") {
        if let Some(packet) = device.set_noise_gate_packet(*activate) {
            device.prepare_write();
            if let Err(err) = device.get_device_state().hid_device.write(&packet) {
                eprintln!("Failed to activate noise gate with error: {:?}", err);
                std::process::exit(1);
            }
        } else {
            eprintln!("ERROR: Activating noise gate is not supported on this device");
            std::process::exit(1);
        }
    }

    std::thread::sleep(Duration::from_secs_f64(0.5));

    // setting an option may cause a response form the headset
    if device.allow_passive_refresh() {
        if let Err(error) = device.passive_refresh_state() {
            eprintln!("{error}");
            std::process::exit(1);
        };
    }

    if let Err(error) = device.active_refresh_state() {
        eprintln!("{error}");
        std::process::exit(1);
    };
    println!("{}", device.get_device_state());
}
