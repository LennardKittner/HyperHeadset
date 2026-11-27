//! The Linux USB quirk the Cloud III S Wireless dongle needs.
//!
//! If the first configuration descriptor request after power-up is Linux's 9 bytes (Windows
//! asks for 255), the dongle stops forwarding the headset's replies until it loses power.
//! Audio, the microphone and commands sent to the headset keep working. The quirk makes the
//! kernel ask for 255 bytes. See issue #36.

use std::{
    fs,
    io::{self, IsTerminal, Read, Write},
    path::Path,
    process::{Command, Stdio},
};

mod grub;

use crate::devices::{
    cloud_iii_s_wireless::{PRODUCT_IDS, VENDOR_IDS},
    connect_compatible_device, DeviceProperties,
};

pub const QUIRKS_PARAM_PATH: &str = "/sys/module/usbcore/parameters/quirks";
const USB_DEVICES_PATH: &str = "/sys/bus/usb/devices";
const CMDLINE_PATH: &str = "/proc/cmdline";
/// The kernel's letter for requesting 255 bytes in the first configuration descriptor read.
const QUIRK_FLAG: char = 'r';
pub const FIX_FLAG: &str = "--fix-usb-quirk";

/// Why a connected Cloud III S isn't answering queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuirkIssue {
    /// The quirk isn't set.
    Missing,
    /// The quirk is set but the dongle still doesn't answer: it hasn't lost power since, or
    /// the kernel doesn't know the quirk and ignores it.
    NotEffective,
}

impl QuirkIssue {
    /// Short lines explaining the issue, narrow enough for a tray menu.
    pub fn explanation(self) -> &'static [&'static str] {
        match self {
            QuirkIssue::Missing => &[
                "The dongle isn't passing on the headset's replies,",
                "so battery level and other states are missing.",
                "A Linux USB quirk fixes this.",
            ],
            QuirkIssue::NotEffective => &[
                "The USB quirk is set, but replies are still missing.",
                "Unplug the dongle, wait 5 s and plug it back in.",
                "If that doesn't help, your kernel may be too old",
                "(needs 7.1.9, 7.2 or a distro backport).",
            ],
        }
    }
}

pub fn is_affected_device(vendor_id: u16, product_id: u16) -> bool {
    VENDOR_IDS.contains(&vendor_id) && PRODUCT_IDS.contains(&product_id)
}

/// Whether the properties show the symptom the quirk fixes: the dongle answers the
/// connected-status query itself, but nothing that needs the headset's reply arrived.
///
/// This looks at the values rather than at which query got an answer, because replies can
/// arrive late: a connected-status reply then looks like the answer to a later query. Several
/// values are checked so one lost reply isn't enough. Mute is left out, as it can also come
/// from the microphone's own reports.
pub fn replies_blocked(properties: &DeviceProperties) -> bool {
    is_affected_device(properties.vendor_id, properties.product_id)
        && properties.connected == Some(true)
        && properties.battery_level.is_none()
        && properties.charging.is_none()
        && properties.automatic_shutdown_after.is_none()
}

/// The issue to report for an affected device whose replies are blocked.
pub fn diagnose(vendor_id: u16, product_id: u16) -> QuirkIssue {
    let quirks = fs::read_to_string(QUIRKS_PARAM_PATH).unwrap_or_default();
    if has_quirk(&quirks, vendor_id, product_id) {
        QuirkIssue::NotEffective
    } else {
        QuirkIssue::Missing
    }
}

/// Splits a `vendor:product:flags` entry of the quirks parameter.
fn parse_entry(entry: &str) -> Option<(u16, u16, &str)> {
    let mut parts = entry.trim().splitn(3, ':');
    let parse_id = |id: &str| u16::from_str_radix(id.trim_start_matches("0x"), 16).ok();
    let vendor_id = parse_id(parts.next()?)?;
    let product_id = parse_id(parts.next()?)?;
    Some((vendor_id, product_id, parts.next()?))
}

fn has_quirk(quirks: &str, vendor_id: u16, product_id: u16) -> bool {
    quirks
        .split(',')
        .filter_map(parse_entry)
        .any(|(v, p, flags)| v == vendor_id && p == product_id && flags.contains(QUIRK_FLAG))
}

/// Returns `quirks` with the quirk added for the device, keeping all other entries and flags.
/// Writing the parameter replaces its whole value, so the others have to be written back.
fn with_quirk(quirks: &str, vendor_id: u16, product_id: u16) -> String {
    let mut entries: Vec<String> = quirks
        .trim()
        .split(',')
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
        .collect();
    if !has_quirk(quirks, vendor_id, product_id) {
        let existing = entries.iter_mut().find(
            |entry| matches!(parse_entry(entry), Some((v, p, _)) if v == vendor_id && p == product_id),
        );
        match existing {
            Some(entry) => entry.push(QUIRK_FLAG),
            None => entries.push(format!("{vendor_id:04x}:{product_id:04x}:{QUIRK_FLAG}")),
        }
    }
    entries.join(",")
}

/// Vendor and product IDs of the plugged in devices that need the quirk.
fn plugged_in_affected_devices(usb_devices: &Path) -> io::Result<Vec<(u16, u16)>> {
    let read_id = |path: &Path| u16::from_str_radix(fs::read_to_string(path).ok()?.trim(), 16).ok();
    let mut devices: Vec<(u16, u16)> = fs::read_dir(usb_devices)?
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let vendor_id = read_id(&path.join("idVendor"))?;
            let product_id = read_id(&path.join("idProduct"))?;
            is_affected_device(vendor_id, product_id).then_some((vendor_id, product_id))
        })
        .collect();
    devices.sort_unstable();
    devices.dedup();
    Ok(devices)
}

/// Writes a root-owned file with `sudo tee`, which prompts on the terminal if needed.
fn sudo_write(path: &str, content: &str) -> Result<(), String> {
    let mut child = Command::new("sudo")
        .args(["tee", path])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to run sudo: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(content.as_bytes())
            .map_err(|e| format!("Failed to pass the value to sudo: {e}"))?;
    }
    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for sudo: {e}"))?;
    if !status.success() {
        return Err(format!("Writing {path} failed: {status}"));
    }
    Ok(())
}

/// The kernel's device numbers of the plugged in affected devices. A replugged device gets a
/// new number.
fn affected_device_numbers(usb_devices: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(usb_devices) else {
        return Vec::new();
    };
    let mut numbers: Vec<String> = entries
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let read = |name: &str| fs::read_to_string(path.join(name)).ok();
            let read_id = |name: &str| u16::from_str_radix(read(name)?.trim(), 16).ok();
            if !is_affected_device(read_id("idVendor")?, read_id("idProduct")?) {
                return None;
            }
            Some(format!(
                "{}-{}",
                read("busnum")?.trim(),
                read("devnum")?.trim()
            ))
        })
        .collect();
    numbers.sort_unstable();
    numbers
}

/// What asking the headset for its state showed.
enum Probe {
    Working,
    Blocked,
    HeadsetOff,
    Failed(String),
}

fn probe_replies() -> Probe {
    let mut headset = match connect_compatible_device() {
        Ok(headset) => headset,
        Err(e) => return Probe::Failed(e.to_string()),
    };
    // Rules out the Bluetooth fallback and other headsets.
    let properties = headset.device_properties();
    if headset.hid_mut().is_none()
        || !is_affected_device(properties.vendor_id, properties.product_id)
    {
        return Probe::Failed("Couldn't open the Cloud III S dongle.".to_string());
    }
    if let Err(e) = headset.active_refresh_state() {
        return Probe::Failed(e.to_string());
    }
    let properties = headset.device_properties();
    if properties.connected != Some(true) {
        Probe::HeadsetOff
    } else if replies_blocked(&properties) {
        Probe::Blocked
    } else {
        Probe::Working
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Key {
    Check,
    Quit,
    Other,
}

/// Classifies the bytes of one key press. Arrow and function keys also start with Escape, so
/// only a lone Escape byte quits. Ctrl-C, Ctrl-D and end of input quit too.
fn classify_key(bytes: &[u8]) -> Key {
    match bytes {
        [b'\r' | b'\n' | b' '] => Key::Check,
        [] | [0x1b | 0x03 | 0x04 | b'q' | b'Q'] => Key::Quit,
        _ => Key::Other,
    }
}

/// Puts the terminal into single key mode until dropped. Ctrl-C arrives as a byte, so it
/// can't leave the terminal in that mode.
struct SingleKeyMode {
    previous: String,
}

impl SingleKeyMode {
    fn enter() -> Result<Self, String> {
        let previous = stty(&["-g"])?;
        stty(&["-icanon", "-echo", "-isig", "min", "1", "time", "0"])?;
        Ok(SingleKeyMode { previous })
    }
}

impl Drop for SingleKeyMode {
    fn drop(&mut self) {
        let _ = stty(&[&self.previous]);
    }
}

/// Runs `stty` on this process' terminal and returns its output.
fn stty(args: &[&str]) -> Result<String, String> {
    let output = Command::new("stty")
        .args(args)
        .stdin(Stdio::inherit())
        .output()
        .map_err(|e| format!("Failed to run stty: {e}"))?;
    if !output.status.success() {
        return Err(format!("stty failed: {}", output.status));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Waits for a key that checks or quits.
fn wait_for_key() -> Result<Key, String> {
    let _mode = SingleKeyMode::enter()?;
    let mut buffer = [0u8; 16];
    loop {
        let length = io::stdin()
            .read(&mut buffer)
            .map_err(|e| format!("Failed to read a key: {e}"))?;
        let key = classify_key(&buffer[..length]);
        if key != Key::Other {
            return Ok(key);
        }
    }
}

/// Asks a yes/no question on the terminal, defaulting to no.
fn ask(question: &str) -> Result<bool, String> {
    print!("{question} (y/N): ");
    io::stdout().flush().map_err(|e| e.to_string())?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|e| e.to_string())?;
    Ok(matches!(answer.trim(), "y" | "Y"))
}

fn print_manual_persistence(quirks: &str) {
    println!("To keep it across reboots, add this to your kernel command line");
    println!("(how depends on your bootloader, see your distribution's documentation):");
    println!("  usbcore.quirks={quirks}");
}

/// Once the quirk is known to work, offers to keep it across reboots.
fn offer_persistence(devices: &[(u16, u16)], quirks: &str) -> Result<(), String> {
    let cmdline = fs::read_to_string(CMDLINE_PATH).unwrap_or_default();
    // With our drop-in there, still check it covers every plugged in device.
    if !grub::is_installed()
        && devices
            .iter()
            .all(|&(vendor_id, product_id)| cmdline_has_quirk(&cmdline, vendor_id, product_id))
    {
        return Ok(());
    }
    println!();
    match grub::current_plan(devices, &cmdline) {
        grub::Plan::AlreadyConfigured(path) if path == Path::new(grub::drop_in_path()) => {
            println!("GRUB keeps the quirk across reboots ({}).", path.display());
        }
        grub::Plan::AlreadyConfigured(path) => {
            println!("{} already sets the quirk for GRUB.", path.display());
            println!("It wasn't written by this tool, so it can't tell whether GRUB's config");
            println!("was regenerated since. If unsure, run: sudo update-grub");
        }
        grub::Plan::OtherQuirks(path) => {
            println!("{} sets other USB quirks, so nothing", path.display());
            println!("is changed automatically. Merge this into that setting:");
            println!("  usbcore.quirks={quirks}");
        }
        grub::Plan::Unsupported => print_manual_persistence(quirks),
        grub::Plan::Install(value) => {
            println!("The quirk is lost on reboot. If you boot with GRUB, it can be kept by");
            println!("writing usbcore.quirks={value} to {}", grub::drop_in_path());
            println!("and running update-grub. This needs root.");
            if !io::stdin().is_terminal() {
                println!("Not asking to install it, because input isn't a terminal. Run");
                println!("hyper_headset_cli {FIX_FLAG} in a terminal to be asked,");
                println!("or add usbcore.quirks={value} to your kernel command line yourself.");
                return Ok(());
            }
            if !ask("Do you want to install it to GRUB now?")? {
                println!("GRUB is unchanged.");
                print_manual_persistence(&value);
                return Ok(());
            }
            grub::install(&value)?;
            println!("Installed. To uninstall, run hyper_headset_cli {FIX_FLAG} again.");
        }
    }
    Ok(())
}

/// If the quirk is installed in GRUB, offers to uninstall it. Returns whether it did.
fn offer_uninstall() -> Result<bool, String> {
    if !grub::is_installed() {
        return Ok(false);
    }
    println!(
        "The USB quirk is installed in GRUB ({}).",
        grub::drop_in_path()
    );
    if !io::stdin().is_terminal() {
        println!("Run {FIX_FLAG} in a terminal to be asked about uninstalling it.");
        return Ok(false);
    }
    if !ask("Do you want to uninstall it from GRUB?")? {
        println!("Keeping it.");
        println!();
        return Ok(false);
    }
    grub::uninstall()?;
    println!("Uninstalled. The running system keeps the quirk until you reboot.");
    Ok(true)
}

/// Asks for a replug and checks the headset's replies on request, until they come through or
/// the user quits. Returns whether they came through.
fn confirm_after_replug(numbers_before: &[String]) -> Result<bool, String> {
    loop {
        println!();
        println!("Unplug the dongle, wait 5 s and plug it back in, with the headset on.");
        println!("Then press Enter or Space to check (Esc or q to quit).");
        if wait_for_key()? == Key::Quit {
            println!("Quit before the fix was confirmed.");
            return Ok(false);
        }
        let numbers = affected_device_numbers(Path::new(USB_DEVICES_PATH));
        if numbers.is_empty() {
            println!("The dongle isn't plugged in.");
        } else if numbers == numbers_before {
            println!("The dongle hasn't been replugged yet.");
        } else {
            println!("Checking...");
            match probe_replies() {
                Probe::Working => {
                    println!("Fixed: the headset's replies are coming through.");
                    return Ok(true);
                }
                Probe::HeadsetOff => {
                    println!("The headset isn't connected yet. Turn it on and wait a few seconds.");
                }
                Probe::Blocked => {
                    println!("Replies are still missing. If replugging doesn't help, your kernel");
                    println!("may be too old and ignore the quirk (it needs 7.1.9, 7.2 or a");
                    println!("distro backport).");
                }
                Probe::Failed(e) => println!("Couldn't check yet: {e}"),
            }
        }
    }
}

/// Asks the user, adds the quirk for every plugged in affected device, then guides them
/// through the replug. If the quirk is installed in GRUB, first offers to uninstall it.
pub fn run_interactive_fix() -> Result<(), String> {
    // Uninstalling doesn't need the dongle.
    if offer_uninstall()? {
        return Ok(());
    }
    let devices = plugged_in_affected_devices(Path::new(USB_DEVICES_PATH))
        .map_err(|e| format!("Failed to list USB devices: {e}"))?;
    if devices.is_empty() {
        return Err("No Cloud III S Wireless dongle found. Is it plugged in?".to_string());
    }
    let current = fs::read_to_string(QUIRKS_PARAM_PATH)
        .map_err(|e| format!("Failed to read {QUIRKS_PARAM_PATH}: {e}"))?;
    let current = current.trim();
    let numbers_before = affected_device_numbers(Path::new(USB_DEVICES_PATH));
    let updated = devices
        .iter()
        .fold(current.to_string(), |quirks, &(vendor_id, product_id)| {
            with_quirk(&quirks, vendor_id, product_id)
        });

    if updated == current {
        println!("The USB quirk is already set: {current}");
    } else {
        println!("On Linux, the Cloud III S dongle can stop passing on the headset's replies.");
        println!("A USB quirk fixes this. This needs root, to change");
        println!("{QUIRKS_PARAM_PATH}");
        println!("  from: \"{current}\"");
        println!("  to:   \"{updated}\"");
        if !io::stdin().is_terminal() {
            return Err(format!(
                "Run {FIX_FLAG} in a terminal to confirm the change."
            ));
        }
        if !ask("Apply it?")? {
            println!("Nothing changed.");
            return Ok(());
        }
        sudo_write(QUIRKS_PARAM_PATH, &updated)?;
        println!("USB quirk set.");
    }

    // With the quirk set before this run, the dongle may need nothing.
    if updated == current && matches!(probe_replies(), Probe::Working) {
        println!();
        println!("The headset's replies are coming through.");
        return offer_persistence(&devices, &updated);
    }
    if !io::stdin().is_terminal() {
        println!();
        println!("If replies are missing, unplug the dongle, wait 5 s and plug it back in.");
        print_manual_persistence(&updated);
        return Ok(());
    }
    if confirm_after_replug(&numbers_before)? {
        offer_persistence(&devices, &updated)
    } else {
        println!("The quirk is lost on reboot. Run this again to confirm it and keep it.");
        Ok(())
    }
}

/// Whether the kernel command line sets the quirk, i.e. it survives a reboot.
fn cmdline_has_quirk(cmdline: &str, vendor_id: u16, product_id: u16) -> bool {
    cmdline
        .split_whitespace()
        .filter_map(|parameter| parameter.strip_prefix("usbcore.quirks="))
        // Only the last setting counts.
        .next_back()
        .is_some_and(|quirks| has_quirk(quirks, vendor_id, product_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VID: u16 = 0x03f0;
    const PID: u16 = 0x06be;

    #[test]
    fn detects_quirk() {
        assert!(has_quirk("03f0:06be:r\n", VID, PID));
        assert!(has_quirk("1234:5678:k,0x03F0:0x06BE:kr", VID, PID));
        assert!(!has_quirk("", VID, PID));
        assert!(!has_quirk("03f0:06be:k", VID, PID));
        assert!(!has_quirk("03f0:02cc:r", VID, PID));
    }

    #[test]
    fn adds_quirk_keeping_other_entries() {
        assert_eq!(with_quirk("\n", VID, PID), "03f0:06be:r");
        assert_eq!(
            with_quirk("1234:5678:k", VID, PID),
            "1234:5678:k,03f0:06be:r"
        );
        assert_eq!(
            with_quirk("03f0:06be:k,1234:5678:g", VID, PID),
            "03f0:06be:kr,1234:5678:g"
        );
        assert_eq!(with_quirk("03f0:06be:r", VID, PID), "03f0:06be:r");
    }

    #[test]
    fn detects_blocked_replies() {
        let mut properties = DeviceProperties::new(PID, VID, None);
        assert!(!replies_blocked(&properties));
        properties.connected = Some(true);
        assert!(replies_blocked(&properties));
        properties.battery_level = Some(85);
        assert!(!replies_blocked(&properties));
        properties.battery_level = None;
        properties.product_id = 0x018b;
        assert!(!replies_blocked(&properties));
    }

    #[test]
    fn classifies_keys() {
        for bytes in [b"\r", b"\n", b" "] {
            assert_eq!(classify_key(bytes), Key::Check);
        }
        for bytes in [&b""[..], b"\x1b", b"q", b"Q", b"\x03", b"\x04"] {
            assert_eq!(classify_key(bytes), Key::Quit);
        }
        assert_eq!(classify_key(b"\x1b[A"), Key::Other);
        assert_eq!(classify_key(b"y"), Key::Other);
    }

    #[test]
    fn detects_persistent_quirk() {
        assert!(cmdline_has_quirk(
            "ro quiet usbcore.quirks=03f0:06be:r\n",
            VID,
            PID
        ));
        assert!(!cmdline_has_quirk("ro quiet", VID, PID));
        assert!(!cmdline_has_quirk(
            "usbcore.quirks=03f0:06be:r usbcore.quirks=1234:5678:k",
            VID,
            PID
        ));
    }

    #[test]
    fn finds_only_affected_devices() {
        let root = std::env::temp_dir().join(format!("hh-usb-quirk-{}", std::process::id()));
        for (name, vendor_id, product_id) in [
            ("1-1", "03f0", "06be"),
            ("1-2", "046d", "c548"),
            ("1-3", "03f0", "02cc"),
        ] {
            let device = root.join(name);
            fs::create_dir_all(&device).unwrap();
            fs::write(device.join("idVendor"), format!("{vendor_id}\n")).unwrap();
            fs::write(device.join("idProduct"), format!("{product_id}\n")).unwrap();
        }
        // Interfaces have no ID files.
        fs::create_dir_all(root.join("1-1:1.0")).unwrap();

        let devices = plugged_in_affected_devices(&root);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(devices.unwrap(), vec![(VID, 0x02cc), (VID, PID)]);
    }
}
