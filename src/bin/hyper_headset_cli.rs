use std::time::Duration;

use clap::{Arg, Command};
use hyper_headset::devices::{connect_compatible_device, ANCState, DeviceEvent};

const SHOW_ALL_OPTIONS: bool = true;

fn main() {
    #[cfg(target_os = "linux")]
    {
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
        .arg(
            Arg::new("equalizer_index")
            .long("equalizer_index")
            .required(false)
            .help("Choose between a predefined list of equalizers.\nCloud mix 2 [0..4]")
            .hide(!SHOW_ALL_OPTIONS
                && !device.can_set_equalizer_index())
            .value_parser(clap::value_parser!(u8)),
            )
        .arg(
            Arg::new("anc_mode")
            .long("anc_state")
            .required(false)
            .help("Set the active noise cancellation mode")
            .hide(!SHOW_ALL_OPTIONS
                && !device.can_set_anc_state())
            .value_parser(clap::builder::EnumValueParser::<ANCState>::new())
        )
        .arg(
            Arg::new("anc_level")
            .long("anc_level")
            .required(false)
            .help("Adjust the active noise cancellation level\nCloud mix 2 [0..2]")
            .hide(!SHOW_ALL_OPTIONS
                && !device.can_set_anc_level())
            .value_parser(clap::value_parser!(u8)),
        )
        .get_matches();

    let mut commands = Vec::new();
    if let Some(delay) = matches.get_one::<u8>("automatic_shutdown") {
        let delay = *delay as u64;
        commands.push(DeviceEvent::AutomaticShutdownAfter(Duration::from_secs(
            delay * 60u64,
        )));
    }

    if let Some(mute) = matches.get_one::<bool>("mute") {
        commands.push(DeviceEvent::Muted(*mute));
    }

    if let Some(enable) = matches.get_one::<bool>("enable_side_tone") {
        commands.push(DeviceEvent::SideToneOn(*enable));
    }

    if let Some(volume) = matches.get_one::<u8>("side_tone_volume") {
        commands.push(DeviceEvent::SideToneVolume(*volume));
    }

    if let Some(enable) = matches.get_one::<bool>("enable_voice_prompt") {
        commands.push(DeviceEvent::VoicePrompt(*enable));
    }

    if let Some(surround_sound) = matches.get_one::<bool>("surround_sound") {
        commands.push(DeviceEvent::SurroundSound(*surround_sound));
    }

    if let Some(mute_playback) = matches.get_one::<bool>("mute_playback") {
        commands.push(DeviceEvent::Silent(*mute_playback));
    }

    if let Some(activate) = matches.get_one::<bool>("activate_noise_gate") {
        commands.push(DeviceEvent::NoiseGateActive(*activate));
    }

    for command in commands {
        if let Err(e) = device.try_apply(command) {
            eprintln!("{e}");
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
    println!("{}", device.get_device_state().device_properties);
}
