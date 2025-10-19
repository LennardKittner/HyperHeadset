# HyperX Headset Feature Capabilities

This document summarizes which features can be controlled vs only monitored for each headset model.

## Cloud II Wireless (Non-DTS)

### Writable Features (Can be SET via HID commands)

- ✅ **Auto Power Off** - Can set automatic shutdown timer (0-30 minutes)
- ✅ **Sidetone** - Can enable/disable sidetone (on/off only, no volume control)

### Read-Only Features (Can only monitor, not control)

- ❌ **Microphone Mute** - Hardware button only, cannot be controlled via HID
- ❌ **Surround Sound (7.1)** - Controlled via Windows DTS APO or physical button, not HID
- ❌ **Battery Level** - Read-only status
- ❌ **Charging Status** - Read-only status
- ❌ **Connection Status** - Read-only status
- ❌ **Firmware Version** - Read-only information

## Cloud II Wireless DTS

### Writable Features

- ✅ **Auto Power Off**
- ✅ **Sidetone** - With volume control (0-100)
- ✅ **Surround Sound** - Via Windows DTS APO system calls (not direct HID)

### Read-Only Features

- ❌ **Microphone Mute** - Hardware button only
- ❌ **Battery Level**
- ❌ **Charging Status**
- ❌ **Connection Status**

## Cloud III Wireless

### Writable Features

- ✅ **Auto Power Off**
- ✅ **Sidetone** - With volume control (0-100)
- ✅ **Microphone Mute** - Can be controlled programmatically
- ✅ **Voice Prompt** - Can enable/disable voice prompts
- ✅ **Playback Mute (Silent Mode)** - Can mute headphone output

### Read-Only Features

- ❌ **Surround Sound** - Not supported via HID
- ❌ **Battery Level**
- ❌ **Charging Status**
- ❌ **Connection Status**
- ❌ **Product Color**

## CLI Error Handling

The CLI application now provides clear error messages when attempting to use unsupported features:

```bash
# Example: Trying to control surround sound on Cloud II Wireless
$ ./hyper_headset_cli --surround_sound true
ERROR: Surround sound control is not supported on this device
       Use the physical headset button or Windows audio settings to toggle surround sound.

# Example: Trying to mute on Cloud II Wireless
$ ./hyper_headset_cli --mute true
ERROR: Microphone mute control is not supported on this device (hardware button only)
```

## Tray Application UI

The system tray application now displays "(read-only)" markers next to features that cannot be controlled:

```
Battery level:            92%
Charging status:          Not charging
Muted:                    false (read-only)
Automatic shutdown after: 20min
Side tone:                false
Surround sound:           true (read-only)
Connected:                true
```

## Implementation Details

- Feature capabilities are checked once during device initialization via `init_capabilities()`
- Capability flags are stored in `DeviceState` structure
- CLI flags for unsupported features are hidden in the help menu
- CLI exits with error code 1 when attempting to use unsupported features
- Tray UI shows read-only markers based on device capabilities

## Protocol Notes

### Undocumented Commands

**Command 4 (Cloud II Wireless)**: An undocumented HID command that occasionally appears as an asynchronous notification from the headset. This command is **not handled** by the official HyperX NGenuity2 software, which simply logs it to debug traces.

- **Appearance**: Sporadic, trigger conditions unknown
- **Official behavior**: Ignored by NGenuity2
- **HyperHeadset behavior**: Logged for debugging purposes
- **Investigation findings**:
  - Does NOT trigger on charging cable connect/disconnect
  - Does NOT trigger on battery level changes
  - Not related to any user-controllable feature
  - May be firmware artifact from Cloud Flight S (which uses cmd 4 for button presses)
  - Cloud II Wireless and Cloud II Wireless DTS both ignore this command

This is documented for transparency but can be safely ignored during normal operation.
