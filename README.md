# HyperHeadset
[![AUR Version](https://img.shields.io/aur/version/hyperheadset-git)](https://aur.archlinux.org/packages/hyperheadset-git)
[![GitHub Release](https://img.shields.io/github/v/release/LennardKittner/HyperHeadset)](https://github.com/LennardKittner/HyperHeadset/releases)
[![GitHub Downloads](https://img.shields.io/github/downloads/LennardKittner/HyperHeadset/total.svg?label=GitHub%20Downloads)](https://github.com/LennardKittner/HyperHeadset/releases)
[![Sponsor](https://img.shields.io/badge/-Sponsor-green?style=flat&logo=github)](https://github.com/sponsors/LennardKittner)

A CLI and tray application for monitoring and managing HyperX headsets.

<img src=./screenshots/tray_app.png alt="tray_app" width="400">

This project is not affiliated with, endorsed by, or associated with HyperX or its parent company in any way. All trademarks and brand names belong to their respective owners.

## Compatibility
The CLI application is compatible with both Linux and MacOS operating systems.
However, the tray application is only functional on Linux.

**Supported Headsets**:
- HyperX Cloud II Wireless HP vendorID
- HyperX Cloud II Wireless HyperX vendorID
- HyperX Cloud II Core Wireless
- HyperX Cloud III Wireless
- HyperX Cloud III S Wireless
- HyperX Cloud Stinger 2 Wireless
- HyperX Cloud Flight S
- HyperX Cloud Alpha Wireless

If your headset is not supported, feel free to open an issue.

## Installation

### Arch Linux (AUR)
No manual setup required (dependencies and udev rules are handled automatically):
```bash
yay -S hyperheadset-git
```

### Prebuilt Binary (Linux/MacOS)

Download from [GitHub releases](https://github.com/LennardKittner/HyperHeadset/releases).

⚠️ Linux users must manually install the udev rule (see Prerequisites below).

## Build from Source

This project uses git submodules, so before building you have to initialize them via:
`git submodule update --init --recursive`

To only build the CLI app on MacOS, use:
`cargo build --release --bin hyper_headset_cli`

To build both applications on Linux, use:
`cargo build --release`

See prerequisites below for installing dependencies and adding the udev rule.

## Prerequisites (Binary Releases / Building from Source Only)

### Hidraw

Make sure you have hidraw installed on your system.

Debian/Ubuntu:

`sudo apt install libhidapi-hidraw0`

Arch:

`sudo pacman -S hidapi`

MacOS:

`brew install hidapi`

### Other Dependencies

These dependencies are probably already installed.

Debian/Ubuntu:

`sudo apt install libdbus-1-dev libusb-1.0-0-dev libudev-dev`

Arch:

`sudo pacman -S dbus libusb`

MacOS:

`brew install libusb`

### Udev (Linux only)

Create a new file, e.g., `/etc/udev/rules.d/99-HyperHeadset.rules` with the following content inside:

```
SUBSYSTEMS=="usb", ATTRS{idProduct}=="018b", ATTRS{idVendor}=="03f0", MODE="0666"
SUBSYSTEMS=="usb", ATTRS{idProduct}=="0696", ATTRS{idVendor}=="03f0", MODE="0666"
SUBSYSTEMS=="usb", ATTRS{idProduct}=="1718", ATTRS{idVendor}=="0951", MODE="0666"
SUBSYSTEMS=="usb", ATTRS{idProduct}=="0d93", ATTRS{idVendor}=="03f0", MODE="0666"
SUBSYSTEMS=="usb", ATTRS{idProduct}=="05b7", ATTRS{idVendor}=="03f0", MODE="0666"
SUBSYSTEMS=="usb", ATTRS{idProduct}=="06be", ATTRS{idVendor}=="03f0", MODE="0666"
SUBSYSTEMS=="usb", ATTRS{idProduct}=="16ea", ATTRS{idVendor}=="0951", MODE="0666"
SUBSYSTEMS=="usb", ATTRS{idProduct}=="16eb", ATTRS{idVendor}=="0951", MODE="0666"
SUBSYSTEMS=="usb", ATTRS{idProduct}=="0c9d", ATTRS{idVendor}=="03f0", MODE="0666"
SUBSYSTEMS=="usb", ATTRS{idProduct}=="098d", ATTRS{idVendor}=="03f0", MODE="0666"
SUBSYSTEMS=="usb", ATTRS{idProduct}=="1765", ATTRS{idVendor}=="03f0", MODE="0666"
SUBSYSTEMS=="usb", ATTRS{idProduct}=="1743", ATTRS{idVendor}=="03f0", MODE="0666"
SUBSYSTEMS=="usb", ATTRS{idProduct}=="069f", ATTRS{idVendor}=="03f0", MODE="0666"

KERNEL=="hidraw*", ATTRS{idProduct}=="0d93", ATTRS{idVendor}=="03f0", MODE="0666"
KERNEL=="hidraw*", ATTRS{idProduct}=="018b", ATTRS{idVendor}=="03f0", MODE="0666"
KERNEL=="hidraw*", ATTRS{idProduct}=="0696", ATTRS{idVendor}=="03f0", MODE="0666"
KERNEL=="hidraw*", ATTRS{idProduct}=="1718", ATTRS{idVendor}=="0951", MODE="0666"
KERNEL=="hidraw*", ATTRS{idProduct}=="05b7", ATTRS{idVendor}=="03f0", MODE="0666"
KERNEL=="hidraw*", ATTRS{idProduct}=="06be", ATTRS{idVendor}=="03f0", MODE="0666"
KERNEL=="hidraw*", ATTRS{idProduct}=="16ea", ATTRS{idVendor}=="0951", MODE="0666"
KERNEL=="hidraw*", ATTRS{idProduct}=="16eb", ATTRS{idVendor}=="0951", MODE="0666"
KERNEL=="hidraw*", ATTRS{idProduct}=="0c9d", ATTRS{idVendor}=="03f0", MODE="0666"
KERNEL=="hidraw*", ATTRS{idProduct}=="098d", ATTRS{idVendor}=="03f0", MODE="0666"
KERNEL=="hidraw*", ATTRS{idProduct}=="1765", ATTRS{idVendor}=="03f0", MODE="0666"
KERNEL=="hidraw*", ATTRS{idProduct}=="1743", ATTRS{idVendor}=="03f0", MODE="0666"
KERNEL=="hidraw*", ATTRS{idProduct}=="069f", ATTRS{idVendor}=="03f0", MODE="0666"
```

Once created, replug the wireless dongle.

## Usage

```
hyper_headset_cli --help
A CLI application for monitoring and managing HyperX headsets.

Usage: hyper_headset_cli [OPTIONS]

Options:
      --automatic_shutdown <automatic_shutdown>
          Set the delay in minutes after which the headset will automatically shutdown.
          0 will disable automatic shutdown.
      --mute <mute>
          Mute or unmute the headset. [possible values: true, false]
      --enable_side_tone <enable_side_tone>
          Enable or disable side tone. [possible values: true, false]
      --side_tone_volume <side_tone_volume>
          Set the side tone volume.
      --enable_voice_prompt <enable_voice_prompt>
          Enable voice prompt. This may not be supported on your device. [possible values: true, false]
      --surround_sound <surround_sound>
          Enables surround sound. This may be on by default and cannot be changed on your device. [possible values: true, false]
      --mute_playback <mute_playback>
          Mute or unmute playback. [possible values: true, false]
      --activate_noise_gate <activate_noise_gate>
          Activates noise gate. [possible values: true, false]
  -h, --help
          Print help
  -V, --version
          Print version

Help only lists commands supported by this headset.
```
`hyper_headset_cli` without any arguments will print all available headset information.

```
hyper_headset --help
A CLI tray application for monitoring HyperX headsets.

Usage: hyper_headset [OPTIONS]

Options:
      --refresh_interval <refresh_interval>  Set the refresh interval (in seconds)
  -h, --help                                 Print help
  -V, --version                              Print version
```

`hyper_headset` without any arguments will start the tray application with a 3s refresh interval.
Once it's open, hover over the headset icon in the system tray or right-click to view details such as the battery level.
You can also exit via the right-clock menu.

## Contributing / TODOs

- [ ] Menu bar app for MacOS.
- [ ] Windows support
- [ ] Update ksni
- [x] Actively configure the headset.
- [x] Query device state instead of only relying on events.

You can contribute code or monitor packets using Wireshark or dnSpy from the HyperX app on Windows.

Reverse engineering proprietary software may be restricted by its license agreement.
Ensure you comply with relevant laws and regulations.

### How to use Wireshark to capture packets

This [guide](https://github.com/liquidctl/liquidctl/blob/main/docs/developer/capturing-usb-traffic.md) is very helpful.
In my case, the filter `usb.idVendor == 0x03f0 && usb.idProduct == 0x018b` only showed on request.
I then only listened to the port on which this request was sent, e.g., `(usb.src == "3.5.0") || (usb.dst =="3.5.0")`.
If you have an older headset, you may have to use a different vendor and product ID `usb.idVendor == 0x0951 && usb.idProduct == 0x1718`.
Once you have set the filters, you can perform various actions and review the packets transmitted to and from the headset.

## Other Projects

This project was inspired by [hyperx-cloud-flight](https://github.com/kondinskis/hyperx-cloud-flight).
