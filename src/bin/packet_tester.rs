use hidapi::HidApi;

fn main() {
    let api = HidApi::new().unwrap();

    for info in api.device_list() {
        if info.vendor_id() != 0x03F0 || info.product_id() != 0x03C0 {
            continue;
        }
        println!(
            "path={:?} usage_page={:#06x} usage={:#06x}",
            info.path(),
            info.usage_page(),
            info.usage()
        );

        let dev = match info.open_device(&api) {
            Ok(d) => d,
            Err(e) => {
                println!("  open failed: {e}");
                continue;
            }
        };

        // Feature report IDs valid for this collection
        let feature_ids: &[u8] = match info.usage_page() {
            0xFF00 => &[0x24],
            0xFF0B => &[0x2A, 0x2B],
            _ => &[],
        };

        for &id in feature_ids {
            let mut buf = [0u8; 61];
            buf[0] = id;
            match dev.get_feature_report(&mut buf) {
                Ok(n) => println!("  feature {id:#04x} ({n} bytes): {:02x?}", &buf[..n]),
                Err(e) => println!("  feature {id:#04x} failed: {e}"),
            }
        }

        for _ in 0..10 {
            let mut inbuf = [0u8; 65];
            match dev.read_timeout(&mut inbuf, 500) {
                Ok(0) => {}
                Ok(n) => println!("  input ({n} bytes): {:02x?}", &inbuf[..n]),
                Err(e) => println!("  read failed: {e}"),
            }
        }
    }
}
