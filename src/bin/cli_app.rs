use hyper_x_cloud_ii_wireless::Device;

fn main() {
    let mut device = match Device::new() {
        Ok(device) => device,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let (battery_level, _) = match device.update_battery_level() {
        Ok(t) => t,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    println!("Battery level: {}%", battery_level);
}