use std::time::Duration;

use clap::{Arg, ArgAction, Command};
use hyper_headset::devices::connect_compatible_device;

const EQ_FREQUENCIES: [(u32, u8); 10] = [
    (32, 0),
    (64, 1),
    (125, 2),
    (250, 3),
    (500, 4),
    (1000, 5),
    (2000, 6),
    (4000, 7),
    (8000, 8),
    (16000, 9),
];

/// Parse a band reference string into a band index (0-9).
///
/// Accepts either:
/// - A bare integer 0-9 (interpreted as band INDEX, not frequency)
/// - A frequency with suffix: e.g. "1khz", "64hz", "250Hz", "16KHZ"
///   Suffixes are case-insensitive. The trailing 'z' is optional (e.g. "1kh" works).
fn parse_band_ref(s: &str) -> Result<u8, String> {
    let s = s.trim();

    // Try bare integer first
    if let Ok(index) = s.parse::<u8>() {
        if index > 9 {
            return Err(format!(
                "Band index '{}' out of range. Must be 0-9.",
                index
            ));
        }
        return Ok(index);
    }

    // Try frequency with suffix
    let lower = s.to_ascii_lowercase();

    // Match patterns: <number>khz, <number>kh, <number>hz, <number>h
    // We find where the numeric part ends and the suffix begins
    let num_end = lower
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .ok_or_else(|| {
            format!(
                "Invalid band reference '{}'. Use a band index (0-9) or frequency like '1khz', '250hz'.",
                s
            )
        })?;

    let (num_str, suffix) = lower.split_at(num_end);
    let base_freq: f64 = num_str.parse().map_err(|_| {
        format!(
            "Invalid number '{}' in band reference '{}'.",
            num_str, s
        )
    })?;

    // Parse suffix to determine multiplier
    let freq_hz: u32 = match suffix {
        "khz" | "kh" | "k" => (base_freq * 1000.0) as u32,
        "hz" | "h" => base_freq as u32,
        _ => {
            return Err(format!(
                "Unknown frequency suffix '{}' in '{}'. Use 'hz' or 'khz'.",
                suffix, s
            ))
        }
    };

    // Look up frequency in the mapping
    for &(freq, index) in &EQ_FREQUENCIES {
        if freq == freq_hz {
            return Ok(index);
        }
    }

    let valid_freqs: Vec<String> = EQ_FREQUENCIES
        .iter()
        .map(|&(f, i)| {
            if f >= 1000 {
                format!("{}khz({})", f / 1000, i)
            } else {
                format!("{}hz({})", f, i)
            }
        })
        .collect();
    Err(format!(
        "Frequency {}Hz does not match any EQ band. Valid frequencies: {}",
        freq_hz,
        valid_freqs.join(", ")
    ))
}

/// Parse a "BAND=DB" pair into (band_index, db_value).
fn parse_eq_pair(s: &str) -> Result<(u8, f32), String> {
    let (band_str, db_str) = s.split_once('=').ok_or_else(|| {
        format!(
            "Invalid EQ pair '{}'. Expected format: BAND=DB (e.g. '5=-12.0' or '1khz=3.0').",
            s
        )
    })?;

    let band_index = parse_band_ref(band_str)?;

    let db_value: f32 = db_str.parse().map_err(|_| {
        format!(
            "Invalid dB value '{}' in '{}'. Expected a number like '3.0' or '-12'.",
            db_str, s
        )
    })?;

    if !(-12.0..=12.0).contains(&db_value) {
        return Err(format!(
            "dB value {} is out of range. Must be between -12.0 and 12.0.",
            db_value
        ));
    }

    Ok((band_index, db_value))
}

fn main() {
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
                    .hide(!device.can_set_automatic_shutdown())
                .value_parser(clap::value_parser!(u8)),
        )
        .arg(
            Arg::new("mute")
                .long("mute")
                .required(false)
                .help("Mute or unmute the headset.")
                .hide(!device.can_set_mute())
                .value_parser(clap::value_parser!(bool)),
        )
        .arg(
            Arg::new("enable_side_tone")
                .long("enable_side_tone")
                .required(false)
                .help("Enable or disable side tone.")
                .hide(!device.can_set_side_tone())
                .value_parser(clap::value_parser!(bool)),
        )
        .arg(
            Arg::new("side_tone_volume")
                .long("side_tone_volume")
                .required(false)
                .help("Set the side tone volume.")
                .hide(!device.can_set_side_tone_volume())
                .value_parser(clap::value_parser!(u8)),
        )
        .arg(
            Arg::new("enable_voice_prompt")
                .long("enable_voice_prompt")
                .required(false)
                .help("Enable voice prompt. This may not be supported on your device.")
                .hide(!device.can_set_voice_prompt())
                .value_parser(clap::value_parser!(bool)),
        )
        .arg(
            Arg::new("surround_sound")
                .long("surround_sound")
                .required(false)
                .help("Enables surround sound. This may be on by default and cannot be changed on your device.")
                .hide(!device.can_set_surround_sound())
                .value_parser(clap::value_parser!(bool)),
        )
        .arg(
            Arg::new("mute_playback")
                .long("mute_playback")
                .required(false)
                .help("Mute or unmute playback.")
                .hide(!device.can_set_silent_mode())
                .value_parser(clap::value_parser!(bool)),
        )
        .arg(
            Arg::new("eq_band")
                .long("eq-band")
                .required(false)
                .action(ArgAction::Append)
                .value_name("BAND=DB")
                .help(
                    "Set an equalizer band. Repeatable.\n\
                     BAND is an index (0-9) or frequency (e.g. 1khz, 250hz).\n\
                     DB is -12.0 to 12.0.\n\
                     Example: --eq-band 5=-12.0 --eq-band 1khz=3.0",
                )
                .hide(!device.can_set_equalizer()),
        )
        .arg(
            Arg::new("eq")
                .long("eq")
                .required(false)
                .value_name("BAND=DB,...")
                .help(
                    "Set multiple equalizer bands in one shot, comma-separated.\n\
                     BAND is an index (0-9) or frequency (e.g. 1khz, 250hz).\n\
                     DB is -12.0 to 12.0.\n\
                     Example: --eq 0=0.0,5=-12.0,16khz=4.0",
                )
                .hide(!device.can_set_equalizer()),
        )
        .get_matches();

    if let Some(delay) = matches.get_one::<u8>("automatic_shutdown") {
        let delay = *delay as u64;
        if let Some(packet) =
            device.set_automatic_shut_down_packet(Duration::from_secs(delay * 60u64))
        {
            device.prepare_write();
            if let Err(err) = device.get_device_state().hid_devices[0].write(&packet) {
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
            if let Err(err) = device.get_device_state().hid_devices[0].write(&packet) {
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
            if let Err(err) = device.get_device_state().hid_devices[0].write(&packet) {
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
            if let Err(err) = device.get_device_state().hid_devices[0].write(&packet) {
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
            if let Err(err) = device.get_device_state().hid_devices[0].write(&packet) {
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
            if let Err(err) = device.get_device_state().hid_devices[0].write(&packet) {
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
            if let Err(err) = device.get_device_state().hid_devices[0].write(&packet) {
                eprintln!("Failed to mute playback with error: {:?}", err);
                std::process::exit(1);
            }
        } else {
            eprintln!("ERROR: Playback mute control is not supported on this device");
            std::process::exit(1);
        }
    }

    // Collect EQ pairs from both --eq-band and --eq
    let mut eq_pairs: Vec<(u8, f32)> = Vec::new();

    if let Some(values) = matches.get_many::<String>("eq_band") {
        for val in values {
            match parse_eq_pair(val) {
                Ok(pair) => eq_pairs.push(pair),
                Err(err) => {
                    eprintln!("ERROR: --eq-band: {}", err);
                    std::process::exit(1);
                }
            }
        }
    }

    if let Some(val) = matches.get_one::<String>("eq") {
        for part in val.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            match parse_eq_pair(part) {
                Ok(pair) => eq_pairs.push(pair),
                Err(err) => {
                    eprintln!("ERROR: --eq: {}", err);
                    std::process::exit(1);
                }
            }
        }
    }

    for (band_index, db_value) in &eq_pairs {
        if let Some(packet) = device.set_equalizer_band_packet(*band_index, *db_value) {
            device.prepare_write();
            if let Err(err) = device.get_device_state().hid_devices[0].write(&packet) {
                eprintln!("Failed to set equalizer band {} with error: {:?}", band_index, err);
                std::process::exit(1);
            }
        } else {
            eprintln!("ERROR: Equalizer control is not supported on this device");
            std::process::exit(1);
        }
    }

    std::thread::sleep(Duration::from_secs_f64(0.5));

    if let Err(error) = device.active_refresh_state() {
        eprintln!("{error}");
        std::process::exit(1);
    };
    println!("{}", device.get_device_state());
}
