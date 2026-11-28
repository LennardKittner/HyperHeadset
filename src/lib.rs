use std::sync::OnceLock;
#[cfg(target_os = "linux")]
use std::{fs, io, process::Command, time::Duration};

#[cfg(target_os = "linux")]
use dialog::{Choice, DialogBox};

// #![warn(missing_docs)]
pub mod devices;
#[cfg(feature = "eq-support")]
pub mod eq;

#[cfg(target_os = "linux")]
pub mod bluetooth;

#[cfg(target_os = "linux")]
mod airoha_race;

pub static VERBOSE: OnceLock<bool> = OnceLock::new();

#[macro_export]
macro_rules! debug_println {
    ($($args:tt)*) => {
        #[cfg(debug_assertions)]
        println!($($args)*);

        #[cfg(not(debug_assertions))]
        if *$crate::VERBOSE.get().unwrap_or(&false) {
            println!($($args)*);
        }
    };
}

pub const UDEV_RULE_PATH_SYSTEM: &str = "/etc/udev/rules.d/99-HyperHeadset.rules";
pub const UDEV_RULE_PATH_USER: &str = "/usr/lib/udev/rules.d/99-HyperHeadset.rules";
pub const UDEV_RULES: &str = include_str!("./../99-HyperHeadset.rules");
pub const NO_AUTO_UDEV_ENV: &str = "HYPERHEADSET_NO_AUTO_UDEV";

#[derive(Debug)]
pub enum RuleState {
    RuleExists(bool),
    RuleMatch(bool),
}

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
pub fn act_as_askpass_handler() -> ! {
    let a = dialog::Password::new("Created rule at /usr/lib/udev/rules.d/99-HyperHeadset.rules")
        .title("HyperHeadset")
        .show()
        .expect("Failed to open dialog");
    println!("{}", a.unwrap_or("".to_string()));
    std::process::exit(0)
}

#[cfg(target_os = "linux")]
pub fn update_rule(path: &str, fall_back_path: &str, rules: &str) {
    let status = if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        Command::new("sudo")
            .arg("sh")
            .arg("-c")
            .arg(format!(
                "(printf '%s\\n' {} > {} || printf '%s\\n' {} > {}) && \
                    udevadm control --reload-rules && \
                    udevadm trigger",
                shell_escape::escape(rules.into()),
                shell_escape::escape(path.into()),
                shell_escape::escape(rules.into()),
                shell_escape::escape(fall_back_path.into()),
            ))
            .status()
    } else {
        Command::new("sudo")
            .env("SUDO_ASKPASS", std::env::current_exe().unwrap())
            .arg("--askpass")
            .arg("sh")
            .arg("-c")
            .arg(format!(
                "(printf '%s\\n' {} > {} || printf '%s\\n' {} > {}) && \
                    udevadm control --reload-rules && \
                    udevadm trigger",
                shell_escape::escape(rules.into()),
                shell_escape::escape(path.into()),
                shell_escape::escape(rules.into()),
                shell_escape::escape(fall_back_path.into()),
            ))
            .status()
    };
    // a little delay so the rules are active before trying to connect
    std::thread::sleep(Duration::from_millis(500));

    match status {
        Ok(exit_status) if exit_status.success() => {
            show_message(&format!("created rule at {path}.\nYou may need to replug your headset for the udev rules to take effect."));
        }
        Ok(e) => {
            show_message(&format!("Failed to create rule at {path}: {}.\nYour headset may not be recognized without the correct udev rules.", e));
        }
        Err(e) => {
            show_message(&format!("Failed to create rule at {path}: {}\nYour headset may not be recognized without the correct udev rules.", e));
        }
    }
}

#[cfg(target_os = "linux")]
fn show_message(message: &str) {
    if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        println!("{message}");
    } else {
        let _ = dialog::Message::new(message.to_string())
            .title("HyperHeadset")
            .show();
    }
}

#[cfg(target_os = "linux")]
fn print_udev_rules_diff(path: &str, expected_rules: &str) {
    use std::io::Write;
    if std::fs::metadata(path).is_err() {
        return;
    }

    let mut child = match Command::new("diff")
        .arg("-u")
        .arg(path)
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(expected_rules.as_bytes());
    }

    let output = match child.wait_with_output() {
        Ok(o) => o,
        Err(_) => return,
    };

    let diff_text = String::from_utf8_lossy(&output.stdout);
    println!("\n--- Diff for {} ---", path);
    for line in diff_text.lines() {
        if line.starts_with("---") {
            println!("\x1b[31m{}\x1b[0m", line); // Red
        } else if line.starts_with("+++") {
            println!("\x1b[32m{}\x1b[0m", line); // Green
        } else if line.starts_with('-') {
            println!("\x1b[31m{}\x1b[0m", line); // Red
        } else if line.starts_with('+') {
            println!("\x1b[32m{}\x1b[0m", line); // Green
        } else if line.starts_with("@@") {
            println!("\x1b[36m{}\x1b[0m", line); // Cyan
        } else {
            println!("{}", line);
        }
    }
    println!("-------------------\n");
}

#[cfg(target_os = "linux")]
fn handle_udev_rule_user_interaction(
    path: &str,
    fall_back_path: &str,
    ask_message: &str,
    decline_message: &str,
) {
    if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        print_udev_rules_diff(path, UDEV_RULES);
        print!("{ask_message} (y/N): ");
        io::Write::flush(&mut io::stdout()).unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        if matches!(input.trim(), "y" | "Y") {
            update_rule(path, fall_back_path, UDEV_RULES);
        } else {
            println!("{decline_message}");
        }
    } else if dialog::Question::new(ask_message.to_string())
        .title("HyperHeadset")
        .show()
        .unwrap_or(Choice::No)
        == Choice::Yes
    {
        update_rule(path, fall_back_path, UDEV_RULES);
    } else {
        let _ = dialog::Message::new(decline_message.to_string())
            .title("HyperHeadset")
            .show();
    }
}

#[cfg(target_os = "linux")]
pub fn prompt_user_for_udev_rule() {
    if matches!(std::env::var(NO_AUTO_UDEV_ENV).as_deref(), Ok("1" | "true")) {
        println!(
            "Automatic udev rules check disabled by the {NO_AUTO_UDEV_ENV} environment variable."
        );
        return;
    }
    let user_rule_state = check_rule(UDEV_RULE_PATH_USER, UDEV_RULES);
    let system_rule_state = check_rule(UDEV_RULE_PATH_SYSTEM, UDEV_RULES);

    debug_println!("user rule: {user_rule_state:?}, system rule: {system_rule_state:?}");
    match (user_rule_state, system_rule_state) {
        (RuleState::RuleMatch(true), _) => (),
        (_, RuleState::RuleMatch(true)) => (),
        (RuleState::RuleMatch(false), _) | (RuleState::RuleExists(true), _) => {
            handle_udev_rule_user_interaction(UDEV_RULE_PATH_USER, UDEV_RULE_PATH_SYSTEM,
                &format!("Udev rules at {UDEV_RULE_PATH_USER} do not have the expected value. Do you want to recreate them?"), 
                "Your headset may not be recognized without the correct udev rules.");
        }
        (RuleState::RuleExists(false), RuleState::RuleMatch(false))
        | (RuleState::RuleExists(false), RuleState::RuleExists(true)) => {
            handle_udev_rule_user_interaction(UDEV_RULE_PATH_SYSTEM, UDEV_RULE_PATH_USER,
                &format!("Udev rules at {UDEV_RULE_PATH_SYSTEM} do not have the expected value. Do you want to recreate them?"), 
                "Your headset may not be recognized without the correct udev rules.");
        }
        (RuleState::RuleExists(false), RuleState::RuleExists(false)) => {
            handle_udev_rule_user_interaction(
                UDEV_RULE_PATH_USER,
                UDEV_RULE_PATH_SYSTEM,
                &format!("No udev rules found. Do you want to create {UDEV_RULE_PATH_USER}?"),
                "Without udev rules your headset can only be accessed when running as root.",
            );
        }
    }
}

/// Returns whether the default microphone is muted, or `None` if `pactl` is unavailable or
/// fails.
#[cfg(target_os = "linux")]
pub fn is_default_microphone_muted() -> Option<bool> {
    let output = Command::new("pactl")
        .args(["get-source-mute", "@DEFAULT_SOURCE@"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    // Output looks like "Mute: yes"; the value isn't translated.
    match String::from_utf8_lossy(&output.stdout)
        .split(':')
        .nth(1)?
        .trim()
    {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

/// Mutes or unmutes the default microphone with `pactl`.
#[cfg(target_os = "linux")]
pub fn set_default_microphone_mute(muted: bool) -> io::Result<()> {
    let status = Command::new("pactl")
        .args([
            "set-source-mute",
            "@DEFAULT_SOURCE@",
            if muted { "1" } else { "0" },
        ])
        .status()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "pactl set-source-mute exited with {status}"
        )));
    }
    Ok(())
}

/// An audio output sink reported by PulseAudio / PipeWire (`pactl list sinks`).
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioSink {
    pub name: String,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
}

#[cfg(target_os = "linux")]
fn parse_hex_u16(raw: &str) -> Option<u16> {
    let trimmed = raw.trim().trim_matches('"');
    let hex = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    u16::from_str_radix(hex, 16).ok()
}

/// Parses the plain-text output of `LC_ALL=C pactl list sinks`.
#[cfg(target_os = "linux")]
pub fn parse_pactl_sinks(output: &str) -> Vec<AudioSink> {
    let mut sinks = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_vid: Option<u16> = None;
    let mut current_pid: Option<u16> = None;

    let flush = |sinks: &mut Vec<AudioSink>,
                 name: &mut Option<String>,
                 vid: &mut Option<u16>,
                 pid: &mut Option<u16>| {
        if let Some(name) = name.take() {
            sinks.push(AudioSink {
                name,
                vendor_id: vid.take(),
                product_id: pid.take(),
            });
        } else {
            *vid = None;
            *pid = None;
        }
    };

    for line in output.lines() {
        if line.starts_with("Sink #") {
            flush(
                &mut sinks,
                &mut current_name,
                &mut current_vid,
                &mut current_pid,
            );
            continue;
        }
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix("Name:") {
            current_name = Some(name.trim().to_string());
        } else if let Some(val) = trimmed.strip_prefix("device.vendor.id =") {
            current_vid = parse_hex_u16(val);
        } else if let Some(val) = trimmed.strip_prefix("device.product.id =") {
            current_pid = parse_hex_u16(val);
        } else if (current_vid.is_none() || current_pid.is_none())
            && trimmed.starts_with("alsa.components =")
        {
            if let Some(val) = trimmed.strip_prefix("alsa.components =") {
                if let Some(usb) = val.trim().trim_matches('"').strip_prefix("USB") {
                    if let Some((v, p)) = usb.split_once(':') {
                        current_vid = current_vid.or_else(|| parse_hex_u16(v));
                        current_pid = current_pid.or_else(|| parse_hex_u16(p));
                    }
                }
            }
        }
    }
    flush(
        &mut sinks,
        &mut current_name,
        &mut current_vid,
        &mut current_pid,
    );
    sinks
}

/// Lists available PulseAudio / PipeWire audio output sinks via `pactl`.
#[cfg(target_os = "linux")]
pub fn list_audio_sinks() -> io::Result<Vec<AudioSink>> {
    let output = Command::new("pactl")
        .env("LC_ALL", "C")
        .args(["list", "sinks"])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "pactl list sinks exited with {}",
            output.status
        )));
    }
    Ok(parse_pactl_sinks(&String::from_utf8_lossy(&output.stdout)))
}

/// Returns the current default audio output sink name, or `None` if `pactl` is unavailable or
/// fails.
#[cfg(target_os = "linux")]
pub fn get_default_audio_sink() -> Option<String> {
    let output = Command::new("pactl")
        .args(["get-default-sink"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!name.is_empty()).then_some(name)
}

/// Sets the default audio output sink and moves active playback streams to it via `pactl`.
#[cfg(target_os = "linux")]
pub fn set_default_audio_sink(sink_name: &str) -> io::Result<()> {
    let status = Command::new("pactl")
        .args(["set-default-sink", sink_name])
        .status()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "pactl set-default-sink exited with {status}"
        )));
    }
    if let Ok(output) = Command::new("pactl")
        .args(["list", "short", "sink-inputs"])
        .output()
    {
        if output.status.success() {
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                if let Some(index) = line.split_whitespace().next() {
                    let _ = Command::new("pactl")
                        .args(["move-sink-input", index, sink_name])
                        .status();
                }
            }
        }
    }
    Ok(())
}

#[cfg(feature = "eq-editor")]
pub fn launch_eq_editor() {
    let mut exe_path = match std::env::current_exe() {
        Ok(path) => path,
        Err(_) => return,
    };
    #[cfg(target_os = "windows")]
    exe_path.set_file_name("hyper_headset_cli.exe");
    #[cfg(not(target_os = "windows"))]
    exe_path.set_file_name("hyper_headset_cli");
    let cli_path = if exe_path.exists() {
        exe_path
    } else {
        std::path::PathBuf::from("hyper_headset_cli")
    };

    let cli_str = cli_path.to_string_lossy();

    #[cfg(target_os = "linux")]
    {
        let shell_cmd = format!(
            "\"{}\" --eq; echo; echo 'Press Enter to close...'; read _",
            cli_str.replace('"', "\\\"")
        );
        let command_args = ["/bin/sh", "-c", &shell_cmd];

        let terminals = [
            ("xdg-terminal-exec", vec![]),
            ("x-terminal-emulator", vec!["-e"]),
            ("gnome-terminal", vec!["--"]),
            ("konsole", vec!["-e"]),
            ("xfce4-terminal", vec!["-x"]),
            ("mate-terminal", vec!["-e"]),
            ("lxterminal", vec!["-e"]),
            ("alacritty", vec!["-e"]),
            ("kitty", vec![]),
            ("xterm", vec!["-e"]),
        ];

        for (term, term_args) in terminals {
            let mut cmd = std::process::Command::new(term);
            cmd.args(&term_args);
            cmd.args(&command_args);
            if cmd.spawn().is_ok() {
                return;
            }
        }

        let error_msg = "Could not open the terminal emulator.\n\n\
                         Would you like to copy the command to your clipboard?\n\n\
                         hyper_headset_cli --eq";
        let choice = dialog::Question::new(error_msg)
            .title("HyperX Equalizer Editor")
            .show();

        if let Ok(Choice::Yes) = choice {
            if !copy_to_clipboard("hyper_headset_cli --eq") {
                let _ = dialog::Message::new(
                    "Failed to copy to clipboard. Please run manually:\n\nhyper_headset_cli --eq",
                )
                .title("HyperX Equalizer Editor")
                .show();
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // `start` treats the first quoted argument as the window title, so an explicit empty
        // title must precede the command. The CLI binary is resolved next to the tray binary;
        // if that fails the bare name is used, which requires hyper_headset_cli to be on PATH.
        // TODO(LennardKittner): improve PATH handling / installer guidance for Windows.
        let mut cmd = std::process::Command::new("cmd.exe");
        cmd.args(&["/c", "start", "", "cmd", "/k", &format!("{} --eq", cli_str)]);
        let _ = cmd.spawn();
    }
}

pub fn open_url(url: &str) {
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "", url])
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
}

pub fn copy_to_clipboard(text: &str) -> bool {
    #[cfg(target_os = "linux")]
    {
        // Try wl-copy first (Wayland)
        if let Ok(mut child) = std::process::Command::new("wl-copy")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if child.wait().is_ok() {
                return true;
            }
        }

        // Try xclip (X11)
        if let Ok(mut child) = std::process::Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if child.wait().is_ok() {
                return true;
            }
        }

        // Try xsel (X11 alternative)
        if let Ok(mut child) = std::process::Command::new("xsel")
            .args(["--clipboard", "--input"])
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if child.wait().is_ok() {
                return true;
            }
        }

        false
    }

    #[cfg(target_os = "windows")]
    {
        use std::io::Write;
        if let Ok(mut child) = std::process::Command::new("clip")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if child.wait().is_ok() {
                return true;
            }
        }
        false
    }

    #[cfg(target_os = "macos")]
    {
        use std::io::Write;
        if let Ok(mut child) = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if child.wait().is_ok() {
                return true;
            }
        }
        false
    }
}

#[cfg(all(test, target_os = "linux"))]
mod audio_sink_tests {
    use super::*;

    #[test]
    fn parses_pactl_sinks_with_vendor_and_product_ids() {
        let sample = "\
Sink #54
\tState: SUSPENDED
\tName: alsa_output.usb-Kingston_HyperX_Quadcast_4110-00.analog-stereo
\tProperties:
\t\tdevice.vendor.id = \"0x0951\"
\t\tdevice.product.id = \"0x16df\"
Sink #56
\tState: RUNNING
\tName: alsa_output.usb-HP__Inc_HyperX_Cloud_III_Wireless_0000000000000000-00.analog-stereo
\tProperties:
\t\tdevice.vendor.id = \"0x03f0\"
\t\tdevice.product.id = \"0x05b7\"
Sink #58
\tState: IDLE
\tName: alsa_output.usb-bestechnic_EDIFIER_M60_20160406.1-00.analog-stereo
\tProperties:
\t\talsa.components = \"USB2d99:a094\"
";
        let sinks = parse_pactl_sinks(sample);
        assert_eq!(
            sinks,
            vec![
                AudioSink {
                    name: "alsa_output.usb-Kingston_HyperX_Quadcast_4110-00.analog-stereo".into(),
                    vendor_id: Some(0x0951),
                    product_id: Some(0x16df),
                },
                AudioSink {
                    name: "alsa_output.usb-HP__Inc_HyperX_Cloud_III_Wireless_0000000000000000-00.analog-stereo".into(),
                    vendor_id: Some(0x03f0),
                    product_id: Some(0x05b7),
                },
                AudioSink {
                    name: "alsa_output.usb-bestechnic_EDIFIER_M60_20160406.1-00.analog-stereo".into(),
                    vendor_id: Some(0x2d99),
                    product_id: Some(0xa094),
                },
            ]
        );
    }
}
