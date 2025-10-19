# HyperX Headset USB HID Protocol Documentation

This document describes the USB HID communication protocol for HyperX wireless headsets, based on analysis of the official NGenuity2 Windows application.

## General Information

All HyperX headsets use USB HID for communication with the dongle. The protocol varies slightly between different headset models, but follows similar patterns.

### Vendor IDs

- **0x0951** - HyperX (Kingston)
- **0x03F0** - HP (for DTS variants)

---

## HyperX Cloud II Wireless (Non-DTS)

**Product IDs:** 0x1718, 0x018B, 0x0b92  
**Dongle Product ID:** 5912 (0x1718)

### Packet Structure

- **Buffer Size:** 62 bytes
- **Report ID:** 0x06 (first byte)

### Base Packet Template

```
Byte    Value   Description
[0]     0x06    HID Report ID
[1]     0x00    Fixed
[2]     0x02    Fixed (for most commands)
[3]     0x00    Fixed
[4]     0x9A    Fixed (154 decimal)
[5]     0x00    Fixed
[6]     0x00    Fixed
[7]     0x68    Fixed (104 decimal)
[8]     0x4A    Fixed (74 decimal)
[9]     0x8E    Fixed (142 decimal)
[10]    0x0A    Fixed (10 decimal)
[11]    0x00    Fixed
[12]    0x00    Fixed
[13]    0x00    Fixed
[14]    0xBB    Fixed (187 decimal)
[15]    CMD     Command ID
[16+]   ...     Parameters (command-specific)
```

### Commands (byte[15])

| Cmd ID | Hex  | Name                   | Description                           |
| ------ | ---- | ---------------------- | ------------------------------------- |
| 1      | 0x01 | Get Connection Status  | Query wireless connection status      |
| 2      | 0x02 | Get Battery Level      | Query current battery percentage      |
| 3      | 0x03 | Get Charging Status    | Query charging state                  |
| 8      | 0x08 | Mute Status (Response) | Microphone mute status (in responses) |
| 9      | 0x09 | Initialization         | Sent during device initialization     |
| 17     | 0x11 | Get Firmware Version   | Query firmware version (4 bytes)      |
| 24     | 0x18 | Set Auto Power Off     | Set automatic shutdown time           |
| 25     | 0x19 | Set Sidetone           | Enable/disable sidetone               |
| 26     | 0x1A | Get Auto Power Off     | Query auto shutdown setting           |
| 29     | 0x1D | Initialization         | Sent during device initialization     |

### Command Examples

#### Get Connection Status (Cmd 1)

```
Send:     [06 00 02 00 9A 00 00 68 4A 8E 0A 00 00 00 BB 01 00 ...]
Response: [0B 00 BB 01 <status> ...]
```

- **Status values:**
  - `0x01` = Connected
  - `0x02` = Pairing mode
  - `0x04` = Connected (alternative)

#### Get Battery Level (Cmd 2)

```
Send:     [06 00 02 00 9A 00 00 68 4A 8E 0A 00 00 00 BB 02 00 ...]
Response: [0B 00 BB 02 00 <data> <data> <battery> ...]
```

- **Battery percentage** is at byte [7] (0-100)

#### Get Charging Status (Cmd 3)

```
Send:     [06 00 02 00 9A 00 00 68 4A 8E 0A 00 00 00 BB 03 00 ...]
Response: [0B 00 BB 03 <status> ...]
```

- **Charging status** values:
  - `0x00` = Not charging
  - `0x01` = Charging (wired)
  - `0x02` = Fully charged
  - `0x03` = Charge error

#### Set Auto Power Off (Cmd 24)

```
Send:     [06 00 02 00 9A 00 00 68 4A 8E 0A 00 00 00 BB 18 <minutes> ...]
Response: [0B 00 BB 1A <minutes> ...]
```

- **byte[16]** = shutdown delay in minutes (0 = disabled, typical: 5, 10, 15, 20, 30)

#### Set Sidetone (Cmd 25)

```
Send:     [06 00 02 00 9A 00 00 68 4A 8E 0A 00 00 00 BB 19 <enable> ...]
Response: [0B 00 BB 19 <status> 01 ...]
```

- **Command byte[16]** = 0x01 to enable sidetone, 0x00 to disable
- **Response byte[4]** = 0x01 for enabled, 0x00 for disabled (NOT inverted)
- **Response byte[5]** = Always 0x01
- **IMPORTANT:** Command and response use SAME logic (1=enabled, 0=disabled)

#### Get Auto Power Off (Cmd 26)

```
Send:     [06 00 02 00 9A 00 00 68 4A 8E 0A 00 00 00 BB 1A 00 ...]
Response: [0B 00 BB 1A <minutes> ...]
```

- **byte[4]** = shutdown delay in minutes

#### Get Firmware Version (Cmd 17)

```
Send:     [06 00 02 00 9A 00 00 68 4A 8E 0A 00 00 00 BB 11 00 ...]
Response: [0B 00 BB 11 <v1> <v2> <v3> <v4> ...]
```

- **Response bytes[4-7]** = firmware version components (e.g., 4.1.0.1)
  - byte[4] = major version
  - byte[5] = minor version
  - byte[6] = build number
  - byte[7] = revision number
- **Note:** This response is typically only logged, not emitted as a DeviceEvent

#### Get Microphone Mute Status (Cmd 1)

```
Send:     [06 00 02 00 9A 00 00 68 4A 8E 0A 00 00 00 BB 01 00 ...]
Response: [0B 00 BB 01 <status> ...]
```

- **byte[4]** = connection/mute status
  - `0x01` = Connected
  - `0x02` = Pairing mode
  - `0x04` = Connected (alternative)
- **Note:** This command ID (1) serves dual purpose - it returns connection status
- **Limitation:** Mute cannot be SET via HID command (hardware button only)

#### Microphone Mute Status (Cmd 8)

```
Response: [0B 00 BB 08 <status> ...]
```

- **byte[4]** = mute status
  - `0x01` = Microphone muted
  - `0x00` = Microphone unmuted
- **Note:** This appears as an unsolicited response when the hardware mute button is toggled on the headset
- **Limitation:** This is a READ-ONLY event; you cannot programmatically mute the microphone

#### Initialization Commands (Cmd 9, 29)

```
Command 9:  [06 00 02 00 9A 00 00 68 4A 8E 0A 00 00 00 BB 09 ...]
Command 29: [06 00 02 00 9A 00 00 68 4A 8E 0A 00 00 00 BB 1D ...]
```

- **Purpose:** These commands are sent during device initialization/connection
- **Behavior:** They appear to prepare the device for subsequent commands
- **Exact function:** Unclear from reverse engineering, but critical for proper device operation

### Special Packet: Get Surround Sound Status

This command uses a different packet structure:

```
Send:     [06 00 00 00 FF 00 00 68 4A 8E 00 00 00 00 00 00 ...]
Response: [0A 00 <dsp_status> 03 ...]
```

- **Report ID:** 0x0A (10 decimal) - different from standard responses
- **Packet structure:** Simplified format (not using BASE_PACKET template)
- **byte[2] & 0x02** = Surround sound enabled if bit 1 is set
- Example: `0x02` = surround OFF, `0x03` = surround ON
- **Limitation:** On Cloud II Wireless (non-DTS), surround sound can only be READ, not SET via HID
  - Surround sound control is handled through Windows DTS Audio Processing Object (APO)
  - The physical headset button or Windows audio settings control this feature
  - The HID protocol only allows monitoring the current state

### Feature Control Capabilities

The Cloud II Wireless (non-DTS) has the following control capabilities:

| Feature              | Read Status | Set/Control | Notes                                   |
| -------------------- | ----------- | ----------- | --------------------------------------- |
| Battery Level        | ✅ Yes      | ❌ No       | Read-only                               |
| Charging Status      | ✅ Yes      | ❌ No       | Read-only                               |
| Connection Status    | ✅ Yes      | ❌ No       | Read-only                               |
| Auto Power Off       | ✅ Yes      | ✅ Yes      | Full control via commands 24/26         |
| Sidetone             | ✅ Yes      | ✅ Yes      | Full control via command 25             |
| Microphone Mute      | ✅ Yes      | ❌ No       | Hardware button only (commands 1/8)     |
| Surround Sound (7.1) | ✅ Yes      | ❌ No       | Controlled via Windows DTS APO, not HID |
| Firmware Version     | ✅ Yes      | ❌ No       | Read-only (command 17)                  |

### Response Packet Format

Most responses use Report ID **0x0B** (11 decimal):

```
Byte    Value       Description
[0]     0x0B        Response Report ID
[1]     0x00        Fixed
[2]     0xBB        Fixed (187 decimal)
[3]     <CMD_ID>    Command ID being responded to
[4+]    <DATA>      Response data
```

Some responses (DSP/surround) use Report ID **0x0A** (10 decimal).

### Initialization Sequence

The Windows application sends a specific initialization sequence when connecting:

1. Get Connection Status (Cmd 1)
2. Get Surround Sound Status (special packet)
3. Get Firmware Version (Cmd 17)
4. Unknown Command (Cmd 29)
5. Unknown Command (Cmd 9)

Before each command, the application calls `GetInputReport(0x06)` to prepare the device.

### Microphone Mute Status

Microphone mute status is reported asynchronously or in response to Get Connection Status:

```
Response: [0B 00 BB 08 <mute_status> ...]
```

- **byte[4]:** 0x01 = muted, 0x00 = unmuted

---

## HyperX Cloud II Wireless DTS

**Vendor ID:** 0x03F0 (HP)  
**Product IDs:** 0x1718, 0x018B, 0x0D93, 0x0696  
**Dongle Product ID:** 395 (0x18B)

### Packet Structure

- **Buffer Size:** 20 bytes
- **Report ID:** 0x06

### Base Packet Template

```
Byte    Value   Description
[0]     0x06    HID Report ID
[1]     0xFF    Fixed (255 decimal)
[2]     0xBB    Fixed (187 decimal)
[3]     CMD     Command ID
[4+]    ...     Parameters
```

### Commands (byte[3])

| Cmd ID | Hex  | Name                | Read/Write | Description                    |
| ------ | ---- | ------------------- | ---------- | ------------------------------ |
| 1      | 0x01 | Get Wireless State  | Read       | Query connection status        |
| 2      | 0x02 | Get Battery Info    | Read       | Query battery level            |
| 3      | 0x03 | Get Charge Status   | Read       | Query charging status          |
| 5      | 0x05 | Get Mic Mute        | Read       | Query microphone mute status   |
| 6      | 0x06 | Get Sidetone Status | Read       | Query sidetone on/off          |
| 7      | 0x07 | Get Auto Shutdown   | Read       | Query auto-off time            |
| 8      | 0x08 | Get Mic Boom Status | Read       | Query if mic boom is connected |
| 9      | 0x09 | Get Pairing Info    | Read       | Query pairing information      |
| 11     | 0x0B | Get Sidetone Volume | Read       | Query sidetone volume level    |
| 32     | 0x20 | Set Mic Mute        | Write      | Set microphone mute            |
| 33     | 0x21 | Set Sidetone Status | Write      | Enable/disable sidetone        |
| 34     | 0x22 | Set Auto Shutdown   | Write      | Set auto-off time              |
| 35     | 0x23 | Set Sidetone Volume | Write      | Set sidetone volume (0-100)    |

### Command Examples

#### Get Battery Level (Cmd 2)

```
Send:     [06 FF BB 02 00 ...]
Response: [06 FF BB 02 00 00 00 <battery> ...]
```

- **byte[7]** = battery percentage (0-100)

#### Get Charging Status (Cmd 3)

```
Send:     [06 FF BB 03 00 ...]
Response: [06 FF BB 03 <status> ...]
```

- **byte[4]** charging status (same values as non-DTS)

#### Get Mic Mute (Cmd 5)

```
Send:     [06 FF BB 05 00 ...]
Response: [06 FF BB 05 <muted> ...]
```

- **byte[4]:** 0x01 = muted, 0x00 = unmuted

#### Get Sidetone Status (Cmd 6)

```
Send:     [06 FF BB 06 00 ...]
Response: [06 FF BB 06 <enabled> ...]
```

- **byte[4]:** 0x01 = enabled, 0x00 = disabled

#### Get Sidetone Volume (Cmd 11)

```
Send:     [06 FF BB 0B 00 ...]
Response: [06 FF BB 0B <volume> ...]
```

- **byte[4]:** volume level 0-100

#### Set Mic Mute (Cmd 32)

```
Send:     [06 FF BB 20 <mute> ...]
Response: [06 FF BB 20 <mute> ...]
```

- **byte[4]:** 0x01 = mute, 0x00 = unmute

#### Set Sidetone Status (Cmd 33)

```
Send:     [06 FF BB 21 <enable> ...]
Response: [06 FF BB 21 <enable> ...]
```

- **byte[4]:** 0x01 = enable, 0x00 = disable

#### Set Sidetone Volume (Cmd 35)

```
Send:     [06 FF BB 23 <volume> ...]
Response: [06 FF BB 23 <volume> ...]
```

- **byte[4]:** volume level 0-100

#### Set Auto Shutdown (Cmd 34)

```
Send:     [06 FF BB 22 <minutes> ...]
Response: [06 FF BB 22 <minutes> ...]
```

- **byte[4]:** shutdown delay in minutes

### Response Format

Responses echo the command structure with the command ID at byte[3] and data starting at byte[4].

### Asynchronous Notifications

The DTS variant sends asynchronous notifications for:

- **Connection status changes** (Cmd 1 responses)
- **Microphone mute changes** (Cmd 32 responses)
- **Sidetone changes** (Cmd 33 responses)

---

## HyperX Cloud Alpha Wireless

**Product IDs:** 5955 (0x1743), 5989 (0x1765), 2445 (0x098D)

### Packet Structure

- **Buffer Size:** 20 bytes
- **Report ID:** 0x21 (33 decimal)

### Base Packet Template

```
Byte    Value   Description
[0]     0x21    HID Report ID (33)
[1]     0xBB    Fixed (187 decimal)
[2]     CMD     Command ID
[3+]    ...     Parameters
```

### Commands (byte[2])

| Cmd ID | Hex  | Name                | Description             |
| ------ | ---- | ------------------- | ----------------------- |
| 1      | 0x01 | Get Wireless State  | Query connection status |
| 11     | 0x0B | Get Battery Info    | Query battery level     |
| 12     | 0x0C | Get Charge Status   | Query charging status   |
| 13     | 0x0D | Get Mic Mute        | Query microphone mute   |
| 14     | 0x0E | Get Sidetone Status | Query sidetone on/off   |
| 15     | 0x0F | Get Sidetone Volume | Query sidetone volume   |
| 32     | 0x20 | Set Mic Mute        | Set microphone mute     |
| 33     | 0x21 | Set Sidetone Status | Enable/disable sidetone |
| 34     | 0x22 | Set Sidetone Volume | Set sidetone volume     |

### Response Format

Responses use the same report ID (0x21) with command ID at byte[2].

---

## HyperX Cloud III Wireless

**Product IDs:** Multiple variants exist

### Packet Structure

- **Buffer Size:** 20 bytes
- **Report ID:** 0x21 (33 decimal)

### Base Packet Template

```
Byte    Value   Description
[0]     0x21    HID Report ID (33)
[1]     CMD     Command ID
[2+]    ...     Parameters
```

### Commands (byte[1])

Similar to Cloud Alpha Wireless but with additional features:

- SIRK (Secure Identity Resolution Key) management
- Silent mode
- Voice prompts
- Product color information

| Cmd ID | Hex  | Name                | Description                  |
| ------ | ---- | ------------------- | ---------------------------- |
| 1      | 0x01 | Get Wireless State  | Query connection status      |
| 2      | 0x02 | Get Battery Info    | Query battery level          |
| 3      | 0x03 | Get Charge Status   | Query charging status        |
| 4      | 0x04 | Set Charge Limit    | Set battery charge limit     |
| 5      | 0x05 | Get Mic Mute        | Query microphone mute        |
| 6      | 0x06 | Get Sidetone Status | Query sidetone on/off        |
| 7      | 0x07 | Get Sidetone Volume | Query sidetone volume        |
| 8      | 0x08 | Get Voice Prompt    | Query voice prompt status    |
| 9      | 0x09 | Get Auto Shutdown   | Query auto-off time          |
| 10     | 0x0A | Get Silent Mode     | Query silent mode status     |
| 32     | 0x20 | Set Mic Mute        | Set microphone mute          |
| 33     | 0x21 | Set Sidetone Status | Enable/disable sidetone      |
| 34     | 0x22 | Set Sidetone Volume | Set sidetone volume          |
| 35     | 0x23 | Set Voice Prompt    | Enable/disable voice prompts |
| 36     | 0x24 | Set Auto Shutdown   | Set auto-off time            |
| 37     | 0x25 | Set Silent Mode     | Enable/disable silent mode   |
| 64     | 0x40 | Get SIRK            | Get SIRK key                 |
| 65     | 0x41 | Reset SIRK          | Reset SIRK to default        |

---

## Common Patterns Across All Models

### Reading Device State

1. Always call `GetInputReport(0x06)` before writing commands (prepares the device)
2. Write command packet using `SetOutputReport()`
3. Wait 20-200ms for response
4. Read response using `GetInputReport()` or wait for async notification

### Battery Levels

- Always reported as percentage (0-100)
- May be at different byte positions depending on model

### Charging Status

- Consistent across models:
  - 0 = Not charging
  - 1 = Charging
  - 2 = Fully charged
  - 3 = Error

### Sidetone

- Most models support on/off toggle
- Some models (DTS, Alpha Wireless, Cloud III) support volume control (0-100)
- **IMPORTANT:** Cloud II Wireless non-DTS sidetone status is NOT inverted:
  - Response byte[4] = 0x01 means enabled
  - Response byte[4] = 0x00 means disabled
  - Command and response use identical logic

### Auto Power Off

- Specified in minutes
- 0 = disabled
- Typical values: 5, 10, 15, 20, 30

---

## DSP/Surround Sound

The DSP mode is model-specific and varies significantly:

### Cloud II Wireless Non-DTS

- **Reading Status:** Uses a special packet structure with Report ID 0x0A
- **Enabling/Disabling:** NOT supported via HID commands
  - Surround sound is controlled through Windows DTS Audio Processing Object (APO)
  - Users must use the physical button on the headset or Windows audio settings
  - The HID protocol only allows reading the current state
- Uses bit flags in the DSP status byte to indicate 7.1 surround sound state

### Cloud II Wireless DTS

Uses Windows DTS APO (Audio Processing Object) system calls instead of direct HID commands.

### Other Models

May use different approaches or not support surround sound at all.

---

## Notes and Gotchas

1. **Input Report Preparation:** Always call `GetInputReport(0x06)` before sending commands on Cloud II Wireless non-DTS models.

2. **Response Timing:** Wait at least 50ms after sending a command before reading the response. The Windows application uses 200ms delays during initialization.

3. **Battery Position:** Battery level byte position varies:

   - Cloud II Wireless: byte[7]
   - Cloud II Wireless DTS: byte[7]
   - Other models: varies

4. **Thread Safety:** The Windows application uses command queues and thread synchronization. Multiple simultaneous commands may cause issues.

5. **Product ID Detection:** Some headsets report different product IDs for dongle vs headset. Always check both.

6. **Initialization Sequence:** Commands 9 and 29 appear during device initialization. While their exact purpose is unclear from reverse engineering, they are part of the device startup sequence.

7. **Firmware Version:** Command 17 returns firmware version information but does not typically generate user-facing events—it's primarily for logging and diagnostics.

8. **Microphone Mute:** Hardware mute button on headset generates unsolicited responses with command ID 8. This allows the application to detect physical button presses. **Cannot be controlled programmatically** - mute is hardware-only.

9. **Surround Sound Control:** Cloud II Wireless (non-DTS) can only READ surround sound status via HID. Enabling/disabling is controlled through Windows DTS APO system, not HID commands. Use the physical headset button or Windows audio settings to toggle surround sound.

10. **Limited Write Commands:** Only Auto Power Off (Cmd 24) and Sidetone (Cmd 25) can be SET via HID. All other features are read-only or hardware-controlled.

---

## Live Testing Verification

The following features have been verified with a physical HyperX Cloud II Wireless headset:

### Successfully Tested

- ✅ **Battery Level Reading** - Correctly reads battery percentage at byte[7] (tested at 92%)
- ✅ **Charging Status** - Accurately detects charging state changes
- ✅ **Sidetone Toggle** - Properly enables/disables sidetone (response[4]: 1=enabled, 0=disabled)
- ✅ **Surround Sound Status** - Correctly reads 7.1 surround state via DSP packet (0x0A)
- ✅ **Auto Power Off** - Successfully reads and sets auto shutdown timer
- ✅ **Firmware Version** - Parses version from bytes[4-7] (tested: 4.1.0.1)
- ✅ **Microphone Mute Detection** - Detects hardware mute button press via command 8 responses
- ✅ **Connection Status** - Properly detects wireless connection state
- ✅ **Initialization Commands** - Commands 9 and 29 handled without errors during device startup

### Response Parsing Notes

All command responses (1, 2, 3, 8, 9, 17, 24, 25, 26, 29) have been verified to parse correctly with no "unknown command" errors when the device is actively refreshing state. The protocol implementation has been tested with continuous polling at 1-second intervals.

---

## Implementation Tips

### Rust/hidapi

```rust
// Prepare device (Cloud II Wireless non-DTS only)
let mut input_report = [0u8; 64];
input_report[0] = 0x06;
device.get_input_report(&mut input_report)?;

// Send command
device.write(&packet)?;

// Wait for response
std::thread::sleep(Duration::from_millis(50));

// Read response
let mut response = [0u8; 256];
let len = device.read_timeout(&mut response, 1000)?;
```

### Response Parsing

Always validate:

1. Response report ID matches expected value
2. Command ID echo matches sent command
3. Packet length is sufficient for expected data

---

## References

- HyperX NGenuity2 Windows Application (decompiled with dnSpy)
- Live packet captures from HyperX Cloud II Wireless dongle
- USB HID 1.11 Specification

---

**Document Version:** 1.0  
**Last Updated:** 2025-10-19  
**Based on:** NGenuity2 application version analyzed
