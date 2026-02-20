# Unified VISCA Protocol Implementation Guide

*(Covering PTZOptics Gen‑2/NDI, Sony ILME‑FR7, Sony BRC‑series, Sony EVI‑H100 S/V, Nearus BRC‑300, and more)*

**Author:** Grafton Machine Shed
**Contact:** [admin@grafton.ai](mailto:admin@grafton.ai)
**Version:** 2025‑07‑16  *(expanded model coverage, added references, clarified IP vs serial differences, and included all known command variations)*

**Notice:** *This document is a work in progress. It may contain inaccuracies or omissions, and new camera models may introduce further variations. Use with caution and verify against official documentation where available.* However, every effort has been made to cite authoritative sources to ensure accuracy of the information presented.

**Validation status (patch 2026-02-10):**
- ✅ **PTZOptics NDI|HX Gen‑2 sections** were cross‑checked against the provided PT12X/PT20X/PT30X NDI|HX user manuals and datasheets (Rev 1.5/1.6 and Rev 1.3, Aug 2020).
- ✅ **Axis range notes** were cross‑checked against the provided Axis VISCA Interface API Description (M1.6, Mar 2021).
- ⚠️ Content about other manufacturers/models (e.g., Sony FR7/BRC/EVI, Nearus) is retained for completeness but was **not** re‑validated in this patch set because those primary manuals were not included in the provided source bundle.


---

## 1. Scope & Intent

This guide captures the VISCA protocol and implementation details for several popular PTZ cameras across multiple manufacturers. It is intended as a unified reference for developers integrating these systems. We provide both the standard VISCA commands and highlight important differences and nuances across various camera models. By consolidating information for Sony, PTZOptics, and other VISCA-speaking devices (including clones by Marshall, AVer, Avonic, Minrray, etc.), this guide offers practical guidance for implementing control software that can accommodate multiple VISCA-capable cameras. The focus is on VISCA commands (over serial and IP), how they behave on different hardware, and best practices to handle these variations.

---

## 2. Terminology

We use standard VISCA terminology throughout this guide. The **controller** refers to the device or software sending commands (always address 0), while the **peripheral** is the camera receiving them (address 1–7 in serial networks, or address 1 in IP mode). Each camera maintains two command *sockets* (buffers) to process up to two commands in parallel. The camera responds to commands with specific reply messages: an **ACK** to acknowledge receipt, a **Completion** when the action is done, or an **Error** if something goes wrong. The table below defines these terms:

| Term                    | Meaning                                                                                                                       |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| **Controller**          | VISCA command originator (serial address `0`; IP source ID `0x00`).                                                           |
| **Peripheral / Camera** | VISCA responder (serial IDs `1‑7`; in IP mode the camera is always ID `1`).                                                   |
| **Socket / Buffer**     | One of two concurrent command execution slots in every Sony-derived PTZ camera.                                               |
| **ACK**                 | Reply `90 4y FF` – command accepted into socket `y` (y = 1 or 2).                                                             |
| **Completion**          | Reply `90 5y FF` – command finished; socket `y` is now freed.                                                                 |
| **Error**               | Reply `90 6X YY FF` – error indication with code `YY` (`02` = syntax error, `03` = buffer full, `41` = not executable, etc.). |

These codes and semantics are defined by the VISCA protocol. For example, `90 60 03 FF` (Error with code `03`) means the camera’s command buffers were full when a new command arrived, so it was rejected.

---

## 3. Physical & Transport Layers

### 3.1 Serial (RS‑232/RS‑422)

VISCA was originally designed for direct serial control, with support for daisy-chaining multiple cameras. Each camera on a serial chain is assigned a unique ID (1–7) and the controller uses ID 0. Commands are sent over RS-232 (typically in a single-chained configuration) or RS-422 (for longer runs or multi-drop setups). Many PTZ cameras provide an 8-pin mini-DIN (Sony standard) or DE-9 (PTZOptics) connector for serial control. The default communication settings are 9,600 or 38,400 bps, 8-N-1 framing. VISCA commands are framed by an address byte (`8x`, where *x* is the camera ID or 8 for broadcast) and a terminator `FF`. The payload can be up to 14 bytes, making the total command packet at most 16 bytes long. Serial VISCA is reliable and inherently ordered due to the point-to-point (or daisy-chain) connection.

| Connector                             | Baud Rate                 | Frame Format                     | Addressing                        |
| ------------------------------------- | ------------------------- | -------------------------------- | --------------------------------- |
| Mini-DIN‑8 (Sony) or DE‑9 (PTZOptics) | 9,600 / 38,400 bps, 8‑N‑1 | `8x ... FF` (1–14 payload bytes) | Controller = `0`, Cameras = `1‑7` |

**Broadcast commands:** ID `8x` with x=8 is reserved for broadcast. Two special broadcast messages, **Address Set** and **I/F Clear**, are used in serial networks (see §3.3).

### 3.2 VISCA-over-IP

VISCA commands can also be sent over an IP network (Ethernet) using UDP or TCP. However, unlike the standardized serial interface, manufacturers have implemented two incompatible approaches for VISCA‑over‑IP.

#### 3.2.1 Two Incompatible “Flavors”

There are two distinct flavors of VISCA‑over‑IP in use:

* **Raw VISCA over IP:** No additional header; the camera expects the same byte sequence as it would over serial. This is used by PTZOptics and many other non-Sony brands.
* **Encapsulated VISCA over IP:** An 8‑byte header is prepended to the VISCA byte frame. Sony and some Sony-compatible implementations (Marshall, AVer, Avonic in Sony mode) use this format.

These methods are not interoperable. The table below summarizes their characteristics:

| Camera Family / Model                              | Envelope Required?                              | Sequence Number?           | Default Port(s)                                   |
| -------------------------------------------------- | ----------------------------------------------- | -------------------------- | ------------------------------------------------- |
| **Sony** PTZ (BRC, SRG, ILME-FR7, etc.)            | **Yes** – 8‑byte header                         | 32‑bit counter (wraps)     | UDP 52381 (Sony default)                          |
| **PTZOptics** (all models/firmware through 2025)   | **No** – *raw VISCA bytes* (direct byte stream) | *Not used* (no header)     | UDP 1259 (also TCP 5678)                          |
| **“Hybrid” Brands** (Marshall, AVer, Avonic, etc.) | Yes (configurable on some)                      | Yes (if using header mode) | UDP 52381 (when header on) or UDP 1259 (raw mode) |

**Important:** PTZOptics documentation (Rev 1.2, Aug 2020) makes no mention of any header or sequence number. Network traces and user reports confirm that PTZOptics cameras consume the standard VISCA byte stream directly over IP with no extra header. In practice, sending a Sony-style 8-byte header to a PTZOptics camera will result in the packet being ignored entirely (no response). Conversely, a Sony (or Marshall/Avonic in Sony mode) expects the header; if you send raw bytes without it, those cameras won’t recognize the command.

#### 3.2.2 Sony Encapsulated Header Format (UDP port 52381)

Sony’s VISCA‑over‑IP protocol wraps the command bytes in an additional header and is typically carried over UDP on port 52381. In this mode, each command, inquiry, and reply is prefixed by an 8‑byte header that provides message type, length, and a sequence number for tracking. The structure of a UDP packet carrying a VISCA command is as follows:

```
┌─────────────── 8 bytes ───────────────┐┌──── VISCA frame (≤16 bytes) ─┐
│ Payload-Type │ Length │ Sequence-No. ││ 8x … <payload> … FF │
└──────────────────────────────────────┘└──────────────────────────────┘
```

* **Payload Type:** 2 bytes identifying the message category (e.g., `0x01 0x00` for a Command, `0x01 0x10` for an Inquiry, `0x01 0x11` for a Reply/ACK/Completion). Sony defines several such types.
* **Length:** 2 bytes (16-bit big-endian) giving the number of bytes in the VISCA command frame (from the `8x` address byte up to the terminating `FF`). For example, a 7-byte VISCA command would have a length field `0x0007`.
* **Sequence No.:** 4 bytes (32-bit) sequence counter. The controller should increment this for each message (typically starting from 1 or 0). The camera will echo the same sequence number in its reply to allow matching.

For example, the raw VISCA inquiry for power status is `81 09 04 00 FF`. Encapsulated with a sequence number 0, it becomes `01 10 00 05 00 00 00 00 81 09 04 00 FF`. Here `0x01 0x10` indicates an inquiry command, `0x0005` length, and `0x00000000` sequence.

To implement Sony encapsulation when sending a command:

1. Set the **Payload Type** – e.g., `0x01 0x00` for a command or `0x01 0x10` for an inquiry (Sony’s protocol documentation defines these and other types).
2. Calculate the **Length** as the number of bytes in the VISCA frame (excluding the header). For instance, command `81 01 04 3F 02 01 FF` (recall preset 1) is 7 bytes, so length = `0x0007`.
3. Assign a **Sequence Number** (32-bit). Increment your sequence counter each time. Modern cameras like the FR7 use all 32 bits; older models effectively used only the lower 16 bits (they ignored the upper half of the sequence field).
4. Construct the UDP packet: an 8-byte header (with fields above in network byte order) followed by the VISCA command bytes. Send this to UDP port 52381 on the camera.

The camera will respond with its own packet containing the same sequence number and a payload type indicating a reply. For an **ACK** or **Completion** from the camera, the payload type will be `0x01 0x11` (Reply) and the sequence No. will match the command, so you can correlate the response.

**Note:** In IP mode, the VISCA device address in the command frame is always 1 (use `0x81` as the address byte for commands, `0x81` or `0x88` for inquiries/broadcast). The controller (source) is always 0, which is implicitly understood in IP mode (the source ID in the header is separate from the VISCA address byte, which should be 0x81).

#### 3.2.3 Retransmissions & Duplicates (Sony IP Mode)

In UDP networks, packets can be lost or delayed. The VISCA-over-IP protocol relies on the controller to handle delivery confirmation. In practice, if the controller does not receive an **ACK** from the camera within about 2 video frames (\~33 ms), it should consider the command lost and retransmit the packet (this guidance comes from Sony documentation and ensures reliability over UDP). **Important:** When retransmitting, always use a **new sequence number** for the re-send. The camera uses sequence IDs to detect duplicates. If a command with the *same* sequence is received twice, the camera will treat it as a duplicate and may respond with an error (some Sony cameras return an “abnormal sequence” error in this case) rather than executing it again. Using a new sequence on a retry tells the camera it’s a fresh command.

Sony documentation explains that if a camera receives a duplicate sequence number, it will not re-execute the command; at most, it might re-send a reply (or an error indicating a sequence issue). As a controller developer, you should be prepared to handle either scenario. A common strategy is to set a retry limit (e.g., retry a command up to 3 times with new sequence IDs). If no ACK or Completion is received after several attempts, assume the camera is offline or unreachable, and alert the user or attempt a reconnection.

This retransmission logic mainly applies to the Sony-style UDP protocol. In **raw VISCA-over-IP** (e.g., PTZOptics on UDP 1259), there is no sequence field to help with duplicate detection. You may still implement a timeout and retry if an ACK isn’t received, but note that the camera cannot distinguish a retry from a new command in raw mode. In practice, many integrators using raw UDP rely on the inherent reliability of a local network or use PTZOptics’ TCP control port (TCP 5678) to avoid packet loss issues. Using TCP ensures commands arrive in order without loss, though you should still adhere to the two-command concurrency rule at the application level (see §4).

### 3.3 Serial Address Initialization – **Address Set** & **I/F Clear**

When using serial VISCA with daisy-chained cameras, the network must be initialized so each camera knows its ID. Two special broadcast commands, **Address Set** and **I/F Clear**, are used for this:

| Command         | Bytes (broadcast format) | Purpose                                                                                                                                                                              | When to send                                                           |
| --------------- | ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------- |
| **Address Set** | `88 30 01 FF`            | Daisy-chain address assignment. Each camera on the chain picks an ID (1…N) in order. The last camera returns `y0 50 0N FF` (to broadcast) indicating how many cameras were assigned. | Once at start-up, or after a “Network Change” notification (hot-plug). |
| **I/F Clear**   | `88 01 00 01 FF`         | Clears command buffers in all devices, cancels any motions, and resets sockets (interface clear).                                                                                    | Immediately after Address Set, or to recover from errors.              |

> **Tip (Serial only):** Always send **Address Set** then **I/F Clear** after power‑up or when cameras are reconnected, so that every camera knows its ID and the command state is clean. The Address Set is a broadcast (device 8) that triggers each camera to assign itself an address. The final reply (`88 50 0N FF`) tells the controller how many cameras responded (N). Then an I/F Clear (also broadcast) resets all cameras’ command buffers and cancels any ongoing actions. These commands are **not used** in VISCA‑over‑IP (IP devices are fixed at ID 1 and there is no concept of dynamic addressing over IP).

---

## 4. Transaction & Timing Model

### 4.1 Command vs Inquiry Lifecycles

VISCA differentiates between **commands** (which change state or perform actions) and **inquiries** (which request information). They follow slightly different message sequences:

* **Command** (e.g., Pan, Zoom, Preset Recall): The controller sends a packet starting with `8x 01 ... FF` (where x is the camera ID, or 1 for IP). The camera immediately replies with an **ACK** (`9x 4y FF`) indicating it accepted the command into socket *y*. Once the action is completed, the camera sends a **Completion** (`9x 5y FF`) for that socket. If an error occurs that prevents completion, an **Error** reply (`9x 6y EE FF`) is returned instead of a normal completion.
* **Inquiry** (e.g., Zoom Position, Power Status): The controller sends `8x 09 ... FF` (note the `09` in place of `01`). The camera does **not** issue an ACK for inquiries. Instead, it directly returns a **Data Reply** (`9x 50 ... FF`) containing the requested information. Essentially, inquiries use a “socket 0” which doesn’t occupy the normal command buffers. If the inquiry cannot be processed or is not supported, an Error (`9x 60/6y EE FF`) is returned (with y=0 for inquiry errors).

The table below summarizes these flows:

| Phase                  | Command (`→`) Sequence                                     | Inquiry (`?`) Sequence                                     |
| ---------------------- | ---------------------------------------------------------- | ---------------------------------------------------------- |
| Controller → Camera    | `8x 01 … FF` (command packet sent)                         | `8x 09 … FF` (inquiry packet sent)                         |
| Camera immediate reply | **ACK** `9x 4y FF` (accepted into socket y)                | *(no ACK for inquiries)*                                   |
| Camera final response  | **Completion** `9x 5y FF` (action done, socket freed)      | **Data Reply** `9x 50 <data…> FF` (answer)                 |
| Errors (if any)        | `9x 6y EE FF` (Error with code EE, in place of Completion) | `9x 60 EE FF` or `9x 6y EE FF` (error with y=0 if inquiry) |

**Note:** All responses from the camera start with `90` in practice (for a single camera ID 1, `9x` becomes `90` since x+8 = 9 for device 1). The `y` in replies is the socket number (1 or 2) for commands; inquiries use `y=0` in their replies. Common error codes `EE` include `02` (syntax error), `03` (buffer full), `04` (command canceled), `05` (no socket to cancel), `41` (command not executable).

### 4.2 Return Messages & Error Codes

All VISCA cameras enforce a **two-socket concurrency** limit for commands. In other words, the camera can process at most two commands at the same time (one in each socket). If a third command is sent while two are still in progress, the camera will respond with a “Command Buffer Full” error (`... 60 03 FF`) and it will not execute that third command. Your controller implementation should prevent this by queuing or delaying additional commands until a socket frees up.

When a camera accepts a command, it replies with an **ACK** (`90 4y FF`) – indicating the command is queued in socket *y*. Once processing is finished, a **Completion** (`90 5y FF`) is sent and socket *y* becomes free for reuse. If a command cannot be executed due to the camera’s current state, the camera will send an **Error** reply in place of a normal completion. For instance, sending a manual focus command while the camera is in auto-focus mode will yield an error `90 6y 41 FF` (“Command Not Executable”) after the ACK. In that case the command is ignored. Controllers can handle this by informing the user or by automatically adjusting the camera’s mode (e.g., send a command to switch to manual focus before issuing focus movements, if an auto-focus mode is detected).

Common VISCA replies and error codes include:

| Reply / Error                  | Hex Pattern    | Meaning and Usage                                                                                      |
| ------------------------------ | -------------- | ------------------------------------------------------------------------------------------------------ |
| **ACK**                        | `90 4y FF`     | Command accepted into socket *y*.                                                                      |
| **Completion**                 | `90 5y FF`     | Command finished successfully; socket *y* is now free.                                                 |
| **Data Reply**                 | `90 50 ... FF` | Inquiry result (contains requested data bytes; no prior ACK was given for inquiry).                    |
| **Syntax Error**               | `90 60 02 FF`  | Command was unrecognized or had illegal parameters.                                                    |
| **Command Buffer Full**        | `90 60 03 FF`  | Both command sockets are busy; command rejected.                                                       |
| **Command Canceled**           | `90 6y 04 FF`  | A running command in socket *y* was canceled; no completion will follow.                               |
| **No Socket (Cancel Invalid)** | `90 6y 05 FF`  | A “Cancel” was issued for an empty socket (no command to cancel there).                                |
| **Command Not Executable**     | `90 6y 41 FF`  | Command cannot execute under current conditions (e.g., focus command in AF mode, or camera is “busy”). |

Some models exhibit additional timing nuances. For example, the **Sony EVI‑H100** and **BRC‑H900** cameras require \~240 ms after completing a preset recall before they are ready to accept new commands (even though a Completion is sent). If you send a command immediately after a preset movement on these models, you might get a `... 41 FF` “Not Executable” error because the camera is still busy adjusting internally. The proper approach is to either delay subsequent commands briefly or catch the error and retry after a short delay. (Sony notes that a *Command Not Executable* error may occur for up to 240 ms after a preset recall on these models, and that the controller should resend the command in that case.)

VISCA also supports a **Command Cancel** function. For example, `81 21 FF` cancels the command in socket 1, and `81 22 FF` cancels socket 2. When a cancel is issued, the camera will respond with `90 6y 04 FF` (y = socket) “Command Canceled” for the affected socket (and no completion message will follow for that command). If no command was in that socket, a `90 6y 05 FF` “No Socket” error is returned. Canceling commands is optional in most applications, but it can be useful for emergency stops or user-interrupt actions. (Note: Sony recommends waiting at least 200 ms after sending a pan/tilt command before issuing a cancel for it, to ensure the move has actually started, and similarly waiting \~200 ms after a cancel before sending a new pan/tilt command. This prevents race conditions where a cancel might be missed or a new move command ignored.)

---

## 5. Common Inquiry Commands (Supported by Most VISCA Cameras)

All inquiry commands use the byte `09` in place of `01` in the command sequence. Replies use the `50` response code and contain the requested data. The table below lists common inquiry commands supported by virtually all VISCA-capable PTZ cameras, along with the format of their replies and the meaning of the returned data. (If a camera doesn’t support a particular inquiry, it typically responds with a `... 41 FF` *Not Executable* error when that inquiry is sent.)

| Category          | Inquiry Name   | Bytes (Controller → Camera) | Typical Reply Format (Camera → Controller) | Data Meaning                                                                   | Supported on: Sony / PTZOptics / FR7           |
| ----------------- | -------------- | --------------------------- | ------------------------------------------ | ------------------------------------------------------------------------------ | ---------------------------------------------- |
| **Power**         | `PowerInq`     | `8x 09 04 00 FF`            | `9x 50 0p FF` (p=`2` or `3`)               | Power status: 02 = On, 03 = Standby (Off)                                      | ✔ / ✔ / ✔ (all support)                        |
| **Zoom**          | `ZoomPosInq`   | `8x 09 04 47 FF`            | `9x 50 0p 0q 0r 0s FF`                     | 16-bit zoom position (optical + digital)                                       | ✔ / ✔ / ✔                                      |
| **Focus**         | `FocusModeInq` | `8x 09 04 38 FF`            | `9x 50 0p FF` (p=`2` or `3`)               | Focus mode: 02 = Auto, 03 = Manual                                             | ✔ / ✔ / ✔                                      |
|                   | `FocusPosInq`  | `8x 09 04 48 FF`            | `9x 50 0p 0q 0r 0s FF`                     | 16-bit focus lens position                                                     | ✔ / ✔ / ✔                                      |
| **Pan/Tilt**      | `PTPosInq`     | `8x 09 06 12 FF`            | `9x 50 PP PP PP PP TT TT TT TT FF`         | Pan & tilt coordinates (see §8.2 for format)                                   | ✔ / ✔ / ✔                                      |
| **Exposure**      | `AEModeInq`    | `8x 09 04 39 FF`            | `9x 50 0p FF` (00/03/0A/0B/0D)             | Exposure mode: 00=Full Auto, 03=Manual, 0A=Shutter Pri, 0B=Iris Pri, 0D=Bright | ✔ / ✔ / ✔ (FR7: Bright mode not present)       |
|                   | `ShutterInq`   | `8x 09 04 4A FF`            | `9x 50 0p 0q 0r 0s FF`                     | Shutter speed setting (as index value)                                         | ✔ / ✔ / ✔                                      |
|                   | `IrisInq`      | `8x 09 04 4B FF`            | `9x 50 0p 0q 0r 0s FF`                     | Iris (aperture) setting (as index value)                                       | ✔ / ✔ / ✔                                      |
|                   | `GainInq`      | `8x 09 04 4C FF`            | `9x 50 0p 0q 0r 0s FF`                     | Gain level setting (as index, 0 = 0dB, higher = +dB)                           | ✔ / ✔ / ✔                                      |
| **White Balance** | `WBModeInq`    | `8x 09 04 35 FF`            | `9x 50 0p FF` (00/01/02/04/05/20)          | WB mode: 00=Auto, 01=Indoor, 02=Outdoor, 04=ATW, 05=Manual, 20=One-push Temp   | ✔ / ✔ / ✔ (FR7: uses Memory A/B instead of 20) |
|                   | `ColorTempInq` | `8x 09 04 20 FF`            | `9x 50 0p 0q FF`                           | Color temperature setting (for WB Color Temp mode)                             | ✔ / ✔ / ✖ (FR7 n/a)                            |
| **Tally**         | `TallyInq`     | `8x 09 7E 01 0A 00 FF`      | `9x 50 0p FF` (p=`2` or `3`)               | Tally lamp state: 02 = On (red), 03 = Off                                      | ✔ / ✖ / ✔\*                                    |
| **Information**   | `VersionInq`   | `8x 09 00 02 FF`            | `9x 50 VV VV MM MM FF FF KK FF`            | Firmware version info + model ID + socket count                                | ✔ / ✔ / ✔                                      |

\*Sony FR7 has separate red/green tally inquiries (`...7E 01 0A 00` for red, `...7E 04 1A 00` for green) since it has two tally lamps (see §9.2).

Virtually every VISCA camera supports these baseline inquiries. If a feature doesn’t exist on a model, the camera usually returns `90 6y 41 FF` (“Not Executable”) for that inquiry (for example, a camera with no tally lamp will error on a Tally inquiry). The **VersionInq** (`8x 09 00 02 FF`) is particularly useful as it returns the camera’s version and some capability flags (e.g., number of sockets). It can help identify the model or at least the manufacturer, which you can use to adjust behavior in your controller.

---

## 6. Baseline Command Set (Universally Recognized Commands)

The following core commands are recognized by all VISCA-compatible PTZ cameras covered by this guide. They handle essential functions like power control, zoom, focus, pan-tilt motion, and preset memories. Each command is given by its VISCA byte sequence and a brief description:

* **Power:** `81 01 04 00 02 FF` (Power On), `81 01 04 00 03 FF` (Standby/Power Off).
* **Zoom:**

  * `81 01 04 07 00 FF` – Stop zoom.
  * `81 01 04 07 02 FF` – Zoom Tele (standard speed); `... 07 03 FF` – Zoom Wide (standard speed).
  * `81 01 04 07 2p FF` – Variable speed zoom tele (p = 0–7, low to high speed); `... 07 3p FF` – Variable wide.
  * `81 01 04 47 0p 0q 0r 0s FF` – Direct zoom position (16-bit). For many cameras, `0x0000` = wide end, and `0x4000` = full optical tele.
* **Focus:**

  * `81 01 04 38 02 FF` – Auto Focus On; `81 01 04 38 03 FF` – Manual Focus mode.
  * `81 01 04 08 00 FF` – Stop focus.
  * `81 01 04 08 02 FF` – Focus Far (tele focus) continuous; `... 08 03 FF` – Focus Near continuous.
  * `81 01 04 08 2p FF` / `... 08 3p FF` – Variable speed focus Far/Near (p = 0–7).
  * `81 01 04 48 0p 0q 0r 0s FF` – Direct focus lens position (16-bit).
* **Pan-Tilt:**

  * `81 01 06 01 VV WW XX YY FF` – Continuous Pan-Tilt motion. `VV` = Pan speed, `WW` = Tilt speed, each 1–24 (or up to 50 on some models). `XX YY` = direction codes: e.g., `0x01 0x03` = pan left, `0x02 0x03` = pan right, `0x03 0x01` = tilt up, `0x03 0x02` = tilt down, `0x03 0x03` = stop (no motion). These direction byte values combine (e.g., `0x01 0x01` = up-left).
  * `81 01 06 02 VV WW 0p 0q 0r 0s 0t 0u 0v 0w FF` – Absolute Pan-Tilt move. Here `0p0q0r0s` is the pan coordinate and `0t0u0v0w` is the tilt coordinate (each 16-bit signed or unsigned depending on model; see §8.2). `VV`,`WW` are the pan/tilt speeds as above. This command moves the camera to the specified absolute position.
  * `81 01 06 03 VV WW 0p 0q 0r 0s 0t 0u 0v 0w FF` – Relative Pan-Tilt move (moves by the offset given). The position parameters here represent a relative signed displacement from the current position.
  * `81 01 06 04 FF` – Home (go to mechanical home position, usually center).
  * `81 01 06 05 FF` – Reset (reinitialize pan-tilt mechanism; often same as home on many models).
  * `81 01 06 07 00 FF` – Set current pan/tilt position as an upper or lower limit (this command is followed by specifying which limit to set – e.g., there are subcommands to set left, right, up, down limits; details vary by model).
  * `81 01 06 07 01 FF` – Clear all pan/tilt limit stops.
* **Preset Memory (Camera Positions):**

  * `81 01 04 3F 00 pp FF` – Reset (Delete) preset *pp*.
  * `81 01 04 3F 01 pp FF` – Set (Store) current position to preset *pp*.
  * `81 01 04 3F 02 pp FF` – Recall (Go to) preset *pp*.
    *(Here pp is the preset number, often in hex. Many cameras use 0-based numbering (0x00–0x0F or higher). Some protocols refer to preset 0 as “preset 1” in UI, so be mindful of off-by-one differences in interfaces.)*

These baseline commands are consistent across Sony VISCA and most third-party cameras that adopted VISCA. For example, PTZOptics cameras use the same codes for these functions, as do older Sony models. Appendix A provides a comprehensive list of opcodes.

---

## 7. Cross‑Model Capability Matrix

Different camera models vary in capabilities even though they share the VISCA protocol. The matrix below highlights key feature differences across select models. Understanding these differences is important when designing a controller for multiple cameras, as certain features (or ranges) may not be available on all devices. A checkmark (✔) indicates support, and a cross (✖) indicates lack of support. Bold text is used to highlight an especially notable value or limit. (Sony’s BRC‑H900 requires an **optional BRBK-IP10** interface card for IP control; otherwise it is serial-only.)

| Feature                     | PTZOptics Gen‑2 (SDI/NDI)                       | Sony ILME‑FR7                           | Sony BRC‑H900                           | Sony EVI‑H100              | Nearus BRC‑300 (Sony OEM) |
| --------------------------- | ----------------------------------------------- | --------------------------------------- | --------------------------------------- | -------------------------- | ------------------------- |
| **VISCA over IP**           | **Raw** UDP 1259 / TCP 5678 (no header)         | Encapsulated UDP (port 52381)           | Encapsulated UDP\* (52381, via IP card) | — (Serial only)            | — (Serial only)           |
| **Preset slots**            | **127** G2 / **255** G3 / **10** (IR remote)    | 100 (0–99)                              | 16 (0–15)                               | 6 (0–5)                    | 6 (0–5)                   |
| **Pan speed steps**         | 1–24 (std. VISCA range)                         | **1–50** (supports “Fine” mode)         | 1–24                                    | 1–24                       | 1–24                      |
| **Focus Lock command**      | ✔ (`81 0A 04 68 02/03 FF`)                      | ✖ (no separate lock; uses AF/MF toggle) | ✖                                       | ✖                          | ✖                         |
| **“Snap” Focus (One-push)** | ✔ (`81 01 04 38 04 FF` triggers one-shot focus) | ✔ (Push AF commands)                    | ✖                                       | ✖ (older models lack this) | ✖                         |
| **ND Filter control**       | ✖ (no ND filter)                                | ✔ (Elec. ND 2–7 stops, 1/4 to 1/128)    | ✖ (no ND; uses optical only)            | ✖                          | ✖                         |
| **Dual Tally lamps**        | Single (one lamp: On/Off/Flash)                 | **Two** (Red & Green, independent)      | Single (red, Hi/Lo brightness)          | ✖ (no tally)               | ✖                         |
| **“Bright” AE mode**        | ✔ (Bright mode available)                       | ✖ (not supported on FR7)                | ✔ (Bright mode supported)               | ✔ (Bright mode supported)  | ✔ (Bright mode supported) |
| **ATW (Auto Trace WB)**     | ✖ (no ATW mode)                                 | ✔ (“ATW” mode available)                | ✖ (no ATW)                              | ✖                          | ✖                         |
| **WB Color Temp mode**      | ✔ (WB mode 0x20 = Color Temp)                   | ✖ (FR7 uses Memory WB A/B instead)      | ✔ (Color Temp WB mode)                  | ✔ (Color Temp WB mode)     | ✖                         |
| **Image Processing**        | Brightness, Luminance, Contrast, Gamma, Sharpness, Saturation, Hue, 2D/3D NR | Brightness, Contrast, Sharpness, Saturation | Brightness, Contrast, Sharpness | Gamma, NR, Brightness, Sharpness | Brightness, Sharpness |

\* Sony BRC‑H900 IP control requires the BRBK-IP10 option; without it, only RS-232/422 control is available.

As seen above, the newer FR7 introduces unique features like a variable electronic ND filter and dual tally lights, not found on older models. PTZOptics has some custom commands (like Focus Lock and Snap Focus) not present on Sony cameras. The number of preset memory slots varies widely: from 6 on entry-level models up to 100+ on newer cameras. PTZOptics NDI|HX Gen‑2 documentation lists **127 presets** (0–127) available via **serial/IP control**, while Gen‑3 models support **255 presets**; the included **IR remote supports 10 presets (0–9)**. When developing a controller, use the profile's `MAX_PRESETS` value and adjust the UI accordingly – for example, disable ND controls for models without ND filters, limit the preset index range offered to the user, or hide options like ATW or dual tally unless the camera supports them.

*(Note: The Sony BRC-X1000 4K camera (2017) is similar to FR7 in many respects and supports up to 100 presets. The older Sony BRC-300 had only 6 presets accessible via remote/serial. Always check model specs.)*

---

## 8. Parameter Ranges & Units

When controlling zoom, pan/tilt, and exposure, it’s important to understand each model’s numeric ranges and scaling factors. VISCA parameters are typically given as big-endian hexadecimal values, which correspond to physical positions or settings. The subsections below detail the ranges for zoom, pan/tilt coordinates, and exposure settings, so you can ensure your commands stay within valid limits for each device.

### 8.1 Zoom (Optical and Digital)

VISCA uses a 16-bit value to represent the zoom lens position. The minimum value (`0x0000`) corresponds to the widest angle (fully zoomed out). Each camera defines its maximum **optical zoom** position according to its lens. Values beyond that can be used for **digital zoom** (if digital zoom is enabled on the camera).

Notably, many Sony cameras (and those following Sony’s convention) standardize the optical range to end at `0x4000` at full telephoto, regardless of actual zoom ratio. The range from `0x4001` up to the camera’s limit represents digital zoom. For example:

* **Sony 20× Optical PTZs (like BRC-H900, EVI-H100):** `0x0000` = wide end (1x). `0x4000` ≈ 20× optical tele end. If 12× digital zoom is enabled, the value can go up to around `0x7AC0` (which represents \~240× total zoom).
* **PTZOptics 20× SDI (Gen2):** Uses a similar scale. `0x0000` = wide, `0x4000` = 20× optical tele. With digital zoom (up to \~4× on that model), maximum value is around `0x7800` (for \~80×). PTZOptics documentation indicates these values but internal testing or ZoomPosInq can reveal exact cutoff.
* **Older 12× Optical models (Sony BRC-300, EVI-D70 etc.):** Sony still used 0x4000 as the 12× optical tele end (even though 12× is less than 20×, they stretched the scale). With 4× digital, the max was 0x7FFF (approx 48× total) for those models.

You can always obtain the current zoom position via the `ZoomPosInq` and see what values the camera returns at full wide and full tele to determine its scaling. When setting zoom via a **Zoom Direct** command, ensure the value does not exceed the camera’s maximum. Many cameras will ignore or clamp values beyond their supported range. (If a command is out of range, some cameras might return a `90 60 02 FF` syntax error or just do nothing.)

### 8.2 Pan / Tilt Coordinates

VISCA defines pan and tilt coordinates as 16-bit values, but **two different conventions** are used across models:

* **Unsigned Range (0x0000–0xFFFF):** Used by older Sony models like the BRC-300 (and clones like Nearus 300). Here, `0x0000` corresponds to one extreme (e.g., far left or top) and `0xFFFF` the opposite extreme (far right or bottom). The midpoint (\~0x7FFF or 0x8000) is roughly center, but note there is no explicit “zero” code – the center is effectively around 0x8000.
* **Signed Centered (Two’s Complement) Range:** Used by most modern cameras (Sony BRC-X series, SRG, FR7, and also PTZOptics, AVer, etc.). In this scheme, the value is interpreted as a signed 16-bit number where `0x0000` represents the center position (0). Values increase positively in one direction (right pan, down tilt) and decrease (under two’s complement) for the opposite direction (left pan, up tilt). For example, `0x0000` = center, `0x2200` might be far right, and `0xDE00` (which is -0x2200 in two’s complement) is far left.

On cameras using the signed scheme, an inquiry will often return values above 0x8000 for one side and below 0x8000 (interpreted as negative) for the other. For instance, an Axis documentation (for a Sony-based PTZ) gives an example: pan range min `0xDE00` (interpreted as -0x2200), max `0x2200`; tilt range min `0xFC00` (=-0x0400), max `0x1200`. In that example, center is 0x0000, and the mechanical range is asymmetrical (tilt up -10° vs tilt down +18° approximately).

In practice:

* **PTZOptics 20× (Gen2):** Pan \~±170°, Tilt -30° (up) to +90° (down). Center returns `0x0000`. A hard right might be around `0x2200`, hard left `0xDE00` (approx values). Tilt down (90°) might be around `0x1200`, tilt up (30°) around `0xF380` (just an example). PTZOptics reports these via inquiries – you can verify exact values with `PTPosInq`.
* **Sony ILME‑FR7:** Pan ±170° (similar coding, ±0x2200 roughly). Tilt range is unusual: -30° to +210° (when inverted mounting, effectively +210 downwards). Sony actually uses more than 16-bit internally for FR7 (20-bit internally) to accommodate the extended range, but via VISCA they still accept a 16-bit value (the extra range is mapped into that). You can effectively treat it as signed as well; any command beyond physical limits is simply clamped.
* **Sony BRC‑300 / Nearus 300:** Pan ±170° but coded as 0x0000 (left) to 0xFFFF (right). So center is \~0x8000. Tilt -30° to +90° also spans 0x0000 to 0xFFFF. You have to calibrate moves based on those extremes. Notably, these older cameras won’t accept negative values in commands (they expect the full 0–FFFF range instead).

**Controller Design Tip:** Implement an abstraction for pan/tilt positions. For example, work in degrees or a normalized -1.0 to +1.0 range internally, and translate to the camera-specific hex. That way, whether a camera expects signed or unsigned, you handle it transparently. For signed models, 0.0 -> 0x0000; for unsigned models, 0.0 -> 0x7FFF (approx). Also account for image flip settings: when a camera is mounted inverted and image flip is on, some models adjust the sign convention for tilt (e.g., tilting “up” might require a positive value instead of negative after flip). Usually, the VISCA protocol itself remains the same (numbers still mean physical directions), but your concept of what is “up” vs “down” might invert with the image.

Finally, if you issue an absolute move that is out of range, cameras typically do the nearest they can (or ignore if far off). There is no specific error for “position out of range” – the camera just won’t go beyond its end stops.

### 8.3 Exposure Parameters (Shutter, Iris, Gain, etc.)

Exposure settings (shutter speed, iris aperture, gain, exposure compensation, etc.) are quantized into discrete steps in VISCA. Most values have standardized ranges, though some models extend them. Key points:

* **Shutter Speed:** Typically 21 to 24 steps on most cameras. Represented as a 16-bit value in inquiries, but effectively an index (not directly in units of seconds). For example, on many Sony cameras: 0x0000 = 1/60 (or 1/50 PAL), 0x0001 = 1/100, ... up to fastest 1/10000 or 1/10000+ (like 0x0014). The exact mapping of index to speed can vary by model (and base framerate). Consult the camera manual for the conversion table. (E.g., index 0x000A often corresponds to 1/1000 sec on Sony.)
* **Iris (Aperture):** Usually around 18 steps from open to close. 0x0000 = F1.6 (wide open on many integrated-lens cameras), and the highest value corresponds to “Close” (no light). For instance, 0x000C might be F8, 0x0010 = Close on some models. An FR7 with an interchangeable lens will quantize the iris differently depending on lens, but via VISCA it still reports a 16-bit value (often 0–255 range for lens with continuous iris).
* **Gain:** Often 0 to 0x0F (0–15) for 0dB to 30dB (2dB per step). Some cameras allow higher gains (e.g., FR7 in high-gain mode or 36dB, which might use values beyond 0x0F). The VISCA inquiry will tell you the exact hex value currently. Just note the scale can change if a camera is in ISO mode (like FR7 CineEI mode locks gain).
* **Exposure Compensation (EV Compensation):** Most cameras: ±7 steps (15 levels total). Represented as 0x0E for +7, down to 0x00 for -7 (with 0x07 being zero). Some newer cameras extended this. For example, the FR7’s Cine EI mode allows a wider EV adjustment range (±9 or more, potentially mapping 0x00 to -9 and 0x12 to +9). Check model docs; if you send a value out of range, you’ll likely get a 0x02 syntax error or it will clamp.
* **White Balance Color Temperature:** If the camera supports a “WB Color Temp” mode (on Sony this is WB mode 0x20), the color temperature is set via `04 20 0p 0q FF` (p,q form a 2-byte value). Typically range is 2500K to 8000K mapped linearly to 0x0000–0x0037 (0 = 2500K, 0x0037 ≈ 8000K). Sony models like BRC-H900, EVI-H100 support this. The FR7 doesn’t use WB mode 0x20 (it has Memory A/B presets instead), so it will likely return an error if you try it.

In summary, while VISCA provides a uniform way to set these parameters, the actual values correspond to model-specific scales. Use inquiries (`ShutterInq`, `IrisInq`, etc.) to get baseline readings. Also, many cameras have *absolute* modes (like “Bright Mode” which overrides Iris/Gain to a combined brightness value, or *Auto Slow Shutter* toggles). Ensure your controller checks which exposure mode is active before sending direct parameter commands (sending iris commands in Full Auto mode will yield a `41 FF` error – you must switch to Manual or Shutter-priority first).

Sony's official VISCA command lists (and some third-party docs like Axis) provide tables for these mappings. Refer to them for precise step definitions if needed.

### 8.4 Image Processing Parameters (Brightness, Luminance, Contrast, Gamma)

VISCA provides commands for adjusting image processing parameters that affect the camera's internal image pipeline. These are separate from exposure controls (Section 8.3) and are applied in post-processing.

* **Brightness (CAM_Bright, opcode `0x4D`):** Controls the camera's "Bright" level, which adjusts overall image brightness in Bright exposure mode. Set via `81 01 04 4D 00 00 0p 0q FF` (pq: brightness position). Inquiry: `81 09 04 4D FF`. Range varies by model (0x00–0x11 on PTZOptics G2, hardware-validated; 0x00–0x11 for Sony FR7/BRC-H900). *Note:* Opcode `0xA0` is documented in some sources but returns Syntax Error on PTZOptics G2 hardware.

* **Luminance (opcode `0xA1`):** Controls luminance (brightness curve) of the image output. Set via `81 01 04 A1 00 00 0p 0q FF` (pq: luminance position, 0x00–0x0E on PTZOptics). Inquiry: `81 09 04 A1 FF`. Response: `y0 50 00 00 0p 0q FF`. Supported on PTZOptics G2/G3/30X. *Note:* PTZOptics documentation labels this "Brightness Direct" but it uses a distinct opcode from exposure Bright (`0x4D`).

* **Contrast (opcode `0xA2`):** Controls the contrast ratio of the image. Set via `81 01 04 A2 00 00 0p 0q FF` (pq: contrast position, 0x00–0x0E on PTZOptics). Inquiry: `81 09 04 A2 FF`. Response: `y0 50 00 00 0p 0q FF`. Supported on PTZOptics G2/G3/30X and Sony models.

* **Gamma (opcode `0x5B`):** Selects the gamma correction curve for the camera's image output. Set via `81 01 04 5B 0p FF` (p: 0=Standard, 1–4=alternate gamma curves). Inquiry: `81 09 04 5B FF`. Response: `y0 50 0p FF`. Hardware-validated on PTZOptics G2 (undocumented in official PTZOptics manual but functional). Also supported on Sony EVI-H100 and similar models.

* **Sharpness (opcode `0x42`):** Controls image edge sharpness. Mode selection via `81 01 04 05 0p FF` (p: 2=Auto, 3=Manual). Direct level set via `81 01 04 42 00 00 0p 0q FF`. Range is model-dependent (0x00–0x0F on PTZOptics G2, hardware-validated; PTZOptics docs say 0x00–0x0B but camera accepts full range).

Other image processing parameters (saturation, hue, noise reduction) are also available but vary more significantly across models — see Section 9 for model-specific details.

---

## 9. Model‑Specific Extensions & Quirks

Many cameras implement additional VISCA commands beyond the baseline set, often to control features unique to that model. In a universal controller, these can be exposed conditionally for specific models. Below we list some notable model-specific commands and behaviors:

### 9.1 PTZOptics (Gen2 and NDI models)

PTZOptics cameras largely follow the standard VISCA command set (their firmware is VISCA-based). However, they added a few extensions:

* **Focus Lock:** `81 0A 04 68 02 FF` (Lock focus) / `81 0A 04 68 03 FF` (Unlock focus). When focus lock is on, the camera’s focus mechanism is held at its current position and any autofocus or manual focus commands are ignored. If you attempt a focus move while locked, the camera will return a `41 FF` *Not Executable* error (because it refuses to change focus). This is useful to temporarily prevent any focus changes.
* **Snap Focus (One-Push AF in Manual):** `81 01 04 38 04 FF`. This command, sometimes called “One Push Trigger” for focus, will trigger an autofocus operation once, even if the camera is in manual focus mode. After the focus operation completes (or times out), the camera remains in Manual focus mode. Essentially, it’s a way to quickly autofocus at a target, then remain in manual (so it won’t continue hunting). On the PTZOptics Move series, this is documented as “Snap Focus”.
* **Image Flip/Mirror:** PTZOptics provides a combined flip command. `81 01 04 A4 0p FF` controls mirroring/orientation. p = 0 (Off, normal), 1 (Horizontal Flip), 2 (Vertical Flip), 3 (Both H+V Flip). This single command replaces the separate Sony commands (`61` and `66` opcodes) used on some other models. Use this when mounting the camera inverted (ceiling) or if you need a mirror image.

  **Important:** PTZOptics G2/G3/30X cameras require using the combined flip command (0xA4) rather than legacy separate commands (0x61/0x66). The legacy commands may be accepted but not actually applied. The library automatically routes flip operations (`enable_flip`, `disable_flip`, etc.) through the combined command for PTZOptics profiles. To persist flip settings across power cycles, call `save_settings()` after changing flip settings.
* **Auto WB Sensitivity:** `81 01 04 A9 00 FF` (High), `... A9 01 FF` (Normal), `... A9 02 FF` (Low). This adjusts how aggressively the Auto White Balance reacts to scene changes. In some PTZOptics models, “High” makes WB change more rapidly (useful for fast lighting changes), while “Low” dampens the response for stability.
* **Multicast Streaming On/Off:** `81 0B 01 23 01 FF` (Enable multicast), `81 0B 01 23 02 FF` (Disable) – specific to NDI models. This toggles the camera’s multicast video stream output. (Not a VISCA camera control per se, but implemented via the VISCA-over-IP interface on PTZOptics NDI cameras).
* **NDI|HX Mode Quality:** `81 0B 01 01 0p FF` – on NDI cameras, sets the NDI stream bandwidth/quality (p=1 High, 2 Medium, 3 Low, 4 Off). Again, not standard VISCA, but PTZOptics extends VISCA commands for some IP configuration settings.
* **Motion Sync Feature:** Some newer PTZOptics (e.g., firmware 1.1.6+ on certain models) have “PTZ Motion Sync”, which coordinates pan, tilt, and zoom to start and stop simultaneously for preset recalls. Commands like `81 0A 11 13 02 FF` (MotionSync On) / `... 13 03 FF` (Off) and `81 0A 11 14 pp FF` (set MotionSync max speed, pp = 0x01–0x18 for speeds 1–24) configure this. When MotionSync is on, recalling a preset will adjust the pan/tilt speeds so that zoom and pan/tilt movements complete at the same time, yielding a more synchronized and smooth arrival on the preset.

* **Image Processing:** PTZOptics G2/G3/30X cameras support a full set of image processing controls (see Section 8.4): Luminance (`0xA1`, range 0–14), Contrast (`0xA2`, range 0–14), Sharpness (`0x42`, range 0–11), Saturation (range 0–14), Hue (range 0–14), and 2D/3D Noise Reduction. Gamma curve control (`0x5B`, values 0–4) is also supported despite not being listed in the official PTZOptics VISCA command reference — hardware testing confirms both gamma inquiry and set commands work correctly.

*(Refer to the PTZOptics VISCA over IP Commands PDF for other vendor-specific codes. For example, some models have a "Preset Speed" setting command, some have OSD menu navigation via VISCA, etc. Most of these are beyond the basic Sony VISCA set.)*

**Note:** Older PTZOptics models (Gen1) had fewer presets and lacked some of these commands. If you query version (`90 50 ... KK FF` part of VersionInq), newer PTZOptics return a “model ID” that can be used to differentiate if needed. In general, Gen2 and later support all the above. The cameras will respond with syntax errors for unsupported commands.

### 9.2 Sony ILME‑FR7 (Full-Frame PTZ)

Sony’s FR7 (Cinema Line PTZ) extends VISCA in many ways for its advanced features:

* **Electronic ND Filter:** The FR7 has a built-in variable ND filter (2 to 7 stops, continuously variable). VISCA commands:

  * `8x 01 7E 04 52 0p FF` – Set ND mode. p = 0 (Preset mode), 1 (Variable mode). In Preset mode, the ND is set to discrete levels (which correspond to user-defined ND presets accessible via CGI or the camera menu). In Variable mode, you can adjust ND in fine steps.
  * `8x 01 7E 04 42 00 0p 0q FF` – Direct ND value. When in Variable mode, this sets the ND filter density directly. The value 0x0000 corresponds to ND 1/4 (i.e., 2 stops in, minimum ND) and 0x0014 corresponds to ND 1/128 (7 stops, maximum density). It’s effectively a linear scale for the optical density (each increment is \~0.5 stop).
  * `8x 01 7E 04 12 02 FF` – ND Filter Up (increase ND one step), `... 04 12 03 FF` – ND Filter Down one step. These bump the ND in small increments (works in Variable mode).
  * `8x 01 7E 04 53 02 FF` – Auto ND On, `... 04 53 03 FF` – Auto ND Off. When Auto ND is On, the camera will automatically engage the ND filter to maintain exposure (like auto-iris, but using ND). This works in conjunction with Auto Exposure modes.
* **Dual Tally Control:** FR7 has two tally lamps (one red, one green). Commands:

  * Red Tally: `8x 01 7E 01 0A 00 02 FF` (Red tally On), `... 0A 00 03 FF` (Red tally Off). Inquiry `8x 09 7E 01 0A 00 FF` returns `90 50 02 FF` or `03 FF` for on/off.
  * Green Tally: `8x 01 7E 04 1A 00 02 FF` (Green tally On), `... 1A 00 03 FF` (Green Off). Inquiry `8x 09 7E 04 1A 00 FF` similarly returns status.
    By design, the FR7 will automatically turn off a tally lamp after \~15 seconds if no repeat “On” command is received. This is a safety to prevent a stuck tally due to a dropped network packet. So if you need a continuous tally, your controller should resend the On command periodically (<15s intervals). (This auto-off behavior is specific to VISCA control; the hardware GPI/CGI tally controls do not time out.)
* **Pan/Tilt Speed Modes:** By default, like most Sonys, the FR7 supports 24 discrete speed levels for pan/tilt. It also has an option for 50 discrete speeds (Fine mode). Command `8x 01 7E 04 1B 02 FF` switches to 50-step speed mode; `... 1B 01 FF` switches back to 24-step mode. When 50-step mode is active, values 1–50 for pan/tilt speed are accepted (interpretation of the `VV` and `WW` bytes in movement commands changes accordingly).
* **Direct Menu Control:** The FR7 features an on-screen menu (like a camera GUI) which can be navigated via VISCA. Extended commands `7E 04 72 ...` allow simulation of button presses and dial turns. For instance, `81 01 7E 04 72 00 01 FF` might correspond to “Menu Open/Close”, and other values can navigate or adjust settings. (Sony provided a table in the FR7 manual for these codes – they are primarily useful if integrating with a hardware controller that wants to drive the camera’s menu remotely.)
* **Scene File & Picture Profile**: The FR7 can store “Scene Files” and has multiple Picture Profiles (like a Sony FX6). VISCA commands exist to recall scene files or switch picture profiles (e.g., `7E 04 3F 00 0p FF` might select Scene File p). These are fairly unique to the FR7 among VISCA cameras. Unless you need those, you might ignore them.
* **Record Control**: If an external recorder is attached (FR7 can output RAW to an Atomos), there are VISCA commands to trigger recording start/stop via Camera (in FR7, `CAM_RECORDING` parameter in Skaarhoj docs suggests press/release commands). This is outside normal PTZ but worth noting for completeness.

Given the FR7’s complexity, if you target it, it’s advisable to refer to Sony’s **ILME-FR7 “Network & VISCA Command Manual”** for the full list. We have covered the highlights here. The FR7 still supports all the baseline commands (power, zoom, focus, etc.) so it is backward compatible with basic VISCA controllers, but to unlock its full potential, your software can use these extended commands.

One more FR7 note: It does **not** support the legacy “Bright” exposure mode (0x0D) – that mode was a simplified AE on older models. If you send `...39 0D FF` on an FR7, it will return an error. Instead, FR7 has a “ISO/Gain” mode (Cine EI) which isn’t directly toggled via the old commands (it’s engaged by Scene file or API). Also, FR7’s Auto Exposure is sophisticated (it will auto-adjust ISO, iris, and shutter depending on settings).

### 9.3 Sony BRC‑H900 (and similar Sony BRC models)

* **Freeze (Image Hold):** `81 01 04 62 02 FF` (Freeze On), `81 01 04 62 03 FF` (Freeze Off). This freezes the camera’s video output on the last frame. It’s often used to avoid showing camera movement during a preset recall (the frame stays frozen until you unfreeze). The BRC-H900 and many other Sony PTZs support this. There is also a *preset freeze* mode in some cameras: `81 01 04 62 22 FF` (Preset Freeze On) means the camera will automatically freeze video when running a preset move, and then unfreeze when done. `... 62 23 FF` turns that off.
* **Tally Brightness (High/Low):** The BRC-H900’s tally lamp brightness can be controlled. Commands: `81 01 7E 01 0A 01 04 FF` (Set tally brightness to Low), `81 01 7E 01 0A 01 05 FF` (High). After sending one of these, subsequent `...0A 00 02 FF` (tally on) commands will illuminate at the chosen brightness. This is relatively unique to higher-end Sony models with a tally (the FR7 instead has two lamps rather than brightness levels).
* **IP Interface Card Notes:** If using the BRBK-IP10 with BRC-H900 (to get VISCA over IP), note that the default port might be 52381 (same as other Sony IP). Sony provided a PC tool to find cameras (the RM-IP Setup Tool). Additionally, that interface card responds to a broadcast discovery packet on port 52380 (not a VISCA packet, but a text-based “ENEQ” message). This is more of a one-time setup step rather than a VISCA command.

Other BRC-series cameras (e.g., BRC-X1000, BRC-X400) share many features with either the H900 or FR7. The BRC-X1000 (4K) supports freeze, has a tally, etc., and also uses the IP protocol with header (52381). The newer BRC-X400 (HDMI/IP 1080p model) is basically a rebranded SRG-X400 and also uses VISCA over IP with header and supports 100 presets. When in doubt, check that model’s VISCA manual; Sony typically reuses opcodes across families.

### 9.4 Sony EVI‑H100 and V-series (legacy EVI models)

* **Advanced Image Settings:** The EVI-H100 (and siblings like EVI-HD1, SRG-120, etc.) include VISCA commands for picture adjustments:

  * *Gamma:* `81 01 04 5B 0p FF` – p=0 Standard, 1–4 different gamma curves. Also hardware-validated on PTZOptics G2 (see Section 9.1). Inquiry: `81 09 04 5B FF`.
  * *High Resolution Mode / Visibility Enhancer:* `81 01 04 52 02 FF` (On), `... 52 03 FF` (Off) on models that have Wide-D or “Visibility Enhancer” (H100 has “High Resolution” which is a wide dynamic range mode).
  * *Noise Reduction:* `81 01 04 53 0p FF` – set NR level (p=0 Off, 1–5 levels).
  * *Flip/Mirror:* Some EVI models use separate horizontal flip (`61 02/03`) and vertical flip (`66 02/03`) commands. (These are the ones consolidated into A4 on PTZOptics.)
* **IR Remote Reporting:** A feature on some EVI cameras is “IR Return”. If enabled via a VISCA setting, whenever a user presses a button on the IR remote, the camera sends a VISCA message (an asynchronous notification) to the controller with the button code. This is rarely used, but it’s mentioned in manuals (to allow an integrator to know if someone manually changed something via remote).
* **VISCA Busy Quirk:** As noted earlier, the EVI-H100 can temporarily refuse commands right after certain moves. Sony documented that after a preset recall, the camera might return `41 FF` for up to 240 ms. Another scenario: if the on-screen menu is open (activated by `81 01 06 06 02 FF`), the camera will generally ignore movement commands or other camera controls (it will typically only listen to menu navigation commands). Thus, as a rule, ensure the OSD menu is closed (`... 06 06 03 FF` to close) before sending PTZ or focus commands. Some controllers do this automatically.

The EVI series was among the earliest VISCA PTZs and set the precedent; many later models maintain backward compatibility. Notably, the EVI-H100 supports only serial VISCA (no IP). If you need to control one remotely over IP, you’d use a serial server or converter.

*(Nearus BRC-300 is essentially identical to Sony BRC-300 in command set, so nothing new there aside from branding.)*

---

## 10. Behavioral Edge Cases & Testing Tips

Even with correct commands, cameras may exhibit certain behaviors that affect control. Below are various scenarios and how cameras are expected to behave, which double as recommended test cases for your implementation:

| Scenario                                          | Expected Behavior / Outcome                                                                                                                                                                                                                                                                                                                                                       |
| ------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Send 3rd command while 2 are in progress**      | Camera replies `90 60 03 FF` (*Command Buffer Full*) – the third command is rejected. Controller should queue it and retry when a slot frees.                                                                                                                                                                                                                                     |
| **Manual focus command while in Auto Focus**      | Camera replies `90 6y 41 FF` (*Not Executable*) – it ignores the focus command because it’s in AF mode. Solution: switch to Manual focus first, or handle the error (the camera remains in AF).                                                                                                                                                                                   |
| **Focus Lock active (PTZOptics) + focus command** | Camera replies `90 6y 41 FF` – focus is locked, so any attempt to change focus is not executable. Solution: unlock focus before issuing focus motor moves.                                                                                                                                                                                                                        |
| **FR7: send ND adjust during a Preset move**      | Camera likely replies `90 6y 41 FF` – during a preset motion (which itself might involve lens adjustments), certain commands are locked out. FR7 specifically will not take ND filter changes while a preset is running. Controller should wait for preset Completion before sending ND or exposure commands.                                                                     |
| **FR7: tally left on without refresh**            | \~15 seconds after the last `Tally On`, the lamp auto-extinguishes. If continuous tally indication is needed, resend the On command periodically (e.g., every 10 seconds). The auto-off is a camera feature to avoid stuck tally; your software should account for it to avoid confusion.                                                                                         |
| **EVI-H100: preset recall + immediate next cmd**  | Often the next command gets a one-time `41 FF` error (camera busy). Controller should catch that and retry after \~200 ms. This happens because the camera hadn’t fully settled from the preset internally. The second attempt usually succeeds (or the camera sends the command once it’s ready in some cases).                                                                  |
| **Mixing up header vs no-header (protocol mode)** | If a Sony-style header packet is sent to a raw-only camera (e.g., PTZOptics), **no response at all** will be received (camera ignores it). If raw bytes are sent to a Sony expecting header, likewise no response. This can lead to a false assumption that the camera is offline. Always ensure the correct IP mode is set. |

Testing these scenarios on each target model can help ensure your control software handles them gracefully. For example, intentionally overfill the command buffer (send three commands in quick succession) to verify your code handles the “buffer full” error by delaying and retrying. Or, try issuing an autofocus command when the camera is in manual to see if you get the expected error – and then use that to trigger an automatic mode switch in your logic (if desired).

In general, robust error handling (with informative user feedback) distinguishes a polished PTZ controller. VISCA errors are relatively informative; use them to guide the user (e.g., if you get “Not Executable” for a focus command, your UI could indicate “Camera is in Auto Focus – cannot adjust focus manually”).

---

## 11. Implementation Checklist for Developers

Finally, here’s a checklist of best practices and considerations when implementing a multi-camera VISCA control system. Ensuring each of these points is addressed will greatly improve compatibility and robustness:

1. **Manage Command Sockets (Two at a time):** Maintain awareness of the two command buffers per camera. Do not send a new command when both are occupied; either queue it or wait until one frees (indicated by a Completion). This prevents “Buffer Full” errors and ensures smoother operation.
2. **Unified Pan/Tilt Abstraction:** Hide the signed vs unsigned difference behind a common interface. For example, define pan/tilt in degrees: convert degrees to the appropriate VISCA hex for each model. Use the VersionInq (which returns model info) or a model selection to know which scheme to use. That way “pan to 0°” or “go to home position” works on all models without special-casing each time.
3. **Model Capabilities Map:** Internally, use the camera’s model or version info to enable/disable features. For instance, if a camera reports itself as “PTZOptics” (vendor ID in VersionInq), you can assume no VISCA header and maybe availability of Focus Lock. If it’s an FR7, enable ND controls, dual tally, etc. Create a small database of known models if possible (e.g., by parsing the model ID from VersionInq’s response or through user selection).
4. **Serial Initialization (Address/IF Clear):** If using serial control, always perform the Address Set (`88 30 01 FF`) and I/F Clear (`88 01 00 01 FF`) at startup (and handle the response from Address Set to know how many cameras are daisy-chained). This is essential for multi-camera serial chains. (Over IP, skip these – not applicable.)
5. **Discovery (IP cameras):** Consider implementing a discovery mechanism for cameras on the network. Sony cameras with an IP interface may support a broadcast query (e.g., sending a UDP packet to port 52380 as mentioned for the BRBK-IP10). Some third-party cameras might not, but you can at least allow manual entry of IP. Alternatively, use protocols like mDNS/SSDP if the cameras support them (many Sony cameras do not broadcast their presence, so this may be manual).
6. **UDP Retries and Timeouts:** For UDP connections (Sony or raw), implement a timeout for responses. If no ACK comes within, say, 100 ms, resend the command (with new sequence if Sony style). But also guard against receiving a very late ACK from the first attempt – your code should handle duplicate replies gracefully (perhaps by matching sequence numbers and ignoring duplicates). Having a reliable command send mechanism will greatly improve user experience on less-than-perfect networks.
7. **OSD Menu Handling:** Ensure that if a user might use the camera’s OSD menu (either via IR remote or a menu invocation command), your controller doesn’t get “stuck.” For instance, if your user opens the camera’s menu via IR, and then tries to send a pan command from your software, the camera might ignore it. One strategy is to periodically send an OSD Close (`81 01 06 06 03 FF`) if you detect no response to commands, in case the menu was left open. Or disable your control UI while the menu is open. Some cameras have an inquiry for OSD open status (FR7 does not, but some older ones had an “Information Display” toggle inquiry).
8. **Command Spacing & Sequencing:** Avoid “flooding” a camera with back-to-back commands, even if using the two sockets. Insert per-profile minimum delays between all sends (commands and inquiries) to avoid spurious `0x02 Syntax Error` responses and dropped completions. Hardware-validated spacing: **PTZOptics G2/G3/30X: 100 ms**, **Sony FR7/BRC-H900: 35 ms**. The library enforces this via the `MIN_COMMAND_SPACING` profile constant at the scheduler layer. Without pacing, cameras with small internal command buffers will intermittently fail under rapid-fire command sequences.
9. **Logging and Diagnostics:** Implement a verbose logging mode that records every byte sent and received (in hex) with timestamps. This is invaluable for debugging. If a camera isn’t doing what’s expected, the log will show if an error was returned or if no reply came. It also helps when seeking support from the camera manufacturer or community – you can share the exact command/reply sequence. Be mindful to disable verbose logs in production by default, but have it available for troubleshooting.

By following this checklist and the detailed guidance throughout this document, you should be well on your way to a robust unified VISCA implementation capable of orchestrating a diverse fleet of PTZ cameras. Each camera will have its peculiarities, but the core protocol consistency and a thoughtful software design will make those differences manageable.

---

## 12. Sources & References

* **PTZOptics – “VISCA over IP Command Set, Rev 1.2”** (Aug 2020) – Primary reference for PTZOptics command codes (focus lock, etc.) and confirmation that no header is used in their IP VISCA.
* **Sony – “VISCA Protocol Specification Version 1.0 & 2.0”** – Classic documentation for VISCA (covering BRC-300 and later updates). Includes command lists, error codes, and basic protocol info.
* **Sony – *EVI-H100S/H100V Technical Manual* (2012)** – Provided extended VISCA commands (gamma, DNR, etc.) and noted timing considerations (e.g., 240 ms preset delay).
* **Sony – *ILME-FR7 Network & VISCA Command Manual* (2024)** – Source for FR7-specific commands (ND filter, dual tally, 50-speed mode, etc.) and confirmation of 32-bit sequence usage.
* **Community Forums (vMix & OBS)** – Reports confirming behavior like tally auto-off and PTZOptics header-less operation.
* **Avonic Support – “Using VISCA over IP”** – Describes switching between raw and encapsulated VISCA (demonstrating an example of adding the 8-byte header and using port 52381).
* **Jon Skeet’s Coding Blog – “Variations in the VISCA protocol”** (Nov 2023) – Explains the two VISCA over IP flavors and some idiosyncrasies found when integrating different camera brands.
* **Axis Communications – *VISCA Interface API Description* (for Q-series PTZ)** – Confirms technical details like zoom value ranges (0x0000–0x4000 optical) and pan/tilt two’s complement ranges.
* **Epiphan Video – “VISCA command list for Lumio”** – Handy compiled list of VISCA commands, including Freeze and Preset Freeze commands not always clearly documented elsewhere.
* **Marshall Electronics – VISCA Commands for IP PTZ (CV730, 2020)** – Cross-verified Sony command compatibility (e.g., they use Sony header by default, and their pan/tilt range aligns with Sony’s signed scheme).
* **Provideocoalition – BRC-X1000 review** – Noted that newer Sony PTZs support 100 presets, aligning with FR7.
* **Manufacturer Manuals and Release Notes:** Various, including AVer tracking camera VISCA guides (for additional command codes), Vaddio documentation (largely Sony VISCA compatible), and PTZOptics firmware logs (for features like MotionSync).

*(The above references were used to verify protocol specifics and ensure accuracy. Inline citations 【】 have been provided throughout the text for specific claims.)*

---

### Appendix A – VISCA Command Opcode Table (CSV Format)

Below is a consolidated list of VISCA command opcodes, inquiries, and their support across models. This table can be used as a quick reference or for building lookup tables in software. (Note: `p, q, r, s` represent variable nibbles/bytes in the command.)

```csv
Category,Mnemonic,Opcode (Hex),Bytes,Sony,PTZOptics,FR7,Notes
Power,CAM_Power On,81 01 04 00 02 FF,6,Yes,Yes,Yes,
Power,CAM_Power Off (Standby),81 01 04 00 03 FF,6,Yes,Yes,Yes,
Power,PowerInq,81 09 04 00 FF,5,Yes,Yes,Yes,Reply 90 50 02/03 FF (On/Off)
Camera,Auto Slow Shutter On,81 01 04 5A 02 FF,6,Yes,No,Yes,PTZOptics uses only in HTTP API
Camera,Auto Slow Shutter Off,81 01 04 5A 03 FF,6,Yes,No,Yes,
Camera,Image Freeze On,81 01 04 62 02 FF,6,Yes,No,No,BRC-H900/BRC-X1000 etc.
Camera,Image Freeze Off,81 01 04 62 03 FF,6,Yes,No,No,
Camera,Preset Freeze On,81 01 04 62 22 FF,6,Yes,No,No,Auto-freeze video during preset
Camera,Preset Freeze Off,81 01 04 62 23 FF,6,Yes,No,No,
Camera,IR Receive On,81 01 04 06 08 FF,6,Yes,Yes,Yes,Enable IR receiver (if off)
Camera,IR Receive Off,81 01 04 06 09 FF,6,Yes,Yes,Yes,Disable IR remote reception
Zoom,Zoom Stop,81 01 04 07 00 FF,6,Yes,Yes,Yes,
Zoom,Zoom Tele Std,81 01 04 07 02 FF,6,Yes,Yes,Yes,
Zoom,Zoom Wide Std,81 01 04 07 03 FF,6,Yes,Yes,Yes,
Zoom,Zoom Tele Var,81 01 04 07 2p FF,6,Yes,Yes,Yes,p = 0–7 (speed)
Zoom,Zoom Wide Var,81 01 04 07 3p FF,6,Yes,Yes,Yes,p = 0–7
Zoom,Zoom Direct,81 01 04 47 0p 0q 0r 0s FF,10,Yes,Yes,Yes,Set absolute zoom position
Zoom,ZoomPosInq,81 09 04 47 FF,5,Yes,Yes,Yes,Reply 90 50 0p0q0r0s FF
Focus,Focus Stop,81 01 04 08 00 FF,6,Yes,Yes,Yes,
Focus,Focus Far Std,81 01 04 08 02 FF,6,Yes,Yes,Yes,
Focus,Focus Near Std,81 01 04 08 03 FF,6,Yes,Yes,Yes,
Focus,Focus Far Var,81 01 04 08 2p FF,6,Yes,Yes,Yes,p = 0–7
Focus,Focus Near Var,81 01 04 08 3p FF,6,Yes,Yes,Yes,p = 0–7
Focus,Focus Direct,81 01 04 48 0p 0q 0r 0s FF,10,Yes,Yes,Yes,
Focus,Auto Focus On,81 01 04 38 02 FF,6,Yes,Yes,Yes,
Focus,Auto Focus Off (Manual),81 01 04 38 03 FF,6,Yes,Yes,Yes,
Focus,Focus One Push (Snap),81 01 04 38 04 FF,6,No,Yes,Yes,PTZOptics Snap Focus, FR7 Push AF (diff opcode)
Focus,Auto/Manual Toggle,81 01 04 38 10 FF,6,No,Yes,No,PTZOptics specific toggle
Focus,Focus Mode Inq,81 09 04 38 FF,5,Yes,Yes,Yes,Reply 90 50 02/03 (Auto/Man)
Focus,Focus Pos Inq,81 09 04 48 FF,5,Yes,Yes,Yes,Reply 90 50 0p0q0r0s FF
Focus,Focus Lock On,81 0A 04 68 02 FF,6,No,Yes,No,
Focus,Focus Lock Off,81 0A 04 68 03 FF,6,No,Yes,No,
Focus,AF Sensitivity High,81 01 04 A9 00 FF,6,No,Yes,No,
Focus,AF Sensitivity Normal,81 01 04 A9 01 FF,6,No,Yes,No,
Focus,AF Sensitivity Low,81 01 04 A9 02 FF,6,No,Yes,No,
Focus,AF Zone Top,81 01 04 AA 00 FF,6,No,Yes,No,PTZOptics (Older models)
Focus,AF Zone Middle,81 01 04 AA 01 FF,6,No,Yes,No,
Focus,AF Zone Bottom,81 01 04 AA 02 FF,6,No,Yes,No,
Focus,Push AF (FR7) Start,81 01 7E 01 0A 00 01 FF,8,No,No,Yes,FR7 one-push AF press
Focus,Push AF (FR7) Stop,81 01 7E 01 0A 00 00 FF,8,No,No,Yes,FR7 one-push AF release
PanTilt,PT Drive (Continuous),81 01 06 01 VV WW XX YY FF,10,Yes,Yes,Yes,VV=Pan speed, WW=Tilt speed
PanTilt,PT Stop,81 01 06 01 00 00 03 03 FF,10,Yes,Yes,Yes,(direction 3,3 = stop)
PanTilt,PT Absolute,81 01 06 02 VV WW 0p0q0r0s 0t0u0v0w FF,15,Yes,Yes,Yes,Absolute pos move
PanTilt,PT Relative,81 01 06 03 VV WW 0p0q0r0s 0t0u0v0w FF,15,Yes,Yes,Yes,Relative move
PanTilt,Home,81 01 06 04 FF,6,Yes,Yes,Yes,
PanTilt,Reset,81 01 06 05 FF,6,Yes,Yes,Yes,
PanTilt,PT Limit Set,81 01 06 07 00 FF,6,Yes,Yes,Yes,Follow with which limit to set (Sony)
PanTilt,PT Limit Clear,81 01 06 07 01 FF,6,Yes,Yes,Yes,Clears limits
PanTilt,PT Pos Inq,81 09 06 12 FF,5,Yes,Yes,Yes,Reply 90 50 PP...TT...FF
Memory,Preset Reset,81 01 04 3F 00 pp FF,7,Yes,Yes,Yes,
Memory,Preset Set,81 01 04 3F 01 pp FF,7,Yes,Yes,Yes,
Memory,Preset Recall,81 01 04 3F 02 pp FF,7,Yes,Yes,Yes,
Memory,Preset Speed (PTZOptics),81 01 06 01 pp FF,6,No,Yes,No,Set preset move speed
Exposure,Auto Exposure (Full Auto),81 01 04 39 00 FF,6,Yes,Yes,Yes,
Exposure,Manual Exposure,81 01 04 39 03 FF,6,Yes,Yes,Yes,
Exposure,Shutter Priority,81 01 04 39 0A FF,6,Yes,Yes,Yes,
Exposure,Iris Priority,81 01 04 39 0B FF,6,Yes,Yes,Yes,
Exposure,Bright Mode,81 01 04 39 0D FF,6,Yes,Yes,No,FR7 does not support Bright
Exposure,Exposure Mode Inq,81 09 04 39 FF,5,Yes,Yes,Yes,Reply 90 50 00/03/0A/0B/0D
Exposure,Exposure Comp On,81 01 04 3E 02 FF,6,Yes,Yes,Yes,
Exposure,Exposure Comp Off,81 01 04 3E 03 FF,6,Yes,Yes,Yes,
Exposure,Exp Comp Direct,81 01 04 4E 00 00 0p 0q FF,10,Yes,Yes,Yes,EV level (00-0E)
Exposure,Backlight On,81 01 04 33 02 FF,6,Yes,Yes,Yes,
Exposure,Backlight Off,81 01 04 33 03 FF,6,Yes,Yes,Yes,
Exposure,Spotlight On,81 01 04 3A 02 FF,6,Yes,No,Yes,FR7 uses as ATW mode? (varies)
Exposure,Spotlight Off,81 01 04 3A 03 FF,6,Yes,No,Yes,
Exposure,Iris Direct,81 01 04 4B 00 00 0p 0q FF,10,Yes,Yes,Yes,
Exposure,Iris Inq,81 09 04 4B FF,5,Yes,Yes,Yes,Reply 90 50 0p0q0r0s
Exposure,Shutter Direct,81 01 04 4A 00 00 0p 0q FF,10,Yes,Yes,Yes,
Exposure,Shutter Inq,81 09 04 4A FF,5,Yes,Yes,Yes,Reply 90 50 0p0q0r0s
Exposure,Gain Direct,81 01 04 0C 00 00 0p 0q FF,10,Yes,Yes,Yes,
Exposure,Gain Inq,81 09 04 4C FF,5,Yes,Yes,Yes,Reply 90 50 0p0q0r0s
Exposure,Bright Direct,81 01 04 0D 00 00 0p 0q FF,10,Yes,Yes,No,Not on FR7
Exposure,Anti-Flicker Off,81 01 04 23 00 FF,6,No,Yes,No,PTZOptics CAM_Flicker
Exposure,Anti-Flicker 50Hz,81 01 04 23 01 FF,6,No,Yes,No,PTZOptics CAM_Flicker
Exposure,Anti-Flicker 60Hz,81 01 04 23 02 FF,6,No,Yes,No,PTZOptics CAM_Flicker
Exposure,Flicker Mode Inq,81 09 04 55 FF,5,No,Yes,No,Reply 90 50 0p FF (PTZOptics CAM_FlickerModeInq)
WB,WB Auto,81 01 04 35 00 FF,6,Yes,Yes,Yes,
WB,WB Indoor,81 01 04 35 01 FF,6,Yes,Yes,Yes,
WB,WB Outdoor,81 01 04 35 02 FF,6,Yes,Yes,Yes,
WB,WB One Push (Mode),81 01 04 35 03 FF,6,Yes,Yes,Yes,
WB,WB ATW,81 01 04 35 04 FF,6,Yes,No,Yes,FR7 supports ATW
WB,WB Manual,81 01 04 35 05 FF,6,Yes,Yes,Yes,
WB,WB Color Temp Mode,81 01 04 35 20 FF,6,Yes,Yes,No,FR7 uses Memory A/B instead
WB,WB Mode Inq,81 09 04 35 FF,5,Yes,Yes,Yes,Reply 90 50 mode
WB,One Push Trigger,81 01 04 10 05 FF,6,Yes,Yes,Yes,
WB,R Gain Direct,81 01 04 43 00 00 0p 0q FF,10,Yes,Yes,Yes,
WB,B Gain Direct,81 01 04 44 00 00 0p 0q FF,10,Yes,Yes,Yes,
WB,Color Temp Direct,81 01 04 20 0p 0q FF,8,Yes,Yes,No,
WB,Color Temp Inq,81 09 04 20 FF,5,Yes,Yes,No,Reply 90 50 0p0q
Image,Sharpness Auto,81 01 04 05 02 FF,6,Yes,Yes,Yes,
Image,Sharpness Manual,81 01 04 05 03 FF,6,Yes,Yes,Yes,
Image,Sharpness Direct,81 01 04 42 00 00 0p 0q FF,10,Yes,Yes,Yes,pq: level
Image,Sharpness Inq,81 09 04 42 FF,5,Yes,Yes,Yes,Reply 90 50 00 00 0p 0q FF
Image,Defog Direct,81 01 04 A0 00 00 0p 0q FF,10,Yes,No,No,Sony only; returns Syntax Error on PTZOptics G2 (hardware-validated)
Image,Defog Inq,81 09 04 A0 FF,5,Yes,No,No,Sony only; Reply 90 50 00 00 0p 0q FF
Image,Luminance Direct,81 01 04 A1 00 00 0p 0q FF,10,No,Yes,No,pq: 0x00–0x0E (PTZOptics)
Image,Luminance Inq,81 09 04 A1 FF,5,No,Yes,No,Reply 90 50 00 00 0p 0q FF
Image,Contrast Direct,81 01 04 A2 00 00 0p 0q FF,10,Yes,Yes,Yes,pq: 0x00–0x0E
Image,Contrast Inq,81 09 04 A2 FF,5,Yes,Yes,Yes,Reply 90 50 00 00 0p 0q FF
Image,Gamma Direct,81 01 04 5B 0p FF,6,Yes,Yes,Yes,p: 0=Standard 1–4=curves (PTZOptics undocumented but functional)
Image,Gamma Inq,81 09 04 5B FF,5,Yes,Yes,Yes,Reply 90 50 0p FF (PTZOptics undocumented but functional)
Image,NR (Legacy),81 01 04 53 0p FF,6,Yes,No,No,p: 0=Off 1–5=level (EVI-H100)
Tally,Tally On (Red),81 01 7E 01 0A 00 02 FF,8,Yes,No,Yes,
Tally,Tally Off (Red),81 01 7E 01 0A 00 03 FF,8,Yes,No,Yes,
Tally,Tally Inq (Red),81 09 7E 01 0A 00 FF,7,Yes,No,Yes,Reply 90 50 02/03
Tally,Tally Bright Lo (H900),81 01 7E 01 0A 01 04 FF,8,Yes,No,No,
Tally,Tally Bright Hi (H900),81 01 7E 01 0A 01 05 FF,8,Yes,No,No,
Tally,Tally On (Green, FR7),81 01 7E 04 1A 00 02 FF,8,No,No,Yes,
Tally,Tally Off (Green, FR7),81 01 7E 04 1A 00 03 FF,8,No,No,Yes,
Tally,Tally Inq (Green),81 09 7E 04 1A 00 FF,7,No,No,Yes,Reply 90 50 02/03
Tally,Tally (PTZOptics) Flash,81 0A 02 02 01 FF,6,No,Yes,No,
Tally,Tally (PTZOptics) On,81 0A 02 02 02 FF,6,No,Yes,No,
Tally,Tally (PTZOptics) Off,81 0A 02 02 03 FF,6,No,Yes,No,
Streaming,USB Audio On,81 2A 02 A0 04 02 FF,7,No,Yes,No,PTZOptics CAM_UACStatus
Streaming,USB Audio Off,81 2A 02 A0 04 03 FF,7,No,Yes,No,PTZOptics CAM_UACStatus
Streaming,USB Audio Inq,81 2A 02 A0 04 FF,6,No,Yes,No,Reply 90 50 0p FF (PTZOptics CAM_UACInq)
System,Address Set (Broadcast),88 30 01 FF,4,Yes,Yes,Yes,Serial only
System,I/F Clear (Broadcast),88 01 00 01 FF,5,Yes,Yes,Yes,Serial only
System,Version Inq,81 09 00 02 FF,5,Yes,Yes,Yes,Reply 90 50 VV VV MM MM FF FF KK FF
System,Command Cancel Socket1,81 21 FF,3,Yes,Yes,Yes,
System,Command Cancel Socket2,81 22 FF,3,Yes,Yes,Yes,
System,H-Flip On (legacy),81 01 04 61 02 FF,6,Yes,Yes,No,Sony/older PTZOptics
System,H-Flip Off (legacy),81 01 04 61 03 FF,6,Yes,Yes,No,
System,V-Flip On (legacy),81 01 04 66 02 FF,6,Yes,Yes,No,
System,V-Flip Off (legacy),81 01 04 66 03 FF,6,Yes,Yes,No,
System,Flip (Combined PTZOptics),81 01 04 A4 0p FF,6,No,Yes,No,p=0/1/2/3 Off/H/V/HV
System,Settings Save (PTZOptics),81 01 04 A5 10 FF,6,No,Yes,No,Save current config
System,PTZ MotionSync On (PTZOptics),81 0A 11 13 02 FF,6,No,Yes,No,
System,PTZ MotionSync Off (PTZOptics),81 0A 11 13 03 FF,6,No,Yes,No,
System,PTZ MotionSync Speed,81 0A 11 14 pp FF,6,No,Yes,No,pp = 01–18 (speed 1–24)
System,FR7 Speed Mode 24,81 01 7E 04 1B 01 FF,7,No,No,Yes,
System,FR7 Speed Mode 50,81 01 7E 04 1B 02 FF,7,No,No,Yes,
System,Menu Display On,81 01 06 06 02 FF,6,Yes,Yes,Yes,
System,Menu Display Off,81 01 06 06 03 FF,6,Yes,Yes,Yes,
System,Menu Select (Enter),81 01 06 06 05 FF,6,Yes,Yes,Yes,
System,Menu Cancel (Back),81 01 06 06 04 FF,6,Yes,Yes,Yes,
System,Menu Up,81 01 06 01 0E 0E 03 01 FF,10,Yes,Yes,Yes,
System,Menu Down,81 01 06 01 0E 0E 03 02 FF,10,Yes,Yes,Yes,
System,Menu Left,81 01 06 01 0E 0E 01 03 FF,10,Yes,Yes,Yes,
System,Menu Right,81 01 06 01 0E 0E 02 03 FF,10,Yes,Yes,Yes,
System,Menu Status Inq,81 09 06 06 FF,5,Yes,Yes,Yes,Reply 90 50 02/03 (Open/Closed). Uses category 06 not 04.
System,Direct Menu Control (FR7),81 01 7E 04 72 pp qq FF,8,No,No,Yes,Complex menu control
Streaming,Reboot (PTZOptics),81 0A 01 06 01 FF,6,No,Yes,No,Reboots camera
Streaming,Multicast On (PTZOptics),81 0B 01 23 01 FF,6,No,Yes,No,
Streaming,Multicast Off (PTZOptics),81 0B 01 23 02 FF,6,No,Yes,No,
Streaming,NDI Quality Set (PTZOptics),81 0B 01 01 0p FF,6,No,Yes,No,p=1–4 (Hi,Med,Low,Off)
```

> *"Bytes" column indicates total bytes including the terminator `FF`. "Yes/No" in model columns denote whether that model/family supports the command (to the best of current knowledge).*

---

## 12. Known Inquiry Limitations

Some features that have setter commands do not have corresponding inquiry commands in the VISCA protocol. This section documents these limitations based on live camera testing.

### 12.1 Auto Slow Shutter

**Setter commands exist:**
- Enable: `81 01 04 5A 02 FF`
- Disable: `81 01 04 5A 03 FF`

**No inquiry command exists.** There is no standard VISCA inquiry to determine if Auto Slow Shutter is currently enabled. PTZOptics specifically notes this feature is "only in HTTP API" for their cameras. Software implementations must either track state locally or use HTTP APIs where available.

### 12.2 Digital Zoom Enable/Disable

Some VISCA documentation references Digital Zoom enable/disable commands using opcode 0x06 under category 0x04:
- Enable: `81 01 04 06 02 FF`
- Disable: `81 01 04 06 03 FF`
- Inquiry: `81 09 04 06 FF`

**PTZOptics cameras do NOT support these commands.** Testing on PTZOptics G2 cameras confirms that all three commands return Syntax Error (0x02). On PTZOptics cameras, opcode 0x06 under category 0x04 is used for IR Receive control, not Digital Zoom. Sony cameras may support these commands.

Note: Digital zoom *position* inquiry (as part of combined zoom position) works normally via `81 09 04 47 FF`.

### 12.3 Menu Status Inquiry Category

The Menu Status Inquiry uses category 0x06 (Pan/Tilt category), **not** category 0x04:
- Correct: `81 09 06 06 FF` → Returns `90 50 02 FF` (Open) or `90 50 03 FF` (Closed)
- Incorrect: `81 09 04 06 FF` → Returns Syntax Error on PTZOptics

This aligns with the menu command bytes which also use category 0x06:
- Menu Open: `81 01 06 06 02 FF`
- Menu Close: `81 01 06 06 03 FF`
