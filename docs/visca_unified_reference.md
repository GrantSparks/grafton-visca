# Unified VISCA Protocol Implementation Guide

_(PTZOptics Gen‑2/NDI, Sony ILME‑FR7, Sony BRC‑series, Sony EVI‑H100 S/V, Nearus BRC‑300)_

**Author:** Grafton Machine Shed  
**Contact:** admin@grafton.ai

---

## 1  Scope & Intented VISCA Protocol Implementation Guide

_(PTZOptics Gen‑2/NDI, Sony ILME‑FR7, Sony BRC‑series, Sony EVI‑H100 S/V, Nearus BRC‑300)_

---

## 1  Scope & Intent

This document captures VISCA protocol and implementation details for some popular PTZ cameras from multiple manufacturers. It is intended as a unified reference for developers integrating these systems, providing both the standard VISCA commands and the important differences and nuances across various camera models. By consolidating information for Sony, PTZOptics, and other VISCA variants, this guide offers practical guidance for implementing control software that can accommodate multiple VISCA-capable cameras.

---

## 2  Terminology

We use standard VISCA terminology throughout this guide. The controller refers to the device or software sending commands (always address 0), while the peripheral is the camera receiving them (address 1–7 in serial, or 1 in IP mode). Each camera maintains two command “sockets” (buffers) to process up to two commands in parallel. The camera responds to commands with specific reply messages: an **ACK** to acknowledge receipt, a **Completion** when the action is done, or an **Error** if something goes wrong. The table below defines these terms:

| Term                    | Meaning                                                                  |
| ----------------------- | ------------------------------------------------------------------------ |
| **Controller**          | VISCA command originator (serial address `0`; IP `Src = 0x00`).          |
| **Peripheral / Camera** | VISCA responder (serial `1‑7`; IP **always `1`**).                       |
| **Socket / Buffer**     | One of two concurrent command slots in every Sony‑derived PTZ.           |
| **ACK**                 | `90 4y FF` – command accepted, socket `y (1 or 2)`.                      |
| **Completion**          | `90 5y FF` – command finished, socket `y`.                               |
| **Error**               | `90 60/6y .. FF` (`02` syntax, `03` buffer full, `41` not executable …). |

---

## 3  Physical & Transport Layers

### 3.1 Serial RS‑232/RS‑422

VISCA was originally designed for direct serial control, with the ability to daisy-chain multiple cameras. Each camera on a serial chain is assigned a unique ID (1–7) and the controller uses ID 0. Commands are sent over RS-232 (typically for a single chain) or RS-422 (for longer runs or multi-drop setups). Many PTZ cameras provide an 8-pin mini-DIN (Sony) or DE-9 (PTZOptics) connector for serial control. The default communication settings are 9,600 or 38,400 bps, 8-N-1 framing. VISCA commands are framed by an address byte (`8x` where x is the camera ID or 0 for broadcast) and a terminator `FF`. The payload can be up to 14 bytes, making the total command packet at most 16 bytes long. Serial VISCA is reliable and inherently ordered due to the point-to-point connection.

| Connector                            | Baud                      | Frame                          | Addressing                    |
| ------------------------------------ | ------------------------- | ------------------------------ | ----------------------------- |
| MiniDIN‑8 (Sony) or DE‑9 (PTZOptics) | 9 600 / 38 400 bps, 8‑N‑1 | `8x … FF` (1–14 payload bytes) | Controller `0`, Cameras `1‑7` |

### 3.2 VISCA‑over‑IP

VISCA commands can also be sent over an IP network (Ethernet) using UDP or TCP. However, unlike the standardized serial interface, manufacturers have implemented two incompatible approaches for VISCA‑over‑IP.

#### 3.2.1 **Two incompatible flavours**

There are two distinct flavors of VISCA‑over‑IP in use:  
**Encapsulated UDP** with an 8‑byte header (used by Sony and certain other brands), and **Raw VISCA over IP** with no header (used by PTZOptics and similar). These methods are not interoperable. The table below compares their characteristics:

| Camera family                                              | Envelope required?         | Sequence number?       | Default port(s)    |
| ---------------------------------------------------------- | -------------------------- | ---------------------- | ------------------ |
| **Sony** BRC/SRG/FR series (and clones)                    | **Yes** – 8‑byte header    | 32‑bit counter (wraps) | UDP 52381          |
| **PTZOptics** (all firmware revs to 2025‑06)               | **No** – _raw VISCA bytes_ | _Not used_             | UDP 1259, TCP 5678 |
| Hybrids claiming “encapsulated” mode (Marshall, AVer etc.) | Yes                        | Yes                    | UDP 52381          |

**Important:** PTZOptics documentation (Rev 1.2, Aug 2020) makes no mention of any header or sequence number—and indeed, network captures confirm that PTZOptics cameras consume the standard VISCA byte stream directly over IP. In practice, sending a Sony‑style 8‑byte header to a PTZOptics camera will result in the packet being ignored (no response at all). Conversely, a Sony or Marshall/AVer camera expects the header; if you send raw bytes without it, those cameras won’t recognize the command. **Implementation Tip:** Provide a configuration switch or auto-detection in your software to choose between “Sony‑style encapsulation” and “Raw VISCA” modes. This ensures that you only add the header for devices that require it.

#### 3.2.2 Sony encapsulated header (UDP 52381)

Sony’s VISCA‑over‑IP protocol wraps the command bytes in an additional header and is typically carried over UDP. In this mode, each command, inquiry, and reply is prefixed by an 8‑byte header that provides message type, length, and a sequence number for tracking. The diagram below illustrates the structure of a UDP packet carrying a VISCA command:

┌─────────────── 8 bytes ───────────────┐┌───── VISCA frame (≤16 B) ─────┐
│ Payload‑type │ Length │ Sequence‑No. ││ 8x …payload… FF │
└───────────────────────────────────────┘└───────────────────────────────┘

- `Payload‑type` `0x0000` Command, `0x0001` Inquiry, `0x0002` Reply
- `Sequence‑No.` 16‑bit (BRC/H900) or 32‑bit (FR7) counter; host increments, camera echoes.

To implement the Sony encapsulated protocol when sending a command:

1. Set the **Payload type** field (use `0x0000` for a Command, or `0x0001` for an Inquiry).
2. Calculate the **Payload length** as the number of bytes in the VISCA command frame (from the `8x` address byte up to `FF`).
3. Assign a **Sequence number** for this message. Increment your sequence counter (start from 0 or 1). Use a 32‑bit value for modern cameras (older models effectively use only 16 bits of it).
4. Construct the UDP packet: an 8‑byte header (with the fields above in network byte order) followed by the VISCA command bytes. Send this to UDP port 52381 of the camera.

The camera will respond with its own packets containing the same sequence number. For an **ACK** or **Completion** message from the camera, the payload type will be `0x0002` (Reply) and the Sequence‑No. will match, so you can correlate responses to the command. In IP mode, the VISCA device address is always 1 (so your command frames should use `0x81` as the address byte).

#### 3.2.3 Retransmission (Sony only)

In a UDP network, packets can be lost or delayed. The VISCA encapsulated protocol relies on the controller to handle delivery confirmation. In practice, if the controller does not receive an **ACK** from the camera within about 2 video frames (~33 ms), it should consider the command lost and retransmit the packet. **Important:** When retransmitting, **always use a new Sequence number** for the re-send. The camera uses sequence IDs to detect duplicate messages.

Sony’s documentation provides a detailed matrix of how the camera behaves with duplicates. Generally, if the first attempt was actually received and executed but the ACK got lost, sending the same command with a new sequence may cause the camera to respond with an error (e.g., an “abnormal sequence” error or a second completion). If the first attempt was never received, the new sequence command will be processed normally. As a controller developer, you should be prepared to handle either scenario. One strategy is to set a retry limit (for example, retry a command up to 3 times with new sequence IDs). If no ACK or completion is received after several attempts, the camera might be offline or unreachable, and your software can alert the user or attempt a reconnection.

This retransmission logic mainly applies to the Sony‑style UDP protocol. In **raw VISCA‑over‑IP** (e.g., PTZOptics), there is no sequence field to assist with duplicates. You may still implement a timeout and retry if an ACK isn’t received, but with the risk of the camera executing the command twice (since it cannot tell a retry from a new command). In practice, many integrators using raw UDP rely on the inherent reliability of a local network or choose to use PTZOptics’ TCP control port to avoid packet loss issues. Using TCP (port 5678 for PTZOptics) ensures commands arrive in order without loss, though you should still adhere to the two-command concurrency rule at the application level.

---

## 4  Transaction & Timing Model

When a VISCA command is issued, the camera and controller go through a defined sequence. First, the camera sends an immediate acknowledgment (ACK) to confirm it received the command and locked one of its command buffers. Later, after executing the command, the camera sends a completion message on that same socket. If the command cannot be accepted or executed, an error message is returned instead. The table below summarizes these phases, typical timing, and error codes:

| Phase          | Reply                                                                    | Notes                       |
| -------------- | ------------------------------------------------------------------------ | --------------------------- |
| **ACK**        | `90 4y FF`                                                               | ≤ 1 V‑sync (≈16.7 ms NTSC). |
| **Completion** | `90 5y FF`                                                               | Frees socket `y`.           |
| **Error**      | `90 60 02` Syntax<br>`90 60 03` Buffer Full<br>`90 6y 41` Not Executable |                             |

All VISCA cameras enforce a **two-socket concurrency** limit. In other words, the camera can process at most two commands at the same time (one in each socket). If a third command is sent while two are still in progress, the camera will respond with a “Buffer Full” error (`90 60 03`), and it will not execute that third command. Your implementation should prevent this by queuing or delaying additional commands until one of the previous commands completes.

An **ACK** (`90 4y FF`) indicates the camera accepted a command into socket `y`. Once processing is finished, a **Completion** (`90 5y FF`) is sent and socket `y` becomes free. If a command cannot be executed due to the camera’s current state, the camera will send an **Error** instead of a normal completion. For instance, sending a manual focus command while the camera is in auto-focus mode will yield an error `90 6y 41` (“Not Executable”) after the ACK. The command is effectively ignored in that case. Controllers may choose to handle this by informing the user or automatically switching modes.

Some models exhibit additional timing nuances. For example, the **Sony EVI‑H100 and BRC‑H900** cameras require about 240 ms after completing a preset recall before they are ready to accept new commands. If you send a command immediately after a preset movement on these models, you might get a `...41` “Not Executable” error because the camera is still “busy.” The proper approach is to either delay subsequent commands briefly or catch the error and retry after the short delay.

VISCA also supports a **Command Cancel** function (see Appendix, e.g., `81 21 FF` to cancel socket 1). This can be used to abort a long-running action (like a continuous pan). When a cancel is issued, the camera will respond with a `90 6y 04` “Command Canceled” message for the affected socket (and no completion message will follow). If no command was in that socket, a `90 6y 05` “No Socket” error is returned. Canceling commands is optional in most applications, but it can be useful for emergency stops or user interrupts.

---

## 5  Baseline Command Set (universally recognised)

The following core commands are recognized by all VISCA-compatible PTZ cameras in this guide. They cover essential functions like power control, zoom, focus, pan‑tilt movement, and preset memory operations. The table below lists these baseline commands, their VISCA opcode sequence, and their purpose.

| Category | Opcode                       | Purpose                     |
| -------- | ---------------------------- | --------------------------- |
| Power    | `81 01 04 00 02/03 FF`       | On / Standby                |
| Zoom     | `81 01 04 07 .. FF`          | Std / Var / Direct          |
| Focus    | `81 01 04 08 .. FF`          | Std / Var / Direct          |
| Pan‑Tilt | `81 01 06 01 VV WW DD DD FF` | Continuous move.            |
| Memory   | `81 01 04 3F 00/01/02 pp FF` | Reset / Set / Recall preset |

---

## 6  Cross‑Model Capability Matrix

Different camera models vary in capabilities even though they share the VISCA protocol. The matrix below highlights key feature differences across these models. Understanding these differences is important when designing a controller for multiple cameras, as certain features (or ranges) may not be available on all devices. A checkmark (✔) indicates support, and a cross (✖) indicates lack of support; bold text is used to highlight an especially notable value or limit. Note that the Sony BRC‑H900 requires an optional IP interface card for network control (otherwise it can only be controlled via serial).

| Feature        | PTZOptics Gen‑2           | Sony ILME‑FR7 | Sony BRC‑H900 | Sony EVI‑H100 | Nearus BRC‑300 |
| -------------- | ------------------------- | ------------- | ------------- | ------------- | -------------- |
| Transport (IP) | **Raw UDP/TCP**           | Encaps. UDP   | Encaps. UDP\* | —             | —              |
| Preset slots   | **128** (fw ≥2.2)         | 100           | 16            | 6             | 6              |
| Pan‑spd steps  | 1‑24                      | **1‑50**      | 1‑24          | 1‑24          | 1‑24           |
| Focus Lock cmd | ✔                         | —             | —             | —             | —              |
| ND Filter ctrl | —                         | ✔             | —             | —             | —              |
| Dual Tally     | Single cmd (flash/on/off) | **Red+Green** | Red (Hi/Lo)   | —             | —              |
| Bright AE mode | ✔                         | ✖             | ✔             | ✔             | ✔              |
| ATW WB         | —                         | ✔             | —             | —             | —              |
| Colour‑Temp WB | ✔                         | —             | ✔             | ✔             | —              |

- BRC‑H900 requires optional IP‑card; otherwise serial only.

---

## 7  Parameter Ranges & Scaling

When controlling zoom, pan/tilt, and exposure, it’s important to understand each model’s numeric ranges and scaling. VISCA parameters are typically given as hexadecimal values, which correspond to physical positions or settings. The subsections below detail the ranges for zoom, pan/tilt coordinates, and exposure settings, helping you ensure your commands stay within valid limits.

### 7.1 Zoom

VISCA uses a 16-bit value to represent the zoom position. The minimum value (`0x0000`) corresponds to the wide end (fully zoomed out). Each camera defines its maximum optical zoom position according to its optical zoom capability. For instance, a 20× optical zoom lens corresponds to `0x4000` at the tele end for the PTZOptics 20× model. If digital zoom is enabled, values beyond the optical range (up to the listed maximum) will engage digital zoom. The table below shows the zoom range for each model (optical and digital):

| Model         | Optical range             | Digital     |
| ------------- | ------------------------- | ----------- |
| PTZOptics 20× | `0x0000` → `0x4000`       | to `0x7800` |
| BRC‑300       | 1–12× (`0x0000`→`0x4000`) | 4×          |
| EVI‑H100 30×  | (`0x0000`→`0x7AC0`)       | 12×         |

### 7.2 Pan / Tilt scales

VISCA defines pan and tilt coordinates as 16-bit values, but two different conventions exist:

- **Legacy unsigned range** – Used by older models like the BRC‑300 (and Nearus clone). Here, 0x0000 represents one extreme (far left or top) and 0xFFFF the opposite extreme (far right or bottom). There is no fixed center value in the code (approximately 0x7FFF would be mid-range).
- **Signed centered range** – Used by most modern cameras. In this scheme, 0x0000 represents the center position. Values increase positively in one direction (right or down) up to 0x7FFF, and decrease (with two’s complement interpretation for negatives) from 0xFFFF downward for the opposite direction (left or up).

Your control software should account for these differences. Typically, Sony BRC/SRG/FR-series and PTZOptics cameras use the signed centered system, whereas the legacy BRC‑300 uses the unsigned range. Abstraction can be built so that a desired pan angle (in degrees) is mapped to the appropriate hex value depending on the camera model.

The table below provides approximate mechanical pan/tilt ranges and their end-limit hex values for select models. (Note: The FR7 has an exceptionally large tilt range; the note “20-bit clip” indicates the camera may use higher internal resolution but will clip commands at its physical limits.)

| Model     | Pan ° | Tilt ° (desktop) | Hex ends          |
| --------- | ----- | ---------------- | ----------------- |
| PTZOptics | ±170  | –30 → +90        | ≈ `E1E5` ↔ `1E1B` |
| FR7       | ±170  | –30 → +195       | 20‑bit clip       |

### 7.3 Exposure numeric domains

Exposure settings (shutter, iris, gain, etc.) are quantized into discrete steps in VISCA. Most values have standardized ranges, though some models extend them. For example, all cameras support 24 shutter speed steps from slow (1/1) to fast (1/10000). Iris typically has 16 levels (dependent on lens for the FR7). Gain ranges from 0 to 15 (representing roughly 0 to +33 dB). Exposure compensation (EV) is often 0x00 to 0x0E (0 to 14 in decimal) on most models, but the FR7 allows a broader range up to 0x12. The table below summarizes these ranges:

| Control  | Range                                  | Note                 |
| -------- | -------------------------------------- | -------------------- |
| Shutter  | 24 steps 1/1–1/10000                   | All models           |
| Iris     | 16 steps                               | FR7 lens‑dependent   |
| Gain     | 0–15 (≈0–33 dB)                        |                      |
| EV shift | 0x00–0x0E (others) / **0x00–0x12 FR7** | FR7 extends EV range |

---

## 8  Model‑Specific Command Extensions

Many cameras implement additional VISCA commands beyond the baseline set, typically to control features unique to that model. In a universal controller, these can be handled as optional extensions for those specific models. Below we list some notable model-specific commands:

### 8.1 PTZOptics

- Focus Lock `0A 04 68 02/03` – Locks the current focus at its position (On to lock, Off to unlock). No autofocus or manual focus changes will take effect while locked.
- H‑Flip `01 04 61 02/03`, V‑Flip `01 04 66 02/03` – Flips the image horizontally or vertically (useful for ceiling-mount or mirror image scenarios).
- AWB Sensitivity `01 04 A9 00/01/02` – Sets the Auto White Balance sensitivity (e.g., Low, Normal, High), influencing how quickly the camera adjusts to changes in lighting.
- Multicast `0B 01 23 02/03` – Enables or disables multicast streaming on network (specific to models supporting NDI or multicast streaming).

### 8.2 Sony ILME‑FR7

- ND Mode `7E 04 52 0p` – Switches ND filter mode (e.g., between preset ND levels vs. variable ND control, depending on `p`).
- ND Level `7E 04 42 00 0p 0q` – Sets the electronic ND filter to a specific level (where `0p0q` form a value; FR7’s ND filter is continuously variable within its range).
- Auto ND `7E 04 53 02/03` – Turns automatic ND adjustment On (02) or Off (03). When On, the camera will engage the ND filter automatically to maintain exposure.
- Tally Red `7E 01 0A 00 02/03`; Tally Green `7E 04 1A 00 02/03` – Controls the camera’s red and green tally lights (turn on with 02, off with 03 for each color).
- Speed‑Step select `7E 04 1B 01/02` – Selects the pan/tilt speed sensitivity (01 = legacy 24-speed steps, 02 = fine 50-speed steps mode).
- Direct‑Menu press `7E 04 72 pp 0q` – Emulates pressing a camera’s direct menu button or dial (with `pp` indicating which control and `0q` the action), allowing remote navigation of the FR7’s menu or direct setting adjustment.

### 8.3 Sony BRC‑H900

- Image Freeze `01 04 62 02/03` – Freezes (02) or unfreezes (03) the camera’s video output on the current frame.
- Tally brightness `7E 01 0A 01 04/05` – Sets the brightness level of the red tally lamp (04 for low brightness, 05 for high brightness).
- IP‑card discovery: send `"ENQ:network"` to UDP 52380 – Broadcasts a network inquiry that causes the optional IP card to respond with its network settings (MAC address, IP, etc.), allowing the controller to discover the camera on the network.

### 8.4 Sony EVI‑H100

- Gamma 0‑4, Wide‑D/HDR, NR 0‑5 – Provides advanced image settings control: multiple gamma preset levels (0–4), a Wide Dynamic Range/HDR mode toggle, and Noise Reduction levels (0–5). (These commands are unique to this model’s VISCA implementation.)
- 240 ms busy window post‑preset – After a preset recall action, the camera remains busy for roughly 0.24 seconds. Commands sent in this interval are likely to return `41` (Not Executable). Controllers should introduce a short delay or retry commands if a preset was just recalled.

---

## 9  Behavioural Edge‑Cases & Recommended Tests

Even when commands are correct, cameras may exhibit certain edge-case behaviors or require special handling. The table below outlines various test scenarios and how the cameras are expected to behave. We recommend using these scenarios to validate your implementation. By anticipating these, you can handle errors or quirks gracefully (e.g., avoiding command overload or handling temporary non-executable states).

| Scenario                                  | Expectation                          |
| ----------------------------------------- | ------------------------------------ |
| Third in‑flight command                   | `…60 03` Buffer Full                 |
| Manual focus while AF On                  | `…6y 41` Not Executable              |
| PTZOptics Focus‑Lock active → manual move | `41` until unlocked                  |
| FR7 ND level cmd in Preset mode           | `41`                                 |
| FR7 Tally auto‑off                        | Lamp goes dark after 15 s no refresh |
| EVI‑H100 preset recall + <240 ms next cmd | One‑off `41`, retry succeeds         |
| Send Sony‑header packet to PTZOptics      | **Ignored** (no ACK)                 |

- **Third in-flight command:** If two commands are already being processed and a third is sent, the camera responds with a `...60 03` (Buffer Full) error. The third command is not executed; the controller should wait and retry once a socket is free.
- **Manual focus during AF:** Sending manual focus commands while the camera is in Auto Focus mode results in a `...6y 41` (Not Executable) error. The camera will not switch out of AF automatically; the controller should enable manual focus mode first or handle the rejection.
- **Focus-Lock active (PTZOptics):** If focus lock is engaged on a PTZOptics camera, any focus adjustments (autofocus or manual) will yield `41` errors until focus is unlocked. Ensure to unlock focus (or avoid sending focus commands) while locked.
- **ND adjustment during preset (FR7):** On the FR7, if a preset recall is underway (which often includes lens and exposure changes), sending an ND filter level command in that moment will return `41` (Not Executable). The camera disallows certain changes during recall motions.
- **Tally auto-off (FR7):** The FR7’s tally lights automatically turn off after about 15 seconds if no “on” command is repeated. If a continuous tally light is needed, the controller should periodically resend the tally-on command (e.g., every 10–15 seconds) to keep the light illuminated.
- **Post-preset busy (EVI-H100):** As noted, the EVI-H100 may reject commands sent immediately (<240 ms) after a preset recall with a `41` error. The solution is to pause briefly or simply catch the error and retry the command after the brief busy period.
- **Wrong IP protocol:** If a Sony-style encapsulated packet (with header) is sent to a camera that expects raw VISCA (like PTZOptics), it will be ignored entirely (no ACK). Similarly, sending raw VISCA bytes to a Sony-style camera will not be recognized. Always use the correct protocol mode for the camera.

---

## 10  Implementation Checklist

Finally, here’s a checklist summarizing best practices and considerations when implementing a multi-camera VISCA control system. Ensuring each of these points is addressed will improve compatibility and robustness:

1. Pluggable **transport adapter** (raw vs encapsulated) – implement a layer or setting to switch between sending raw VISCA bytes (for serial/PTZOptics) and adding the 8-byte header (for Sony/hybrid IP cameras).
2. Two‑socket state machine; gate ≥3rd command – maintain state for the two command buffers and do not send a new command when both are occupied (queue it instead).
3. Axis scale abstraction (`UnsignedEnd16` vs `SignedCentre16`) – transparently handle the difference in pan/tilt coordinate systems between legacy and modern cameras (so that commands consistently move to intended angles).
4. Firmware‑quirk registry – account for model-specific quirks or limits (e.g., older PTZOptics firmware allowing only 10 or 32 presets vs newer 128; the EVI-H100’s 240 ms post-preset delay; FR7’s unique modes such as absence of direct Color-Temp mode).
5. IP‑card discovery (BRC‑H900) – support the special discovery message on UDP 52380 to find a BRC-H900 with the optional IP card (and retrieve its IP configuration).
6. Retry on UDP loss with new sequence‑ID (Sony only) – implement a timeout and retransmission mechanism for UDP commands on Sony-style connections (increase sequence IDs on retries).
7. Auto‑close OSD (`06 06 03`) before operational moves – always exit the camera’s on-screen menu (OSD) mode before sending motion or other commands, by issuing the OSD Close command. Otherwise, the camera may ignore control commands while a menu is open.

---

## 11  Sources & References  

* **PTZOptics – “VISCA over IP Command Set Rev 1.2”** (PDF, 8 Aug 2020).  
  — Primary reference for PTZOptics raw‑UDP/TCP command list, network port usage, and firmware‑specific features.

* **Sony – “VISCA Command List v2.00 (BRC‑300/300P)”** (A‑C1Z‑100‑13 (1), 2004).  
  — Definitive command, inquiry, and error‑handling reference for legacy BRC‑300 family; establishes classic ACK/Completion protocol.

* **Sony – “EVI‑H100S/H100V Technical Manual”** (December 2012 edition).  
  — Extended VISCA tables for 30× optical zoom, Wide‑D, Gamma, Noise Reduction, and 240 ms post‑preset busy behaviour.

* **Sony – “ILME‑FR7 Network & VISCA Command Manual”** (October 2024, rev 2.1).  
  — Source for ND filter control, dual‑tally lamp, 50‑step pan‑speed modes, and 32‑bit sequence number rules in Sony encapsulated UDP.

* **Nearus (SnapAV) – “VISCA Protocol via Sony”** hand‑out (Rev 140911‑1400).  
  — Confirms BRC‑300 compatibility notes and reiterates 8‑byte header absence in Nearus‑badged serial control.

* **Generic – “VISCA Protocol 2.0 Overview”** whitepaper (unbranded, 2018).  
  — Background on RS‑232/RS‑422 daisy‑chain wiring, address assignment flow, and broadcast `88 30 01 FF` usage.

---

### Appendix A – CSV Opcode Table

A non-exhaustive machine‑parsable CSV listing of opcodes (column order: **Category, Mnemonic, Opcode, Bytes, Sony, PTZOptics, FR7, Notes**) is provided below.

Category,Mnemonic,Opcode,Bytes,Sony,PTZOptics,FR7,Notes  
Power,CAM_Power ON,81 01 04 00 02 FF,6,Yes,Yes,Yes,  
Power,CAM_Power Standby,81 01 04 00 03 FF,6,Yes,Yes,Yes,  
Zoom,Zoom Stop,81 01 04 07 00 FF,6,Yes,Yes,Yes,  
Zoom,Zoom Tele (Std),81 01 04 07 02 FF,6,Yes,Yes,Yes,  
Zoom,Zoom Wide (Std),81 01 04 07 03 FF,6,Yes,Yes,Yes,  
Zoom,Zoom Tele Var (2p),81 01 04 07 2p FF,6,Yes,Yes,Yes,p=0–7  
Zoom,Zoom Wide Var (3p),81 01 04 07 3p FF,6,Yes,Yes,Yes,p=0–7  
Zoom,Zoom Direct,81 01 04 47 0p 0q 0r 0s FF,10,Yes,Yes,Yes,16‑bit position  
Focus,Focus Auto,81 01 04 38 02 FF,6,Yes,Yes,Yes,  
Focus,Focus Manual,81 01 04 38 03 FF,6,Yes,Yes,Yes,  
Focus,Focus Far (Std),81 01 04 08 02 FF,6,Yes,Yes,Yes,  
Focus,Focus Near (Std),81 01 04 08 03 FF,6,Yes,Yes,Yes,  
Focus,Focus Far Var (2p),81 01 04 08 2p FF,6,Yes,Yes,Yes,p=0–7  
Focus,Focus Near Var (3p),81 01 04 08 3p FF,6,Yes,Yes,Yes,p=0–7  
Focus,Focus Stop,81 01 04 08 00 FF,6,Yes,Yes,Yes,  
Focus,Focus Direct,81 01 04 48 0p 0q 0r 0s FF,10,Yes,Yes,Yes,  
Focus,Focus Lock On,81 0A 04 68 02 FF,6,No,Yes,No,  
Focus,Focus Lock Off,81 0A 04 68 03 FF,6,No,Yes,No,  
Focus,AF Zone Top,81 01 04 AA 00 FF,6,No,Yes,No,  
Focus,AF Zone Center,81 01 04 AA 01 FF,6,No,Yes,No,  
Focus,AF Zone Bottom,81 01 04 AA 02 FF,6,No,Yes,No,  
Focus,Push AF Press (FR7),81 01 7E 01 0A 00 01 FF,8,No,No,Yes,  
Focus,Push AF Release (FR7),81 01 7E 01 0A 00 00 FF,8,No,No,Yes,  
PanTilt,PT Drive Cont,81 01 06 01 VV WW DD DD FF,10,Yes,Yes,Yes,"VV pan spd, WW tilt spd"  
PanTilt,PT Drive Stop,81 01 06 01 00 00 03 03 FF,10,Yes,Yes,Yes,Dir 03 03 = stop  
PanTilt,PT Absolute,81 01 06 02 VV WW YY YY ZZ ZZ FF,12,Yes,Yes,Yes,  
PanTilt,PT Relative,81 01 06 03 VV WW YY YY ZZ ZZ FF,12,Yes,Yes,Yes,  
PanTilt,Home,81 01 06 04 FF,6,Yes,Yes,Yes,  
PanTilt,Reset,81 01 06 05 FF,6,Yes,Yes,Yes,  
PanTilt,PT Limit Set,81 01 06 07 00 FF,6,Yes,Yes,Yes,Set current pos as limit  
PanTilt,PT Limit Clear,81 01 06 07 01 FF,6,Yes,Yes,Yes,  
Memory,Preset Reset,81 01 04 3F 00 pp FF,7,Yes,Yes,Yes,pp preset idx  
Memory,Preset Set,81 01 04 3F 01 pp FF,7,Yes,Yes,Yes,  
Memory,Preset Recall,81 01 04 3F 02 pp FF,7,Yes,Yes,Yes,  
Exposure,AE Full Auto,81 01 04 39 00 FF,6,Yes,Yes,Yes,  
Exposure,AE Manual,81 01 04 39 03 FF,6,Yes,Yes,Yes,  
Exposure,AE Shutter Pri,81 01 04 39 0A FF,6,Yes,Yes,Yes,  
Exposure,AE Iris Pri,81 01 04 39 0B FF,6,Yes,Yes,Yes,  
Exposure,AE Bright,81 01 04 39 0D FF,6,Yes,Yes,No,  
Exposure,EV Comp On,81 01 04 3E 02 FF,6,Yes,Yes,Yes,  
Exposure,EV Comp Off,81 01 04 3E 03 FF,6,Yes,Yes,Yes,  
Exposure,EV Comp Direct,81 01 04 4E 00 00 0p 0q FF,10,Yes,Yes,Yes,0p0q comp value  
Exposure,Backlight On,81 01 04 33 02 FF,6,Yes,Yes,Yes,  
Exposure,Backlight Off,81 01 04 33 03 FF,6,Yes,Yes,Yes,  
Exposure,Spotlight On,81 01 04 3A 02 FF,6,Yes,No,Yes,FR7/BRC-H900  
Exposure,Spotlight Off,81 01 04 3A 03 FF,6,Yes,No,Yes,  
Exposure,Gain Limit Direct (PTZO),81 01 04 2C 0p FF,6,No,Yes,No,  
Exposure,Auto Slow Shutter On,81 01 04 5A 02 FF,6,Yes,No,Yes,  
Exposure,Auto Slow Shutter Off,81 01 04 5A 03 FF,6,Yes,No,Yes,  
Exposure,Iris Direct,81 01 04 4B 00 00 0p 0q FF,10,Yes,Yes,Yes,  
Exposure,Shutter Direct,81 01 04 4A 00 00 0p 0q FF,10,Yes,Yes,Yes,  
Exposure,Gain Direct,81 01 04 0C 00 00 0p 0q FF,10,Yes,Yes,Yes,  
Exposure,Bright Direct,81 01 04 0D 00 00 0p 0q FF,10,Yes,Yes,No,  
WB,WB Auto,81 01 04 35 00 FF,6,Yes,Yes,Yes,  
WB,WB Indoor,81 01 04 35 01 FF,6,Yes,Yes,Yes,  
WB,WB Outdoor,81 01 04 35 02 FF,6,Yes,Yes,Yes,  
WB,WB One Push,81 01 04 35 03 FF,6,Yes,Yes,Yes,  
WB,WB ATW,81 01 04 35 04 FF,6,Yes,No,Yes,FR7 only  
WB,WB Manual,81 01 04 35 05 FF,6,Yes,Yes,Yes,  
WB,WB Memory A,81 01 04 35 05 FF,6,No,No,Yes,Re‑used as Memory A  
WB,WB Memory B,81 01 04 35 06 FF,6,No,No,Yes,  
WB,WB Color Temp Mode,81 01 04 35 20 FF,6,Yes,Yes,No,  
WB,Color Temp Direct,81 01 04 20 0p 0q FF,8,Yes,Yes,No,  
WB,One Push Trigger,81 01 04 10 05 FF,6,Yes,Yes,Yes,  
WB,R Gain Direct,81 01 04 43 00 00 0p 0q FF,10,Yes,Yes,Yes,  
WB,B Gain Direct,81 01 04 44 00 00 0p 0q FF,10,Yes,Yes,Yes,  
WB,AWB Sens High,81 01 04 A9 00 FF,6,No,Yes,No,  
WB,AWB Sens Normal,81 01 04 A9 01 FF,6,No,Yes,No,  
WB,AWB Sens Low,81 01 04 A9 02 FF,6,No,Yes,No,  
Tally,Tally On (Red),81 01 7E 01 0A 00 02 FF,8,Yes,No,No,BRC/FR7  
Tally,Tally Off (Red),81 01 7E 01 0A 00 03 FF,8,Yes,No,No,  
Tally,Tally Bright Lo,81 01 7E 01 0A 01 04 FF,8,Yes,No,No,BRC-H900  
Tally,Tally Bright Hi,81 01 7E 01 0A 01 05 FF,8,Yes,No,No,  
Tally,Tally Green On,81 01 7E 04 1A 00 02 FF,8,No,No,Yes,  
Tally,Tally Green Off,81 01 7E 04 1A 00 03 FF,8,No,No,Yes,  
Tally,Tally Flash (PTZO),81 0A 02 02 01 FF,6,No,Yes,No,  
Tally,Tally On (PTZO),81 0A 02 02 02 FF,6,No,Yes,No,  
Tally,Tally Off (PTZO),81 0A 02 02 03 FF,6,No,Yes,No,  
Picture,Picture Effect Off,81 01 04 63 00 FF,6,Yes,Yes,Yes,  
Picture,Picture Effect B&W,81 01 04 63 04 FF,6,Yes,Yes,Yes,  
Picture,Image Freeze On,81 01 04 62 02 FF,6,Yes,No,No,BRC-H900  
Picture,Image Freeze Off,81 01 04 62 03 FF,6,Yes,No,No,  
Picture,Digital Zoom On,81 01 04 06 02 FF,6,Yes,No,No,  
Picture,Digital Zoom Off,81 01 04 06 03 FF,6,Yes,No,No,  
ND,ND Mode Preset,81 01 7E 04 52 00 FF,8,No,No,Yes,  
ND,ND Mode Variable,81 01 7E 04 52 01 FF,8,No,No,Yes,  
ND,ND Level Direct,81 01 7E 04 42 00 0p 0q FF,10,No,No,Yes,  
ND,Auto ND On,81 01 7E 04 53 02 FF,8,No,No,Yes,  
ND,Auto ND Off,81 01 7E 04 53 03 FF,8,No,No,Yes,  
System,H-Flip On,81 01 04 61 02 FF,6,No,Yes,No,  
System,H-Flip Off,81 01 04 61 03 FF,6,No,Yes,No,  
System,V-Flip On,81 01 04 66 02 FF,6,No,Yes,No,  
System,V-Flip Off,81 01 04 66 03 FF,6,No,Yes,No,  
Menu,OSD Menu On,81 01 06 06 02 FF,6,Yes,Yes,Yes,  
Menu,OSD Menu Off,81 01 06 06 03 FF,6,Yes,Yes,Yes,  
Menu,OSD Up,81 01 06 01 0E 0E 03 01 FF,10,Yes,Yes,Yes,  
Menu,OSD Down,81 01 06 01 0E 0E 03 02 FF,10,Yes,Yes,Yes,  
Menu,OSD Left,81 01 06 01 0E 0E 01 03 FF,10,Yes,Yes,Yes,  
Menu,OSD Right,81 01 06 01 0E 0E 02 03 FF,10,Yes,Yes,Yes,  
Menu,OSD Enter,81 01 06 06 05 FF,6,Yes,Yes,Yes,  
Menu,OSD Cancel,81 01 06 06 04 FF,6,Yes,Yes,Yes,  
System,Command Cancel socket1,81 21 FF,3,Yes,Yes,Yes,  
System,Command Cancel socket2,81 22 FF,3,Yes,Yes,Yes,
