# Unified VISCA Protocol Implementation Guide

*(PTZOptics Gen‑2/NDI, Sony ILME‑FR7, Sony BRC‑series, Sony EVI‑H100 S/V, Nearus BRC‑300)*

**Author:** Grafton Machine Shed
**Contact:** [admin@grafton.ai](mailto:admin@grafton.ai)
**Version:** 2025‑07‑10  *(adds Inquiry commands, AddressSet/I‑F Clear, and full error‑code coverage)*

**Notice:** *This document is a work in progress. It is not yet complete and may contain inaccuracies or omissions. Please use with caution and verify against official documentation where available.*

---

## 1. Scope & Intent

This document captures VISCA protocol and implementation details for several popular PTZ cameras from multiple manufacturers. It is intended as a unified reference for developers integrating these systems, providing both the standard VISCA commands and the important differences and nuances across various camera models. By consolidating information for Sony, PTZOptics, and other VISCA variants, this guide offers practical guidance for implementing control software that can accommodate multiple VISCA-capable cameras.

---

## 2. Terminology

We use standard VISCA terminology throughout this guide. The **controller** refers to the device or software sending commands (always address 0), while the **peripheral** is the camera receiving them (address 1–7 in serial mode, or address 1 in IP mode). Each camera maintains two command *sockets* (buffers) to process up to two commands in parallel. The camera responds to commands with specific reply messages: an **ACK** to acknowledge receipt, a **Completion** when the action is done, or an **Error** if something goes wrong. The table below defines these terms:

| Term                    | Meaning                                                                                                 |
| ----------------------- | ------------------------------------------------------------------------------------------------------- |
| **Controller**          | VISCA command originator (serial address `0`; IP `Src = 0x00`).                                         |
| **Peripheral / Camera** | VISCA responder (serial IDs `1‑7`; IP **always `1`**).                                                  |
| **Socket / Buffer**     | One of two concurrent command slots in every Sony-derived PTZ.                                          |
| **ACK**                 | Reply `90 4y FF` – command accepted into socket `y` (1 or 2).                                           |
| **Completion**          | Reply `90 5y FF` – command finished; socket `y` freed.                                                  |
| **Error**               | Reply `90 6X YY FF` – error with code (`02` syntax error, `03` buffer full, `41` not executable, etc.). |

---

## 3. Physical & Transport Layers

### 3.1 Serial RS‑232/RS‑422

VISCA was originally designed for direct serial control, with the ability to daisy-chain multiple cameras. Each camera on a serial chain is assigned a unique ID (1–7) and the controller uses ID 0. Commands are sent over RS-232 (typically for a single chain) or RS-422 (for longer runs or multi-drop setups). Many PTZ cameras provide an 8-pin mini-DIN (Sony) or DE-9 (PTZOptics) connector for serial control. The default communication settings are 9,600 or 38,400 bps, 8-N-1 framing. VISCA commands are framed by an address byte (`8x`, where *x* is the camera ID or 0 for broadcast) and a terminator `FF`. The payload can be up to 14 bytes, making the total command packet at most 16 bytes long. Serial VISCA is reliable and inherently ordered due to the point-to-point connection.

| Connector                            | Baud                      | Frame Format                     | Addressing                    |
| ------------------------------------ | ------------------------- | -------------------------------- | ----------------------------- |
| MiniDIN‑8 (Sony) or DE‑9 (PTZOptics) | 9,600 / 38,400 bps, 8‑N‑1 | `8x ... FF` (1–14 payload bytes) | Controller `0`, Cameras `1‑7` |

### 3.2 VISCA‑over‑IP

VISCA commands can also be sent over an IP network (Ethernet) using UDP or TCP. However, unlike the standardized serial interface, manufacturers have implemented two incompatible approaches for VISCA‑over‑IP.

#### 3.2.1 Two Incompatible Flavors

There are two distinct flavors of VISCA‑over‑IP in use: **Encapsulated UDP** with an 8‑byte header (used by Sony and certain other brands), and **Raw VISCA over IP** with no header (used by PTZOptics and similar). These methods are not interoperable. The table below compares their characteristics:

| Camera family                                                       | Envelope required?         | Sequence number?          | Default port(s)     |
| ------------------------------------------------------------------- | -------------------------- | ------------------------- | ------------------- |
| **Sony** BRC/SRG/FR series (and similar clones)                     | **Yes** – 8‑byte header    | 32‑bit counter (wraps)    | UDP 52381           |
| **PTZOptics** (all firmware revs to 2025‑06)                        | **No** – *raw VISCA bytes* | *Not used*                | UDP 1259, TCP 5678  |
| Hybrids claiming “encapsulated” mode (Marshall, AVer, Avonic, etc.) | Yes (user-selectable)      | Yes (follows Sony format) | UDP 52381 (usually) |

**Important:** PTZOptics documentation (Rev 1.2, Aug 2020) makes no mention of any header or sequence number, and indeed network traces confirm that PTZOptics cameras consume the standard VISCA byte stream directly over IP (no extra header). In practice, sending a Sony‑style 8‑byte header to a PTZOptics camera will result in the packet being ignored entirely (no response). Conversely, a Sony or Marshall/AVer camera expects the header; if you send raw bytes without it, those cameras won’t recognize the command. **Implementation Tip:** Provide a configuration switch or auto-detection in your software to choose between “Sony‑style encapsulation” and “Raw VISCA” modes. This ensures that you only add the header for devices that require it (for example, an Avonic/Marshall camera can be put into Sony-compatible mode, whereas a PTZOptics camera should use raw mode).

#### 3.2.2 Sony Encapsulated Header (UDP 52381)

Sony’s VISCA‑over‑IP protocol wraps the command bytes in an additional header and is typically carried over UDP. In this mode, each command, inquiry, and reply is prefixed by an 8‑byte header that provides message type, length, and a sequence number for tracking. The diagram below illustrates the structure of a UDP packet carrying a VISCA command:

```
┌─────────────── 8 bytes ───────────────┐┌──── VISCA frame (≤16 B) ────┐  
│ Payload‑Type │ Length │ Sequence‑No. ││ 8x … <payload> … FF │  
└──────────────────────────────────────┘└──────────────────────────────┘
```

* **Payload Type:** Identifies the message category. For example, `0x01 0x00` for a Command, `0x01 0x10` for an Inquiry, `0x01 0x11` for a Reply (ACK/Completion/Error). (Other types like device-setting commands use different codes as defined by Sony.)
* **Length:** A 16-bit big-endian field giving the number of bytes in the VISCA command frame (from the `8x` address byte up to the terminating `FF`). For example, a 7-byte VISCA command would have a length field `0x0007`.
* **Sequence No.:** A 32-bit sequence counter. The controller should increment this for each message (start from 0 or 1). The camera will echo the same sequence number in its reply. (Older models like the Sony BRC-H900 effectively used only 16 bits of this field, but modern cameras like the FR7 utilize all 32 bits.)

To implement the Sony encapsulated protocol when sending a command:

1. Set the **Payload Type** field – use `0x01 0x00` for a Command, or `0x01 0x10` for an Inquiry (as per Sony’s definitions).
2. Calculate the **Payload Length** as the number of bytes in the VISCA command frame (excluding the header). For example, a command `81 01 04 3F 02 01 FF` (preset recall 1) is 7 bytes long, so the length field would be `0x0007`.
3. Assign a **Sequence Number** for this message. Increment your sequence counter each time. Use a 32‑bit value for modern cameras (older models will ignore the upper bits).
4. Construct the UDP packet: an 8‑byte header (with the fields above in network byte order) followed by the VISCA command bytes. Send this to UDP port 52381 of the camera.

The camera will respond with its own packet containing the same sequence number. For an **ACK** or **Completion** message from the camera, the payload type will be `0x01 0x11` (Reply) and the Sequence No. will match, so you can correlate responses to the command. In IP mode, the VISCA device address is always 1 (so your command frames should use `0x81` as the address byte), and the source (controller) address is always 0 in replies.

#### 3.2.3 Retransmission & Duplicates (Sony IP Mode)

In a UDP network, packets can be lost or delayed. The VISCA encapsulated protocol relies on the controller to handle delivery confirmation. In practice, if the controller does not receive an **ACK** from the camera within about 2 video frames (\~33 ms), it should consider the command lost and retransmit the packet. **Important:** When retransmitting, always use a **new Sequence number** for the re-send. The camera uses sequence IDs to detect duplicate messages. If a command with the same sequence is received twice, the camera may respond with an error indicating an *abnormal sequence* (to avoid executing the same command twice). Using a new sequence on retry tells the camera it’s a fresh command.

Sony’s documentation provides guidance on how the camera behaves with duplicate sequence numbers. Generally, if the first attempt was actually received and executed but the ACK/Completion got lost, sending the same command with a new sequence may cause the camera to respond with an error or a second completion. If the first attempt was never received, the new sequence command will be processed normally. As a controller developer, you should be prepared to handle either scenario. One strategy is to set a retry limit (for example, retry a command up to 3 times with new sequence IDs). If no ACK or completion is received after several attempts, the camera might be offline or unreachable, and your software can alert the user or attempt a reconnection.

This retransmission logic mainly applies to the Sony-style UDP protocol. In **raw VISCA-over-IP** (e.g. PTZOptics), there is no sequence field to assist with duplicate detection. You may still implement a timeout and retry if an ACK isn’t received, but note that the camera cannot distinguish a retry from a new command. In practice, many integrators using raw UDP rely on the inherent reliability of a local network or choose to use PTZOptics’ TCP control port to avoid packet loss issues. Using TCP (port 5678 for PTZOptics) ensures commands arrive in order without loss, though you should still adhere to the two-command concurrency rule at the application level (see §4).

### 3.3 Serial Address Initialization – **Address Set** & **I/F Clear**

When using serial VISCA with daisy-chained cameras, the network must be initialized so each camera knows its ID. Two special broadcast commands, **Address Set** and **I/F Clear**, are used for this:

| Command         | Bytes (broadcast format) | Purpose                                                                                                                                                                | When to send                                                |
| --------------- | ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| **Address Set** | `88 30 01 FF`            | Daisy-chain address assignment. Each camera takes an ID 1…N in physical chain order. The last camera returns `88 30 0N FF` to tell the controller how many were found. | Once at start-up, or after a “Network Change” notification. |
| **I/F Clear**   | `88 01 00 01 FF`         | Clears command buffers in every device, cancels any motions, and resets sockets (interface clear).                                                                     | Immediately after Address Set, or to recover from errors.   |

> **Tip (serial only):** Always send **Address Set** then **I/F Clear** after power‑up or hot‑plugging cameras so that every camera knows its ID and the command state is clean. The Address Set command should be sent as a broadcast (ID 8) and will automatically assign IDs to all connected cameras; the final response indicates the count of cameras. Then I/F Clear (also broadcast ID 8) will reset all cameras’ command buffers. These commands are **not used** in VISCA‑over‑IP (IP devices always use ID 1 and there is no concept of dynamic addressing over IP).

---

## 4. Transaction & Timing Model

### 4.1 Command vs Inquiry Lifecycle

VISCA differentiates between **commands** (which change state or perform actions) and **inquiries** (which request information). They follow slightly different sequences:

* **Command** (e.g. Pan, Zoom, Preset Recall): The controller sends a packet starting with `8x 01 ... FF` (where x is the camera ID or 1 for IP). The camera will immediately reply with an **ACK** (`9x 4y FF`) indicating it accepted the command into socket *y*. Once the action is completed, the camera sends a **Completion** (`9x 5y FF`) for that socket. If an error occurs, an **Error** reply (`9x 6y .. FF`) is returned instead of a normal completion.
* **Inquiry** (e.g. Zoom Position, Power Status): The controller sends `8x 09 ... FF` (note the `09` byte instead of `01`). The camera **does not** issue an ACK for inquiries. Instead, it directly returns a **Data Reply** (`9x 50 ... FF`) containing the requested information. The reply’s socket number is effectively 0 for inquiries (since they don’t use the command sockets). If the inquiry cannot be processed, an Error (`9x 60/6y .. FF`) is returned (with y=0 for inquiry error).

The table below summarizes this lifecycle:

| Phase                     | Command (`→`) sequence                            | Inquiry (`?`) sequence                     |
| ------------------------- | ------------------------------------------------- | ------------------------------------------ |
| Controller → Camera       | `8x 01 … FF` (command packet sent)                | `8x 09 … FF` (inquiry packet sent)         |
| Camera immediate response | **ACK** `9x 4y FF` (accepted to socket y)         | *(no ACK for inquiries)*                   |
| Camera final response     | **Completion** `9x 5y FF` (action done)           | **Data Reply** `9x 50 <data…> FF` (answer) |
| Errors (if any)           | `9x 6y EE FF` (error code in place of completion) | Same format (with y=0 for inquiry errors)  |

### 4.2 Return Messages & Error Codes

All VISCA cameras enforce a **two-socket concurrency** limit. In other words, the camera can process at most two commands at the same time (one in each socket). If a third command is sent while two are still in progress, the camera will respond with a “buffer full” error (`90 60 03 FF`), and it will not execute that third command. Your implementation should prevent this by queuing or delaying additional commands until one of the previous commands completes.

An **ACK** (`90 4y FF`) indicates the camera accepted a command into socket *y*. Once processing is finished, a **Completion** (`90 5y FF`) is sent and socket *y* becomes free. If a command cannot be executed due to the camera’s current state, the camera will send an **Error** reply instead of a normal completion. For instance, sending a manual focus command while the camera is in auto-focus mode will yield an error `90 6y 41 FF` (“Not Executable”) after the ACK. The command is effectively ignored in that case. Controllers may choose to handle this by informing the user or by automatically switching the camera mode (e.g., turn off auto-focus before sending manual focus commands).

The table below lists common VISCA replies and error codes:

| Reply/Error                    | Hex Pattern    | Meaning and Usage                                                                                  |
| ------------------------------ | -------------- | -------------------------------------------------------------------------------------------------- |
| **ACK**                        | `90 4y FF`     | Command accepted into socket *y*.                                                                  |
| **Completion**                 | `90 5y FF`     | Command finished; socket *y* now free.                                                             |
| **Data Reply**                 | `90 50 ... FF` | Inquiry result (contains requested data; no prior ACK).                                            |
| **Syntax Error**               | `90 60 02 FF`  | Command was unrecognized or had illegal parameters.                                                |
| **Buffer Full**                | `90 60 03 FF`  | Both command sockets are busy; command rejected.                                                   |
| **Command Canceled**           | `90 6y 04 FF`  | A running command in socket *y* was canceled (no completion will follow).                          |
| **No Socket (Cancel Invalid)** | `90 6y 05 FF`  | A “Cancel” was issued for an empty socket (no command to cancel).                                  |
| **Not Executable**             | `90 6y 41 FF`  | Command cannot execute under current conditions (e.g. focus command in AF mode, or camera “busy”). |

Some models exhibit additional timing nuances. For example, the **Sony EVI‑H100** and **BRC‑H900** cameras require about 240 ms after completing a preset recall before they are ready to accept new commands. If you send a command immediately after a preset movement on these models, you might get a `... 41 FF` “Not Executable” error because the camera is still busy internally. The proper approach is to either delay subsequent commands briefly or catch the error and retry after the short delay. (Sony notes that a *Command not executable* error may occur for up to 240 ms after a preset recall on these models, and that the controller should resend the command in that case.)

VISCA also supports a **Command Cancel** function (see Appendix, e.g. `81 21 FF` to cancel socket 1). This can be used to abort a long-running action (like a continuous pan). When a cancel is issued, the camera will respond with a `90 6y 04 FF` “Command Canceled” message for the affected socket (and no completion message will follow). If no command was in that socket, a `90 6y 05 FF` “No Socket” error is returned. Canceling commands is optional in most applications, but it can be useful for emergency stops or user interrupts. (Note: Sony recommends waiting at least 200 ms after sending a pan/tilt command before issuing a cancel for it, to ensure the command has started, and similarly waiting \~200 ms after a cancel before sending a new pan/tilt move. This prevents race conditions where a cancel might be missed or a new move command ignored.)

---

## 5. Inquiry Command Set (Universally Recognized)

All inquiries replace the byte `01` (command) with `09` (inquiry). Replies use the `50` code and contain the requested data. The table below lists common inquiry commands supported by virtually all VISCA-capable PTZ cameras, along with the format of their replies and the meaning of the returned data. (If a camera doesn’t support a particular inquiry, it typically responds with a `... 41 FF` *Not Executable* error when that inquiry is sent.)

| Category              | Inquiry Name   | Bytes Sent (Command)   | Typical Reply Format               | Data Meaning                                                              | Supported on: Sony / PTZOptics / FR7               |
| --------------------- | -------------- | ---------------------- | ---------------------------------- | ------------------------------------------------------------------------- | -------------------------------------------------- |
| **Power State**       | `PowerInq`     | `8x 09 04 00 FF`       | `9x 50 02 FF` or `9x 50 03 FF`     | 02 = On, 03 = Standby                                                     | ✔ / ✔ / ✔ (all support)                            |
| **Zoom Position**     | `ZoomPosInq`   | `8x 09 04 47 FF`       | `9x 50 0p 0q 0r 0s FF`             | 16-bit zoom position (0x0000 = wide end)                                  | ✔ / ✔ / ✔                                          |
| **Focus Mode**        | `FocusModeInq` | `8x 09 04 38 FF`       | `9x 50 02 FF` or `9x 50 03 FF`     | 02 = Auto, 03 = Manual focus mode                                         | ✔ / ✔ / ✔                                          |
| **Focus Position**    | `FocusPosInq`  | `8x 09 04 48 FF`       | `9x 50 0p 0q 0r 0s FF`             | 16-bit focus lens position                                                | ✔ / ✔ / ✔                                          |
| **Pan/Tilt Position** | `PTPosInq`     | `8x 09 06 12 FF`       | `9x 50 PP PP PP PP TT TT TT TT FF` | 16-bit pan & tilt coordinates (see §8.2)                                  | ✔ / ✔ / ✔                                          |
| **Exposure Mode**     | `AEModeInq`    | `8x 09 04 39 FF`       | `9x 50 00/03/0A/0B/0D FF`          | 00 = Full Auto, 03 = Manual, 0A = Shutter Pri, 0B = Iris Pri, 0D = Bright | ✔ / ✔ / ✔ (FR7: no Bright mode)                    |
| **Shutter Speed**     | `ShutterInq`   | `8x 09 04 4A FF`       | `9x 50 0p 0q 0r 0s FF`             | 16-bit shutter index (see §8.3)                                           | ✔ / ✔ / ✔                                          |
| **Iris Level**        | `IrisInq`      | `8x 09 04 4B FF`       | `9x 50 0p 0q 0r 0s FF`             | 16-bit iris (aperture) level                                              | ✔ / ✔ / ✔                                          |
| **Gain**              | `GainInq`      | `8x 09 04 4C FF`       | `9x 50 0p 0q 0r 0s FF`             | 16-bit gain value (0 = 0dB, higher = +dB)                                 | ✔ / ✔ / ✔                                          |
| **WB Mode**           | `WBModeInq`    | `8x 09 04 35 FF`       | `9x 50 00/01/02/04/05/20 FF`       | 00=Auto, 01=Indoor, 02=Outdoor, 04=ATW, 05=Manual, 20=One-push Color Temp | ✔ / ✔ / ✔ (ATW only on FR7)                        |
| **Color Temp**        | `ColorTempInq` | `8x 09 04 20 FF`       | `9x 50 0p 0q FF`                   | White balance color temp (merits, if supported)                           | ✔ / ✔ / ✖ (FR7 uses memory WB)                     |
| **Tally State**       | `TallyInq`     | `8x 09 7E 01 0A 00 FF` | `9x 50 02 FF` or `9x 50 03 FF`     | 02 = Tally On (red), 03 = Tally Off                                       | ✔ / ✖ / ✔\* (FR7 uses separate inquiries per lamp) |
| **Device Info**       | `VersionInq`   | `8x 09 00 02 FF`       | `9x 50 VV VV MM MM FF FF KK FF`    | Returns firmware version info and socket count                            | ✔ / ✔ / ✔                                          |

\*FR7 has separate Red/Green tally inquiries (`...7E 01 0A 00` for red, `...7E 04 1A 00` for green) since it has two tally lamps.

Almost every VISCA camera supports these baseline inquiries; if a feature doesn’t exist on a model, the camera usually returns a `... 41 FF` error to indicate “Not Executable” (for example, a camera with no tally lamp will error on TallyInq). The **VersionInq** is a useful command that returns the camera’s vendor ID, model ID, firmware version, and the number of command sockets supported (almost always 2 for modern cameras).

---

## 6. Baseline Command Set (Universally Recognized)

The following core commands are recognized by all VISCA-compatible PTZ cameras in this guide. They cover essential functions like power control, zoom, focus, pan‑tilt movement, and preset memory operations. Each command is given by its VISCA byte sequence and a brief description:

* **Power:** `81 01 04 00 02 FF` (Power On), `81 01 04 00 03 FF` (Standby/Off).
* **Zoom:** `81 01 04 07 00 FF` (Stop zoom), `... 07 02 FF` (Zoom Tele – standard speed), `... 07 03 FF` (Zoom Wide – standard), `... 07 2p FF` (Variable Tele, speed p=0–7), `... 07 3p FF` (Variable Wide, p=0–7), `... 47 0p 0q 0r 0s FF` (Direct set to 16-bit position).
* **Focus:** `81 01 04 38 02 FF` (Auto Focus On), `... 38 03 FF` (Manual Focus). `81 01 04 08 00 FF` (Stop focus), `... 08 02 FF` (Focus Far – momentary), `... 08 03 FF` (Focus Near), plus variable speed and direct position `... 48 0p 0q 0r 0s FF`.
* **Pan-Tilt:** `81 01 06 01 VV WW XX YY FF` – Continuous Pan-Tilt with speeds (VV for pan, WW for tilt) and direction codes (XX,YY) where e.g. `0x03 0x01` = UpLeft, `0x03 0x03` = Stop. Also: `... 06 02 ... FF` (Absolute move to coordinates), `... 06 03 ... FF` (Relative move), `... 06 04 FF` (Home), `... 06 05 FF` (Reset).
* **Memory (Preset):** `81 01 04 3F 00 pp FF` (Reset/Delete preset *pp*), `... 3F 01 pp FF` (Set/Store preset *pp*), `... 3F 02 pp FF` (Recall preset *pp*).

These baseline commands are consistent across Sony and Sony-derived VISCA cameras, as well as PTZOptics (which largely adopted the same command set in their firmware). Refer to Appendix A for a comprehensive list of opcodes.

---

## 7. Cross‑Model Capability Matrix

Different camera models vary in capabilities even though they share the VISCA protocol. The matrix below highlights key feature differences across these models. Understanding these differences is important when designing a controller for multiple cameras, as certain features (or ranges) may not be available on all devices. A checkmark (✔) indicates support, and a cross (✖) indicates lack of support. (Bold text is used to highlight an especially notable value or limit.) Note that the Sony BRC‑H900 requires an **optional IP interface card** for network control (otherwise it can only be controlled via serial).

| Feature             | PTZOptics Gen‑2             | Sony ILME‑FR7                 | Sony BRC‑H900                    | Sony EVI‑H100             | Nearus BRC‑300  |
| ------------------- | --------------------------- | ----------------------------- | -------------------------------- | ------------------------- | --------------- |
| **VISCA over IP**   | **Raw UDP/TCP** (no header) | Encaps. UDP (52381)           | Encaps. UDP\* (requires IP card) | — (serial only)           | — (serial only) |
| Preset slots        | **128** (fw ≥2.2)           | 100                           | 16                               | 6                         | 6               |
| Pan speed steps     | 1–24                        | **1–50** (fine mode)          | 1–24                             | 1–24                      | 1–24            |
| Focus Lock command  | ✔ (focus lock On/Off)       | —                             | —                                | —                         | —               |
| ND Filter control   | —                           | ✔ (elec. ND 1/4–1/128)        | —                                | —                         | —               |
| Dual Tally lamps    | Single (flash/on/off)       | **Red + Green** (independent) | Red only (Hi/Lo brightness)      | —                         | —               |
| “Bright” AE mode    | ✔ (Bright Exposure mode)    | ✖ (not supported)             | ✔ (Bright mode available)        | ✔ (Bright mode available) | ✔ (Bright mode) |
| ATW (Auto Trace WB) | —                           | ✔ (Auto Tracing WB mode)      | —                                | —                         | —               |
| Color-Temp WB mode  | ✔ (WB Mode = 0x20)          | — (uses Memory A/B instead)   | ✔ (WB Color Temp mode)           | ✔ (WB Color Temp mode)    | —               |

\* BRC‑H900 requires optional IP interface board (BRBK-IP10) for network control; otherwise only RS-232/422 control is available.

As seen above, the newer FR7 introduces some unique features (like variable ND filter and dual tally lights) not found on older models, while PTZOptics has some custom commands (like Focus Lock) not present on Sony cameras. The number of preset memory slots varies widely: from 6 on entry-level models up to 100+ on newer cameras (PTZOptics firmware 2.2 expanded presets to 128, up from 10 in earlier firmware). When developing, it’s wise to query the camera’s version or model info and adjust UI/controls accordingly (e.g., disable ND controls for models without ND filter, limit preset index range, etc.).

---

## 8. Parameter Ranges & Scaling

When controlling zoom, pan/tilt, and exposure, it’s important to understand each model’s numeric ranges and scaling. VISCA parameters are typically given as big-endian hexadecimal values, which correspond to physical positions or settings. The subsections below detail the ranges for zoom, pan/tilt coordinates, and exposure settings, helping you ensure your commands stay within valid limits for each device.

### 8.1 Zoom

VISCA uses a 16-bit value to represent the zoom lens position. The minimum value (`0x0000`) corresponds to the wide end (fully zoomed out). Each camera defines its maximum **optical zoom** position according to its lens capability, and values beyond that can be used for **digital zoom** (if enabled). Notably, many Sony cameras standardize the optical range to end at `0x4000` at full telephoto, regardless of actual zoom ratio, and use the range above for digital zoom. For example, on a 20x optical zoom camera:

* `0x0000` – Wide end (1x)
* `0x4000` – Tele end of optical zoom (20x optical on many models)
* `0x4000`–`0x7FFF` – Digital zoom range (if digital zoom is 2x, the max value might be around 0x7FFF; if 12x, it might reach near 0x7AC0 or 0x7FFF depending on how the camera scales steps).

Examples for specific models:

* **PTZOptics 20× SDI:** `0x0000` (wide) to `0x4000` (optical 20× tele). With digital zoom up to \~4×, the maximum value extends to around `0x7800` (which represents about 80× total zoom).
* **Sony BRC‑300 (12× optical):** `0x0000` to `0x4000` for 12× optical (Sony scaled 12× to the same 0x4000 endpoint). With 4× digital, up to \~`0x7FFF` for 48× total.
* **Sony EVI‑H100 (20× optical, 12× digital):** `0x0000` to `0x4000` for 20× optical, and up to `0x7AC0` at 12× digital (for 240× total).

In general, you can obtain the current zoom position via the `ZoomPosInq` and use the returned values to calibrate your software’s expectations. When setting zoom via `Zoom Direct` commands, ensure the value does not exceed the camera’s maximum (some cameras will ignore or clamp values beyond their range).

### 8.2 Pan / Tilt Coordinates

VISCA defines pan and tilt coordinates as 16-bit values, but **two different conventions** exist:

* **Legacy Unsigned Range:** Used by older models like the BRC‑300. Here, `0x0000` represents one extreme (say far left or top) and `0xFFFF` the opposite extreme (far right or bottom). The midpoint (\~0x7FFF) is roughly center, but there is no explicit “zero” center code in the protocol for these models.
* **Signed Centered Range (Two’s Complement):** Used by most modern cameras (Sony BRC/SRG/FR series, ILME-FR7, PTZOptics, etc.). In this scheme, `0x0000` represents the center position. Values increase positively in one direction (e.g. to the right or downward) up to `+0x7FFF`, and decrease (two’s complement negative values) from `0xFFFF` downward for the opposite direction. For instance, on a camera with ±170° pan range, the far right might be around `0x1E1B` and far left around `0xE1E5` (which is −0x1E1B in two’s complement).

Your control software should account for these differences. Typically, all cameras in this guide except the legacy BRC-300 (and its clone, Nearus 300) use the signed centered system. The BRC-300 (circa 2004) uses the 0–FFFF scheme. If you send a “Pan Absolute” command to a BRC-300, you would specify the target as a raw position from 0x0000 to 0xFFFF. For a modern camera, you would specify it as an offset from center (e.g. 0xFF80 might mean a small left move, 0x0080 a small right move).

Below are approximate mechanical ranges and end-limit codes for select models (values are in hex, representing their two-byte pan/tilt coordinates):

* **PTZOptics 20× SDI (Gen2):** Pan \~±170°, Tilt –30° (up) to +90° (down). Center is `0x0000`. Far right ≈ `0x1E1B`, far left ≈ `0xE1E5`. Tilt down (90°) ≈ `0x0FF0`, tilt up (–30°) ≈ `0xFC75` (when not flipped).
* **Sony ILME‑FR7:** Pan ±170° (similar coding as above). Tilt –30° to +195° (it can look upward when mounted inverted). Internally, the FR7 actually extends tilt coding beyond 16-bit (it uses 20-bit internally but clips to physical range) – however, any command beyond physical limits will just result in the camera moving to its stop. (e.g., FR7 tilt might use codes beyond 0x8000 due to the extreme range).
* **Sony BRC‑300 / Nearus 300:** Pan \~±170° as well, but coded 0x0000–0xFFFF (unsigned). So 0x0000 = –170°, 0xFFFF = +170° approximately. Tilt \~–30° to +90° correspondingly in that full range.

For practical use: It’s recommended to implement an abstraction where you deal in real angles (degrees) or normalized range, and have model-specific transformations to VISCA codes. Also consider offering an “image flip” setting (see §9) for ceiling-mounted cameras, as flipping usually also swaps or offsets the tilt coordinate interpretation (e.g., as seen in the EVI-H100 values above where the tilt range codes changed when image flip was ON).

### 8.3 Exposure Parameter Ranges

Exposure settings (shutter, iris, gain, exposure compensation, etc.) are quantized into discrete steps in VISCA. Most values have standardized ranges, though some models extend them:

* **Shutter Speed:** Typically 21–24 steps from slowest (open shutter 1/4s or 1/1s depending on model) to fastest (1/10,000s). Represented as a 16-bit value in some manuals, but effectively an index (0 = 1/60, 0x0A = 1/1000, etc., varies by model specs).

* **Iris (Aperture):** Usually 18 steps for most lenses (F1.6 to F14 or closed). For example, 0x0000 = F1.6 (open iris), 0x000C = F8, 0x0010 = Close. The FR7, having interchangeable lenses, will quantize iris differently if using a servo lens, but the VISCA command still uses a 16-bit value (Sony notes 16 steps in many cameras).

* **Gain:** 0 to 0x0F in hex (0 to 15) for 0 dB up to typically +30 dB (each step \~+2 dB). Some newer cameras allow a bit beyond (FR7 might map these differently since it has an ISO concept, but in VISCA it still reports a value 0–15 for gain in standard mode).

* **Exposure Compensation (EV):** Most cameras offer EV ±7 steps (14 total plus zero) which correspond to 0x00–0x0E in the VISCA command (when exposure compensation is on). 0x07 might represent 0 EV (depending on implementation, or 0x0E might be +7, 0x00 = –7). The FR7 notably extends the range to allow ±9 or more. In fact, FR7’s documentation indicates it accepts values up to 0x12 for exposure compensation in Cine EI mode (giving a wider adjustment range). Thus FR7’s EV Shift goes beyond the 0–14 of other models.

* **White Balance Color Temperature:** When WB is in “Color Temp” mode (on models that support it), the Color Temp value is typically a 2-byte setting. For Sony models supporting it (e.g., BRC-H900, EVI-H100, PTZOptics), the range is from 2500K up to 8000K, mapped linearly to 0x0000–0x0037 (for 0x0000 = 2500K, 0x0037 ≈ 8000K). The FR7 doesn’t use this mode; instead it has Memory A/B white balance presets or direct Kelvin via its web UI (VISCA WB mode 0x20 is not applicable on FR7).

In summary, while VISCA provides a uniform way to set these parameters, the meaning of the values can differ. Always refer to model-specific documentation for exact mappings (e.g., the EVI-H100 technical manual details the EV step mapping to dB, and the zoom step mapping). The Appendix provides a CSV of command codes, but not the detailed mapping of values to real-world units – for that, use manufacturer references.

---

## 9. Model‑Specific Command Extensions

Many cameras implement additional VISCA commands beyond the baseline set, typically to control features unique to that model. In a universal controller, these can be handled as optional extensions for those specific models. Below we list some notable model-specific commands:

### 9.1 PTZOptics (Gen2 firmware)

* **Focus Lock:** `81 0A 04 68 02 FF` (Lock focus) / `... 68 03 FF` (Unlock focus). When focus lock is on, the camera’s focus motor is held at its current position and any autofocus or manual focus commands are ignored. (Camera returns `... 41 FF` if you attempt a focus move while locked.)
* **H-Flip and V-Flip:** PTZOptics provides an image flip/mirror command. In newer firmware, this is a single command `81 01 04 A4 0p FF` where p=1 (Horizontal flip), 2 (Vertical flip), 3 (Both), 0 (Off). (In older or Sony context, some cameras used separate bits for H and V flip – e.g., Sony SRG series uses a different command, but PTZOptics consolidated it into one). This is used when mounting cameras upside-down or for mirror-image needs.
* **Auto White Balance Sensitivity:** `81 01 04 A9 00 FF` (High), `... A9 01 FF` (Normal), `... A9 02 FF` (Low). This adjusts how aggressively the auto white balance responds to scene changes. By default it’s Normal; High makes WB react faster (useful for frequent lighting changes), Low makes it more stable.
* **Multicast On/Off:** `81 0B 01 23 01 FF` (Enable multicast streaming), `... 23 02 FF` (Disable multicast). This is specific to NDI-enabled PTZOptics models or those supporting multicast streaming of their video feed. It doesn’t affect VISCA control, but rather the camera’s network video output mode.
* **NDI Mode Quality:** `81 0B 01 01 0p FF` – for NDI models, sets stream bandwidth (p=1 High, 2 Medium, 3 Low, 4 Off). This is outside pure VISCA camera control, but PTZOptics exposes it via the VISCA-over-IP interface.
* **Motion Sync Feature:** Some PTZOptics firmware versions introduced “PTZ Motion Sync” to coordinate pan, tilt and zoom to start/stop together. Commands like `81 0A 11 13 02 FF` (On) / `... 13 03 FF` (Off) and speed limits `81 0A 11 14 pq FF` control this. (This is advanced; enabling it makes the camera internally adjust speeds so that a preset move with pan+tilt+zoom finishes all movements simultaneously.)

*(Ensure you consult PTZOptics’ latest VISCA command document for other model-specific commands; the above are some highlights from v1.2 spec.)*

### 9.2 Sony ILME‑FR7

Sony’s FR7 (full-frame PTZ) extends VISCA in many ways to accommodate its cinema features:

* **Electronic ND Filter:** The FR7 has a built-in variable ND filter. Commands: `8x 01 7E 04 52 0p FF` selects ND mode (p=0 Preset, 1 Variable). In Preset mode, you can directly jump to preset ND levels (which are set via web UI or CGI). In Variable mode, you can adjust ND continuously. Use `8x 01 7E 04 12 02 FF` (ND filter step *up*) and `... 04 12 03 FF` (step *down*) to nudge ND in small increments, or the direct command `8x 01 7E 04 42 00 0p 0q FF` to set an exact ND value. The range is 0x0000 (ND 1/4, i.e. 2 stops) to 0x0014 (ND 1/128, 7 stops). There’s also `8x 01 7E 04 53 0p FF` to turn **Auto ND** On (p=02) or Off (p=03) – when On, the camera will automatically engage the ND to maintain exposure, like auto iris but using ND.
* **Tally Red/Green:** The FR7 has two tally lamps. Commands: `8x 01 7E 01 0A 00 0p FF` controls the Red tally (p=02 On, 03 Off); `8x 01 7E 04 1A 00 0p FF` controls the Green tally (p=02 On, 03 Off). (The green tally is often used as a “preview” tally in multi-camera broadcast setups.) If you query `7E 01 0A 00` or `7E 04 1A 00` with an inquiry, the camera returns `90 50 02 FF` or `03 FF` for on/off respectively.
* **Pan/Tilt Speed Mode:** The FR7 supports a fine-grained speed control option. By default, many Sony PTZs have 24 discrete speed levels. FR7 can switch to 50 levels mode. The command `8x 01 7E 04 1B 02 FF` enables 50-step speed mode, whereas `... 1B 01 FF` sets legacy 24-step mode. (This affects how the camera interprets the pan/tilt speed byte values VV and WW in the continuous/absolute move commands.)
* **Direct Menu Control:** The FR7 has an on-screen menu (accessible via remote or web UI). VISCA provides extended commands to simulate button presses on the camera’s control interface. For example, `8x 01 7E 04 72 0p 0q FF` – this command is used to operate the menu or assignable dials. In Sony’s documentation, certain values of `pp` and `0q` correspond to actions (like selecting menu items, adjusting an assigned setting, etc.). This is an advanced feature and typically used in conjunction with the FR7’s direct menu mode.
* **Others:** The FR7 also supports features like *Picture Profile selection*, *Scene File recall*, etc., through VISCA over IP (often via device-setting commands). For instance, it has commands to load/adjust scene files, to toggle a color bars output (`7E 04 7D 02 FF` on/off), and more. These are all detailed in the FR7 VISCA Command List manual. Given the FR7’s unique status (it’s essentially a cinema camera on a PT mechanism), it’s recommended to refer to Sony’s official “Network & VISCA Command Manual” for the FR7 for any advanced integrations.

One more note: The FR7’s tally implementation automatically turns off a tally lamp \~15 seconds after the last “On” command, as a safety to prevent a stuck tally. If a continuous tally indication is needed, your controller should resend the On command periodically (within 15s intervals) to refresh it. (The auto-off does not occur if the tally was turned on via the hardware GPI or CGI – it only applies to VISCA-triggered tally.)

### 9.3 Sony BRC‑H900 (and similar)

* **Image Freeze:** `81 01 04 62 02 FF` (Freeze On), `... 62 03 FF` (Freeze Off). This freezes the camera’s video output on the last frame. Useful during preset moves or adjustments to avoid broadcasting the motion. Not all cameras have this (BRC-H900 does, as do some SRG models; PTZOptics does not).
* **Tally Brightness (High/Low):** `81 01 7E 01 0A 01 04 FF` (Tally lamp low brightness), `... 0A 01 05 FF` (Tally high brightness). The BRC-H900’s red tally lamp can be dimmed or brightened. Typically you would send this command once to set the tally brightness level (04 for dim, 05 for bright) and then use the standard Tally On/Off command (`...0A 00 02` / `03`) to control it. The brightness setting persists.
* **IP Interface Card Discovery:** The BRC-H900 with the optional BRBK-IP10 board listens on a specific port (52380) for a broadcast “Discovery” or “ENQ” message. While not a VISCA command per se, sending a UDP packet like “ENEQ\:network” to port 52380 can prompt the camera’s IP card to respond with its IP address or MAC (this is described in the IP card’s manual). This helps find the camera on the network when its IP is not known. In practice, a simpler way is using the Sony “RM-IP Setup Tool” when available. For VISCA control, once the IP is known, the camera behaves like other Sony VISCA-over-IP (8-byte header on port 52381).

Apart from these, the BRC-H900 supports the typical Sony commands and inquiries listed earlier. It does not have some of the new features of FR7 (no dual tally or ND filter, etc., since it’s an older HD camera).

### 9.4 Sony EVI‑H100 (and related EVI models)

* **Advanced Image Settings:** The EVI-H100 series (and some EVI/SRG models) include VISCA commands for settings like *Gamma* (`81 01 04 5B 0p FF`, p=0 Standard, 1–4 various gamma curves), *Wide Dynamic Range* (called “High Resolution mode” and “Visibility Enhancer” in some models) toggles (`... 04 52 02/03 FF` for HR on/off on H100), and *Noise Reduction* level (`81 01 04 53 0p FF`, p=0 Off, 1–5 increasing NR). These allow fine-tuning the image.

* **IR Remote Control Reporting:** Some EVI models have a setting to output a VISCA message when an IR remote button is pressed (so an external system can know). For example, the H100 has an “IR Return” setting (`7D 01 ...`) as seen in its manual.

* **VISCA Busy Behavior:** As mentioned earlier, the EVI-H100 has the quirk of returning `41 FF` errors if new commands come too quickly after certain actions (presets, etc.). Sony explicitly notes the 240 ms busy window post-preset recall. Additionally, if the On-Screen Display menu is open (`06 06 02 FF` to open), the camera will ignore most VISCA commands (other than menu navigation). So a best practice is to ensure the OSD is closed (`06 06 03 FF` to close) before sending critical commands, or to send the close command preemptively (see Checklist item 7).

* **Nearus BRC-300 clone:** While not Sony, the Nearus (SnapAV) BRC-300 is essentially a rebranded BRC-300 that speaks standard VISCA. One interesting note from a Nearus document confirms that it does not use any IP header (since it’s serial only) and fully relies on the base VISCA protocol. (This is expected given it’s an older design; we include it here because some integrators might encounter these rebrands.)

In summary, when working with model-specific functions, consult the camera’s VISCA command list if available. Sony publishes command lists for each camera model or family (as PDFs), and other manufacturers (PTZOptics, Marshall, AVer) often provide their VISCA command references. The **Appendix A** CSV table provides a unified listing, including some model-specific opcodes and which model they apply to.

---

## 10. Behavioral Edge-Cases & Recommended Tests

Even when commands are correct, cameras may exhibit certain edge-case behaviors or require special handling. The table below outlines various test scenarios and how the cameras are expected to behave. We recommend using these scenarios to validate your implementation. By anticipating these, you can handle errors or quirks gracefully (e.g., avoiding command overload or dealing with temporary non-executable states).

| Scenario                                           | Expected Camera Behavior                                                                                                                                                                                                                                                                                     |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Send 3rd command while 2 are in progress**       | Camera replies `90 60 03 FF` (*Command Buffer Full*) – third command is rejected. Controller should queue or retry later.                                                                                                                                                                                    |
| **Manual focus command while AF is On**            | Camera replies `90 6y 41 FF` (*Not Executable*) – it ignores the focus command because it’s in Auto Focus mode. Solution: switch to Manual focus first, or handle the error (camera stays in AF).                                                                                                            |
| **Focus Lock active (PTZOptics) → any focus move** | Camera replies `90 6y 41 FF` for focus attempts – focus is locked, so it won’t move (not executable). Solution: unlock focus before issuing focus commands.                                                                                                                                                  |
| **FR7: send ND adjust during a Preset recall**     | Camera likely replies `90 6y 41 FF` – during a preset motion (which might include its own ND or exposure actions), certain commands are not accepted. The FR7 locks out ND changes while a preset is running. Controller should wait for preset completion.                                                  |
| **FR7: tally left on without refresh**             | About 15 seconds after last `Tally On`, the lamp auto-extinguishes. If continuous tally is required, resend the On command periodically (the FR7 does **not** auto-off the lamp if it was turned on via hardware or CGI, only VISCA, as noted).                                                              |
| **EVI-H100: preset recall + immediate next cmd**   | The next command sometimes gets a one-time `41 FF` error (busy). If detected, wait \~0.25s and retry the command. The second attempt should succeed.                                                                                                                                                         |
| **Protocol mix-up (header vs no-header)**          | If a Sony-style header packet is sent to a raw-only camera (e.g. PTZOptics), it will be **ignored** (no ACK, as if nothing was received). If raw bytes are sent to a Sony expecting header, it will also ignore them. This can confuse a controller, so ensure the correct mode is set as discussed in §3.2. |

Testing these scenarios on each model you integrate can help ensure your control software handles them smoothly. For example, intentionally overfill the command buffer to see the error handling, or try a focus command in AF mode to confirm you catch the `41 FF` and maybe auto-switch the camera to manual focus mode in response.

---

## 11. Implementation Checklist

Finally, here’s a checklist summarizing best practices and considerations when implementing a multi-camera VISCA control system. Ensuring each of these points is addressed will improve compatibility and robustness:

1. **Pluggable Transport (Raw vs Encapsulated):** Implement a configuration or detection mechanism to switch between sending raw VISCA bytes (for serial and PTZOptics-style IP) and adding the 8-byte header (for Sony/encapsulated IP). Getting this wrong will result in no responses (if header sent to a camera expecting none, or vice versa).
2. **Two-Socket Command Queue:** Maintain state for the two command buffers. Do not send a new command when both are occupied; either queue it or delay until a socket frees up. This prevents “Buffer Full” errors.
3. **Pan/Tilt Coordinate Scaling:** Transparently handle the difference in pan/tilt coordinate systems (signed centered vs unsigned range). For example, abstract a “pan angle” so that 0° maps to the appropriate code on each model (0x0000 on modern cameras, \~0x7FFF on legacy). This avoids confusion where moving to “center” would require different values.
4. **Firmware Quirk Registry:** Account for model-specific quirks or limits. E.g., older PTZOptics firmware only had 10 presets (versus 128 now), the EVI-H100’s 240 ms post-preset busy period, FR7’s extended exposure modes (no “Bright” mode, but has Cine EI, etc.), or the need to refresh FR7 tally. Use the camera’s model info to apply conditional handling in your code.
5. **Address Set / IF Clear on Serial:** If using serial control, always perform the Address Set (`88 30 01 FF`) and IF Clear (`88 01 00 01 FF`) sequence on startup. This ensures all cameras are properly indexed and no old commands are still running. (Skip these in IP mode – they’re not applicable).
6. **IP Discovery (if needed):** For cameras like the BRC-H900, consider implementing the discovery mechanism (send a special packet to provoke a reply) so users can find the camera’s IP easily. Alternatively, provide instructions to use vendor tools if appropriate.
7. **UDP Retries (Sony IP mode):** Implement a timeout and retransmission for UDP commands on encapsulated connections. If no ACK in e.g. 100 ms, resend with next sequence ID. But also be prepared for the edge-case of a late ACK from the first attempt – your code should handle out-of-order or duplicate replies gracefully.
8. **Ensure OSD is Closed:** If your camera has an on-screen menu (most do), and your user might use it (via IR remote, etc.), it’s wise to send an OSD Close command (`81 01 06 06 03 FF`) before sending movement or preset commands, just in case. Many cameras ignore pan/tilt/zoom commands while the menu is open (the menu typically takes over control). Closing the OSD (or detecting the OSD open via the *Information Display On/Off* inquiry if available) avoids this pitfall.
9. **Sequential Command Timing:** Avoid flooding a camera with commands faster than it can handle. Even if you manage the two sockets, if you send a new command immediately *after* a completion, some cameras (as noted) might still be internally busy (e.g. driving focus or exposure adjustments after a preset). A small delay (e.g. 50–100 ms) between high-level actions or a quick inquiry to confirm readiness (e.g. maybe a ZoomPosInq returns only when zoom drive stopped) can improve reliability.
10. **Logging and Diagnostics:** Implement verbose logging (at least as an option) of bytes sent/received. This greatly aids in diagnosing issues with VISCA. If something isn’t working (camera not responding, or returning an error), having the hex trace will allow you (or vendor support, or community forums) to pinpoint the problem faster. For instance, seeing `90 60 02 FF` in logs clearly shows a syntax error — perhaps you had a typo in the command.

By following this checklist and the guidance throughout this document, you should be well on your way to a robust unified VISCA implementation capable of orchestrating a diverse fleet of PTZ cameras.

---

## 12. Sources & References

* **PTZOptics – “VISCA over IP Command Set Rev 1.2”** (PDF, Aug 2020) – Primary reference for PTZOptics raw UDP/TCP commands and device-specific features.
* **Sony – “VISCA Command List v2.00 (BRC‑300/300P)”** (2004) – Definitive command, inquiry, and error handling reference for legacy VISCA (e.g., BRC-300).
* **Sony – “EVI‑H100S/H100V Technical Manual”** (Dec 2012) – Extended VISCA tables for 20× optical zoom, Wide-D (High Res) mode, Gamma, NR levels, and notes on 240 ms post-preset busy behavior.
* **Sony – “ILME‑FR7 Network & VISCA Command Manual”** (Oct 2024, rev 2.1) – Source for FR7’s ND filter control, dual tally, 50-speed mode, 32-bit sequence numbers, and detailed VISCA-over-IP header specs.
* **Nearus (SnapAV) – “VISCA Protocol via Sony” hand-out** (2014) – Confirms BRC-300 compatibility and absence of IP header in Nearus serial control (effectively identical to Sony VISCA 1.0).
* **Jon Skeet’s Coding Blog – “Variations in the VISCA protocol”** (Nov 2023) – Background on raw vs encapsulated VISCA over IP and example of needed adjustments between camera brands.
* **Sony Community Forum** (2021) – Discussion confirming that PTZOptics cameras ignore Sony-style headers.
* **Avonic Support – “Visca”** (2021) – Describes use of Sony VISCA-over-IP (52381 port with header) vs raw port 1259, with example packets.
* **Marshall Cameras – VISCA over IP Command Set** (2020) – Another manufacturer’s documentation (for CV630-IP) matching Sony’s encapsulated protocol, included here for cross-reference on header structure and payload types.
* **Sony Press Release – FR7 Launch (Sept 2022)** – Confirms FR7 supports 100 presets and details on its pan/tilt range and ND filter.
* *Additional references:* Sony VISCA Protocol Spec v1.4 (for command socket details), various user manuals (PTZOptics, AIDA Imaging’s PTZ which mirrors PTZOptics) for preset counts, OBS open-source VISCA control discussions, etc., were consulted to validate behaviors.
---

### Appendix A – CSV Opcode Table

Category,Mnemonic,Opcode,Bytes,Sony,PTZOptics,FR7,Notes
```

> *“Bytes” = total hex bytes including the terminating `FF`.*

```csv
Category,Mnemonic,Opcode,Bytes,Sony,PTZOptics,FR7,Notes
Power,CAM_Power ON,81 01 04 00 02 FF,6,Yes,Yes,Yes,
Power,CAM_Power Standby,81 01 04 00 03 FF,6,Yes,Yes,Yes,
Power,PowerInq,81 09 04 00 FF,5,Yes,Yes,Yes,Reply 90 50 02/03 FF
Zoom,Zoom Stop,81 01 04 07 00 FF,6,Yes,Yes,Yes,
Zoom,Zoom Tele Std,81 01 04 07 02 FF,6,Yes,Yes,Yes,
Zoom,Zoom Wide Std,81 01 04 07 03 FF,6,Yes,Yes,Yes,
Zoom,Zoom Tele Var,81 01 04 07 2p FF,6,Yes,Yes,Yes,p=0–7
Zoom,Zoom Wide Var,81 01 04 07 3p FF,6,Yes,Yes,Yes,p=0–7
Zoom,Zoom Direct,81 01 04 47 0p 0q 0r 0s FF,10,Yes,Yes,Yes,16‑bit position
Zoom,ZoomPosInq,81 09 04 47 FF,5,Yes,Yes,Yes,Reply 90 50 0p 0q 0r 0s FF
Focus,Focus Auto,81 01 04 38 02 FF,6,Yes,Yes,Yes,
Focus,Focus Manual,81 01 04 38 03 FF,6,Yes,Yes,Yes,
Focus,FocusModeInq,81 09 04 38 FF,5,Yes,Yes,Yes,Reply 90 50 02/03 FF
Focus,Focus Far Std,81 01 04 08 02 FF,6,Yes,Yes,Yes,
Focus,Focus Near Std,81 01 04 08 03 FF,6,Yes,Yes,Yes,
Focus,Focus Far Var,81 01 04 08 2p FF,6,Yes,Yes,Yes,p=0–7
Focus,Focus Near Var,81 01 04 08 3p FF,6,Yes,Yes,Yes,p=0–7
Focus,Focus Stop,81 01 04 08 00 FF,6,Yes,Yes,Yes,
Focus,Focus Direct,81 01 04 48 0p 0q 0r 0s FF,10,Yes,Yes,Yes,
Focus,FocusPosInq,81 09 04 48 FF,5,Yes,Yes,Yes,Reply 90 50 0p 0q 0r 0s FF
Focus,Focus Lock On,81 0A 04 68 02 FF,6,No,Yes,No,
Focus,Focus Lock Off,81 0A 04 68 03 FF,6,No,Yes,No,
Focus,AF Zone Top,81 01 04 AA 00 FF,6,No,Yes,No,
Focus,AF Zone Center,81 01 04 AA 01 FF,6,No,Yes,No,
Focus,AF Zone Bottom,81 01 04 AA 02 FF,6,No,Yes,No,
Focus,Push AF Press (FR7),81 01 7E 01 0A 00 01 FF,8,No,No,Yes,
Focus,Push AF Release (FR7),81 01 7E 01 0A 00 00 FF,8,No,No,Yes,
PanTilt,PT Drive Cont,81 01 06 01 VV WW DD DD FF,10,Yes,Yes,Yes,VV pan spd WW tilt spd
PanTilt,PT Drive Stop,81 01 06 01 00 00 03 03 FF,10,Yes,Yes,Yes,Dir 03 03 = stop
PanTilt,PT Absolute,81 01 06 02 VV WW YY YY ZZ ZZ FF,12,Yes,Yes,Yes,
PanTilt,PT Relative,81 01 06 03 VV WW YY YY ZZ ZZ FF,12,Yes,Yes,Yes,
PanTilt,Home,81 01 06 04 FF,6,Yes,Yes,Yes,
PanTilt,Reset,81 01 06 05 FF,6,Yes,Yes,Yes,
PanTilt,PT Limit Set,81 01 06 07 00 FF,6,Yes,Yes,Yes,Set current pos as limit
PanTilt,PT Limit Clear,81 01 06 07 01 FF,6,Yes,Yes,Yes,
PanTilt,PTPosInq,81 09 06 12 FF,5,Yes,Yes,Yes,Reply 90 50 PP…TT…FF
Memory,Preset Reset,81 01 04 3F 00 pp FF,7,Yes,Yes,Yes,pp preset #
Memory,Preset Set,81 01 04 3F 01 pp FF,7,Yes,Yes,Yes,
Memory,Preset Recall,81 01 04 3F 02 pp FF,7,Yes,Yes,Yes,
Exposure,AE Full Auto,81 01 04 39 00 FF,6,Yes,Yes,Yes,
Exposure,AE Manual,81 01 04 39 03 FF,6,Yes,Yes,Yes,
Exposure,AE Shutter Pri,81 01 04 39 0A FF,6,Yes,Yes,Yes,
Exposure,AE Iris Pri,81 01 04 39 0B FF,6,Yes,Yes,Yes,
Exposure,AE Bright Mode,81 01 04 39 0D FF,6,Yes,Yes,No,Not on FR7
Exposure,AEModeInq,81 09 04 39 FF,5,Yes,Yes,Yes,Reply 90 50 00/03/0A/0B/0D FF
Exposure,EV Comp On,81 01 04 3E 02 FF,6,Yes,Yes,Yes,
Exposure,EV Comp Off,81 01 04 3E 03 FF,6,Yes,Yes,Yes,
Exposure,EV Comp Direct,81 01 04 4E 00 00 0p 0q FF,10,Yes,Yes,Yes,
Exposure,Backlight On,81 01 04 33 02 FF,6,Yes,Yes,Yes,
Exposure,Backlight Off,81 01 04 33 03 FF,6,Yes,Yes,Yes,
Exposure,Spotlight On,81 01 04 3A 02 FF,6,Yes,No,Yes,
Exposure,Spotlight Off,81 01 04 3A 03 FF,6,Yes,No,Yes,
Exposure,Gain Limit Direct (PTZO),81 01 04 2C 0p FF,6,No,Yes,No,
Exposure,Auto Slow Shutter On,81 01 04 5A 02 FF,6,Yes,No,Yes,
Exposure,Auto Slow Shutter Off,81 01 04 5A 03 FF,6,Yes,No,Yes,
Exposure,Iris Direct,81 01 04 4B 00 00 0p 0q FF,10,Yes,Yes,Yes,
Exposure,IrisInq,81 09 04 4B FF,5,Yes,Yes,Yes,Reply 90 50 0p 0q 0r 0s FF
Exposure,Shutter Direct,81 01 04 4A 00 00 0p 0q FF,10,Yes,Yes,Yes,
Exposure,ShutterInq,81 09 04 4A FF,5,Yes,Yes,Yes,Reply 90 50 0p 0q 0r 0s FF
Exposure,Gain Direct,81 01 04 0C 00 00 0p 0q FF,10,Yes,Yes,Yes,
Exposure,GainInq,81 09 04 4C FF,5,Yes,Yes,Yes,Reply 90 50 0p 0q 0r 0s FF
Exposure,Bright Direct,81 01 04 0D 00 00 0p 0q FF,10,Yes,Yes,No,Not on FR7
WB,WB Auto,81 01 04 35 00 FF,6,Yes,Yes,Yes,
WB,WB Indoor,81 01 04 35 01 FF,6,Yes,Yes,Yes,
WB,WB Outdoor,81 01 04 35 02 FF,6,Yes,Yes,Yes,
WB,WB One Push,81 01 04 35 03 FF,6,Yes,Yes,Yes,
WB,WB ATW,81 01 04 35 04 FF,6,Yes,No,Yes,FR7 only
WB,WB Manual,81 01 04 35 05 FF,6,Yes,Yes,Yes,
WB,WB Color Temp,81 01 04 35 20 FF,6,Yes,Yes,No,Not on FR7
WB,WBModeInq,81 09 04 35 FF,5,Yes,Yes,Yes,Reply 90 50 00/01/02/04/05/20 FF
WB,Color Temp Direct,81 01 04 20 0p 0q FF,8,Yes,Yes,No,
WB,ColorTempInq,81 09 04 20 FF,5,Yes,Yes,No,Reply 90 50 0p 0q FF
WB,One Push Trigger,81 01 04 10 05 FF,6,Yes,Yes,Yes,
WB,R Gain Direct,81 01 04 43 00 00 0p 0q FF,10,Yes,Yes,Yes,
WB,B Gain Direct,81 01 04 44 00 00 0p 0q FF,10,Yes,Yes,Yes,
WB,AWB Sens High,81 01 04 A9 00 FF,6,No,Yes,No,
WB,AWB Sens Normal,81 01 04 A9 01 FF,6,No,Yes,No,
WB,AWB Sens Low,81 01 04 A9 02 FF,6,No,Yes,No,
Tally,Tally Red On,81 01 7E 01 0A 00 02 FF,8,Yes,No,Yes,
Tally,Tally Red Off,81 01 7E 01 0A 00 03 FF,8,Yes,No,Yes,
Tally,TallyInq Red,81 09 7E 01 0A 00 FF,7,Yes,No,Yes,Reply 90 50 02/03 FF
Tally,Tally Bright Lo (H900),81 01 7E 01 0A 01 04 FF,8,Yes,No,No,
Tally,Tally Bright Hi (H900),81 01 7E 01 0A 01 05 FF,8,Yes,No,No,
Tally,Tally Green On (FR7),81 01 7E 04 1A 00 02 FF,8,No,No,Yes,
Tally,Tally Green Off (FR7),81 01 7E 04 1A 00 03 FF,8,No,No,Yes,
Tally,TallyInq Green,81 09 7E 04 1A 00 FF,7,No,No,Yes,Reply 90 50 02/03 FF
Tally,Tally Flash (PTZO),81 0A 02 02 01 FF,6,No,Yes,No,
Tally,Tally On (PTZO),81 0A 02 02 02 FF,6,No,Yes,No,
Tally,Tally Off (PTZO),81 0A 02 02 03 FF,6,No,Yes,No,
Picture,Picture Effect Off,81 01 04 63 00 FF,6,Yes,Yes,Yes,
Picture,Picture Effect B&W,81 01 04 63 04 FF,6,Yes,Yes,Yes,
Picture,Image Freeze On,81 01 04 62 02 FF,6,Yes,No,No,BRC‑H900
Picture,Image Freeze Off,81 01 04 62 03 FF,6,Yes,No,No,
Picture,Digital Zoom On,81 01 04 06 02 FF,6,Yes,No,No,
Picture,Digital Zoom Off,81 01 04 06 03 FF,6,Yes,No,No,
ND,ND Mode Preset (FR7),81 01 7E 04 52 00 FF,8,No,No,Yes,
ND,ND Mode Variable (FR7),81 01 7E 04 52 01 FF,8,No,No,Yes,
ND,ND Level Direct (FR7),81 01 7E 04 42 00 0p 0q FF,10,No,No,Yes,
ND,ND Step Up (FR7),81 01 7E 04 12 02 FF,7,No,No,Yes,Increment ND (variable mode)
ND,ND Step Down (FR7),81 01 7E 04 12 03 FF,7,No,No,Yes,
ND,Auto ND On (FR7),81 01 7E 04 53 02 FF,8,No,No,Yes,
ND,Auto ND Off (FR7),81 01 7E 04 53 03 FF,8,No,No,Yes,
System,H‑Flip On (legacy),81 01 04 61 02 FF,6,No,Yes,No,
System,H‑Flip Off (legacy),81 01 04 61 03 FF,6,No,Yes,No,
System,V‑Flip On (legacy),81 01 04 66 02 FF,6,No,Yes,No,
System,V‑Flip Off (legacy),81 01 04 66 03 FF,6,No,Yes,No,
System,Flip Combined (PTZO),81 01 04 A4 0p FF,6,No,Yes,No,p=0 off 1 H 2 V 3 HV
System,VersionInq,81 09 00 02 FF,5,Yes,Yes,Yes,Reply 90 50 VV VV MM MM FF FF KK FF
System,Address Set,88 30 01 FF,4,Yes,Yes,Yes,Broadcast (serial only)
System,I/F Clear,88 01 00 01 FF,5,Yes,Yes,Yes,Broadcast (serial only)
System,Command Cancel socket1,81 21 FF,3,Yes,Yes,Yes,
System,Command Cancel socket2,81 22 FF,3,Yes,Yes,Yes,
System,PTZ Motion‑Sync On (PTZO),81 0A 11 13 02 FF,6,No,Yes,No,
System,PTZ Motion‑Sync Off (PTZO),81 0A 11 13 03 FF,6,No,Yes,No,
System,PTZ Sync Max‑Speed (PTZO),81 0A 11 14 pp FF,6,No,Yes,No,pp = 1–24
System,FR7 Speed‑Step 24‑mode,81 01 7E 04 1B 01 FF,7,No,No,Yes,
System,FR7 Speed‑Step 50‑mode,81 01 7E 04 1B 02 FF,7,No,No,Yes,
Menu,OSD Menu On,81 01 06 06 02 FF,6,Yes,Yes,Yes,
Menu,OSD Menu Off,81 01 06 06 03 FF,6,Yes,Yes,Yes,
Menu,OSD Up,81 01 06 01 0E 0E 03 01 FF,10,Yes,Yes,Yes,
Menu,OSD Down,81 01 06 01 0E 0E 03 02 FF,10,Yes,Yes,Yes,
Menu,OSD Left,81 01 06 01 0E 0E 01 03 FF,10,Yes,Yes,Yes,
Menu,OSD Right,81 01 06 01 0E 0E 02 03 FF,10,Yes,Yes,Yes,
Menu,OSD Enter,81 01 06 06 05 FF,6,Yes,Yes,Yes,
Menu,OSD Cancel,81 01 06 06 04 FF,6,Yes,Yes,Yes,
Menu,Direct Menu Key (FR7),81 01 7E 04 72 pp 0q FF,8,No,No,Yes,pp = control id
Stream,Multicast On (PTZO),81 0B 01 23 01 FF,6,No,Yes,No,
Stream,Multicast Off (PTZO),81 0B 01 23 02 FF,6,No,Yes,No,
Stream,NDI Quality (PTZO),81 0B 01 01 0p FF,6,No,Yes,No,p=1 Hi 2 Med 3 Low 4 Off
```

### How to use

* **Filter by model**: Use the *Sony* / *PTZOptics* / *FR7* columns as quick flags.
* **Variable bytes** (e.g. `p`, `q`, `r`, `s`) are placeholders—substitute the required hex nibble(s).
* All new commands introduced in § 9 are now included (FR7 ND filter set/step, dual‑tally green, 50‑step speed mode, PTZOptics Motion‑Sync, combined image flip, multicast, etc.).
* Legacy commands remain for backward compatibility (e.g., separate H‑Flip/V‑Flip bits).