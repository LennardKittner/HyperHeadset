use std::{fs, io, process::Command, time::Duration};

// #![warn(missing_docs)]
pub mod devices;

#[macro_export]
macro_rules! debug_println {
    ($($args:tt)*) => {
        #[cfg(debug_assertions)]
        println!($($args)*);
    };
}

pub const UDEV_RULE_PATH_SYSTEM: &str = "/etc/udev/rules.d/99-HyperHeadset.rules";
pub const UDEV_RULE_PATH_USER: &str = "/usr/lib/udev/rules.d/99-HyperHeadset.rules";
pub const UDEV_RULES: &str = include_str!("./../99-HyperHeadset.rules");

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
    let status = Command::new("sudo")
        .arg("sh")
        .arg("-c")
        .arg(format!(
            "echo {} > {} && udevadm control --reload-rules && udevadm trigger",
            shell_escape::escape(rules.into()),
            shell_escape::escape(path.into())
        ))
        .status();
    // a little delay so the rules are active before trying to connect
    std::thread::sleep(Duration::from_millis(500));

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

pub fn prompt_user_for_udev_rule() {
    let user_rule_state = check_rule(UDEV_RULE_PATH_USER, UDEV_RULES);
    let system_rule_state = check_rule(UDEV_RULE_PATH_SYSTEM, UDEV_RULES);

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
