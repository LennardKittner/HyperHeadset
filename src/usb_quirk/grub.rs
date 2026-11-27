//! Keeps the USB quirk across reboots with a GRUB drop-in file.
//!
//! Only automated where GRUB reads `/etc/default/grub.d` and has `update-grub`, as on Debian
//! and Ubuntu: it needs no edits to existing files and is undone by deleting one file.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use super::{has_quirk, sudo_write, with_quirk};

const DEFAULT_PATH: &str = "/etc/default/grub";
const DROP_IN_DIR: &str = "/etc/default/grub.d";
const DROP_IN_PATH: &str = "/etc/default/grub.d/hyper-headset-usb-quirk.cfg";
const MKCONFIG_PATHS: [&str; 2] = ["/usr/sbin/grub-mkconfig", "/usr/bin/grub-mkconfig"];
const UPDATE_GRUB_PATHS: [&str; 2] = ["/usr/sbin/update-grub", "/usr/bin/update-grub"];
const PARAMETER: &str = "usbcore.quirks=";

/// What can be done to keep the quirk across reboots.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Plan {
    /// The GRUB configuration already sets it.
    AlreadyConfigured(PathBuf),
    /// Another file sets different USB quirks. Only the last setting counts, so adding one
    /// could drop quirks the user relies on.
    OtherQuirks(PathBuf),
    /// This system's setup isn't one we can safely change.
    Unsupported,
    /// Write the drop-in with this parameter value.
    Install(String),
}

/// The values of `usbcore.quirks=` in a GRUB configuration file, ignoring comments.
fn configured_values(config: &str) -> impl Iterator<Item = &str> {
    config
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .flat_map(|line| line.split(PARAMETER).skip(1))
        .map(|rest| {
            let end = rest
                .find(|c: char| !c.is_ascii_alphanumeric() && c != ':' && c != ',')
                .unwrap_or(rest.len());
            &rest[..end]
        })
}

/// `configs` are the GRUB configuration files in the order GRUB reads them.
fn plan(
    devices: &[(u16, u16)],
    configs: &[(PathBuf, String)],
    cmdline: &str,
    supported: bool,
) -> Plan {
    let add_devices = |quirks: &str| {
        devices
            .iter()
            .fold(quirks.to_string(), |quirks, &(vendor_id, product_id)| {
                with_quirk(&quirks, vendor_id, product_id)
            })
    };
    let settings: Vec<(&Path, &str)> = configs
        .iter()
        .flat_map(|(path, config)| {
            configured_values(config).map(move |value| (path.as_path(), value))
        })
        .collect();

    // The last setting wins.
    if let Some(&(path, value)) = settings.last() {
        if devices
            .iter()
            .all(|&(vendor_id, product_id)| has_quirk(value, vendor_id, product_id))
        {
            return Plan::AlreadyConfigured(path.to_path_buf());
        }
    }
    if let Some((path, _)) = settings
        .iter()
        .find(|(path, _)| *path != Path::new(DROP_IN_PATH))
    {
        return Plan::OtherQuirks(path.to_path_buf());
    }
    if !supported {
        return Plan::Unsupported;
    }
    // Keep what our drop-in or else the running kernel already sets.
    let base = settings.last().map(|&(_, value)| value).or_else(|| {
        cmdline
            .split_whitespace()
            .filter_map(|parameter| parameter.strip_prefix(PARAMETER))
            .next_back()
    });
    Plan::Install(add_devices(base.unwrap_or_default()))
}

fn drop_in_content(value: &str) -> Result<String, String> {
    // The file is read by a shell, and part of the value comes from the kernel command line.
    if !value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == ':' || c == ',')
    {
        return Err(format!("Unexpected characters in the USB quirks: {value}"));
    }
    Ok(format!(
        "# Added by hyper_headset_cli for the HyperX Cloud III S Wireless dongle.\n\
         # To undo, delete this file and run update-grub.\n\
         GRUB_CMDLINE_LINUX_DEFAULT=\"$GRUB_CMDLINE_LINUX_DEFAULT {PARAMETER}{value}\"\n"
    ))
}

fn is_supported() -> bool {
    Path::new(DEFAULT_PATH).exists()
        && UPDATE_GRUB_PATHS
            .iter()
            .any(|path| Path::new(path).exists())
        && MKCONFIG_PATHS.iter().any(|path| {
            fs::read_to_string(path).is_ok_and(|script| script.contains("default/grub.d"))
        })
}

fn read_configs(supported: bool) -> Vec<(PathBuf, String)> {
    let mut paths = vec![PathBuf::from(DEFAULT_PATH)];
    if supported {
        let mut drop_ins: Vec<PathBuf> = fs::read_dir(DROP_IN_DIR)
            .into_iter()
            .flatten()
            .filter_map(|entry| Some(entry.ok()?.path()))
            .filter(|path| path.extension().is_some_and(|extension| extension == "cfg"))
            .collect();
        drop_ins.sort_unstable();
        paths.append(&mut drop_ins);
    }
    paths
        .into_iter()
        .filter_map(|path| {
            let config = fs::read_to_string(&path).ok()?;
            Some((path, config))
        })
        .collect()
}

pub(super) fn current_plan(devices: &[(u16, u16)], cmdline: &str) -> Plan {
    let supported = is_supported();
    plan(devices, &read_configs(supported), cmdline, supported)
}

/// Writes the drop-in and regenerates the GRUB configuration, both with `sudo`.
pub(super) fn install(value: &str) -> Result<(), String> {
    let content = drop_in_content(value)?;
    let run = |args: &[&str]| {
        let status = Command::new("sudo")
            .args(args)
            .status()
            .map_err(|e| format!("Failed to run sudo: {e}"))?;
        if !status.success() {
            return Err(format!("sudo {} failed: {status}", args.join(" ")));
        }
        Ok(())
    };
    run(&["mkdir", "-p", DROP_IN_DIR])?;
    sudo_write(DROP_IN_PATH, &content)?;
    run(&["update-grub"]).map_err(|e| {
        format!("{e}\n{DROP_IN_PATH} was written but isn't active until update-grub succeeds.")
    })
}

pub(super) fn drop_in_path() -> &'static str {
    DROP_IN_PATH
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEVICES: [(u16, u16); 1] = [(0x03f0, 0x06be)];

    fn configs(files: &[(&str, &str)]) -> Vec<(PathBuf, String)> {
        files
            .iter()
            .map(|(path, config)| (PathBuf::from(path), config.to_string()))
            .collect()
    }

    #[test]
    fn installs_keeping_the_running_quirks() {
        let configs = configs(&[(
            DEFAULT_PATH,
            "# usbcore.quirks=1111:2222:k\nGRUB_CMDLINE_LINUX_DEFAULT=\"quiet\"\n",
        )]);
        assert_eq!(
            plan(&DEVICES, &configs, "ro quiet", true),
            Plan::Install("03f0:06be:r".to_string())
        );
        assert_eq!(
            plan(&DEVICES, &configs, "ro usbcore.quirks=1234:5678:k", true),
            Plan::Install("1234:5678:k,03f0:06be:r".to_string())
        );
        assert_eq!(plan(&DEVICES, &configs, "ro", false), Plan::Unsupported);
    }

    #[test]
    fn extends_its_own_drop_in() {
        let configs = configs(&[(
            DROP_IN_PATH,
            "GRUB_CMDLINE_LINUX_DEFAULT=\"$GRUB_CMDLINE_LINUX_DEFAULT usbcore.quirks=03f0:02cc:r\"\n",
        )]);
        assert_eq!(
            plan(&DEVICES, &configs, "ro", true),
            Plan::Install("03f0:02cc:r,03f0:06be:r".to_string())
        );
    }

    #[test]
    fn leaves_existing_settings_alone() {
        let set = configs(&[(DEFAULT_PATH, "X=\"quiet usbcore.quirks=03f0:06be:r\"")]);
        assert_eq!(
            plan(&DEVICES, &set, "ro", true),
            Plan::AlreadyConfigured(PathBuf::from(DEFAULT_PATH))
        );
        let other = configs(&[(DEFAULT_PATH, "X=\"usbcore.quirks=1234:5678:k\"")]);
        assert_eq!(
            plan(&DEVICES, &other, "ro", true),
            Plan::OtherQuirks(PathBuf::from(DEFAULT_PATH))
        );
    }

    #[test]
    fn refuses_unexpected_characters() {
        assert!(drop_in_content("03f0:06be:r").is_ok());
        assert!(drop_in_content("$(reboot)").is_err());
    }
}
