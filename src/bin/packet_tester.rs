use std::u8;

use hidapi::{DeviceInfo, HidApi};

const VENDOR_IDS: [u16; 2] = [0x0951, 0x03F0];
// Possible Cloud II Wireless product IDs
const PRODUCT_IDS: [u16; 3] = [0x1718, 0x018B, 0x0b92];

const BASE_PACKET: [u8; 62] = {
    let mut tmp = [0u8; 62];
    tmp[0] = 0x06;
    tmp[1] = 0x00;
    tmp[2] = 0x02;
    tmp[3] = 0x00;
    tmp[4] = 0x9A;
    tmp[5] = 0x00;
    tmp[6] = 0x00;
    tmp[7] = 0x68;
    tmp[8] = 0x4A;
    tmp[9] = 0x8E;
    tmp[10] = 0x0A;
    tmp[11] = 0x00;
    tmp[12] = 0x00;
    tmp[13] = 0x00;
    tmp[14] = 0xBB;
    tmp[15] = 0x01;
    tmp[16] = 0x00;
    tmp[17] = 0x00;
    tmp[18] = 0x00;
    tmp[19] = 0x00;
    tmp[20] = 0x00;
    tmp[21] = 0x00;
    tmp[22] = 0x00;
    tmp[23] = 0x00;
    tmp[24] = 0x00;
    tmp[25] = 0x00;
    tmp[26] = 0x00;
    tmp[27] = 0x00;
    tmp[28] = 0x00;
    tmp[29] = 0x00;
    tmp[30] = 0x00;
    tmp[31] = 0x00;
    tmp[32] = 0x00;
    tmp[33] = 0x00;
    tmp[34] = 0x00;
    tmp[35] = 0x00;
    tmp[36] = 0x00;
    tmp[37] = 0x00;
    tmp[38] = 0x00;
    tmp[39] = 0x00;
    tmp[40] = 0x00;
    tmp[41] = 0x00;
    tmp[42] = 0x00;
    tmp[43] = 0x00;
    tmp[44] = 0x00;
    tmp[45] = 0x00;
    tmp[46] = 0x00;
    tmp[47] = 0x00;
    tmp[48] = 0x00;
    tmp[49] = 0x00;
    tmp[50] = 0x00;
    tmp[51] = 0x00;
    tmp[52] = 0x00;
    tmp[53] = 0x00;
    tmp[54] = 0x00;
    tmp[55] = 0x00;
    tmp[56] = 0x00;
    tmp[57] = 0x00;
    tmp[58] = 0x00;
    tmp[59] = 0x00;
    tmp[60] = 0x00;
    tmp[61] = 0x00;
    tmp
};

const PACKET1: [u8; 62] = {
    let mut tmp = BASE_PACKET;
    tmp[2] = 0;
    tmp[4] = 0xFF;
    tmp[10] = 0x00;
    tmp[14] = 0x00;
    tmp[15] = 0x00;
    tmp
};
const PACKET2: [u8; 62] = {
    let mut tmp = BASE_PACKET;
    tmp[15] = 0x11;
    tmp
};
const PACKET3: [u8; 62] = {
    let mut tmp = BASE_PACKET;
    tmp[15] = 0x1D;
    tmp
};
const PACKET4: [u8; 62] = {
    let mut tmp = BASE_PACKET;
    tmp[15] = 0x09;
    tmp
};
const PACKET5: [u8; 62] = {
    let mut tmp = BASE_PACKET;
    tmp[15] = 0x02;
    tmp
};
const PACKET6: [u8; 62] = {
    let mut tmp = BASE_PACKET;
    tmp[15] = 0x03;
    tmp
};
const PACKET7: [u8; 62] = {
    let mut tmp = BASE_PACKET;
    tmp[15] = 0x1A;
    tmp
};
const PACKET8: [u8; 62] = PACKET5;
const PACKET9: [u8; 62] = PACKET6;
const PACKET10: [u8; 62] = PACKET5;
const PACKET11: [u8; 62] = PACKET6;

const PACKETS: [&[u8]; 12] = [
    &BASE_PACKET,
    &PACKET1,
    &PACKET2,
    &PACKET3,
    &PACKET4,
    &PACKET5,
    &PACKET6,
    &PACKET7,
    &PACKET8,
    &PACKET9,
    &PACKET10,
    &PACKET11,
];

fn main() {
    let hidapi = HidApi::new().unwrap();
    for device in hidapi.device_list() {
        if VENDOR_IDS.contains(&device.vendor_id()) && PRODUCT_IDS.contains(&device.product_id()) {
            test_device(device);
        }
    }
}

fn test_device(device_info: &DeviceInfo) {
    println!(
        "Testing device: {}:{}:{}",
        device_info.vendor_id(),
        device_info.product_id(),
        device_info.interface_number()
    );
    let hidapi = HidApi::new().unwrap();
    let device = device_info.open_device(&hidapi).unwrap();

    for packet in PACKETS {
        let mut response_buffer = [0u8; 20];
        let mut input_report_buffer = [0u8; 64];
        input_report_buffer[0] = 6;
        println!("  packet: {:?}", packet);
        device.get_input_report(&mut input_report_buffer).unwrap();
        let _ = device.write(packet).map_err(|err| println!("{err}"));
        match device.read_timeout(&mut response_buffer, 1000) {
            Err(err) => println!("{err}"),
            Ok(len) => {
                println!("  response: {:?}\n", &response_buffer[..len]);
            }
        }
    }
}
