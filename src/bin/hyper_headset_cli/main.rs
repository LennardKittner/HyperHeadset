#[cfg(feature = "eq-editor")]
mod eq_editor;

use std::time::Duration;

use clap::{Arg, ArgAction, Command};
use hyper_headset::devices::{connect_compatible_device, DeviceEvent};

const SHOW_ALL_OPTIONS: bool = false;

// Frequency-to-index mapping for CLI band references.
// Must stay in sync with hyper_headset::eq::EQ_FREQUENCIES.
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

    let can_set_eq = device.can_set_equalizer();

    #[cfg(not(feature = "eq-support"))]
    if can_set_eq {
        eprintln!("Tip: This headset supports EQ. Rebuild with --features eq-support for EQ presets, or --features eq-editor for the TUI equalizer.");
    }
    #[cfg(all(feature = "eq-support", not(feature = "eq-editor")))]
    if can_set_eq {
        eprintln!("Tip: This headset supports EQ. Rebuild with --features eq-editor for the TUI equalizer.");
    }

    #[allow(unused_mut)]
    let mut cmd = Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about("A CLI application for monitoring and managing HyperX headsets.")
        .after_help("Help only lists commands supported by this headset.")
        .arg(
            Arg::new("automatic-shutdown")
                .long("automatic-shutdown")
                .alias("automatic_shutdown")
                .required(false)
                .help(
                    "Set the delay in minutes after which the headset will automatically shutdown.\n0 will disable automatic shutdown.",
                )
                    .hide(!SHOW_ALL_OPTIONS && !device.can_set_automatic_shutdown())
                .value_parser(clap::value_parser!(u8)),
        )
        .arg(
            Arg::new("mute")
                .long("mute")
                .required(false)
                .help("Mute or unmute the headset.")
                .hide(!SHOW_ALL_OPTIONS && !device.can_set_mute())
                .value_parser(clap::value_parser!(bool)),
        )
        .arg(
            Arg::new("enable-side-tone")
                .long("enable-side-tone")
                .alias("enable_side_tone")
                .required(false)
                .help("Enable or disable side tone.")
                .hide(!SHOW_ALL_OPTIONS && !device.can_set_side_tone())
                .value_parser(clap::value_parser!(bool)),
        )
        .arg(
            Arg::new("side-tone-volume")
                .long("side-tone-volume")
                .alias("side_tone_volume")
                .required(false)
                .help("Set the side tone volume.")
                .hide(!SHOW_ALL_OPTIONS && !device.can_set_side_tone_volume())
                .value_parser(clap::value_parser!(u8)),
        )
        .arg(
            Arg::new("enable-voice-prompt")
                .long("enable-voice-prompt")
                .alias("enable_voice_prompt")
                .required(false)
                .help("Enable voice prompt. This may not be supported on your device.")
                .hide(!SHOW_ALL_OPTIONS && !device.can_set_voice_prompt())
                .value_parser(clap::value_parser!(bool)),
        )
        .arg(
            Arg::new("surround-sound")
                .long("surround-sound")
                .alias("surround_sound")
                .required(false)
                .help("Enables surround sound. This may be on by default and cannot be changed on your device.")
                .hide(!SHOW_ALL_OPTIONS && !device.can_set_surround_sound())
                .value_parser(clap::value_parser!(bool)),
        )
        .arg(
            Arg::new("mute-playback")
                .long("mute-playback")
                .alias("mute_playback")
                .required(false)
                .help("Mute or unmute playback. This may not be supported on your device.")
                .hide(!SHOW_ALL_OPTIONS && !device.can_set_silent_mode())
                .value_parser(clap::value_parser!(bool)),
        )
        .arg(
            Arg::new("activate-noise-gate")
                .long("activate-noise-gate")
                .alias("activate_noise_gate")
                .required(false)
                .help("Activates noise gate.")
                .hide(!SHOW_ALL_OPTIONS && !device.can_set_noise_gate())
                .value_parser(clap::value_parser!(bool)),
        )
        .arg(
            Arg::new("eq-profile")
                .long("eq-profile")
                .required(false)
                .value_name("BAND=DB,...")
                .help(
                    "Set full EQ profile. Unspecified bands reset to 0 dB.\n\
                     BAND: index 0-9 or frequency (1khz, 250hz). Bare integers are indices, not Hz.\n\
                     \x20 [0=32Hz, 1=64Hz, 2=125Hz, 3=250Hz, 4=500Hz, 5=1kHz, 6=2kHz, 7=4kHz, 8=8kHz, 9=16kHz]\n\
                     DB: -12.0 to 12.0.\n\
                     Example: --eq-profile 5=-12.0,1khz=3.0,16khz=4.0",
                )
                .hide(!SHOW_ALL_OPTIONS && !can_set_eq),
        )
        .arg(
            Arg::new("eq-band")
                .long("eq-band")
                .required(false)
                .action(ArgAction::Append)
                .value_name("BAND=DB[,...]")
                .help(
                    "Adjust specific bands. Repeatable, comma-separated (last write wins per band).\n\
                     Others unchanged. Use alone or with --eq-profile (overrides on top of the profile).\n\
                     See --eq-profile for band/dB reference.\n\
                     Example: --eq-band 5=-12.0,1khz=3.0 --eq-band 1=-12.0",
                )
                .hide(!SHOW_ALL_OPTIONS && !can_set_eq),
        );

    // Feature-gated TUI editor arg
    #[cfg(feature = "eq-editor")]
    {
        cmd = cmd.arg(
            Arg::new("eq")
                .long("eq")
                .action(ArgAction::SetTrue)
                .help("Open interactive EQ editor (TUI).\nThis may not be supported on your device.")
                .hide(!SHOW_ALL_OPTIONS && !can_set_eq),
        );
    }

    let matches = cmd.get_matches();

    // EQ TUI editor (--eq) - returns early
    #[cfg(feature = "eq-editor")]
    {
        if matches.get_flag("eq") {
            use crate::eq_editor::{EditorResult, EqEditor};
            use hyper_headset::eq::presets;

            // Capture current profile state before TUI starts (for restore on cancel)
            let previous_profile = presets::load_selected_profile();

            let editor = EqEditor::new();
            match editor.run(Some(&mut *device)) {
                Ok(EditorResult::Saved { name, bands }) => {
                    // Save preset file
                    let preset = presets::EqPreset {
                        name: name.clone(),
                        bands,
                    };
                    if let Err(err) = presets::save_preset(&preset) {
                        eprintln!("Failed to save preset: {:?}", err);
                    }
                    // Always keep TUI.json in sync with last TUI session state
                    if name != "TUI" {
                        let tui_preset = presets::EqPreset {
                            name: "TUI".to_string(),
                            bands,
                        };
                        if let Err(err) = presets::save_preset(&tui_preset) {
                            eprintln!("Failed to update TUI preset: {:?}", err);
                        }
                    }
                    // TUI sends bands live — headset already has them, so synced=true
                    let profile = presets::SelectedProfile {
                        active_preset: Some(name.clone()),
                        synced: true,
                    };
                    if let Err(err) = presets::save_selected_profile(&profile) {
                        eprintln!("Failed to save selected profile: {:?}", err);
                    }
                    println!("EQ preset '{}' saved.", name);
                    std::process::exit(0);
                }
                Ok(EditorResult::Cancelled { name, bands: _ }) => {
                    // Restore previous profile with its original synced state
                    let profile = presets::SelectedProfile {
                        active_preset: Some(name),
                        synced: previous_profile.synced,
                    };
                    if let Err(err) = presets::save_selected_profile(&profile) {
                        eprintln!("Failed to restore selected profile: {:?}", err);
                    }
                    println!("EQ editing cancelled.");
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("EQ editor error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }

    // Collect device commands from CLI args
    let mut commands: Vec<DeviceEvent> = Vec::new();

    if let Some(delay) = matches.get_one::<u8>("automatic-shutdown") {
        commands.push(DeviceEvent::AutomaticShutdownAfter(Duration::from_secs(
            *delay as u64 * 60,
        )));
    }
    if let Some(mute) = matches.get_one::<bool>("mute") {
        commands.push(DeviceEvent::Muted(*mute));
    }
    if let Some(enable) = matches.get_one::<bool>("enable-side-tone") {
        commands.push(DeviceEvent::SideToneOn(*enable));
    }
    if let Some(volume) = matches.get_one::<u8>("side-tone-volume") {
        commands.push(DeviceEvent::SideToneVolume(*volume));
    }
    if let Some(enable) = matches.get_one::<bool>("enable-voice-prompt") {
        commands.push(DeviceEvent::VoicePrompt(*enable));
    }
    if let Some(surround_sound) = matches.get_one::<bool>("surround-sound") {
        commands.push(DeviceEvent::SurroundSound(*surround_sound));
    }
    if let Some(mute_playback) = matches.get_one::<bool>("mute-playback") {
        commands.push(DeviceEvent::Silent(*mute_playback));
    }
    if let Some(activate) = matches.get_one::<bool>("activate-noise-gate") {
        commands.push(DeviceEvent::NoiseGateActive(*activate));
    }

    for command in commands {
        if let Err(e) = device.try_apply(command) {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }

    // --eq-profile: full profile (unspecified bands reset to 0 dB)
    // --eq-band: adjust specific bands only
    // If both are given, --eq-profile sets the base profile, --eq-band overrides on top.
    let mut eq_pairs: Option<Vec<(u8, f32)>> = None;

    if let Some(val) = matches.get_one::<String>("eq-profile") {
        // Start with all bands at 0 dB
        let mut eq_map: std::collections::BTreeMap<u8, f32> =
            (0u8..10).map(|i| (i, 0.0f32)).collect();
        for part in val.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            match parse_eq_pair(part) {
                Ok((band, db)) => { eq_map.insert(band, db); }
                Err(err) => {
                    eprintln!("ERROR: --eq-profile: {}", err);
                    std::process::exit(1);
                }
            }
        }
        eq_pairs = Some(eq_map.into_iter().collect());
    }

    if let Some(values) = matches.get_many::<String>("eq-band") {
        let pairs = eq_pairs.get_or_insert_with(Vec::new);
        for val in values {
            for part in val.split(',') {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                match parse_eq_pair(part) {
                    Ok((band, db)) => {
                        // Override if band already exists, otherwise append
                        if let Some(existing) = pairs.iter_mut().find(|(b, _)| *b == band) {
                            existing.1 = db;
                        } else {
                            pairs.push((band, db));
                        }
                    }
                    Err(err) => {
                        eprintln!("ERROR: --eq-band: {}", err);
                        std::process::exit(1);
                    }
                }
            }
        }
    }

    if let Some(ref pairs) = eq_pairs {
        if let Some(packets) = device.set_equalizer_bands_packets(pairs) {
            for packet in packets {
                device.prepare_write();
                if let Err(err) = device.get_device_state().hid_device.write(&packet) {
                    eprintln!("Failed to set equalizer with error: {:?}", err);
                    std::process::exit(1);
                }
                std::thread::sleep(Duration::from_millis(3));
            }
        } else {
            eprintln!("ERROR: Equalizer control is not supported on this device");
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
