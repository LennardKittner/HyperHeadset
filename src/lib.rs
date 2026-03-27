#[cfg(target_os = "linux")]
use std::{fs, io, process::Command, time::Duration};

#[cfg(target_os = "linux")]
use dialog::{Choice, DialogBox};

// #![warn(missing_docs)]
pub mod devices;
#[cfg(feature = "eq-support")]
pub mod eq;

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
pub fn update_rule(path: &str, rules: &str) {
    let status = if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        Command::new("sudo")
            .arg("sh")
            .arg("-c")
            .arg(format!(
                "echo {} > {} && udevadm control --reload-rules && udevadm trigger",
                shell_escape::escape(rules.into()),
                shell_escape::escape(path.into())
            ))
            .status()
    } else {
        Command::new("sudo")
            .env("SUDO_ASKPASS", std::env::current_exe().unwrap())
            .arg("--askpass")
            .arg("sh")
            .arg("-c")
            .arg(format!(
                "echo {} > {} && udevadm control --reload-rules && udevadm trigger",
                shell_escape::escape(rules.into()),
                shell_escape::escape(path.into())
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
fn handle_udev_rule_user_interaction(path: &str, ask_message: &str, decline_message: &str) {
    if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        print!("{ask_message} (y/N): ");
        io::Write::flush(&mut io::stdout()).unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        if matches!(input.trim(), "y" | "Y") {
            update_rule(path, UDEV_RULES);
        } else {
            println!("{decline_message}");
        }
    } else if dialog::Question::new(ask_message.to_string())
        .title("HyperHeadset")
        .show()
        .unwrap_or(Choice::No)
        == Choice::Yes
    {
        update_rule(path, UDEV_RULES);
    } else {
        let _ = dialog::Message::new(decline_message.to_string())
            .title("HyperHeadset")
            .show();
    }
}

#[cfg(target_os = "linux")]
pub fn prompt_user_for_udev_rule() {
    let user_rule_state = check_rule(UDEV_RULE_PATH_USER, UDEV_RULES);
    let system_rule_state = check_rule(UDEV_RULE_PATH_SYSTEM, UDEV_RULES);

    debug_println!("user rule: {user_rule_state:?}, system rule: {system_rule_state:?}");
    match (user_rule_state, system_rule_state) {
        (RuleState::RuleMatch(true), _) => (),
        (_, RuleState::RuleMatch(true)) => (),

        (RuleState::RuleMatch(false), _) | (RuleState::RuleExists(true), _) => {
            handle_udev_rule_user_interaction(UDEV_RULE_PATH_USER,
                &format!("Udev rules at {UDEV_RULE_PATH_USER} do not have the expected value. Do you want to recreate them?"), 
                "Your headset may not be recognized without the correct udev rules.");
        }
        (RuleState::RuleExists(false), RuleState::RuleMatch(false))
        | (RuleState::RuleExists(false), RuleState::RuleExists(true)) => {
            handle_udev_rule_user_interaction(UDEV_RULE_PATH_SYSTEM,
                &format!("Udev rules at {UDEV_RULE_PATH_SYSTEM} do not have the expected value. Do you want to recreate them?"), 
                "Your headset may not be recognized without the correct udev rules.");
        }

        (RuleState::RuleExists(false), RuleState::RuleExists(false)) => {
            handle_udev_rule_user_interaction(
                UDEV_RULE_PATH_USER,
                &format!("No udev rules found. Do you want to create {UDEV_RULE_PATH_USER}?"),
                "Without udev rules your headset can only be accessed when running as root.",
            );
        }
    }
}
