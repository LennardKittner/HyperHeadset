# HyperHeadset
[![AUR Git Version](https://img.shields.io/aur/version/hyper-headset-git)](https://aur.archlinux.org/packages/hyper-headset-git)
[![AUR Bin Version](https://img.shields.io/aur/version/hyper-headset-bin)](https://aur.archlinux.org/packages/hyper-headset-bin)
[![GitHub Release](https://img.shields.io/github/v/release/LennardKittner/HyperHeadset)](https://github.com/LennardKittner/HyperHeadset/releases)
[![GitHub Downloads](https://img.shields.io/github/downloads/LennardKittner/HyperHeadset/total.svg?label=GitHub%20Downloads)](https://github.com/LennardKittner/HyperHeadset/releases)
[![Sponsor](https://img.shields.io/badge/-Sponsor-green?style=flat&logo=github)](https://github.com/sponsors/LennardKittner)

A CLI and tray application for monitoring and managing HyperX headsets.

| OS | Tooltip | Context Menu |
|:---:|:---:|:---:|
| **Linux** | <img src=./screenshots/tray_linux.png width="280"> | <img src=./screenshots/tray_linux_2.png width="280"> |
| **macOS** | <img src=./screenshots/tray_macOS.png width="280"> | <img src=./screenshots/tray_macOS_2.png width="280"> |
| **Windows** | <img src=./screenshots/tray_windows.png width="280"> | <img src=./screenshots/tray_windows_2.png width="280"> |

This project is not affiliated with, endorsed by, or associated with HyperX or its parent company in any way. All trademarks and brand names belong to their respective owners.

## Compatibility
Both the CLI and tray applications are compatible with Linux, MacOS, and Windows.

**Supported Headsets**:
- HyperX Cloud II Wireless HP vendor ID
- HyperX Cloud II Wireless HyperX vendor ID
- HyperX Cloud II Core Wireless
- HyperX Cloud III Wireless
- HyperX Cloud III S Wireless
- HyperX Cloud Stinger 2 Wireless
- HyperX Cloud Flight S
- HyperX Cloud Alpha Wireless
- HyperX Cloud MIX 2

If your headset is not supported, feel free to open an issue; be sure to include the name, product ID, and vendor ID.

## Installation

### Arch Linux (AUR)
No manual setup required (dependencies and udev rules are handled automatically):
```bash
yay -S hyper-headset-git
```
or 
```bash
yay -S hyper-headset-bin
```

### Prebuilt Binary (Linux/MacOS/Windows)

Download from [GitHub releases](https://github.com/LennardKittner/HyperHeadset/releases).

⚠️**Linux Only**: The required udev rules will be installed automatically when the program is launched if they are missing.
You will be prompted to allow the installation.

If automatic installation fails, you can install them manually (see Prerequisites -> Udev below)

## Build from Source

This project uses git submodules, so before building, you have to initialize them via:
`git submodule update --init --recursive`

To build both applications, use:
`cargo build --release`

See prerequisites below for installing dependencies.
If the required udev rules are missing on Linux, the program will prompt you to install them automatically.

## Prerequisites 

### Dependencies

These dependencies are probably already installed.

Debian/Ubuntu:

`sudo apt install libdbus-1-dev libusb-1.0-0-dev libudev-dev`

Arch:

`sudo pacman -S dbus libusb`

MacOS:

`brew install libusb`

### Udev (Linux only)

Normally the program installs the required udev rules automatically on first launch.

If that fails, create the file: `/etc/udev/rules.d/99-HyperHeadset.rules` with the following content inside:

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
SUBSYSTEMS=="usb", ATTRS{idProduct}=="0995", ATTRS{idVendor}=="03f0", MODE="0666"
SUBSYSTEMS=="usb", ATTRS{idProduct}=="0fae", ATTRS{idVendor}=="03f0", MODE="0666"

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
KERNEL=="hidraw*", ATTRS{idProduct}=="0995", ATTRS{idVendor}=="03f0", MODE="0666"
KERNEL=="hidraw*", ATTRS{idProduct}=="0fae", ATTRS{idVendor}=="03f0", MODE="0666"
```

Once created, replug the wireless dongle.

## Usage

<!-- TODO: update -->
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
A tray application for monitoring HyperX headsets.

Usage: hyper_headset [OPTIONS]

Options:
      --refresh_interval <refresh_interval>
          Set the refresh interval (in seconds) [default: 3]
      --press_mute_key <press_mute_key>
          The app will simulate pressing the microphone mute key whoever the headsets is muted or unmuted. [default: true] [possible values: true, false]
  -h, --help
          Print help
  -V, --version
          Print version
```

`hyper_headset` without any arguments will start the tray application with a 3s refresh interval.
Once it's open, hover over the headset icon in the system tray or right-click to view details such as the battery level.
You can also change device properties or exit via the right-click menu.
By default, the tray app sends a MicMute key press whenever the headset is muted or unmuted.
Since there is no MicMute key on Windows and MacOS f20 is used instead.
This allows applications such as Discord to react when the hardware mute button on the headset is pressed.

To set this up, start the tray app, open Discord, and create a new keybind via **User Settings** -> **Keybinds** -> **Add a Keybind**.
For the action, select *Toggle Mute*, then click *Record Keybind* and press the headset's mute button while recording.

Discord should now automatically mute and unmute when the headset does.
Because the action only toggles Discord's state, you may need to synchronize it once by manually muting or unmuting Discord.

## Contributing / TODOs

- [ ] Update ksni
- [ ] Add Docs
- [ ] Add to crates.io
- [ ] Let CLI periodically output the state 
- [ ] Optional CLI output in JSON
- [ ] Waybar applet
- [x] Menu bar app for MacOS.
- [x] Windows support
- [x] Allow configuration via tray app
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

## Attribution
<a href="https://www.flaticon.com/free-icons/headphones" title="headphones icons">Headphones icons created by sonnycandra - Flaticon</a>
