# Consolidated VISCA / PTZOptics Gen‑2 / Axis Reference

**Document status:** Final consolidated reference - gap recommendations implemented
**Date:** 2026-05-15
**Primary scope:** PTZOptics Gen‑2 NDI®\|HX cameras (`PT12X‑NDI`, `PT20X‑NDI`, `PT30X‑NDI`), Axis VISCA Interface API behavior, and only the Sony VISCA references needed to resolve protocol/opcode conflicts.

This is the repository's canonical protocol reference. It replaces the former
repo-local PTZOptics, Axis, and unified VISCA protocol notes; those source names
may still appear in the appendices only as provenance for the consolidated
material.

The built-in inquiry inventory contains only sourced query families. Unsourced
`09 04 2B` iris status, inverted generic/vendor booleans, the combined tally
reply, defog, and orphan decode-only kinds are withdrawn (#878). A downstream
integration can declare a custom request and decoder for a packet that its own
model documentation establishes. A register mention in a negative finding
does not establish a typed grant.

## 1. Validation basis and source hierarchy

This document accepts as fact the information that is either:

1. consistent across the uploaded source bundle;
2. model/vendor-specific and not contradicted by another relevant source; or
3. resolved by a stronger primary source, such as an official manufacturer datasheet, command manual, or firmware changelog.

Information that is plausible but not fully validated is not mixed into the main reference. It is moved to **Appendix A — Remaining validation notes and issue links**.


### 1.1 Revision notes for this final version

This version implements the gap report recommendations by adding complete Axis camera-command coverage, the Axis focus-near-limit and shutter tables, the PTZOptics inquiry and block-inquiry references, missing PTZOptics extension commands, expanded implementation guardrails, and product/spec appendices.

### Source hierarchy used

1. Official PTZOptics product pages, datasheets, manuals, command references, joystick manuals, and firmware changelog.
2. Official Axis VISCA Interface API Description.
3. Official Sony VISCA command manuals and Sony technical manuals, only where they resolve VISCA transport or opcode semantics.
4. Uploaded patched/consolidated notes and hardware-test notes. These are accepted where they clarify known errata or live-device behavior, but hardware-test-only claims remain labeled as such.

## 2. Scope and profile boundaries

### 2.1 Validated PTZOptics Gen‑2 NDI®\|HX scope

The PTZOptics Gen‑2 NDI®\|HX scope covers:

- `PT12X‑NDI`
- `PT20X‑NDI`
- `PT30X‑NDI`

The command set is substantially shared across these models. Optical limits, lens ranges, field of view, and a few path-specific capabilities are model-dependent.

`PtzOpticsG3` has model-specific coverage in R10 and the official current
G2/G3 Developer Portal (R14). Those sources independently document G3 `04 39`
AE modes, standard `04 0B`/`04 4B` iris controls, and `09 04 39`/`09 04 4B`
inquiries, in addition to the retained vendor controls. They do not document
the distinct `09 04 2B` iris auto/manual status inquiry. This evidence does not
expand the Gen-2 scope or generalize any other unlisted G3 behavior.

`PtzOptics30X` is the library profile for the legacy PT30X SDI/NDI G2/Gen-2
line, not for a generic or newer "30X" product. PTZOptics' official G2/Legacy
index explicitly lists the 12X, 20X, and 30X SDI/NDI G2 models. Neither that
membership nor a shared name grants these rows to Move, Link, or newer 30X
models; each typed profile remains source-scoped.

### 2.2 Validated Axis scope

The Axis VISCA Interface API values are **Axis profile values**. They must not be generalized to PTZOptics unless PTZOptics primary documentation or live calibration confirms the same behavior.

### 2.3 Sony references used only for protocol/opcode resolution

Sony references are used here for:

- Sony-style encapsulated VISCA-over-IP packet format and UDP port `52381`.
- VISCA ACK/completion/error semantics.
- Standard opcode interpretation where PTZOptics or Axis documentation contains a likely typo.
- Standard commands such as Bright Direct `04 4D`, Gamma `04 5B`, and Sony digital-zoom inquiry behavior.

This document does **not** claim full validation of all Sony FR7, Sony BRC, Sony EVI, Nearus, Marshall, AVer, Avonic, or other non-PTZOptics/non-Axis model profiles. Those broader profiles remain in Appendix A unless specifically validated in a row below.

## 3. Executive decisions

| Topic | Final decision | Status |
|---|---|---|
| PT30X focal length | Use `f4.42mm–132.6mm, F1.8–F2.8`. Treat `88.5mm` for PT30X as a PT20X copy/paste error. | Validated |
| PTZOptics raw IP control | Use raw VISCA bytes over UDP `1259` or TCP `5678`; do not add Sony’s 8-byte header on those raw PTZOptics ports. | Validated |
| PTZOptics Sony VISCA mode | Sony VISCA mode uses UDP `52381`; support is firmware-gated. | Validated with firmware caveat |
| Sony VISCA-over-IP | Use an 8-byte header: payload type, payload length, sequence number, then the VISCA frame. UDP port is `52381`. | Validated |
| Sony UDP retransmission | For Sony-style VISCA-over-IP timeout recovery, retransmit using the **same sequence number**, per Sony’s command manual guidance. | Validated correction |
| PTZOptics presets | Treat preset count as control-path-specific: IR = 10; spec capacity = 255 via serial/IP; raw VISCA command table documents `0–127`; web/HTTP UI sources describe `0–254`. Do not guarantee raw VISCA `>0x7F` until target hardware is tested. | Validated with implementation caveat |
| Bright Direct | Use `81 01 04 4D 00 00 0p 0q FF`. Treat PTZOptics `04 0D` Direct row as a documentation error. | Validated erratum |
| Gamma | `81 01 04 5B 0p FF`; inquiry `81 09 04 5B FF`. Sony validates the opcode; PTZOptics support is supported by uploaded patched/hardware-test notes. | Validated with PTZOptics hardware-test label |
| PT12X 720p120 | Available as an IP / NDI®\|HX network-stream feature, not a universal HDMI/SDI output mode. | Validated with path caveat |
| Axis zoom/focus/PT coordinate ranges | Axis-only. Do not apply to PTZOptics. | Validated |
| Axis absolute zoom command | Use standard direct zoom `04 47`; the public Axis PDF row showing `04 07 02` under absolute zoom is internally inconsistent with its `pqrs` position comments and its own `04 47` zoom-position inquiry. | Validated erratum |
| Menu status inquiry | Use category `06`: `81 09 06 06 FF`. Do not use `81 09 04 06 FF`. | Validated |
| PTZ Motion Sync, OnePush/Snap Focus, image freeze | Firmware-specific; not baseline Gen‑2 guarantees. | Appendix / gated |

## 4. PTZOptics Gen‑2 NDI®\|HX validated model data

### 4.1 Optical and camera data

| Model | Optical zoom | Lens / focal length | HFOV tele → wide | VFOV tele → wide | Min lux | Weight | Notes |
|---|---:|---|---|---|---|---:|---|
| `PT12X‑NDI` | 12× | `f3.5mm–42.3mm, F1.8–F2.8` | `6.9° → 72.5°` | `3.9° → 44.8°` | `0.05 Lux @ F1.8, AGC ON` | `3.20 lb / 1.45 kg` | `720p120` is IP/NDI®\|HX-only. |
| `PT20X‑NDI` | 20× | `f4.42mm–88.5mm, F1.8–F2.8` | `3.36° → 60.7°` | `1.89° → 34.1°` | `0.05 Lux @ F1.8, AGC ON` | `3.00 lb / 1.36 kg` | Max-height metric row in some manual/spec material is inconsistent; see Appendix A. |
| `PT30X‑NDI` | 30× | `f4.42mm–132.6mm, F1.8–F2.8` | `2.28° → 60.7°` | `1.28° → 34.1°` | `0.05 Lux @ F1.8, AGC ON` | `3.05 lb / 1.39 kg` | Datasheet and current PTZOptics NDI page confirm `132.6mm`; reject `88.5mm` for PT30X. |

### 4.2 Common PTZOptics Gen‑2 capabilities

The following are accepted across the three PTZOptics Gen‑2 NDI®\|HX models unless a model-specific caveat is stated:

| Area | Validated details |
|---|---|
| Video outputs | NDI®\|HX / IP, HDMI 1.3, 3G‑SDI, CVBS. |
| Network | RJ‑45 10/100/1000 Ethernet. |
| Streaming codecs | NDI®\|HX, H.264, H.265, M‑JPEG. |
| Streams | Two IP video streams are supported. |
| Audio | 3.5mm line-in; AAC audio over supported stream/output paths. |
| Serial | RS‑232 and RS‑485 with VISCA / Pelco‑D / Pelco‑P. |
| PTZ mechanics | Pan ±170°; tilt −30° to +90°. |
| PTZ speed | Pan 1.7°/s to 100°/s; tilt 1.7°/s to 69.9°/s. |
| Presets | 255 via serial/IP capability statements; IR remote supports 10 presets; raw VISCA command table only documents `0–127`. |
| Preset accuracy | 0.1°. |
| Power | DC 12V or PoE 802.3af; max power 12W; max current 1.0A. |
| Environment | Operating −10°C to 40°C; storage −40°C to 60°C; operating humidity 10% to 80%. |
| Sensor / scanning | 1/2.7 in CMOS, 2.07M effective pixels; progressive scanning. |
| Camera features | Backlight compensation, 2D/3D digital noise reduction, image flip, image mirror, and image freeze are product capabilities. |
| Unsupported product features | Face detection and local storage are not supported in the checked Gen‑2 specs. |
| MTBF | >30,000 hours. |
| PT12X power caution | Do not power by DC and PoE simultaneously. |

### 4.3 IP streaming and protocol specifications

The following IP streaming rows are common to the checked PT12X/PT20X/PT30X Gen‑2 NDI®\|HX manuals/spec tables:

| Item | Validated details |
|---|---|
| First stream resolutions | `1920x1080`, `1280x720`, `1024x576`, `960x540`, `640x480`, `640x360`. |
| Second stream resolutions | `1280x720`, `1024x576`, `720x480`, `720x408`, `640x360`, `480x270`, `320x240`, `320x180`. |
| Video bitrate | `32 Kbps–102400 Kbps`. |
| Bitrate type | Variable Rate or Fixed Rate. |
| Frame rate | 50 Hz mode: `1–50 FPS`; 60 Hz mode: `1–60 FPS`. |
| Audio compression | AAC. |
| Audio bitrate | `96 Kbps`, `128 Kbps`, `256 Kbps`. |
| Supported protocols | TCP/IP, HTTP, RTSP, RTMP, DHCP, Multicast, and related IP streaming/control protocols. |
| HTTP‑CGI examples | PTZ control path `/cgi-bin/ptzctrl.cgi?ptzcmd&...`; pan speed `1–24`, tilt speed `1–20`, zoom speed `1–7`. |

### 4.4 Video system and interface details

| Item | PT12X‑NDI | PT20X‑NDI | PT30X‑NDI |
|---|---|---|---|
| Video system, manual table | `1080p60/50/30/25/59.94*/29.97*`, `1080i60/50/59.94*`, `720p60/50/59.94*`; CVBS `576i/480i`. Broadcast frame rates marked beta. | Same manual table family. | Same manual table family. |
| Additional datasheet video note | Datasheet additionally lists `720p120` and `720p30/25`; `720p120` is only over NDI®\|HX / IP streaming. | Datasheet lists 720p `60/50/30/25` and repeats 1080p/1080i sets. | Datasheet lists 720p `60/50/30/25` and carries the simultaneous-output limitation. |
| HDMI | 1 x HDMI 1.3. | 1 x HDMI 1.3. | 1 x HDMI 1.3. |
| 3G‑SDI | 1 x 3G‑SDI BNC, 800mVp‑p, 75Ω, SMPTE 424M. | Same. | Same. |
| IP / NDI | 1 x RJ‑45 NDI®\|HX / IP network stream, 10/100/1000 Ethernet. | Same. | Same. |
| CVBS | 1 x RCA, 1Vp‑p, 75Ω. | Same. | Same. |
| Audio input | 1-channel 3.5mm line-in, unbalanced stereo; manual path note says NDI®\|HX and IP network stream. | Same. | Same. |
| USB | 1 x USB 2.0 Type‑A; datasheets describe it as future use. | Same. | Same. |
| RS‑232 IN | 8-pin Mini-DIN, max 30m, VISCA / Pelco‑D / Pelco‑P. | Same. | Same. |
| RS‑232 OUT | 8-pin Mini-DIN, max 30m, VISCA network use only. | Same. | Same. |
| RS‑485 | 2-pin Phoenix, max 1200m, VISCA / Pelco‑D / Pelco‑P. | Same. | Same. |
| Power jack | JEITA-type DC IN 12V. | Same. | Same. |

### 4.5 Mechanical, package, and model-color details

| Item | PT12X‑NDI | PT20X‑NDI | PT30X‑NDI |
|---|---|---|---|
| Color variants | `PT12X‑NDI‑GY`, `PT12X‑NDI‑WH`. | `PT20X‑NDI‑GY`, `PT20X‑NDI‑WH`. | `PT30X‑NDI‑GY`, `PT30X‑NDI‑WH`. |
| Base size | `5.6 in W x 6.7 in D x 6.5 in H`; `142mm x 169mm x 164mm`. | Same base size. | Same base size. |
| Max height with tilt | Manual/datasheet conversion rows are inconsistent; use detailed drawings for installation-critical checks. | `7.8 in` with max tilt; datasheet `~198mm`; reject `168mm` as inconsistent. | `7.8 in` with max tilt; datasheet `198mm`; reject `168mm` as inconsistent. |
| Camera weight | `3.20 lb / 1.45 kg`. | `3.00 lb / 1.36 kg`. | `3.05 lb / 1.39 kg`. |
| Box weight | `5.4 lb / 2.45 kg`. | `5.4 lb / 2.45 kg`. | `5.4 lb / 2.45 kg`. |
| Box contents | Camera, power adapter + cord, IR remote, RS‑232C cable, quick start guide, two AAA batteries. | Same package family. | Same package family. |

### 4.6 Video path caveats

- `PT12X‑NDI` `720p120` should be treated as **network stream only**, specifically IP / NDI®\|HX. Do not list it as a universal HDMI/SDI mode.
- `PT30X‑NDI` cannot perform simultaneous `1080p60` over IP stream and SDI/HDMI. This final reference does **not** assert a universal `1080p30` fallback rule because the checked source establishes the simultaneous-`1080p60` prohibition but not a complete fallback matrix. For implementation, prevent simultaneous `1080p60` on both paths and verify the allowed alternate-path mode on target firmware or in the camera UI.

## 5. PTZOptics control paths and ports

| Control or stream path | Port / behavior | Validation decision |
|---|---|---|
| Raw UDP PTZ control | UDP `1259` | Use raw VISCA-format bytes such as `81 01 ... FF`. |
| Raw TCP PTZ control | TCP `5678` | Use raw VISCA-format bytes. TCP helps avoid UDP loss but does not remove VISCA socket/concurrency limits. |
| HTTP-CGI / web UI | TCP `80` by default | Use HTTP-CGI examples from manual where applicable. |
| RTSP | TCP/UDP `554` by default | Stream path. |
| SRT | `4578` by default | SRT encryption options are Off, AES‑128, AES‑192, AES‑256. |
| Sony VISCA mode | UDP `52381` | Separate protocol mode. Firmware-gated; PTZOptics firmware changelog says Sony VISCA over IP support was added in SOC `6.3.12` for PoE models. |
| NDI control | NDI®\|HX path | Supported; not the same as raw VISCA TCP/UDP. |
| RS‑232 / RS‑485 | VISCA / Pelco‑D / Pelco‑P | Serial path. |

### 5.1 Raw PTZOptics VISCA over IP

For raw PTZOptics UDP/TCP control:

```text
81 01 ... FF
81 09 ... FF
```

There is no Sony-style VISCA-over-IP header on UDP `1259` or TCP `5678`.

### 5.2 Sony VISCA-over-IP mode

For Sony VISCA-over-IP mode, use UDP `52381` and the Sony 8-byte header:

```text
Byte 0-1: payload type
Byte 2-3: payload length
Byte 4-7: sequence number
Byte 8+:  VISCA payload, ending in FF (FR7 position commands use 17 bytes)
```

Common payload types:

| Payload type | Meaning |
|---|---|
| `01 00` | VISCA command |
| `01 10` | VISCA inquiry |
| `01 11` | VISCA reply |
| `01 20` | VISCA device setting command |
| `02 00` | Control command |
| `02 01` | Control reply |

In Sony VISCA-over-IP, the VISCA device address is fixed as camera `1`, so normal commands use `81 ... FF`.

Sony's sequence-number RESET is a control command with a one-byte `01`
payload: `02 00 00 01 00 00 00 00 01`. The header sequence field is ignored,
and successful processing resets the next VISCA sequence to zero. A control
reply uses `02 01`; the documented success payload is `01`, while protocol
errors use two bytes such as `0F 01` (sequence abnormality) or `0F 02` (message
type abnormality). These payloads are not terminated VISCA frames and must not
be used to complete or correlate a VISCA request. The library leaves RESET
disabled on connect unless the caller opts in, and exposes received control
codes through diagnostics.

`DirectMenuControl` deliberately rejects the data pair `FF 81..88`. Other
`FF` data values, including a final `FF`, remain byte-for-byte and receive a
separate trailing terminator. An address-shaped byte immediately after a
delimiter-valued data byte could instead be interpreted by a delimiter-scanning
camera or intermediary as the start of a second addressed VISCA frame, so that
specific pair is outside the safe typed-command grammar (#683, #727).

### 5.3 Sony UDP retransmission correction

Earlier drafts and some secondary summaries say to retry Sony VISCA-over-IP with a new sequence number. Sony’s command manual instead describes retransmitting the timed-out message using the **same sequence number** so the controller can infer which message was lost and whether the camera already accepted the original command. This document adopts Sony’s same-sequence retransmission guidance.

## 6. VISCA transaction model

### 6.1 Command lifecycle

A normal command begins with `8x 01 ... FF` over serial or `81 01 ... FF` for single-camera IP use.

Expected response sequence:

```text
Controller → Camera: 81 01 ... FF
Camera → Controller: 90 4y FF   # ACK, accepted into socket y
Camera → Controller: 90 5y FF   # Completion, command finished
```

`y` is the socket number. Most cameras have two command sockets.

For raw VISCA, do not send a second command for the same target while its
first command is still unacknowledged. The one raw candidate spans `Sending`,
`AwaitingAck`, and `AwaitingCompletion`. The `AwaitingCompletion` phase is the
completion-only shape: it holds the target channel exclusively and never earns
a socket. For an ACK-bearing command,
once the ACK establishes the first command's socket, the scheduler may use the
camera's remaining socket capacity while that command executes. Raw ACK and
error routing never uses a
command FIFO or temporal recency. A socketless error routes the unique
unacknowledged command only when no inquiry owner is live; with no unacknowledged
command it may route the legitimate per-target inquiry FIFO, while a
command-plus-inquiry collision is ignored. An explicit socket routes only its
exact target/socket owner, and a socketless error never targets `Executing`.
A named ACK socket is exact evidence as well: if another request owns that
socket on raw VISCA, the camera's new assignment supersedes the stale local
owner (#721). The old request fails immediately and leaves an inert keyed
`PreAck` hold; the new request owns the named socket. A socketless ACK may select the first free
registered socket. Sony-encapsulated commands may pipeline before ACK because
their envelope sequence number provides exact correlation; they retain the
bounded #620/#682 other-free-socket compatibility fallback.

The fixed response forms are length-exact: `z0 4y FF` ACK, `z0 5y FF`
nonzero-socket completion, `z0 6y zz FF` error, and `z0 38 FF` network-change
notification. In the ACK, completion, and error forms, `y=0` is the valid
socketless compatibility spelling, `y=1`/`2` are sockets S1/S2, and `y=3..F`
is malformed. A known fixed prefix with trailing bytes is malformed rather
than an `Unknown` response. Variable data replies are accepted only for
socket 0 with more than three bytes; the canonical `z0 50 FF` socket-0
completion remains valid.

### 6.2 Inquiry lifecycle

An inquiry begins with `8x 09 ... FF` over serial or `81 09 ... FF` for single-camera IP use.

Expected response sequence:

```text
Controller → Camera: 81 09 ... FF
Camera → Controller: 90 50 ... FF   # Data reply
```

Inquiries do not receive an ACK before the data reply.

### 6.3 Errors

| Error | Packet pattern | Meaning |
|---|---|---|
| Syntax Error | `90 60 02 FF` or `90 6y 02 FF` | Bad command format, unsupported command, or illegal parameter. |
| Command Buffer Full | `90 60 03 FF` or `90 6y 03 FF` | Two command sockets are already in use. |
| Command Canceled | `90 6y 04 FF` | Command in socket `y` was canceled; no normal completion follows. |
| No Socket | `90 6y 05 FF` | Cancel requested for an empty or invalid socket. |
| Command Not Executable | `90 6y 41 FF` | Camera state prevents execution, such as manual focus movement while autofocus is active. |

## 7. PTZOptics validated baseline command reference

The following command families are validated for PTZOptics Gen‑2 unless a caveat or appendix note says otherwise.

### 7.1 Power

| Function | Packet |
|---|---|
| Power On | `81 01 04 00 02 FF` |
| Power Off / Standby | `81 01 04 00 03 FF` |
| Power Inquiry | `81 09 04 00 FF` |

### 7.2 Zoom

| Function | Packet / range |
|---|---|
| Stop | `81 01 04 07 00 FF` |
| Tele standard | `81 01 04 07 02 FF` |
| Wide standard | `81 01 04 07 03 FF` |
| Tele variable | `81 01 04 07 2p FF`, `p = 0–7` |
| Wide variable | `81 01 04 07 3p FF`, `p = 0–7` |
| Direct zoom | `81 01 04 47 0p 0q 0r 0s FF` |
| Zoom position inquiry | `81 09 04 47 FF` |

PTZOptics specs list 16× digital zoom, but the checked PTZOptics Gen‑2 command references do not establish a safe Sony-style digital-zoom enable/disable command. See Appendix A.2.

### 7.3 Focus

| Function | Packet / range |
|---|---|
| Stop | `81 01 04 08 00 FF` |
| Far standard | `81 01 04 08 02 FF` |
| Near standard | `81 01 04 08 03 FF` |
| Far variable | `81 01 04 08 2p FF`, `p = 0–7` |
| Near variable | `81 01 04 08 3p FF`, `p = 0–7` |
| Direct focus | `81 01 04 48 0p 0q 0r 0s FF`; no PTZOptics source gives numeric bounds for `pqrs` (the `0x1000–0xF000` range in §8 is Axis-only), so the library admits the full 16-bit wire domain. |
| Auto focus | `81 01 04 38 02 FF` |
| Manual focus | `81 01 04 38 03 FF` |
| Auto/manual toggle | `81 01 04 38 10 FF` |
| Focus lock | `81 0A 04 68 02 FF` |
| Focus unlock | `81 0A 04 68 03 FF` |
| Focus mode inquiry | `81 09 04 38 FF` |
| Focus position inquiry | `81 09 04 48 FF` |

OnePush / Snap Focus should be firmware-gated. See Appendix A.5.

### 7.4 White balance, color temperature, and color tuning

| Function | Packet / range | Notes |
|---|---|---|
| WB Auto | `81 01 04 35 00 FF` | Normal auto white balance. |
| WB Indoor | `81 01 04 35 01 FF` | Indoor mode. |
| WB Outdoor | `81 01 04 35 02 FF` | Outdoor mode. |
| WB OnePush mode | `81 01 04 35 03 FF` | Enables one-push WB mode. |
| WB Manual | `81 01 04 35 05 FF` | Manual white balance. |
| WB Color Temperature mode | `81 01 04 35 20 FF` | Color-temperature white balance mode. |
| OnePush trigger | `81 01 04 10 05 FF` | Triggers one-push WB while in the appropriate mode. |
| Red gain reset/up/down | `81 01 04 03 00/02/03 FF` | Manual R-gain relative control. |
| Red gain direct | `81 01 04 43 00 00 0p 0q FF` | `pq = R gain`. |
| Blue gain reset/up/down | `81 01 04 04 00/02/03 FF` | Manual B-gain relative control. |
| Blue gain direct | `81 01 04 44 00 00 0p 0q FF` | `pq = B gain`. |
| Color temperature reset/up/down | `81 01 04 20 00/02/03 FF` | Relative/default color-temperature controls. |
| Color temperature direct | `81 01 04 20 0p 0q FF` | `0x00 = 2500K`, `0x37 = 8000K`. |
| Red tuning direct | `81 01 04 43 00 00 00 pq FF` | Shares opcode `04 43` with red gain direct (above); the tuning form forces the byte before the code to `00` and carries the whole code in `pq`. `0x00 = -10`, `0x0A = 0`, `0x14 = +10`. See the color-tuning erratum below. |
| Blue tuning direct | `81 01 04 44 00 00 00 pq FF` | Shares opcode `04 44` with blue gain direct (above); the tuning form forces the byte before the code to `00` and carries the whole code in `pq`. `0x00 = -10`, `0x0A = 0`, `0x14 = +10`. See the color-tuning erratum below. |
| WB mode inquiry | `81 09 04 35 FF` | See section 7.10 for reply values. |

#### Red/blue color-tuning erratum

Red and blue *tuning* do not use a separate `0A 01 12` / `0A 01 13` vendor
extension. Earlier drafts of this reference documented one, but it does not match
the shipped encoder and was never validated against a PTZOptics command sheet. The
library encodes tuning on the same opcode as the corresponding *gain-direct*
command:

- Red tuning: `81 01 04 43 00 00 00 pq FF`
- Blue tuning: `81 01 04 44 00 00 00 pq FF`

where `pq` is the tuning code (`0x00 = -10`, `0x0A = 0`, `0x14 = +10`). The only
nominal difference from gain direct (`81 01 04 43 00 00 0p 0q FF` /
`81 01 04 44 00 00 0p 0q FF`) is that tuning forces the byte before the code to `00`
and places the whole code in the final data byte. Because a gain value `<= 0x0F`
also leaves that byte `00`, the two forms overlap on the wire and cannot be reliably
told apart by a passive listener; only tuning codes above `0x0F` (i.e. `+6..+10`)
land a value the gain-direct path never puts there. The matching inquiries are
byte-identical as well: red tuning and red gain both query `81 09 04 43 FF`, and
blue tuning and blue gain both query `81 09 04 44 FF`. Treat tuning and gain as one
opcode family (`04 43` / `04 44`) when reasoning about profile capabilities and
inquiry routing.

### 7.5 Exposure

| Function | Packet / range | Notes |
|---|---|---|
| AE Full Auto | `81 01 04 39 00 FF` | Automatic exposure. |
| AE Manual | `81 01 04 39 03 FF` | Manual exposure. |
| AE Shutter Priority | `81 01 04 39 0A FF` | Shutter priority. |
| AE Iris Priority | `81 01 04 39 0B FF` | Iris priority. |
| AE Bright Mode | `81 01 04 39 0D FF` | Bright mode / manual bright control. |
| Iris reset/up/down | `81 01 04 0B 00/02/03 FF` | Relative iris control. |
| Iris direct | `81 01 04 4B 00 00 0p 0q FF` | `pq = Iris Position`. |
| Shutter reset/up/down | `81 01 04 0A 00/02/03 FF` | Relative shutter control. |
| Shutter direct | `81 01 04 4A 00 00 0p 0q FF` | `pq = Shutter Position`. |
| Gain reset/up/down | `81 01 04 0C 00/02/03 FF` | Relative gain control. |
| Gain direct | `81 01 04 4C 00 00 0p 0q FF` | `pq = Gain Position`. |
| Gain limit | `81 01 04 2C 0p FF` | `p = 0x0–0xF`. |
| Bright reset/up/down | `81 01 04 0D 00/02/03 FF` | Relative bright control. |
| Bright direct | `81 01 04 4D 00 00 0p 0q FF` | Use corrected direct opcode `04 4D`. |
| Exposure compensation on/off | `81 01 04 3E 02/03 FF` | Exposure compensation mode. |
| Exposure compensation reset/up/down | `81 01 04 0E 00/02/03 FF` | Relative exposure compensation. |
| Exposure compensation direct | `81 01 04 4E 00 00 0p 0q FF` | `pq = ExpComp Position`. |
| Backlight on/off | `81 01 04 33 02/03 FF` | Back-light compensation. |
| Anti-flicker / flicker | `81 01 04 23 0p FF`, `0 = Off`, `1 = 50Hz`, `2 = 60Hz` | PTZOptics flicker setting. |
| Dynamic Range Control direct | `81 01 04 25 00 00 00 0p FF` | PTZOptics extension; `p = 0x0–0x8`. |

#### Bright Direct erratum

The official PTZOptics VISCA-over-IP command sheet lists Bright Direct as `81 01 04 0D 00 00 0p 0q FF`, but this conflicts with standard VISCA semantics and with the Bright inquiry opcode. Use `04 4D` for Bright Direct.

### 7.6 Pan/Tilt

| Function | Packet / range |
|---|---|
| Up | `81 01 06 01 VV WW 03 01 FF` |
| Down | `81 01 06 01 VV WW 03 02 FF` |
| Left | `81 01 06 01 VV WW 01 03 FF` |
| Right | `81 01 06 01 VV WW 02 03 FF` |
| Up-left | `81 01 06 01 VV WW 01 01 FF` |
| Up-right | `81 01 06 01 VV WW 02 01 FF` |
| Down-left | `81 01 06 01 VV WW 01 02 FF` |
| Down-right | `81 01 06 01 VV WW 02 02 FF` |
| Stop | `81 01 06 01 VV WW 03 03 FF` |
| Absolute position | `81 01 06 02 VV WW 0Y 0Y 0Y 0Y 0Z 0Z 0Z 0Z FF` |
| Relative position | `81 01 06 03 VV WW 0Y 0Y 0Y 0Y 0Z 0Z 0Z 0Z FF` |
| Home | `81 01 06 04 FF` |
| Reset | `81 01 06 05 FF` |
| Limit set | `81 01 06 07 00 0W 0Y 0Y 0Y 0Y 0Z 0Z 0Z 0Z FF` |
| Limit clear | `81 01 06 07 01 0W 07 0F 0F 0F 07 0F 0F 0F FF` |
| Position inquiry | `81 09 06 12 FF` |

PTZOptics documented speed ranges in the command table:

- Pan speed `VV`: `0x01` low to `0x18` high.
- Tilt speed `WW`: `0x01` low to `0x14` high.

Those are the PTZOptics two-speed limits. The library's `TiltSpeed` syntax
also accepts `0x18` for profiles that document it, such as Sony BRC-300's
single-`VV` position framing; every typed request still checks the selected
profile's advertised tilt-speed range.

### 7.7 Preset memory

| Function | Packet | Validated note |
|---|---|---|
| Reset / delete preset | `81 01 04 3F 00 pp FF` | Raw command table documents `pp = 0–127`. |
| Set preset | `81 01 04 3F 01 pp FF` | Raw command table documents `pp = 0–127`. |
| Recall preset | `81 01 04 3F 02 pp FF` | Raw command table documents `pp = 0–127`. |
| Preset recall speed | `81 01 06 01 pp FF` | `pp = 0x01–0x18`. |

Final path-specific preset treatment:

| Path | Documented capacity / behavior | Implementation decision |
|---|---|---|
| IR remote | 10 presets, usually `0–9` | Safe to expose 10 for IR mode. |
| Serial/IP spec capacity | 255 presets | Safe to document as device capacity. |
| HTTP/web UI | `0–254` in web/manual material | Treat as 255 slots, zero-based. |
| Raw PTZOptics VISCA command table | `0–127` | Do not guarantee raw VISCA values above `0x7F` without target-device test. |

### 7.8 Image processing

| Function | Packet / range | Status |
|---|---|---|
| Luminance direct | `81 01 04 A1 00 00 0p 0q FF` | Validated two-nibble form; `pq = 0x00–0x0E` in patched notes. |
| Luminance inquiry | `81 09 04 A1 FF` | Validated in patched notes/manual tables. |
| Contrast direct | `81 01 04 A2 00 00 0p 0q FF` | Validated two-nibble form; `pq = 0x00–0x0E` in patched notes. |
| Contrast inquiry | `81 09 04 A2 FF` | Validated in patched notes/manual tables. |
| Gamma direct | `81 01 04 5B 0p FF` | Standard VISCA validated by Sony; PTZOptics support hardware-tested/patched-note validated. |
| Gamma inquiry | `81 09 04 5B FF` | Same caveat as Gamma direct. |
| Sharpness mode | `81 01 04 05 0p FF`, `p = 2 Auto`, `3 Manual` | Validated. |
| Sharpness reset/up/down | `81 01 04 02 00/02/03 FF` | Validated. |
| Sharpness direct | `81 01 04 42 00 00 0p 0q FF` | Packet validated; broad numeric range remains profile/hardware-test dependent. |
| Saturation / color gain direct | `81 01 04 49 00 00 00 0p FF`, `0x0 = 60%`, `0xE = 200%` | Validated. |
| Hue direct | `81 01 04 4F 00 00 00 0p FF`, `0x0 = −14°`, `0xE = +14°` | Validated. |
| Picture effect off | `81 01 04 63 00 FF` | Validated from PTZOptics command table. |
| Picture effect B&W | `81 01 04 63 04 FF` | Validated from PTZOptics command table. |
| Noise 2D mode | `81 01 04 50 02/03 FF` | R14 current command input: `02` Auto, `03` Manual. |
| Noise 2D level | `81 01 04 53 0p FF` | R14 current command input: `p = 0` off, `1–5` level. |
| Noise 3D level | `81 01 04 54 0p FF` | R14 current command input: `p = 0` off, `1–8` level. |

For the current G2/G3 scope, R14 separately documents these command-input
domains and the inquiry-output domains in section 7.10. The current `09 04 53`
and `09 04 54` query rows return `0–5`; that narrower output range does not
retract the separately documented `04 54` input range through `8`. R1 is an
archived inquiry source, not a setter source: it contains the `09 04 50`/`53`/`54`
inquiries and records a `09 04 54` result range of `0–8`.

Typed session and direct structural decoding accept that full public `0–8`
domain on every supporting profile (#717). The profile marker still controls
whether the inquiry is available, but whole-profile identity no longer changes
the numeric range or rejects an otherwise well-formed setter readback [R1,
R14, R15].

### 7.9 Flip, OSD, NDI, multicast, and UAC

| Function | Packet / range | Status |
|---|---|---|
| Combined video flip off | `81 01 04 A4 00 FF` | Validated; preferred PTZOptics Gen‑2 orientation command. |
| Combined flip-H | `81 01 04 A4 01 FF` | Validated; preferred PTZOptics Gen‑2 orientation command. |
| Combined flip-V | `81 01 04 A4 02 FF` | Validated; preferred PTZOptics Gen‑2 orientation command. |
| Combined flip-HV | `81 01 04 A4 03 FF` | Validated; preferred PTZOptics Gen‑2 orientation command. |
| Legacy H-flip on/off | `81 01 04 61 02/03 FF` | Present in PTZOptics manual table; use combined `A4` for Gen‑2 implementation unless validating legacy behavior. |
| Legacy V-flip on/off | `81 01 04 66 02/03 FF` | Present in PTZOptics manual table; use combined `A4` for Gen‑2 implementation unless validating legacy behavior. |
| Save settings | `81 01 04 A5 10 FF` | Validated. Use after persistent orientation/configuration changes when required. |
| AWB sensitivity high/normal/low | `81 01 04 A9 00/01/02 FF` | Validated. |
| AF zone top/center/bottom | `81 01 04 AA 00/01/02 FF` | Validated. |
| AF zone value `03` | `81 01 04 AA 03 FF` | PTZOptics G2 bench (B1, 2026-10-04, #795): ACK `90 42`, completion `90 52`, reads back `03`. Not named by any source; see §7.13. |
| OSD open/close shortcut | `81 01 04 3F 02 5F FF` | Validated from PTZOptics table. |
| OSD navigation up/down/left/right | `81 01 06 01 0E 0E ... FF` | Validated. |
| OSD enter | `81 01 06 06 05 FF` | Validated. |
| OSD return/back | `81 01 06 06 04 FF` | Validated. |
| Menu status inquiry | `81 09 06 06 FF` | Validated; category `06`, not `04`. |
| NDI mode high/medium/low/off | `81 0B 01 01 01/02/03/04 FF` | Validated as PTZOptics vendor extension. Fixed opcode `0B 01 01`; the final data byte selects `01 = high`, `02 = medium`, `03 = low`, `04 = off`. |
| Multicast mode | `81 0B 01 23 0p FF`, `1 = On`, `2 = Off` | Validated as PTZOptics vendor extension. |
| USB audio / UAC | `81 2A 02 A0 04 0p FF`, `2 = On`, `3 = Off` | Validated as PTZOptics vendor extension. |
| UAC inquiry | `81 2A 02 A0 04 FF` | Validated. |

### 7.10 PTZOptics inquiry reference

The following inquiry rows consolidate the PTZOptics Gen‑2 inquiry tables.
Replies are shown for single-camera IP use (`90 ...`) where the source table
used `y0 ...` placeholders. The R14 noise-reduction rows are current G2/G3
evidence; R1 is an archived legacy inquiry reference whose distinct 3D output
range is called out below. Separately, R10/R14 independently document the G3
AE/iris rows identified below. R14 is a G3 source exactly for the rows it lists
itself; entries without their own G3 source row remain out of G3 scope. R14's
Queries page (checked 2026-10-05) lists no `09 04 AA` AF-zone or UAC row, so
those inquiries are G2/30X only.

| Inquiry | Packet | Reply / values |
|---|---|---|
| Power | `81 09 04 00 FF` | `90 50 02 FF` on; `90 50 03 FF` standby; `90 50 04 FF` internal power circuit error. |
| Zoom position | `81 09 04 47 FF` | `90 50 0p 0q 0r 0s FF`; `pqrs = Zoom Position`. |
| Focus AF mode | `81 09 04 38 FF` | `90 50 02 FF` auto; `90 50 03 FF` manual. |
| Focus position | `81 09 04 48 FF` | `90 50 0p 0q 0r 0s FF`; `pqrs = Focus Position`. |
| WB mode | `81 09 04 35 FF` | `00` auto, `01` indoor, `02` outdoor, `03` one-push, `05` manual, `20` color-temperature mode. |
| Color temperature | `81 09 04 20 FF` | Not in the vendor tables. The PTZOptics G2 bench (hardware inquiry test, #498) replies with one data byte, `90 50 pq FF`, where `pq` is the `04 20` direct code (`0x00` = 2500K … `0x37` = 8000K). No Sony source documents this reply, so the decoded layout is source-backed only for the PTZOptics G2 family (`PtzOpticsG2` and the legacy G2 `PtzOptics30X`). |
| Red gain | `81 09 04 43 FF` | `90 50 00 00 0p 0q FF`; `pq = R Gain`. |
| Blue gain | `81 09 04 44 FF` | `90 50 00 00 0p 0q FF`; `pq = B Gain`. |
| AE mode | `81 09 04 39 FF` | `00` full auto, `03` manual, `0A` shutter priority, `0B` iris priority, `0D` bright. R10/R14 independently document this row for G3. |
| Shutter position | `81 09 04 4A FF` | `90 50 00 00 0p 0q FF`; `pq = Shutter Position`. |
| Iris position | `81 09 04 4B FF` | `90 50 00 00 0p 0q FF`; `pq = Iris Position`. R10/R14 independently document this row for G3. |
| Bright position | `81 09 04 4D FF` | `90 50 00 00 0p 0q FF`; `pq = Bright Position`. |
| Exposure compensation mode | `81 09 04 3E FF` | `90 50 02 FF` on; `90 50 03 FF` off. |
| Exposure compensation position | `81 09 04 4E FF` | `90 50 00 00 0p 0q FF`; `pq = ExpComp Position`. |
| Backlight mode | `81 09 04 33 FF` | `90 50 02 FF` on; `90 50 03 FF` off. |
| Noise 2D mode | `81 09 04 50 FF` | R14 current G2/G3 reply: `90 50 02 FF` auto; `90 50 03 FF` manual. The source-table phrase "manual noise 3D" is a label inconsistency, not a distinct register. |
| Noise 2D level | `81 09 04 53 FF` | R14 current G2/G3 reply: `90 50 0p FF`; `p = 0–5`. |
| Noise 3D level | `81 09 04 54 FF` | R14 current G2/G3 reply: `90 50 0p FF`; `p = 0–5`. R1's archived legacy inquiry instead records `p = 0–8`. |
| Flicker mode | `81 09 04 55 FF` | `90 50 0p FF`; `p = 0` off, `1` 50Hz, `2` 60Hz. |
| Sharpness / aperture mode | `81 09 04 05 FF` | `90 50 02 FF` auto sharpness; `90 50 03 FF` manual sharpness. |
| Sharpness / aperture gain | `81 09 04 42 FF` | `90 50 00 00 0p 0q FF`; `pq = Aperture Gain`. |
| Menu mode | `81 09 06 06 FF` | `90 50 02 FF` on/open; `90 50 03 FF` off/closed. |
| Picture effect mode | `81 09 04 63 FF` | `90 50 02 FF` off; `90 50 04 FF` B&W. |
| Legacy LR reverse | `81 09 04 61 FF` | `90 50 02 FF` on; `90 50 03 FF` off. |
| Legacy picture flip | `81 09 04 66 FF` | `90 50 02 FF` on; `90 50 03 FF` off. |
| Color gain | `81 09 04 49 FF` | `90 50 00 00 00 0p FF`; `p = 0x0–0xE`. |
| Pan/tilt position | `81 09 06 12 FF` | `90 50 0w 0w 0w 0w 0z 0z 0z 0z FF`; `wwww = Pan Position`, `zzzz = Tilt Position`. |
| Gain limit | `81 09 04 2C FF` | `90 50 0q FF`; source comment labels `q`/`p` as gain limit. |
| AF sensitivity | `81 09 04 58 FF` | `90 50 01 FF` high; `90 50 02 FF` normal; `90 50 03 FF` low. |
| Luminance / brightness | `81 09 04 A1 FF` | `90 50 00 00 0p 0q FF`; `pq = Brightness Position`. |
| Contrast | `81 09 04 A2 FF` | `90 50 00 00 0p 0q FF`; `pq = Contrast Position`. |
| Gamma | `81 09 04 5B FF` | `90 50 0p FF`; `p = Gamma Curve Position`. |
| Combined flip | `81 09 04 A4 FF` | `90 50 00 FF` off, `01` flip-H, `02` flip-V, `03` flip-HV. |
| AF zone | `81 09 04 AA FF` | `90 50 00 FF` top, `01` center, `02` bottom. The PTZOptics G2 bench also returns `90 50 03 FF` (B1, 2026-10-04, #795); see §7.13. |
| Color hue | `81 09 04 4F FF` | `90 50 00 00 00 0p FF`; `p = hue setting`. |
| AWB sensitivity | `81 09 04 A9 FF` | `90 50 00 FF` high, `01` normal, `02` low. |
| UAC / USB audio | `81 2A 02 A0 04 FF` | `90 50 02 FF` on; `90 50 03 FF` off. |

R10/R14 independently establish the G3 `04 39` AE modes, iris
reset/up/down/direct (`04 0B`/`04 4B`), and matching `09 04 39`/`09 04 4B`
queries. Neither source lists the distinct standard `09 04 2B` iris auto/manual
status query, so that query is not exposed by any built-in profile. Sony's FR7
command list instead documents a vendor-relative `7E 04 4B` Iris Up/Down family
and `05 34` Auto Iris inquiry; it does not establish the shared `04 39`
AE-mode command/inquiry family, standard Iris Direct command, or `09 04 4B`
position inquiry. A generic VISCA opcode or encoder range alone does not
establish those typed or targeted-inquiry guarantees for FR7. BRC-H900,
EVI-H100, BRC-300 and Nearus BRC-300 use their own R11, R8, R12 and R21 rows;
Generic VISCA grants only the families that R8, R12 and R21 all document.

### 7.11 PTZOptics block inquiries

Block inquiries return compact multi-field status packets. The following rows are retained because the PTZOptics command source includes them explicitly.

| Block inquiry | Packet | Reply format / field notes |
|---|---|---|
| Lens block | `81 09 7E 7E 00 FF` | `90 50 0u 0u 0u 0u 00 00 0v 0v 0v 0v 00 0w 00 FF`; `uuuu = Zoom Position`, `vvvv = Focus Position`, `w.bit0 = Focus Mode` (`1 = Auto`, `0 = Manual`). |
| Camera block | `81 09 7E 7E 01 FF` | Reply carries R gain, B gain, WB mode, aperture, AE mode, backlight bit, exposure-compensation bit, shutter, iris, bright, and exposure-compensation position. |
| Other block | `81 09 7E 7E 02 FF` | Reply carries power bit, LR reverse bit, and picture-effect mode bits. |
| Enlargement block | `81 09 7E 7E 03 FF` | Reply carries AF sensitivity, picture-flip bit, color gain bits, combined flip value, NR2D level, and gain limit. |

### 7.12 PTZOptics extension status summary

| Extension / command family | Final treatment |
|---|---|
| Dynamic Range Control `81 01 04 25 00 00 00 0p FF` | Included in exposure command table as a PTZOptics extension. |
| Red/blue tuning `81 01 04 43 00 00 00 pq FF` / `81 01 04 44 00 00 00 pq FF` | Included in white-balance/color tuning table; shares the `04 43` / `04 44` opcode with red/blue gain direct (see the color-tuning erratum in 7.4). |
| Picture effect `81 01 04 63 00/04 FF` | Included in image-processing table and inquiry table. |
| Legacy `04 61` / `04 66` flip rows | Included for source completeness, but implementation should prefer combined `04 A4` on PTZOptics Gen‑2. |
| PTZ Motion Sync `81 0A 11 13 ...`, `81 0A 11 14 ...` | Kept firmware-gated in Appendix A.4 rather than baseline. |
| PTZOptics tally flash/on/off and reboot rows from broader unified notes | Not promoted into the validated baseline. They are retained as validation candidates in Appendix A.11. |

### 7.13 PTZOptics G2 bench results outside the vendor tables (2026-10-04)

Bench record B1 (#795) was taken on 2026-10-04 on the PTZOptics G2 bench: five
cameras (PT30X-NDI G2, PT20X-NDI G2 and PT12X-NDI G2; firmware ARM
6.3.51THI, 6.3.76THI and 6.4.18SHI) over raw VISCA on TCP `5678`.

| Exchange | Observed bytes | Treatment |
|---|---|---|
| AF zone inquiry | `81 09 04 AA FF` → `90 50 03 FF` on all five cameras | Decoded as `FocusZone::Zone03`. |
| AF zone set `03` | `81 01 04 AA 03 FF` → `90 42`, `90 52`; a following inquiry reads `03` | `FocusZoneCommand::new(FocusZone::Zone03)` encodes it. |
| AF zone set `01` | `81 01 04 AA 01 FF` → ACK and completion; a following inquiry reads `01` | `03` behaves as a peer of the documented values. |
| Version inquiry | `81 09 00 02 FF` → `90 50 00 52 FF` (2-byte payload `00 52`) on all five cameras | Not decoded. The PTZOptics profiles do not implement `HasVersionInquiry`; use `raw::Inquiry` for the bytes. |
| Socket cancel (2026-10-05) | With a slow absolute pan/tilt move executing on socket y (ACK `90 4y`), `81 2y FF` → `90 60 02 FF` (syntax error); the move continued to its target and completed (`90 5y`). Observed on PT30X-NDI G2 (ARM 6.3.51THI) and PT12X-NDI G2 (ARM 6.4.18SHI). | The G2 family does not implement socket cancel: `PtzOpticsG2` and `PtzOptics30X` report `supports_command_cancel = false`. G3 was not tested and no source documents the command, so `PtzOpticsG3` keeps it off as well. |

**AF zone `03`.** No source names value `03`. The R14 portal's Focus page (R20)
(`CAM_AFZone`, "AF Zone weight select") lists only Top `00`, Center `01` and
Bottom `02`, and so do the `CAM_AFZone` command and inquiry rows of the
MSolutions command guide (R16) and the mirrored VISCA list (R19). The Minrray
UV1201 manual (R18) has no `CAM_AFZone` row; its OSD lists three options,
`AF-Zone: Up, middle, down`. PTZOptics G2 user manuals list the web/OSD
`AF-Zone` options as Top, Center and Bottom (seen only in search excerpts; the
PDFs returned HTTP 404 when fetched on 2026-10-04). The camera web UI's autofocus
lock/unlock control is the separate focus-lock command `81 0A 04 68 02/03 FF`
(§7.3); nothing links it to `04 AA 03`. The library therefore names the value
`FocusZone::Zone03` and claims only that it is a readable and restorable zone
value. The typed setter admits it only for the profiles whose cameras were on
the bench (`PtzOpticsG2` and the legacy G2 `PtzOptics30X`); `PtzOpticsG3`
admits the documented values only.

**Version inquiry `00 52`.** No source defines a 2-byte reply. The Sony layout
in R12 is `Y0 50 GG GG HH HH JJ JJ KK FF` (vendor, model, ROM revision,
maximum socket). The OEM lists for related firmware use other, longer layouts:
R16 gives `y0 50 ab cd mn pq rs tu vw FF` (factory code, hardware version, ARM
version, FPGA version, camera model), R18 gives vendor `0220`, model, ARM
version and a reserved byte, and R17 gives Sony vendor `00 01` with model,
ROM and socket. Neither R1 nor R14 documents `09 00 02` for PTZOptics. Because
`00 52` matches none of these, the library assigns its bytes no meaning.

## 8. Axis VISCA profile reference

Axis VISCA behavior is validated as an Axis profile. Do not copy these numeric ranges into PTZOptics profiles.

### 8.1 Axis camera commands

| Function | Packet / range | Notes |
|---|---|---|
| Continuous zoom stop | `8x 01 04 07 00 FF` | Axis. |
| Continuous zoom tele/wide fixed | `8x 01 04 07 02/03 FF` | Axis. |
| Continuous zoom tele/wide variable | `8x 01 04 07 2p/3p FF`, `p = 0–7` | Axis. |
| Absolute zoom | `8x 01 04 47 0p 0q 0r 0s FF` | Use corrected direct-zoom opcode `04 47`; `pqrs = position`. |
| Axis optical zoom range | `0x0000–0x4000` | Axis-only. |
| Axis digital zoom range | `0x4001–0x7AC0` | Axis-only. |
| Continuous focus stop | `8x 01 04 08 00 FF` | Axis. |
| Continuous focus far/near fixed | `8x 01 04 08 02/03 FF` | Axis. |
| Continuous focus far/near variable | `8x 01 04 08 2p/3p FF`, `p = 0–7` | Axis. |
| Absolute focus | `8x 01 04 48 0p 0q 0r 0s FF` | `pqrs = position`; Axis focus range `0x1000–0xF000`. |
| Auto focus | `8x 01 04 38 pq FF` | `0x10 = toggle`, `0x02 = on`, `0x03 = off`. |
| Focus near limit | `8x 01 04 28 00 00 00 0p FF` | `p = distance`; see section 8.2. |
| White balance | `8x 01 04 35 0p FF` | `0x0` auto, `0x1` fixed indoor, `0x2` fixed outdoor, `0x4` auto outdoor, `0x5` manual. |
| White balance one-push trigger | `8x 01 04 10 05 FF` | Axis camera command. |
| Relative red gain | `8x 01 04 03 0p FF` | `0x0` reset to 50%, `0x2` up/more, `0x3` down/less. |
| Absolute red gain | `8x 01 04 43 00 00 0p 0q FF` | `pq = gain`, `0x00–0xFF`. |
| Relative blue gain | `8x 01 04 04 0p FF` | `0x0` reset to 50%, `0x2` up/more, `0x3` down/less. |
| Absolute blue gain | `8x 01 04 44 00 00 0p 0q FF` | `pq = gain`, `0x00–0xFF`. |
| Auto exposure | `8x 01 04 39 0p FF` | `0x0` full auto, `0x3` full manual, `0xA` shutter priority, `0xB` iris priority, `0xD` bright mode. |
| Relative shutter | `8x 01 04 0A 0p FF` | `0x0` reset, `0x2` up/shorter, `0x3` down/longer. |
| Absolute shutter | `8x 01 04 4A 00 00 0p 0q FF` | `pq = time`; see section 8.3. |
| Tally control | `8x 01 7E 01 0A 00 0p FF` | Axis extended command; `0x2 = on`, `0x3 = off`. |

### 8.2 Axis focus-near-limit distance table

| `p` value | Distance |
|---|---:|
| `0x2` | 20.00 m |
| `0x3` | 10.00 m |
| `0x4` | 6.00 m |
| `0x5` | 4.20 m |
| `0x6` | 3.10 m |
| `0x7` | 2.50 m |
| `0x8` | 2.00 m |
| `0x9` | 1.65 m |
| `0xA` | 1.40 m |
| `0xB` | 1.20 m |
| `0xC` | 0.80 m |
| `0xD` | 0.30 m |
| `0xE` | 0.11 m |

### 8.3 Axis absolute shutter speed values

| Value | 50Hz Mode | 60Hz Mode |
|---|---|---|
| `0x00` | 1/1s | 1/1s |
| `0x01` | 1/2s | 1/2s |
| `0x02` | 1/3s | 1/4s |
| `0x03` | 1/6s | 1/8s |
| `0x04` | 1/12s | 1/15s |
| `0x05` | 1/25s | 1/30s |
| `0x06` | 1/50s | 1/60s |
| `0x07` | 1/75s | 1/90s |
| `0x08` | 1/100s | 1/100s |
| `0x09` | 1/120s | 1/125s |
| `0x0A` | 1/150s | 1/180s |
| `0x0B` | 1/215s | 1/250s |
| `0x0C` | 1/300s | 1/350s |
| `0x0D` | 1/425s | 1/500s |
| `0x0E` | 1/600s | 1/725s |
| `0x0F` | 1/1000s | 1/1000s |
| `0x10` | 1/1250s | 1/1500s |
| `0x11` | 1/1750s | 1/2000s |
| `0x12` | 1/2500s | 1/3000s |
| `0x13` | 1/3500s | 1/4000s |
| `0x14` | 1/6000s | 1/6000s |
| `0x15` | 1/10000s | 1/10000s |

### 8.4 Axis pan/tilt commands and ranges

| Function | Packet / range | Notes |
|---|---|---|
| Continuous pan/tilt | `8x 01 06 01 pp tt xx yy FF` | `pp = 0x01–0x18`; `tt = 0x01–0x17`. |
| Absolute pan/tilt | `8x 01 06 02 pp tt 0g 0h 0i 0j 0k 0l 0m 0n FF` | Axis two’s-complement coordinates. |
| Axis absolute pan range | `0xDE00–0x2200` | `0xDE00 = −0x2200`. |
| Axis absolute tilt range | `0xFC00–0x2200` for absolute command; `0xFC00–0x1200` for limits/inquiry | Axis-only. |
| Relative pan range | `0xBC00–0x4400` | `0xBC00 = −0x4400`. |
| Relative tilt range | `0xEA00–0x1600` | `0xEA00 = −0x1600`. |
| Home | `8x 01 06 04 FF` | Axis. |
| Reset pan/tilt | `8x 01 06 05 FF` | Axis. |
| Set pan/tilt limits | `8x 01 06 07 00 0p ... FF` | `p = corner`; down-left `0x0`, up-right `0x1`. |

### 8.5 Axis inquiries

| Inquiry | Packet | Reply behavior |
|---|---|---|
| Version | `X 09 00 02 FF` | `9y 50 41 58 56 pq rs 00 02 FF`; `pqrs = product number`, such as `5925` for AXIS V5925. |
| Power | `8x 09 04 00 FF` | Fixed to on: `y0 50 02 FF`. |
| Auto focus | `8x 09 04 38 FF` | `0x2 = on`, `0x3 = off`. |
| White balance | `8x 09 04 35 FF` | Axis WB modes. |
| Auto exposure | `8x 09 04 39 FF` | Axis AE modes: `0x0`, `0x3`, `0xA`, `0xB`, `0xD`. |
| Backlight compensation | `8x 09 04 33 FF` | `0x2 = on`, `0x3 = off`. |
| Spotlight mode | `8x 09 04 3A FF` | Fixed off. |
| Exposure compensation | `8x 09 04 3E FF` | Fixed off. |
| Zoom position | `8x 09 04 47 FF` | `y0 50 0p 0q 0r 0s FF`; `pqrs` position; Axis optical/digital ranges. |
| Pan/tilt position | `8x 09 06 12 FF` | `y0 50 0g 0h 0i 0j 0k 0l 0m 0n FF`; Axis two’s-complement ranges. |
| Menu display | `8x 09 06 06 FF` | Fixed off in Axis API. |

### 8.6 Axis absolute zoom erratum

The public Axis VISCA Interface API PDF’s camera command table shows the absolute zoom row as `8x 01 04 07 02 FF`, while the same row describes `pqrs = position` and optical/digital zoom ranges. That packet is the standard continuous tele command, not a direct-position packet. The Axis zoom-position inquiry is `8x 09 04 47 FF`, and standard VISCA direct zoom is `04 47`. Therefore the validated implementation command is:

```text
8x 01 04 47 0p 0q 0r 0s FF
```

## 9. Standard Sony VISCA notes used in this document

### 9.1 Encapsulated VISCA-over-IP

Sony VISCA-over-IP uses:

- UDP transport.
- Port `52381`.
- An 8-byte message header.
- The payload length is carried in the header. The usual command families fit
  `1–16` bytes; FR7 position commands use 17 bytes (R7 p. 18). The library
  checks each profile's supported request capacity before admission.
- A sequence number stored in bytes 4–7.
- Camera address fixed to `1` for VISCA-over-IP.

### 9.2 Command and inquiry behavior

Sony’s command manual confirms the same basic VISCA behavior used throughout this document:

- Commands receive ACK then Completion.
- Inquiries do not receive ACK; they return the reply data.
- The camera has two command sockets.
- Common error packets include Syntax Error, Command Buffer Full, Command Canceled, No Socket, and Command Not Executable.

### 9.3 Bright, Gamma, spotlight, and automatic slow shutter

Sony’s EVI-H100S/H100V technical manual confirms:

- Bright up/down use `04 0D`.
- Bright direct uses `04 4D`.
- Gamma set uses `04 5B`; Gamma inquiry uses `09 04 5B`.

The fixed typed serializers are model-specific and use this narrow matrix:

| Model | Source | Retained fixed command family | Deliberately absent family |
| --- | --- | --- | --- |
| Sony FR7 | R7 | Spotlight `04 3A` | Automatic slow shutter `04 5A` |
| Sony BRC-H900 | R11 | Spotlight `04 3A` | Automatic slow shutter `04 5A` |
| Sony EVI-H100 | R8 | Automatic slow shutter `04 5A` | Spotlight `04 3A` |
| Sony BRC-300 | R12 | Automatic slow shutter `04 5A` | Spotlight `04 3A` |
| Nearus BRC-300 | R21 | Automatic slow shutter `04 5A` | Spotlight `04 3A` |

For PTZOptics Gen-2, the checked sources do not establish the same
automatic-slow-shutter inquiry, so this table is not a generic-VISCA grant.

### 9.4 Sony FR7 typed extension rows

R7 p. 12 gives pan `09CA7` (+170° CCW) to `F6359` (−170° CW), desktop
tilt `FE45B` (−30°) to `0B3B0` (+195°), and ceiling tilt `F3E7D` (−210°)
to `00DD2` (+15°). Both axes have magnitude 235.9 units/degree. Logical
rightward pan is clockwise, so the library uses a negative pan scale; upward
tilt uses a positive scale. R7's nominal +195° and +15° endpoints sit in a
half-unit rounding cell; conversion snaps that cell to the documented integer
endpoint and otherwise rounds to nearest with halves away from zero.

R7 pp. 18 and 22 use five signed nibbles for **each** pan/tilt axis: absolute
`8x 01 06 02 vv ww 0p 0p 0p 0p 0p 0t 0t 0t 0t 0t FF`, relative
`8x 01 06 03 vv ww 0p 0p 0p 0p 0p 0t 0t 0t 0t 0t FF`, and position reply
`y0 50 0p 0p 0p 0p 0p 0t 0t 0t 0t 0t FF`. Limit set uses the same words;
limit clear writes `07 0F 0F 0F 0F` on each axis. These complete command frames
are seventeen bytes, despite the older R7 transport introduction stating a
sixteen-byte payload ceiling; the model command rows take precedence for these
FR7 commands. This does not establish a general Sony payload maximum.


The following R7 rows are the narrow source basis for FR7-only or
Sony-professional typed surfaces. `8x` is the VISCA camera address before Sony
IP framing normalizes it to camera 1; application code does not build the
eight-byte envelope itself.

| Typed surface | Command or inquiry packet | R7 value domain |
| --- | --- | --- |
| Automatic ND | `8x 01 7E 04 53 0p FF` | `p = 2` on; `p = 3` off. |
| ND variable direct | `8x 01 7E 04 42 00 00 0p 0p FF` | R7 v4 p. 14 uses two zero padding nibbles before the two-digit density code. |
| ND variable inquiry | `8x 09 7E 04 42 FF` | Reply `y0 50 00 00 0p 0p FF`; variable-density code `00..14` hex, from 1/4 to 1/128. R7 v4 p. 21 gives the packet and p. 25 the value table. `NdFilterValue` represents this code; it is distinct from clear/preset selection. |
| ND preset inquiry | `8x 09 7E 01 53 FF` | Reply `y0 50 0p FF`; `p = 0` clear, `1..=3` preset 1–3. The legacy `09 04 66` register in §7.10 is picture-flip state, not this inquiry. |
| Red tally | `8x 01 7E 01 0A 00 0p FF` | `p = 2` on; `p = 3` off. Inquiry: `8x 09 7E 01 0A FF`. |
| Green tally | `8x 01 7E 04 1A 00 0p FF` | `p = 2` on; `p = 3` off. Inquiry: `8x 09 7E 04 1A FF`. |
| ND mode | `8x 01 7E 04 52 0p FF` | R7 p. 14: `0` preset, `1` variable. |
| ND step | `8x 01 7E 04 12 02/03 FF` | R7 p. 14: up/down, with the density domain established by the separate variable-ND table. |
| Push AF / Push MF | `8x 01 7E 04 58 0p FF` | `p = 0` release; `p = 1` press. This is distinct from the red-tally `7E 01 0A` family. |
| Pan/tilt speed-step range | `8x 01 06 45 pp FF` | `pp = 08` normal (24 steps); `pp = 18` extended (50 steps). `7E 04 1B` is FR7 preset-speed selection, not this control. |
| Auto-tracking white balance | `8x 01 04 35 04 FF` | `04` selects ATW; inquiry `8x 09 04 35 FF` reports `04`. The `09 04 A9` AWB-sensitivity row in §7.10 is a separate PTZOptics register. |
| Direct menu | `8x 01 7E 04 72 pp 0q FF` | `pp = 00` ND Filter, `01` Iris, `02` ISO/Gain, `03` Shutter, `04` AE Level/Mode, or `7F` exit; `q = 0/1` is release/press (R7 v4.00 p. 20). `DirectMenuTarget` and `MenuButton` close both typed domains. |

The variable-density query is not the unsourced legacy `09 04 64` packet.
Its four-nibble reply requires zero padding and a code in `00..14` hex; typed
readback and settlement compare `NdFilterValue` density codes (#844). A setter
or a preset query does not establish a different inquiry register.

These rows do not grant FR7 the shared `04 39` exposure-mode or standard
`04 4B` iris families; the profile boundary in §2.3 and Appendix A.9 still
applies.

## 10. Resolved inconsistencies and final treatments

| # | Issue | Final treatment | Confidence |
|---:|---|---|---|
| 1 | PT30X focal length: `88.5mm` vs `132.6mm` | Use `132.6mm`. The `88.5mm` value belongs to 20× and is inconsistent with 30× zoom and PT30X tele HFOV. | High |
| 2 | PT30X/PT20X max height metric conversion | Reject `168mm` as a conversion/copy error where paired with `7.8in`; use model datasheet values for mechanical planning and verify with drawings before installation drawings. | Medium |
| 3 | PT12X `720p120` | Treat as IP/NDI®\|HX stream-only. | High |
| 4 | PTZOptics raw ports | UDP `1259`, TCP `5678`; raw VISCA bytes. | High |
| 5 | PTZOptics Sony VISCA port | UDP `52381`; firmware-gated. | High for port; medium for all legacy firmware. |
| 6 | Raw vs encapsulated VISCA-over-IP | Keep as distinct protocol modes. | High |
| 7 | Preset count | Path-specific: IR 10, spec capacity 255, raw VISCA table 0–127, HTTP/web UI 0–254. | High for source statements; medium for raw `>0x7F`. |
| 8 | Bright Direct opcode | Use `04 4D`; treat `04 0D` Direct as erratum. | High |
| 9 | Gamma `0x5B` | Include; PTZOptics label as hardware-tested/patched-note support. | Medium-high for PTZOptics; high for Sony standard. |
| 10 | Luminance/contrast parameter width | Use `0p 0q`. | High |
| 11 | Sharpness range | Packet accepted; full `0x00–0x0F` range should remain hardware-profile-specific. | Medium |
| 12 | PTZ Motion Sync | Firmware-specific / experimental; not baseline. | Medium-high |
| 13 | OnePush/Snap Focus | Firmware-specific; verify exact target firmware/command path. | Medium-high |
| 14 | Image freeze | Firmware-specific; verify exact command path. | Medium |
| 15 | Axis numeric ranges | Axis-only. | High |
| 16 | PTZOptics digital zoom via VISCA | Specs list digital zoom; safe VISCA enable/disable behavior remains unvalidated. | Medium |
| 17 | Auto slow shutter inquiry | Sony documents it; PTZOptics Gen‑2 sources checked here do not. Treat as profile-specific. | High |
| 18 | Menu status inquiry category | Use `06`: `81 09 06 06 FF`. | High |
| 19 | Axis red/blue gain, AE, shutter, and focus-near-limit details | Include in the Axis profile section with Axis-only range labels and the full shutter/distance tables. | High |
| 20 | PTZOptics inquiry coverage | Include individual inquiry and block-inquiry tables; avoid inventing unsupported gain/digital-zoom inquiries. | High |
| 21 | PTZOptics legacy flip rows | Document `04 61` and `04 66` as source-table rows but prefer combined `04 A4` for Gen‑2 implementation. | Medium-high |
| 22 | PTZOptics tally/reboot rows from broader unified guide | Keep as Appendix A validation candidates rather than baseline commands until hardware-tested. | Medium |
| 23 | Product/spec details | Include stream, interface, dimensions, packaging, and box-content tables in Appendix B rather than mixing them into protocol rows. | High |
| 24 | Pelco-D/Pelco-P | Explicitly scope out of the VISCA main reference; preserve source traceability and compact serial-reference notes in Appendix A.12. | High |
| 25 | Broader unified-guide model sections | Background only until each model is validated against primary manuals. | High |

## 11. Implementation guardrails

### 11.1 Select the profile before sending bytes

Pick exactly one protocol profile for a connection: PTZOptics raw VISCA, PTZOptics Sony VISCA mode, Axis VISCA, or Sony encapsulated VISCA-over-IP. Do not infer profile from opcode shape alone; the same payload bytes can require different transport envelopes and may have different numeric ranges.

### 11.2 Transport envelope rules

| Path | Rule |
|---|---|
| PTZOptics raw UDP `1259` | Send bare VISCA bytes such as `81 09 04 00 FF`; no Sony header. |
| PTZOptics raw TCP `5678` | Send bare VISCA bytes; TCP improves delivery ordering but does not remove VISCA socket limits. |
| Sony VISCA-over-IP / PTZOptics Sony VISCA mode | Use UDP `52381` and prepend Sony’s 8-byte header. |
| Axis VISCA | Use Axis-profile commands and ranges; do not apply Axis numeric ranges to PTZOptics. |

### 11.3 Serial startup and recovery

The following commands are serial-only. Do not send them to raw or encapsulated IP VISCA connections.

| Command | Packet | Purpose | Expected handling |
|---|---|---|---|
| Address Set | `88 30 01 FF` | Broadcast daisy-chain address assignment. | Send after power-up or network-change/hot-plug in a serial chain; final reply indicates the number of assigned cameras. |
| I/F Clear | `88 01 00 01 FF` | Broadcast interface clear. | Owner-coordinated serial setup/recovery control; send after Address Set or to recover from command-buffer state; clears sockets and cancels pending serial commands. |

Address Set and I/F Clear belong to the serial handshake owner; they are not
generic public commands. Socket cancellation is likewise owner-only because
the owner alone knows which live operation owns a camera-assigned socket.

### 11.4 Command scheduling, sockets, and cancel

VISCA cameras generally expose two command sockets. Maintain an owner-only
per-camera scheduler. Raw VISCA normally gates the target's one unacknowledged
command, then tracks the ACK-assigned sockets and frees each socket on
Completion or an error. One intrinsically `Urgent` command may cross one open
candidate for safety (#714); while both are open, ACK/error evidence binds to
neither rather than using FIFO order or recency. Sony sequence numbers permit
pre-ACK pipeline and exact reply correlation.

| Action | Packet / behavior |
|---|---|
| Cancel socket 1 | `81 21 FF` |
| Cancel socket 2 | `81 22 FF` |
| Cancel success | Camera returns `90 6y 04 FF`; no normal completion follows for that canceled command. |
| Cancel invalid/empty socket | Camera returns `90 6y 05 FF`. |

Use cancel for explicit user interrupts or emergency stops, not as a normal substitute for command pacing. Owner-issued cancellation is selected ahead of ordinary queued work, but it still obeys the shared profile-specific minimum spacing between sends and advances that spacing deadline. The uploaded unified guide gives `100 ms` as a hardware-tested PTZOptics G2/G3/30X starting point and `35 ms` for Sony FR7/BRC-H900; treat those as scheduler defaults that should be verified per target firmware and network path.

### 11.5 Retry policy

One total retry wall-clock budget starts at admission and remains active through
every later noncancelled attempt phase: backoff, ready, send, ACK, execution,
and reply.
When that budget expires, the terminal result preserves the cause that
authorized the prior retry rather than replacing it with an incidental timeout
from a later phase. The architecture guide owns the detailed
[raw unconfirmed-outcome rule](architecture_2_0.md#raw-unconfirmed-outcomes-and-strict-recovery).
Empty UDP datagrams are discarded while receiving and do not reset or extend
the one overall receive deadline; the async adapter yields cooperatively before
polling again.

| Transport | Recommended retry behavior |
|---|---|
| Raw UDP PTZOptics | Never replay a successfully sent command whose result is ambiguous. Follow the architecture guide's per-request quarantine rule; a conclusive camera rejection may still be retried. Prefer TCP `5678` for high-reliability control. |
| Raw TCP PTZOptics | TCP preserves byte delivery/order but does not prove camera execution. Use the same canonical raw quarantine rule; retain socket concurrency after an ACK and never infer an ambiguous outcome from FIFO order. |
| Sony encapsulated UDP | Use the Sony sequence field to correlate replies. This document adopts the Sony-manual correction in §5.3: timeout recovery should retransmit the timed-out message with the same sequence number, rather than blindly issuing a new logical command. The library applies this only while the message is unacknowledged: once the camera ACKs it, acceptance is proven and the message is never retransmitted (#795). |
| Axis | Respect Axis profile ranges and handle fixed replies, especially for inquiries documented as fixed on/off. |

A receive that proves the connection is gone is reported as
`ConnectionClosed`, with the underlying transport error's text retained in the
closure reason. `StreamPoisoned` is reserved for a stream whose framing or
write position is unknowable; a failed read consumes no bytes and is not, by
itself, evidence of stream poison.

### 11.6 Version inquiry and capability mapping

Use Version Inquiry as a profile-identification aid, not as the only source of truth:

```text
81 09 00 02 FF
```

The reply format and product fields are vendor-specific. A robust controller should combine user-selected profile, transport path, version reply, and target-firmware observations to enable or disable features such as Motion Sync, image freeze, Sony VISCA mode, raw preset numbers above `0x7F`, and digital-zoom toggles.

### 11.7 OSD and menu handling

The menu-status inquiry is:

```text
81 09 06 06 FF
```

Do not use `81 09 04 06 FF` for menu status. If the OSD/menu is open, some cameras may ignore or reject normal PTZ/camera controls. A controller should either disable non-menu controls while the menu is open, provide an explicit close/back operation, or retry after closing the menu when appropriate.

### 11.8 Diagnostics and logging

For each camera connection, log at least: transport profile, destination IP/port or serial device, exact bytes sent, exact bytes received, timestamps, sequence number if applicable, socket number, and decoded ACK/Completion/Error state. Keep raw hex logs available for support/debugging, but disable verbose logging by default in production.

### 11.9 Safe validation sequence for a new target

1. Confirm profile and transport by sending Power Inquiry on the intended path.
2. Confirm ACK/Completion behavior with a harmless command such as zoom stop or pan/tilt stop.
3. Confirm Version Inquiry and record the reply.
4. Confirm Bright Direct `04 4D`, Gamma `04 5B`, and the menu-status category before exposing those controls.
5. Only then test firmware-gated or unresolved features from Appendix A.

# Appendix A — Remaining validation notes and issue links

The items in this appendix are **not** accepted as fully validated implementation facts. They are retained so future validation work has context, links, and explicit test criteria.

## A.1 Raw PTZOptics VISCA preset behavior above `0x7F`

**Current treatment:** The main document accepts PTZOptics device capacity as 255 presets via serial/IP, but only guarantees the raw VISCA command table’s documented `pp = 0–127` range.

**Why it remains open:** PTZOptics specs and product pages say 255 presets via serial/IP, while the raw VISCA command sheet says memory number `0–127`. Web/manual paths may use zero-based `0–254`, but that does not prove raw VISCA packet behavior for `0x80–0xFE`.

**Validation needed:** On at least one representative Gen‑2 target, test set/recall/reset for preset values `0x7F`, `0x80`, `0xFE`, and `0xFF`, then record ACK/completion/error behavior and whether presets persist.

**Helpful sources:**

- PTZOptics VISCA over IP Commands, Rev 1.2, memory row documents `0–127`: https://ptzoptics.com/wp-content/uploads/2020/11/PTZOptics-VISCA-over-IP-Rev-1_2-8-20.pdf
- PTZOptics NDI camera page, preset capacity rows: https://ptzoptics.com/ndi/
- Uploaded PTZOptics Gen‑2 NDI®\|HX specs file: `PTZOptics-NDI-HX-Gen2-Specs(1).md`

## A.2 PTZOptics digital zoom enable/disable via VISCA

**Current treatment:** The main document accepts that PTZOptics Gen‑2 specs list digital zoom, but it does not expose Sony-style digital-zoom enable/disable as a validated PTZOptics Gen‑2 VISCA feature.

**Why it remains open:** Sony EVI-H100 documents `CAM_DZoomModeInq` under `09 04 06`, and Axis documents optical/digital zoom ranges in its own profile. The checked PTZOptics Gen‑2 command list establishes direct zoom `04 47` but does not establish Sony-style digital zoom toggle behavior. Uploaded notes indicate PTZOptics testing returned syntax errors for the Sony-style digital-zoom toggle/inquiry, so profile-safe code should not expose it without target testing.

**Validation needed:** Test `81 01 04 06 02 FF`, `81 01 04 06 03 FF`, and `81 09 04 06 FF` on the target firmware. Also test direct zoom values beyond the optical endpoint, if a calibrated optical endpoint is known.

**Helpful sources:**

- PTZOptics NDI camera specs list digital zoom: https://ptzoptics.com/ndi/
- Sony EVI-H100S/H100V Technical Manual, digital zoom inquiry family: https://www.sony.com/electronics/support/res/manuals/AE4U/AE4U1001M.pdf
- Axis VISCA Interface API, Axis-only optical/digital ranges: https://www.axis.com/dam/public/70/3d/31/visca-interface-api-description-en-US-266656.pdf
- Uploaded Unified VISCA Protocol Implementation Guide notes: `visca_unified_reference(2).md`

## A.3 Sharpness / aperture direct range on PTZOptics

**Current treatment:** PTZOptics built-in profiles expose typed sharpness over
`0x00–0x0F` and encode `81 01 04 42 00 00 0p 0q FF` for that full range.
Profiles without source-backed sharpness support do not expose the typed
sharpness control or inquiry accessors.

**Why the scope stays profile-specific:** Uploaded patched notes report hardware
acceptance of `0x00–0x0F`, while some generic documentation uses narrower or
unspecified ranges. The wider range is therefore PTZOptics-profile metadata, not
a global VISCA default.

**Validation needed:** Sweep `0x00–0x0F` on additional supported firmware
families and record the resulting inquiry value from `81 09 04 42 FF`.

**Helpful sources:**

- PTZOptics VISCA over IP Commands, sharpness packet: https://ptzoptics.com/wp-content/uploads/2020/11/PTZOptics-VISCA-over-IP-Rev-1_2-8-20.pdf
- Uploaded PTZOptics G2 patched command reference: `PTZOptics-G2-VISCA-over-IP-Command-List(2).md`

## A.4 PTZ Motion Sync support and behavior

**Current treatment:** Motion Sync opcodes are documented as raw/vendor-specific references, not baseline Gen‑2 support.

**Why it remains open:** PTZOptics command tables include Motion Sync opcodes, and the firmware changelog labels PTZ Motion Sync as experimental in 2019-era firmware. Newer changelog entries discuss Motion Sync UI text and drift after preset recall, showing that the feature exists on at least some products/firmware, but not proving uniform Gen‑2 support.

**Validation needed:** For each target firmware, test:

```text
81 0A 11 13 02 FF   # Motion Sync on
81 0A 11 13 03 FF   # Motion Sync off
81 0A 11 14 pp FF   # lower speed limit / speed stage
```

Record ACK/completion behavior, preset recall behavior, and any drift or overshoot after manual PT commands.

**Helpful sources:**

- PTZOptics VISCA over IP Commands, Motion Sync rows: https://ptzoptics.com/wp-content/uploads/2020/11/PTZOptics-VISCA-over-IP-Rev-1_2-8-20.pdf
- PTZOptics firmware changelog, experimental Motion Sync and later Motion Sync notes: https://ptzoptics.com/firmware-changelog/

## A.5 OnePush Auto Focus / Snap Focus

**Current treatment:** Firmware-specific. Do not expose as a universal Gen‑2 capability without firmware checks.

**Why it remains open:** PTZOptics firmware changelog says OnePush Auto Focus VISCA / VISCA-over-IP / HTTP-CGI commands were added in SOC `6.2.97` for PoE models, and later changelog entries add a Snap Focus UI feature. The checked command tables do not provide a complete, stable opcode mapping for all target firmware branches.

**Validation needed:** Identify exact firmware branch and test the documented/available command path. Record whether the feature is a VISCA opcode, HTTP-CGI route, web UI function, or only a later UI abstraction.

**Helpful sources:**

- PTZOptics firmware changelog, OnePush Auto Focus and Snap Focus entries: https://ptzoptics.com/firmware-changelog/
- PTZOptics VISCA over IP Commands: https://ptzoptics.com/wp-content/uploads/2020/11/PTZOptics-VISCA-over-IP-Rev-1_2-8-20.pdf
- Uploaded PTZOptics Gen‑2 command notes: `PTZOptics-G2-VISCA-over-IP-Command-List(2).md`

## A.6 Image freeze command path on PTZOptics

**Current treatment:** Image freeze is supported as a product/firmware feature, but the exact VISCA command path is not validated for PTZOptics Gen‑2.

**Why it remains open:** PTZOptics specs list image freeze, and the firmware changelog says network feed image freeze was supported in SOC `6.3.12` for PoE models. Sony models often use freeze opcode `81 01 04 62 02/03 FF`, but this command path is not established for PTZOptics Gen‑2 by the checked primary PTZOptics command table.

**Validation needed:** Test `81 01 04 62 02 FF` and `81 01 04 62 03 FF`; also check whether freeze is OSD-only, HTTP-only, stream-only, or requires camera restart.

**Helpful sources:**

- PTZOptics firmware changelog, network feed image freeze: https://ptzoptics.com/firmware-changelog/
- PTZOptics NDI page/specs list image features: https://ptzoptics.com/ndi/
- Sony VISCA references for the Sony freeze family should be checked per target model before reuse.

## A.7 PTZOptics Sony VISCA mode on target firmware

**Current treatment:** UDP `52381` is accepted as the Sony VISCA port, and PTZOptics firmware support is known to be firmware-gated.

**Why it remains open:** The port value is validated by PTZOptics controller documentation, and firmware changelog confirms Sony VISCA-over-IP was added in SOC `6.3.12` for PoE models. A target camera may run older firmware or a branch that behaves differently.

**Validation needed:** Confirm firmware version and test a Sony-style encapsulated inquiry, such as Power Inquiry with payload type `01 10`, payload length `00 05`, sequence number, and payload `81 09 04 00 FF` on UDP `52381`.

**Helpful sources:**

- PTZOptics SuperJoy G1 User Manual, port assignments: https://ptzoptics.com/wp-content/uploads/2021/03/PT-SUPERJOY-G1-User-Manual.pdf
- PTZOptics firmware changelog, SOC `6.3.12`: https://ptzoptics.com/firmware-changelog/
- Sony ILME-FR7 VISCA Command List, Version 4.00, encapsulated VISCA-over-IP format: https://pro.sony/s3/2022/09/03065933/VISCA_Command_List_v4.pdf

## A.8 Current PTZOptics NDI page field-of-view inconsistencies

**Current treatment:** Use the validated uploaded PTZOptics NDI®\|HX Gen‑2 specs table and model datasheets for field-of-view values.

**Why it remains open:** The current PTZOptics NDI page confirms the key focal lengths and many network/control values, but its text extraction contains apparent field-of-view inconsistencies for some 20×/30× rows. For example, the page confirms the 30× focal length as `F4.42mm–132.6mm`, but other extracted FOV rows appear to repeat values inconsistently. Use primary per-model datasheets and mechanical drawings for installation-critical values.

**Validation needed:** Re-check the rendered page/table manually or use current downloadable datasheets from PTZOptics for PT12X/PT20X/PT30X before publishing installation documents.

**Helpful sources:**

- PTZOptics NDI camera page: https://ptzoptics.com/ndi/
- PT30X-NDI datasheet: https://f.hubspotusercontent20.net/hubfs/418770/PTZOptics%20Documentation/PT30X-NDI-xx/PT30X-NDI-xx%20Data%20Sheet.pdf
- Uploaded PTZOptics Gen‑2 NDI®\|HX specs: `PTZOptics-NDI-HX-Gen2-Specs(1).md`

## A.9 Broader Sony FR7 / BRC / EVI / Nearus / third-party model sections

**Current treatment:** Background only. Do not present those profiles as fully validated in this PTZOptics/Axis document.

**Narrow registry exception:** The built-in Sony FR7 entry uses the primary
FR7 command-list evidence in R7 for variable-ND controls/inquiries and fixed
`04 3A` spotlight controls. R7 also documents a distinct vendor-relative
`7E 04 4B` iris Up/Down family and `05 34` Auto Iris inquiry, retained only
through the raw-command escape hatch until they have their own typed models.
R7 does not establish the shared `04 39` AE-mode command/inquiry family. That
narrow exception does not generalize typed ND support to the other Sony, EVI,
Nearus, or generic profiles, and it does not make the broader FR7 profile fully
validated here.

**Narrow shared exposure/iris exceptions:** R11 lines 706–717, 1003, and 1012
establish BRC-H900 `04 39` exposure modes, standard iris controls, and matching
inquiries. R12 lines 440–454 and 609–617 establish the same families for
BRC-300, R21 for Nearus BRC-300, and R8's command list (2/4) and inquiry list
(1/3) for EVI-H100. Generic VISCA grants only the families that R8, R12 and R21
all document identically. These narrow grants do not validate unrelated model
features or the distinct `09 04 2B` iris-status inquiry.

**Narrow BRC-300 backlight exception:** R12's exact `CAM_BackLight` control
rows are `8x 01 04 33 02 FF` (On) and `8x 01 04 33 03 FF` (Off) on manual p. 11
(source text lines 404–405). Its `CAM_BackLightModeInq` row is
`8x 09 04 33 FF`, returning `y0 50 02 FF` for On or `y0 50 03 FF` for Off on
manual p. 14 (source text lines 539–540). Those paired rows justify only the
source-backed backlight control/inquiry under `image()`; they are not evidence
for the other image-processing rows.

**Narrow BRC-300 coordinate exception:** R12's pan/tilt value table maps
positive signed raw pan to left (`08A58`) and positive signed raw tilt to up
(`493D`); the negative endpoints (`F75A8` and `E796`) are right and down. The
library's degree convention is positive pan right and positive tilt up, so the
BRC-300 profile uses a negative signed degree-to-unit scale for pan and a
positive one for tilt while retaining the documented signed wire fields. The
reversed pan axis is profile-specific and must not be generalized to other
VISCA profiles.

R12 also settles the position-frame layout independently of the common table
in §7.6. Absolute position is
`8x 01 06 02 VV 00 0Y 0Y 0Y 0Y 0Y 0Z 0Z 0Z 0Z FF`, and relative position
changes only the opcode to `06 03`: there is one speed field `VV` (`01`–`18`),
then a fixed `00`, five signed pan nibbles, and four signed tilt nibbles. It is
not the standard two-speed `VV WW` form. The public request vocabulary retains
separate `PanSpeed` and `TiltSpeed` values for cross-profile consistency, so
the BRC-300 codec requires them to be equal rather than silently dropping one.
Limit set/clear use the same five-pan/four-tilt coordinate widths and `W=01`
for UpRight. Exact profile-path golden vectors pin all four layouts.

**Why it remains open:** The uploaded unified guide explicitly says the non-PTZOptics and non-Axis sections were not re-validated in the prior patch set. This final document used Sony manuals only to resolve transport and opcode semantics, not to validate every model-family capability.

**Validation needed:** For each target model family, validate against the current primary model manual/command list:

- Sony ILME‑FR7 / FR7K
- Sony BRC-H900 / BRC-X1000 / BRC-X400 / SRG families
- Sony EVI-H100S/H100V and older EVI models
- Nearus BRC‑300 / Sony BRC‑300 equivalents
- Marshall / AVer / Avonic / other VISCA-compatible brands

**Helpful sources:**

- Sony ILME‑FR7 VISCA Command List, Version 4.00: https://pro.sony/s3/2022/09/03065933/VISCA_Command_List_v4.pdf
- Sony EVI-H100S/H100V Technical Manual: https://www.sony.com/electronics/support/res/manuals/AE4U/AE4U1001M.pdf
- Axis VISCA Interface API Description: https://www.axis.com/dam/public/70/3d/31/visca-interface-api-description-en-US-266656.pdf
- Uploaded Unified VISCA Protocol Implementation Guide: `visca_unified_reference(2).md`

## A.10 Live-device validation matrix

Recommended minimum target tests:

| Test | PT12X | PT20X | PT30X | Notes |
|---|---:|---:|---:|---|
| Raw UDP `1259` Power Inquiry | ☐ | ☐ | ☐ | Expect `90 50 02/03 FF`. |
| Raw TCP `5678` Power Inquiry | ☐ | ☐ | ☐ | Same packet, TCP path. |
| Sony VISCA UDP `52381` encapsulated Power Inquiry | ☐ | ☐ | ☐ | Requires supported firmware. |
| Bright Direct `04 4D` set/inquiry | ☐ | ☐ | ☐ | Verify erratum resolution. |
| Gamma `04 5B` set/inquiry | ☐ | ☐ | ☐ | Confirm PTZOptics support on firmware. |
| Presets `0x7F`, `0x80`, `0xFE` | ☐ | ☐ | ☐ | Resolve raw preset upper range. |
| Digital zoom toggle/inquiry | ☐ | ☐ | ☐ | Confirm unsupported/supported behavior. |
| Sharpness `0x00–0x0F` sweep | ☐ | ☐ | ☐ | Confirm accepted range and inquiry. |
| Motion Sync on/off | ☐ | ☐ | ☐ | Firmware-specific. |
| OnePush/Snap Focus | ☐ | ☐ | ☐ | Firmware-specific. |
| Image freeze command/path | ☐ | ☐ | ☐ | Firmware/path-specific. |


## A.11 PTZOptics tally and reboot rows from broader unified notes

**Current treatment:** PTZOptics tally flash/on/off and reboot rows are retained as validation candidates, not baseline Gen‑2 implementation facts.

**Why it remains open:** The broader unified guide lists PTZOptics tally flash/on/off and reboot opcodes, but the checked primary PTZOptics Gen‑2 NDI®\|HX command tables in this source bundle do not establish them with the same confidence as the main command families.

**Candidate rows requiring validation:**

| Candidate | Packet | Validation requirement |
|---|---|---|
| Tally flash | `81 0A 02 02 01 FF` | Confirm ACK/completion and visible tally behavior on target model/firmware. |
| Tally on | `81 0A 02 02 02 FF` | Confirm support on target model/firmware. |
| Tally off | `81 0A 02 02 03 FF` | Confirm support on target model/firmware. |
| Reboot | `81 0A 01 06 01 FF` | Confirm target support and document operational impact before exposing in user-facing controls. |

## A.12 Pelco-D and Pelco-P protocol scope

**Current treatment:** Pelco-D and Pelco-P are supported serial control protocols on the PTZOptics Gen‑2 NDI®\|HX cameras, but they are outside this VISCA reference.

**Why they are not mixed into the main command tables:** The main reference is organized around VISCA packet semantics, VISCA transaction behavior, VISCA-over-IP transport distinctions, and VISCA opcode conflicts. Pelco-D and Pelco-P have different frame formats, checksums, addressing, and command semantics. Including the full Pelco tables in the main reference would blur the protocol boundary.

# Appendix B — PTZOptics Gen‑2 product/spec detail tables

This appendix keeps product/spec data consolidated without expanding the main VISCA command sections.

## B.1 Streaming capabilities

| Item | Value |
|---|---|
| Video compression | NDI®\|HX / H.264 / H.265 / M‑JPEG |
| IP video streams | Two |
| First stream resolutions | `1920x1080`, `1280x720`, `1024x576`, `960x540`, `640x480`, `640x360` |
| Second stream resolutions | `1280x720`, `1024x576`, `720x480`, `720x408`, `640x360`, `480x270`, `320x240`, `320x180` |
| Video bitrate | `32 Kbps–102400 Kbps` |
| Bitrate type | Variable Rate / Fixed Rate |
| Frame rate | 50 Hz: `1–50 FPS`; 60 Hz: `1–60 FPS` |
| Audio compression | AAC |
| Audio bitrate | `96 Kbps`, `128 Kbps`, `256 Kbps` |
| Supported protocols | TCP/IP, HTTP, RTSP, RTMP, DHCP, Multicast, and related IP protocols |

## B.2 Physical interfaces

| Interface | Common detail |
|---|---|
| HDMI | 1 x HDMI 1.3 |
| 3G‑SDI | 1 x BNC, 800mVp‑p, 75Ω, SMPTE 424M |
| IP/NDI | 1 x RJ45 NDI®\|HX / IP stream, 10/100/1000 Ethernet |
| CVBS | 1 x RCA, 1Vp‑p, 75Ω |
| Audio input | 1-channel 3.5mm line-in, unbalanced stereo |
| USB | 1 x USB 2.0 Type‑A; future-use note in datasheets |
| RS‑232 IN | 8-pin Mini-DIN, max 30m, VISCA / Pelco-D / Pelco-P |
| RS‑232 OUT | 8-pin Mini-DIN, max 30m, VISCA network use only |
| RS‑485 | 2-pin Phoenix, max 1200m, VISCA / Pelco-D / Pelco-P |
| Power | JEITA-type DC IN 12V and PoE 802.3af |

## B.3 Model details and packaging

| Model | Color variants | Camera weight | Box weight | Included items |
|---|---|---:|---:|---|
| PT12X‑NDI | Gray `PT12X‑NDI‑GY`; white `PT12X‑NDI‑WH` | 3.20 lb / 1.45 kg | 5.4 lb / 2.45 kg | Camera, power adapter + cord, IR remote, RS‑232C cable, quick start guide, two AAA batteries. |
| PT20X‑NDI | Gray `PT20X‑NDI‑GY`; white `PT20X‑NDI‑WH` | 3.00 lb / 1.36 kg | 5.4 lb / 2.45 kg | Same package family. |
| PT30X‑NDI | Gray `PT30X‑NDI‑GY`; white `PT30X‑NDI‑WH` | 3.05 lb / 1.39 kg | 5.4 lb / 2.45 kg | Same package family. |

## B.4 Mechanical dimensions and validation notes

| Model | Base dimensions | Max-height note |
|---|---|---|
| PT12X‑NDI | `5.6 in W x 6.7 in D x 6.5 in H`; `142mm x 169mm x 164mm`. | PT12X source rows contain inconsistent max-height conversions; use detailed mechanical drawings for installation-critical checks. |
| PT20X‑NDI | Same base dimensions. | `7.8 in` with max tilt is approximately `198mm`; reject `168mm` when paired with `7.8 in`. |
| PT30X‑NDI | Same base dimensions. | `7.8 in` with max tilt is approximately `198mm`; reject `168mm` when paired with `7.8 in`. |

# Appendix C — Source and link catalog

## C.1 Official PTZOptics sources

| Ref | Source | Link | Used for |
|---|---|---|---|
| R1 | PTZOptics VISCA over IP Commands, Rev 1.2, 8/24/2020 (archived) | https://web.archive.org/web/20240616063126id_/https://ptzoptics.com/wp-content/uploads/2020/11/PTZOptics-VISCA-over-IP-Rev-1_2-8-20.pdf | PTZOptics raw command packets, ACK/errors, memory table, image-processing packets, vendor extensions, and the `09 04 50`/`53`/`54` NR inquiry rows. Its legacy `09 04 54` output is `0–8`; it contains no NR setter rows. |
| R2 | PTZOptics NDI camera page | https://ptzoptics.com/ndi/ | Current PTZOptics NDI model page, focal lengths, ports, presets, firmware links. |
| R3 | PT30X‑NDI‑xx Data Sheet | https://f.hubspotusercontent20.net/hubfs/418770/PTZOptics%20Documentation/PT30X-NDI-xx/PT30X-NDI-xx%20Data%20Sheet.pdf | PT30X focal length, FOV, presets, dimensions, simultaneous-output limitation. |
| R4 | PTZOptics Firmware Changelog | https://ptzoptics.com/firmware-changelog/ | Sony VISCA-over-IP firmware support, SRT, image freeze, OnePush AF, Motion Sync, Snap Focus. |
| R5 | PTZOptics SuperJoy G1 User Manual | https://ptzoptics.com/wp-content/uploads/2021/03/PT-SUPERJOY-G1-User-Manual.pdf | Port separation: PTZOptics UDP `1259`, TCP `5678`, Sony VISCA UDP `52381`. |
| R10 | PTZOptics Move 4K G3 User Manual (manufacturer manual, reseller-hosted copy) | https://www.rcblogic.co.uk/images/product/PDFDocs/Product-Documentation-PT-4K-xx-G3-User-Manual.pdf | G3-specific `04 39` AE, `04 0B`/`04 4B` iris, and `09 04 39`/`09 04 4B` query rows; plus anti-flicker, settings-save, preset-recall-speed, multicast-streaming, and NDI-quality command families retained by `PtzOpticsG3`. It does not list `09 04 2B`. |
| R14 | PTZOptics Developer Portal, API v1.0 (portal dated 2026-08-31) | https://docs.ptzoptics.com/articles/miscellaneous/misc-cameras/pt-limits-packet-sender/; https://docs.ptzoptics.com/dev/visca-api/exposure/; https://docs.ptzoptics.com/dev/visca-api/queries/ | Official current command portal described by PTZOptics as the full VISCA list for G2 and G3. Used for G2/G3 AE, iris, query, image, and NR rows; it documents `04 39`, `04 0B`/`04 4B`, and `09 04 39`/`09 04 4B`, but not `09 04 2B`. Its Exposure page documents NR command inputs `04 50` Auto/Manual, `04 53` off/`1–5`, and `04 54` off/`1–8`; its Queries page separately documents current `09 04 50` and `09 04 53`/`54` results of `0–5`, and lists no `09 04 AA` AF-zone or UAC inquiry (checked 2026-10-05). It is G3 evidence only for rows it lists itself. |
| R15 | PTZOptics G2/Legacy camera index | https://docs.ptzoptics.com/docs/cameras/g2-legacy/ | Official legacy-scope index. It explicitly lists the 12X, 20X, and 30X SDI/NDI G2 models used to constrain the legacy `PtzOptics30X` profile; it does not generalize support to newer 30X, Move, or Link models. |
| R20 | PTZOptics Developer Portal, Focus page (API v1.0) | https://docs.ptzoptics.com/dev/visca-api/focus/ | `CAM_AFZone` "AF Zone weight select": Top `81 01 04 AA 00 FF`, Center `01`, Bottom `02`. No other zone value. Checked 2026-10-04. The portal's Queries page (https://docs.ptzoptics.com/dev/visca-api/queries/) has no `CAM_VersionInq` row. |

## C.2 Official Axis source

| Ref | Source | Link | Used for |
|---|---|---|---|
| R6 | Axis VISCA Interface API Description, User Manual M1.6, March 2021 | https://www.axis.com/dam/public/70/3d/31/visca-interface-api-description-en-US-266656.pdf | Axis zoom/focus/pan/tilt ranges, Axis inquiries, Axis menu display inquiry, Axis tally command. |

## C.3 Official Sony sources

| Ref | Source | Link | Used for |
|---|---|---|---|
| R7 | Sony ILME‑FR7 / FR7K VISCA Command List, Version 4.00 | https://pro.sony/s3/2022/09/03065933/VISCA_Command_List_v4.pdf | Sony VISCA-over-IP UDP `52381`, 8-byte header, payload types, sequence number, socket behavior, errors, retransmission guidance; FR7 vendor-relative `7E 04 4B` iris Up/Down, `05 34` Auto Iris inquiry, variable-ND controls/inquiries, and fixed `04 3A` spotlight controls. The Version 4.00 PDF was downloaded and checked on 2026-11-15: ND variable inquiry on p. 21 and density-code table on p. 25 establish the current typed readback in §9.4. It does not establish the shared `04 39` AE-mode command/inquiry family, absolute iris direct control, shared `04 58` autofocus sensitivity, shared `04 50`/`04 53`/`04 54` noise reduction, or a `09 04 4B` position inquiry. `CAM_VersionInq` (`8X 09 00 02 FF` → `Y0 50 GG GG HH HH JJ JJ KK FF`): vendor ID `0001` (Sony), model ID `051E` (ILME-FR7/FR7K), ROM revision, maximum socket `02`; read on 2026-10-05 from the indexed text of the Version 2.00 edition (https://pro.sony/s3/2022/09/14131603/VISCA-Command-List-Version-2.00.pdf) because both editions refused direct download (HTTP 403). The shutter table has not been transcribed into the checked profile inventory, so `SonyFR7` still advertises no shutter codes. |
| R8 | Sony EVI‑H100S/H100V Technical Manual | https://www.sony.com/electronics/support/res/manuals/AE4U/AE4U1001M.pdf | Bright Direct `04 4D`, Gamma `04 5B`, digital zoom inquiry, and fixed `04 5A` automatic slow-shutter commands. Line items checked on 2026-10-05 against a distributor copy of the same manual (A-E4U-100-11(1)), https://www.daitron.com/documents/evih100s_manual.pdf: command list (2/4) `CAM_AE` Full Auto/Manual/Shutter Priority/Iris Priority/Bright `8x 01 04 39 00/03/0A/0B/0D FF`, `CAM_Iris` reset/up/down `8x 01 04 0B 00/02/03 FF` and direct `8x 01 04 4B 00 00 0p 0q FF`; inquiry list (1/3) `CAM_AEModeInq` `8x 09 04 39 FF`, `CAM_IrisPosInq` `8x 09 04 4B FF`, `CAM_FocusNearLimitInq` `8x 09 04 28 FF`; iris table (p. 44) `00` CLOSE and `05`..`11` (F14..F1.6); focus position `1000` (far) to `F000` (near); `CAM_WB` Auto/Indoor/Outdoor/One Push/Manual with the `04 10 05` trigger and no color-temperature mode or `04 20` command; `CAM_RGain`/`CAM_BGain` `04 03`/`04 04` and direct `04 43`/`04 44` R/B gain `00..FF` with `09 04 43`/`44` inquiries (absolute gain, not signed tuning); `CAM_ExpComp` `04 3E`/`04 0E`/`04 4E` with `09 04 3E`/`4E` (`00`..`0E` = −7..+7); `CAM_Bright` `04 0D`/`04 4D` with `09 04 4D` (`00`, `05`..`1F`); `CAM_WD` `04 3D` (not the `04 25` dynamic-range command); gain `00` (−3 dB) to `0F` (+28 dB); the official manual inquiry list p. 35, rechecked 2026-11-15, gives `CAM_GainPosInq` `8x 09 04 4C FF` with reply `y0 50 00 00 0p 0q FF`; no `04 61`/`04 66` flip/mirror command (image flip is a rear switch); `8x 2p FF` command cancel; command list (4/4) pan `E1E5`..`1E1B` (−170..+170°), tilt `FC75`..`0FF0` (−20..+90°, IMAGE FLIP off), pan speed `01`..`18`, tilt speed `01`..`17`; `CAM_Memory` p = 0..5; command list note 4: after a `CAM_Memory` recall completion, a following command may be answered `Command not executable` for at most 240 ms and should be sent again. `CAM_VersionInq` (`8X 09 00 02 FF` → `Y0 50 GG GG HH HH JJ JJ KK FF`): vendor ID `0001` (Sony), model ID `050E` (EVI-H100V) / `050F` (EVI-H100S), ROM revision, maximum socket `02`. Also: VISCA over RS-232C/RS-422 only (9,600 or 38,400 bps; no IP transport); shutter table (exposure control 1/2, 60/30 mode) `00` 1/1 s through `15` 1/10000 s (`05` 1/30, `0F` 1/1000); `CAM_Focus` One Push Trigger `04 18 01`; `CAM_DZoom` `04 06 02/03` with `09 04 06`; digital zoom positions `4000`..`7AC0`; AF sensitivity Normal `04 58 02` and Low `03` only; command list (3/4) `CAM_NR` `8x 01 04 53 0p FF` (0 off, 1–5) with `CAM_NRInq` `09 04 53` → `y0 50 0p FF`, `CAM_ColorGain` `04 49 00 00 00 0p` and `CAM_ColorHue` `04 4F 00 00 00 0p` (`0h`..`Eh`) with `09 04 49`/`4F` inquiries, `CAM_Aperture` reset/up/down `04 02` and direct `04 42 00 00 0p 0q` (level `00`..`0F`) with `09 04 42`, `CAM_PictureEffect` Off/Neg.Art/B&W `04 63 00/02/04` with `CAM_PictureEffectModeInq` replies `00`/`02`/`04`; no `04 50` NR mode, `04 54` 3D NR or `04 05` aperture mode; limit table UpRight pan `0001`..`1E1B`, tilt `0001`..`0FF0` (increasing raw pan is right, increasing raw tilt is up). |
| R9 | Sony EVI‑H100S support/manuals page | https://www.sony.com.au/electronics/support/network-camera-systems-ptz-cameras/evi-h100s/manuals | Official support page that links the Technical Manual. |
| R11 | Sony BRC-H900 VISCA Command List, Version 2.20, A-E99-100-14(1) | https://pro.sony/s3/cms-static-content/uploadfile/59/1237493025759.pdf | The current indexed Version 2.20 edition was checked on 2026-11-15; direct downloads and PDF screenshots were unavailable. Its pp. 14–17 and 20–22 establish shared `04 39` Full Auto/Manual/Shutter-priority/Iris-priority commands, standard `04 0B`/`04 4B` iris controls, matching exposure/iris queries, and the extended `09 7E 04 00` power-status query. Position commands use five-nibble pan, four-nibble tilt and one speed; the position reply has nine nibbles. The `CAM_BackLight` and spotlight text extraction shows the On setter, but does not positively establish Off; an inquiry reply pair alone does not establish both setters. This edition supersedes the earlier Version 1.00 transcription where stated; it does not make missing or contradictory rows positive evidence. It does not establish the fixed `04 5A` automatic-slow-shutter, shared brightness, shared noise-reduction, or picture-effect families. `CAM_VersionInq` (`8X 09 00 02 FF` → `Y0 50 GG GG HH HH JJ JJ KK FF`): vendor ID `0001` (Sony), model ID `050B` (BRC-H900), ROM revision, maximum socket `02`; read on 2026-10-05 from the indexed text of the Command List Version 1.00 (A-E99-100-11) because direct downloads were refused. Its full shutter table has not been transcribed into the profile inventory, so `SonyBRCH900` advertises no shutter codes. The final continuation of the individual inquiry list (p. 23) explicitly establishes `09 04 47` zoom position and `09 04 48` focus position, each with a four-nibble reply; these rows were missed in the earlier partial extraction. The lens-block inquiry is a separate format. |
| R12 | Sony BRC-300 Technical Manual | https://www.sony.jp/aii/contents/smojsdmk/b2b_index/manual_pdf/remote_camera/AC1Y100131.pdf | BRC-300 shared `04 39` exposure-mode and standard `04 0B`/`04 4B` iris controls (lines 440–454), matching `09 04 39`/`09 04 4B` inquiries (lines 609–617), and the exact paired `CAM_BackLight` rows: control `8x 01 04 33 02/03 FF` (manual p. 11; source text lines 404–405) and inquiry `8x 09 04 33 FF` with `y0 50 02/03 FF` replies (manual p. 14; source text lines 539–540). It also documents fixed `04 5A` automatic-slow-shutter commands and the `VV 00` one-speed position grammar with five-nibble signed pan/four-nibble signed tilt commands, limits, inquiries, and endpoints (manual pp. 12 and 22). `CAM_WB` lists One Push WB `8x 01 04 35 03 FF` and the One Push trigger `8x 01 04 10 05 FF`, and the iris table lists `00` CLOSE through `11` F1.6; `CAM_ImgFlip` `04 66 02/03` with `09 04 66` (no `04 61` mirror); `CAM_RGain`/`CAM_BGain` `04 43`/`04 44` `00..FF`; `CAM_ExpComp` `04 3E`/`04 4E` (−7..+7); `CAM_Bright` `04 4D` (`00`..`17`); gain `0`..`7` (checked 2026-10-05). It does not establish the fixed `04 3A` spotlight, the `04 28` focus near-limit command or inquiry, or a broad image-processing family. `CAM_VersionInq`: vendor ID `0001` (Sony), model ID `040F` (BRC-300) / `0410` (BRU-300), ROM revision, maximum socket `02`. Also: VISCA over RS-232C/RS-422 only; shutter table `02` 1/4 s through `15` 1/10000 s (BRC-300 column); x12 lens, optical zoom `0000`..`4000` and digital `4000`..`7F00` (x4) via `CAM_Zoom` Direct `04 47`; `CAM_DZoom` on/off `04 06 02/03` with `CAM_DZoomModeInq` `09 04 06`; `CAM_Focus` One Push Trigger `04 18 01`. |

## C.4 Supplemental explanatory source

| Ref | Source | Link | Used for |
|---|---|---|---|
| R13 | Jon Skeet, “Variations in the VISCA protocol” | https://codeblog.jonskeet.uk/2023/11/25/variations-in-the-visca-protocol/ | Secondary explanatory context for raw vs encapsulated VISCA-over-IP differences. Not used as a primary authority where manufacturer manuals exist. |

## C.4a OEM firmware-family sources, related-model sources and bench records

R16–R19 are used only to bound what the PTZOptics G2 firmware family
documents; none is a PTZOptics model source. They were checked on 2026-10-04.
R21 is the Nearus model source for the Nearus BRC-300 profile.

| Ref | Source | Link | Used for |
|---|---|---|---|
| R16 | MSolutions, "VISCA Commands Guide For all MSolutions PTZ Cameras", Revision 1.0, August 2024 | https://www.m4sol.com/wp-content/uploads/2024/08/VISCA-Commands-Guide-2.pdf | `CAM_AFZone` Top/Center/Bottom `8x 01 04 AA 00/01/02 FF` and inquiry `y0 50 00/01/02 FF` only. `CAM_VersionInq` `8x 09 00 02 FF` → `y0 50 ab cd mn pq rs tu vw FF`: "ab: Factory Code(00: VHD, 01:MR, 08:T)", "cd: Hardware Version", "mnpq: ARM Version", "rstu: FPGA Version", "vw: Camera model". |
| R17 | HuddleCamHD, "SimplTrack2 / HuddleView VISCA over IP Commands" (updated 2020/2/26) | https://f.hubspotusercontent20.net/hubfs/418770/HuddleCamHD%20Documentation/Misc/SimplTrack2_HuddleView%20VISCA%20over%20IP%20Commands.pdf | `CAM_VersionInq` → `y0 50 00 01 mn pq rs tu vw FF`, "mnpq: Model Code (0504) rstu: ROM version vw: Socket Number (=02)". No AF-zone row. |
| R18 | Minrray UV1201 "HD Color Video Camera User Manual" | https://www.rmaelectronics.com/content/minrray/Minrray%20UV1201%20User%20Manual.pdf | OSD "AF-Zone: Up, middle, down". `CAM_VersionInq` → `y0 50 ab cd mn pq rs tu vw FF`, "vender ID ( 0220 )", "model ID", "ARM Version", "vw: reserve". |
| R19 | "VISCA Command List" (third-party mirror at ptzprotocols.com) | https://ptzprotocols.com/2%20Misc%20Protocols/Sony/VISCA%20Commands%20List.pdf | `CAM_AFZone` Top/Center/Bottom `8x 01 04 AA 00/01/02 FF` and the matching inquiry only. Secondary; not a manufacturer-hosted copy. |
| B1 | PTZOptics G2 bench (#795): PT30X/PT20X/PT12X-NDI G2, firmware ARM 6.3.51THI, 6.3.76THI, 6.4.18SHI, 2026-10-04 | §7.13 | AF zone `03` read, set and read-back; `09 00 02` reply `90 50 00 52 FF`. |
| R21 | Nearus, "Nearus VISCA Protocol via Sony" (BRC-300/300P Command List Version 1.20 text) | https://www.snapav.com/wcsstore/ExtendedSitesCatalogAssetStore/attachments/documents/USBProducts/ManualsAndGuides/Visca%20Protocol2.pdf | `CAM_VersionInq` `8X 09 00 02 FF` → `Y0 50 GG GG HH HH JJ JJ KK FF`: "GGGG = Vender ID (0001: Sony)", "HHHH = Model ID 040F: BRC-300/P 0410: BRU-300/P", "JJJJ = ROM revision", "KK = Maximum socket # (02)"; the command list repeats it as `y0 50 00 01 mn pq rs tu vw FF`, "mnpq: Model Code (04xx)", "vw: Socket Number (02)". The same text is the BRC-300/300P command list: `CAM_AE` `04 39` modes and `09 04 39`, `CAM_Iris` `04 0B`/`04 4B` and `09 04 4B` (iris `00` CLOSE through `11` F1.6), `CAM_WB` One Push mode and `04 10 05` trigger, `CAM_SlowShutter` `04 5A` and `09 04 5A`, `CAM_BackLight` `04 33`, `CAM_ImgFlip` `04 66` and `09 04 66`, R/B gain `04 43`/`04 44` `00..FF`, `CAM_ExpComp` `04 3E`/`04 4E`, `CAM_Bright` `04 4D`, gain `0`..`7`, `CAM_Memory` p = 0..5, `Cmd_PT_M_Speed` 1..24, direct zoom `04 47`, `8x 2p FF` cancel; it lists no `04 49` saturation, no `04 28` focus near-limit and no `04 3A` spotlight row. Checked 2026-10-05. Also: VISCA over RS-232C/RS-422 only; `Pan-tiltDrive` speeds `01`..`18h`; one-speed absolute/relative frames with a five-nibble pan; limits pan `08A58`/`F75A8`, tilt `493D`/`E796`; shutter table `02` 1/4 s through `15` 1/10000 s (BRC-300 column); x12 lens, optical zoom `0000`..`4000` and digital `4000`..`7F00` (x4) via `CAM_Zoom` Direct `04 47`; `CAM_DZoom` on/off `04 06 02/03` with `CAM_DZoomModeInq` `09 04 06`; `CAM_Focus` One Push Trigger `04 18 01`. |

## C.5 Uploaded source bundle

| Ref | Uploaded file | Used for |
|---|---|---|
| U0 | `visca_reference.md` | Original final consolidated draft used as the structural base for this completed reference. |
| U1 | `PTZOptics-G2-VISCA-over-IP-Command-List(2).md` | Patched PTZOptics command reference; complete PTZOptics command/inquiry/block-inquiry rows; Pelco source tables; hardware-test notes for Gamma, Luminance/Contrast width, Bright Direct erratum, and raw transport. |
| U2 | `PTZOptics-NDI-HX-Gen2-Specs(1).md` | Consolidated PT12X/PT20X/PT30X specs, streaming tables, interface details, dimensions, package contents, and cross-document PTZOptics validation notes. |
| U3 | `axis_visca-interface-api-description(2).md` | Patched Axis API text including corrected absolute zoom command, Axis-only ranges, focus-near-limit table, red/blue gain, exposure/shutter rows, shutter-speed table, and Axis inquiries. |
| U4 | `visca_unified_reference(2).md` | Broad unified guide; used only as background except where cross-validated. Non-PTZOptics/non-Axis sections remain unvalidated unless supported by primary sources. |

R21 p. 21 gives BRC-300/P focus reference positions `1000` (Over Inf)
through `C000` (1 cm): numerical increase moves toward near focus. The same
orientation applies to the Sony BRC-300 and Nearus profiles, and to the Generic
VISCA intersection with EVI-H100. The PTZOptics references give a raw position
packet without proving near/far numerical orientation; those profiles report
that fact as unknown instead of inheriting a Sony or Axis convention.

## Checked positive source evidence

This table is the exact positive lookup used by the source invariant tests.
A custom profile has caller-owned protocol authority and does not acquire a
vendor-verified claim from this built-in catalog.

<!-- BEGIN CHECKED SOURCE EVIDENCE -->

| Evidence key | Source | Models | Subject | Packet | Boolean reply | Location |
|---|---|---|---|---|---|---|
| r1-ptzopticsg2-power-query | R1 | PtzOpticsG2 | PowerInquiry | 09 04 00 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-power-query | R14 | PtzOpticsG3 | PowerInquiry | 09 04 00 | - | Queries page; current model scope |
| r1-ptzoptics30x-power-query | R1 | PtzOptics30X | PowerInquiry | 09 04 00 | - | §7.10; command inquiry list |
| r7-sonyfr7-power-query | R7 | SonyFr7 | PowerInquiry | 09 04 00 | - | FR7 inquiry command list |
| r11-sonybrch900-power-query | R11 | SonyBrcH900 | PowerInquiry | 09 7E 04 00 | - | Version 2.20 pp.20–22 |
| r8-sonyevih100-power-query | R8 | SonyEviH100 | PowerInquiry | 09 04 00 | - | Technical Manual inquiry list |
| r12-sonybrc300-power-query | R12 | SonyBrc300 | PowerInquiry | 09 04 00 | - | Technical Manual inquiry list |
| r21-nearusbrc300-power-query | R21 | NearusBrc300 | PowerInquiry | 09 04 00 | - | Command List inquiry list |
| r8-genericvisca-power-query | R8 | GenericVisca | PowerInquiry | 09 04 00 | - | Technical Manual inquiry list |
| r12-genericvisca-power-query | R12 | GenericVisca | PowerInquiry | 09 04 00 | - | Technical Manual inquiry list |
| r21-genericvisca-power-query | R21 | GenericVisca | PowerInquiry | 09 04 00 | - | Command List inquiry list |
| r7-sonyfr7-version-query | R7 | SonyFr7 | VersionInquiry | 09 00 02 | - | FR7 inquiry command list |
| r11-sonybrch900-version-query | R11 | SonyBrcH900 | VersionInquiry | 09 00 02 | - | Version 2.20 pp.20–22 |
| r8-sonyevih100-version-query | R8 | SonyEviH100 | VersionInquiry | 09 00 02 | - | Technical Manual inquiry list |
| r12-sonybrc300-version-query | R12 | SonyBrc300 | VersionInquiry | 09 00 02 | - | Technical Manual inquiry list |
| r21-nearusbrc300-version-query | R21 | NearusBrc300 | VersionInquiry | 09 00 02 | - | Command List inquiry list |
| r8-genericvisca-version-query | R8 | GenericVisca | VersionInquiry | 09 00 02 | - | Technical Manual inquiry list |
| r12-genericvisca-version-query | R12 | GenericVisca | VersionInquiry | 09 00 02 | - | Technical Manual inquiry list |
| r21-genericvisca-version-query | R21 | GenericVisca | VersionInquiry | 09 00 02 | - | Command List inquiry list |
| r1-ptzopticsg2-pantiltposition-query | R1 | PtzOpticsG2 | PanTiltPositionInquiry | 09 06 12 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-pantiltposition-query | R14 | PtzOpticsG3 | PanTiltPositionInquiry | 09 06 12 | - | Queries page; current model scope |
| r1-ptzoptics30x-pantiltposition-query | R1 | PtzOptics30X | PanTiltPositionInquiry | 09 06 12 | - | §7.10; command inquiry list |
| r7-sonyfr7-pantiltposition-query | R7 | SonyFr7 | PanTiltPositionInquiry | 09 06 12 | - | FR7 inquiry command list |
| r11-sonybrch900-pantiltposition-query | R11 | SonyBrcH900 | PanTiltPositionInquiry | 09 06 12 | - | Version 2.20 pp.20–22 |
| r8-sonyevih100-pantiltposition-query | R8 | SonyEviH100 | PanTiltPositionInquiry | 09 06 12 | - | Technical Manual inquiry list |
| r12-sonybrc300-pantiltposition-query | R12 | SonyBrc300 | PanTiltPositionInquiry | 09 06 12 | - | Technical Manual inquiry list |
| r21-nearusbrc300-pantiltposition-query | R21 | NearusBrc300 | PanTiltPositionInquiry | 09 06 12 | - | Command List inquiry list |
| r8-genericvisca-pantiltposition-query | R8 | GenericVisca | PanTiltPositionInquiry | 09 06 12 | - | Technical Manual inquiry list |
| r12-genericvisca-pantiltposition-query | R12 | GenericVisca | PanTiltPositionInquiry | 09 06 12 | - | Technical Manual inquiry list |
| r21-genericvisca-pantiltposition-query | R21 | GenericVisca | PanTiltPositionInquiry | 09 06 12 | - | Command List inquiry list |
| r1-ptzopticsg2-zoomposition-query | R1 | PtzOpticsG2 | ZoomPositionInquiry | 09 04 47 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-zoomposition-query | R14 | PtzOpticsG3 | ZoomPositionInquiry | 09 04 47 | - | Queries page; current model scope |
| r1-ptzoptics30x-zoomposition-query | R1 | PtzOptics30X | ZoomPositionInquiry | 09 04 47 | - | §7.10; command inquiry list |
| r7-sonyfr7-zoomposition-query | R7 | SonyFr7 | ZoomPositionInquiry | 09 04 47 | - | FR7 inquiry command list |
| r8-sonyevih100-zoomposition-query | R8 | SonyEviH100 | ZoomPositionInquiry | 09 04 47 | - | Technical Manual inquiry list |
| r12-sonybrc300-zoomposition-query | R12 | SonyBrc300 | ZoomPositionInquiry | 09 04 47 | - | Technical Manual inquiry list |
| r21-nearusbrc300-zoomposition-query | R21 | NearusBrc300 | ZoomPositionInquiry | 09 04 47 | - | Command List inquiry list |
| r8-genericvisca-zoomposition-query | R8 | GenericVisca | ZoomPositionInquiry | 09 04 47 | - | Technical Manual inquiry list |
| r12-genericvisca-zoomposition-query | R12 | GenericVisca | ZoomPositionInquiry | 09 04 47 | - | Technical Manual inquiry list |
| r21-genericvisca-zoomposition-query | R21 | GenericVisca | ZoomPositionInquiry | 09 04 47 | - | Command List inquiry list |
| r1-ptzopticsg2-focusposition-query | R1 | PtzOpticsG2 | FocusPositionInquiry | 09 04 48 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-focusposition-query | R14 | PtzOpticsG3 | FocusPositionInquiry | 09 04 48 | - | Queries page; current model scope |
| r1-ptzoptics30x-focusposition-query | R1 | PtzOptics30X | FocusPositionInquiry | 09 04 48 | - | §7.10; command inquiry list |
| r7-sonyfr7-focusposition-query | R7 | SonyFr7 | FocusPositionInquiry | 09 04 48 | - | FR7 inquiry command list |
| r8-sonyevih100-focusposition-query | R8 | SonyEviH100 | FocusPositionInquiry | 09 04 48 | - | Technical Manual inquiry list |
| r12-sonybrc300-focusposition-query | R12 | SonyBrc300 | FocusPositionInquiry | 09 04 48 | - | Technical Manual inquiry list |
| r21-nearusbrc300-focusposition-query | R21 | NearusBrc300 | FocusPositionInquiry | 09 04 48 | - | Command List inquiry list |
| r8-genericvisca-focusposition-query | R8 | GenericVisca | FocusPositionInquiry | 09 04 48 | - | Technical Manual inquiry list |
| r12-genericvisca-focusposition-query | R12 | GenericVisca | FocusPositionInquiry | 09 04 48 | - | Technical Manual inquiry list |
| r21-genericvisca-focusposition-query | R21 | GenericVisca | FocusPositionInquiry | 09 04 48 | - | Command List inquiry list |
| r1-ptzopticsg2-exposuremode-query | R1 | PtzOpticsG2 | ExposureModeInquiry | 09 04 39 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-exposuremode-query | R14 | PtzOpticsG3 | ExposureModeInquiry | 09 04 39 | - | Queries page; current model scope |
| r1-ptzoptics30x-exposuremode-query | R1 | PtzOptics30X | ExposureModeInquiry | 09 04 39 | - | §7.10; command inquiry list |
| r11-sonybrch900-exposuremode-query | R11 | SonyBrcH900 | ExposureModeInquiry | 09 04 39 | - | Version 2.20 pp.20–22 |
| r8-sonyevih100-exposuremode-query | R8 | SonyEviH100 | ExposureModeInquiry | 09 04 39 | - | Technical Manual inquiry list |
| r12-sonybrc300-exposuremode-query | R12 | SonyBrc300 | ExposureModeInquiry | 09 04 39 | - | Technical Manual inquiry list |
| r21-nearusbrc300-exposuremode-query | R21 | NearusBrc300 | ExposureModeInquiry | 09 04 39 | - | Command List inquiry list |
| r8-genericvisca-exposuremode-query | R8 | GenericVisca | ExposureModeInquiry | 09 04 39 | - | Technical Manual inquiry list |
| r12-genericvisca-exposuremode-query | R12 | GenericVisca | ExposureModeInquiry | 09 04 39 | - | Technical Manual inquiry list |
| r21-genericvisca-exposuremode-query | R21 | GenericVisca | ExposureModeInquiry | 09 04 39 | - | Command List inquiry list |
| r1-ptzopticsg2-exposurecompensation-query | R1 | PtzOpticsG2 | ExposureCompensationInquiry | 09 04 4E | - | §7.10; command inquiry list |
| r14-ptzopticsg3-exposurecompensation-query | R14 | PtzOpticsG3 | ExposureCompensationInquiry | 09 04 4E | - | Queries page; current model scope |
| r1-ptzoptics30x-exposurecompensation-query | R1 | PtzOptics30X | ExposureCompensationInquiry | 09 04 4E | - | §7.10; command inquiry list |
| r8-sonyevih100-exposurecompensation-query | R8 | SonyEviH100 | ExposureCompensationInquiry | 09 04 4E | - | Technical Manual inquiry list |
| r12-sonybrc300-exposurecompensation-query | R12 | SonyBrc300 | ExposureCompensationInquiry | 09 04 4E | - | Technical Manual inquiry list |
| r21-nearusbrc300-exposurecompensation-query | R21 | NearusBrc300 | ExposureCompensationInquiry | 09 04 4E | - | Command List inquiry list |
| r8-genericvisca-exposurecompensation-query | R8 | GenericVisca | ExposureCompensationInquiry | 09 04 4E | - | Technical Manual inquiry list |
| r12-genericvisca-exposurecompensation-query | R12 | GenericVisca | ExposureCompensationInquiry | 09 04 4E | - | Technical Manual inquiry list |
| r21-genericvisca-exposurecompensation-query | R21 | GenericVisca | ExposureCompensationInquiry | 09 04 4E | - | Command List inquiry list |
| r1-ptzopticsg2-exposurecompensationmode-query | R1 | PtzOpticsG2 | ExposureCompensationModeInquiry | 09 04 3E | 02=on;03=off | §7.10; command inquiry list |
| r14-ptzopticsg3-exposurecompensationmode-query | R14 | PtzOpticsG3 | ExposureCompensationModeInquiry | 09 04 3E | 02=on;03=off | Queries page; current model scope |
| r1-ptzoptics30x-exposurecompensationmode-query | R1 | PtzOptics30X | ExposureCompensationModeInquiry | 09 04 3E | 02=on;03=off | §7.10; command inquiry list |
| r8-sonyevih100-exposurecompensationmode-query | R8 | SonyEviH100 | ExposureCompensationModeInquiry | 09 04 3E | 02=on;03=off | Technical Manual inquiry list |
| r12-sonybrc300-exposurecompensationmode-query | R12 | SonyBrc300 | ExposureCompensationModeInquiry | 09 04 3E | 02=on;03=off | Technical Manual inquiry list |
| r21-nearusbrc300-exposurecompensationmode-query | R21 | NearusBrc300 | ExposureCompensationModeInquiry | 09 04 3E | 02=on;03=off | Command List inquiry list |
| r8-genericvisca-exposurecompensationmode-query | R8 | GenericVisca | ExposureCompensationModeInquiry | 09 04 3E | 02=on;03=off | Technical Manual inquiry list |
| r12-genericvisca-exposurecompensationmode-query | R12 | GenericVisca | ExposureCompensationModeInquiry | 09 04 3E | 02=on;03=off | Technical Manual inquiry list |
| r21-genericvisca-exposurecompensationmode-query | R21 | GenericVisca | ExposureCompensationModeInquiry | 09 04 3E | 02=on;03=off | Command List inquiry list |
| r1-ptzopticsg2-iris-query | R1 | PtzOpticsG2 | IrisInquiry | 09 04 4B | - | §7.10; command inquiry list |
| r14-ptzopticsg3-iris-query | R14 | PtzOpticsG3 | IrisInquiry | 09 04 4B | - | Queries page; current model scope |
| r1-ptzoptics30x-iris-query | R1 | PtzOptics30X | IrisInquiry | 09 04 4B | - | §7.10; command inquiry list |
| r11-sonybrch900-iris-query | R11 | SonyBrcH900 | IrisInquiry | 09 04 4B | - | Version 2.20 pp.20–22 |
| r8-sonyevih100-iris-query | R8 | SonyEviH100 | IrisInquiry | 09 04 4B | - | Technical Manual inquiry list |
| r12-sonybrc300-iris-query | R12 | SonyBrc300 | IrisInquiry | 09 04 4B | - | Technical Manual inquiry list |
| r21-nearusbrc300-iris-query | R21 | NearusBrc300 | IrisInquiry | 09 04 4B | - | Command List inquiry list |
| r8-genericvisca-iris-query | R8 | GenericVisca | IrisInquiry | 09 04 4B | - | Technical Manual inquiry list |
| r12-genericvisca-iris-query | R12 | GenericVisca | IrisInquiry | 09 04 4B | - | Technical Manual inquiry list |
| r21-genericvisca-iris-query | R21 | GenericVisca | IrisInquiry | 09 04 4B | - | Command List inquiry list |
| r1-ptzopticsg2-shutter-query | R1 | PtzOpticsG2 | ShutterInquiry | 09 04 4A | - | §7.10; command inquiry list |
| r14-ptzopticsg3-shutter-query | R14 | PtzOpticsG3 | ShutterInquiry | 09 04 4A | - | Queries page; current model scope |
| r1-ptzoptics30x-shutter-query | R1 | PtzOptics30X | ShutterInquiry | 09 04 4A | - | §7.10; command inquiry list |
| r11-sonybrch900-shutter-query | R11 | SonyBrcH900 | ShutterInquiry | 09 04 4A | - | Version 2.20 pp.20–22 |
| r8-sonyevih100-shutter-query | R8 | SonyEviH100 | ShutterInquiry | 09 04 4A | - | Technical Manual inquiry list |
| r12-sonybrc300-shutter-query | R12 | SonyBrc300 | ShutterInquiry | 09 04 4A | - | Technical Manual inquiry list |
| r21-nearusbrc300-shutter-query | R21 | NearusBrc300 | ShutterInquiry | 09 04 4A | - | Command List inquiry list |
| r8-genericvisca-shutter-query | R8 | GenericVisca | ShutterInquiry | 09 04 4A | - | Technical Manual inquiry list |
| r12-genericvisca-shutter-query | R12 | GenericVisca | ShutterInquiry | 09 04 4A | - | Technical Manual inquiry list |
| r21-genericvisca-shutter-query | R21 | GenericVisca | ShutterInquiry | 09 04 4A | - | Command List inquiry list |
| r1-ptzopticsg2-brightness-query | R1 | PtzOpticsG2 | BrightnessInquiry | 09 04 4D | - | §7.10; command inquiry list |
| r14-ptzopticsg3-brightness-query | R14 | PtzOpticsG3 | BrightnessInquiry | 09 04 4D | - | Queries page; current model scope |
| r1-ptzoptics30x-brightness-query | R1 | PtzOptics30X | BrightnessInquiry | 09 04 4D | - | §7.10; command inquiry list |
| r8-sonyevih100-brightness-query | R8 | SonyEviH100 | BrightnessInquiry | 09 04 4D | - | Technical Manual inquiry list |
| r12-sonybrc300-brightness-query | R12 | SonyBrc300 | BrightnessInquiry | 09 04 4D | - | Technical Manual inquiry list |
| r21-nearusbrc300-brightness-query | R21 | NearusBrc300 | BrightnessInquiry | 09 04 4D | - | Command List inquiry list |
| r8-genericvisca-brightness-query | R8 | GenericVisca | BrightnessInquiry | 09 04 4D | - | Technical Manual inquiry list |
| r12-genericvisca-brightness-query | R12 | GenericVisca | BrightnessInquiry | 09 04 4D | - | Technical Manual inquiry list |
| r21-genericvisca-brightness-query | R21 | GenericVisca | BrightnessInquiry | 09 04 4D | - | Command List inquiry list |
| r1-ptzopticsg2-whitebalancemode-query | R1 | PtzOpticsG2 | WhiteBalanceModeInquiry | 09 04 35 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-whitebalancemode-query | R14 | PtzOpticsG3 | WhiteBalanceModeInquiry | 09 04 35 | - | Queries page; current model scope |
| r1-ptzoptics30x-whitebalancemode-query | R1 | PtzOptics30X | WhiteBalanceModeInquiry | 09 04 35 | - | §7.10; command inquiry list |
| r7-sonyfr7-whitebalancemode-query | R7 | SonyFr7 | WhiteBalanceModeInquiry | 09 04 35 | - | FR7 inquiry command list |
| r11-sonybrch900-whitebalancemode-query | R11 | SonyBrcH900 | WhiteBalanceModeInquiry | 09 04 35 | - | Version 2.20 pp.20–22 |
| r8-sonyevih100-whitebalancemode-query | R8 | SonyEviH100 | WhiteBalanceModeInquiry | 09 04 35 | - | Technical Manual inquiry list |
| r12-sonybrc300-whitebalancemode-query | R12 | SonyBrc300 | WhiteBalanceModeInquiry | 09 04 35 | - | Technical Manual inquiry list |
| r21-nearusbrc300-whitebalancemode-query | R21 | NearusBrc300 | WhiteBalanceModeInquiry | 09 04 35 | - | Command List inquiry list |
| r8-genericvisca-whitebalancemode-query | R8 | GenericVisca | WhiteBalanceModeInquiry | 09 04 35 | - | Technical Manual inquiry list |
| r12-genericvisca-whitebalancemode-query | R12 | GenericVisca | WhiteBalanceModeInquiry | 09 04 35 | - | Technical Manual inquiry list |
| r21-genericvisca-whitebalancemode-query | R21 | GenericVisca | WhiteBalanceModeInquiry | 09 04 35 | - | Command List inquiry list |
| r1-ptzopticsg2-colortemperature-query | R1 | PtzOpticsG2 | ColorTemperatureInquiry | 09 04 20 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-colortemperature-query | R14 | PtzOpticsG3 | ColorTemperatureInquiry | 09 04 20 | - | Queries page; current model scope |
| r1-ptzoptics30x-colortemperature-query | R1 | PtzOptics30X | ColorTemperatureInquiry | 09 04 20 | - | §7.10; command inquiry list |
| r1-ptzopticsg2-redgain-query | R1 | PtzOpticsG2 | RedGainInquiry | 09 04 43 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-redgain-query | R14 | PtzOpticsG3 | RedGainInquiry | 09 04 43 | - | Queries page; current model scope |
| r1-ptzoptics30x-redgain-query | R1 | PtzOptics30X | RedGainInquiry | 09 04 43 | - | §7.10; command inquiry list |
| r8-sonyevih100-redgain-query | R8 | SonyEviH100 | RedGainInquiry | 09 04 43 | - | Technical Manual inquiry list |
| r12-sonybrc300-redgain-query | R12 | SonyBrc300 | RedGainInquiry | 09 04 43 | - | Technical Manual inquiry list |
| r21-nearusbrc300-redgain-query | R21 | NearusBrc300 | RedGainInquiry | 09 04 43 | - | Command List inquiry list |
| r8-genericvisca-redgain-query | R8 | GenericVisca | RedGainInquiry | 09 04 43 | - | Technical Manual inquiry list |
| r12-genericvisca-redgain-query | R12 | GenericVisca | RedGainInquiry | 09 04 43 | - | Technical Manual inquiry list |
| r21-genericvisca-redgain-query | R21 | GenericVisca | RedGainInquiry | 09 04 43 | - | Command List inquiry list |
| r1-ptzopticsg2-bluegain-query | R1 | PtzOpticsG2 | BlueGainInquiry | 09 04 44 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-bluegain-query | R14 | PtzOpticsG3 | BlueGainInquiry | 09 04 44 | - | Queries page; current model scope |
| r1-ptzoptics30x-bluegain-query | R1 | PtzOptics30X | BlueGainInquiry | 09 04 44 | - | §7.10; command inquiry list |
| r8-sonyevih100-bluegain-query | R8 | SonyEviH100 | BlueGainInquiry | 09 04 44 | - | Technical Manual inquiry list |
| r12-sonybrc300-bluegain-query | R12 | SonyBrc300 | BlueGainInquiry | 09 04 44 | - | Technical Manual inquiry list |
| r21-nearusbrc300-bluegain-query | R21 | NearusBrc300 | BlueGainInquiry | 09 04 44 | - | Command List inquiry list |
| r8-genericvisca-bluegain-query | R8 | GenericVisca | BlueGainInquiry | 09 04 44 | - | Technical Manual inquiry list |
| r12-genericvisca-bluegain-query | R12 | GenericVisca | BlueGainInquiry | 09 04 44 | - | Technical Manual inquiry list |
| r21-genericvisca-bluegain-query | R21 | GenericVisca | BlueGainInquiry | 09 04 44 | - | Command List inquiry list |
| r1-ptzopticsg2-sharpnessmode-query | R1 | PtzOpticsG2 | SharpnessModeInquiry | 09 04 05 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-sharpnessmode-query | R14 | PtzOpticsG3 | SharpnessModeInquiry | 09 04 05 | - | Queries page; current model scope |
| r1-ptzoptics30x-sharpnessmode-query | R1 | PtzOptics30X | SharpnessModeInquiry | 09 04 05 | - | §7.10; command inquiry list |
| r1-ptzopticsg2-saturation-query | R1 | PtzOpticsG2 | SaturationInquiry | 09 04 49 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-saturation-query | R14 | PtzOpticsG3 | SaturationInquiry | 09 04 49 | - | Queries page; current model scope |
| r1-ptzoptics30x-saturation-query | R1 | PtzOptics30X | SaturationInquiry | 09 04 49 | - | §7.10; command inquiry list |
| r8-sonyevih100-saturation-query | R8 | SonyEviH100 | SaturationInquiry | 09 04 49 | - | Technical Manual inquiry list |
| r1-ptzopticsg2-hue-query | R1 | PtzOpticsG2 | HueInquiry | 09 04 4F | - | §7.10; command inquiry list |
| r14-ptzopticsg3-hue-query | R14 | PtzOpticsG3 | HueInquiry | 09 04 4F | - | Queries page; current model scope |
| r1-ptzoptics30x-hue-query | R1 | PtzOptics30X | HueInquiry | 09 04 4F | - | §7.10; command inquiry list |
| r8-sonyevih100-hue-query | R8 | SonyEviH100 | HueInquiry | 09 04 4F | - | Technical Manual inquiry list |
| r1-ptzopticsg2-gain-query | R1 | PtzOpticsG2 | GainInquiry | 09 04 4C | - | §7.10; command inquiry list |
| r14-ptzopticsg3-gain-query | R14 | PtzOpticsG3 | GainInquiry | 09 04 4C | - | Queries page; current model scope |
| r1-ptzoptics30x-gain-query | R1 | PtzOptics30X | GainInquiry | 09 04 4C | - | §7.10; command inquiry list |
| r11-sonybrch900-gain-query | R11 | SonyBrcH900 | GainInquiry | 09 04 4C | - | Version 2.20 pp.20–22 |
| r8-sonyevih100-gain-query | R8 | SonyEviH100 | GainInquiry | 09 04 4C | - | Technical Manual inquiry list |
| r12-sonybrc300-gain-query | R12 | SonyBrc300 | GainInquiry | 09 04 4C | - | Technical Manual inquiry list |
| r21-nearusbrc300-gain-query | R21 | NearusBrc300 | GainInquiry | 09 04 4C | - | Command List inquiry list |
| r8-genericvisca-gain-query | R8 | GenericVisca | GainInquiry | 09 04 4C | - | Technical Manual inquiry list |
| r12-genericvisca-gain-query | R12 | GenericVisca | GainInquiry | 09 04 4C | - | Technical Manual inquiry list |
| r21-genericvisca-gain-query | R21 | GenericVisca | GainInquiry | 09 04 4C | - | Command List inquiry list |
| r1-ptzopticsg2-gainlimit-query | R1 | PtzOpticsG2 | GainLimitInquiry | 09 04 2C | - | §7.10; command inquiry list |
| r14-ptzopticsg3-gainlimit-query | R14 | PtzOpticsG3 | GainLimitInquiry | 09 04 2C | - | Queries page; current model scope |
| r1-ptzoptics30x-gainlimit-query | R1 | PtzOptics30X | GainLimitInquiry | 09 04 2C | - | §7.10; command inquiry list |
| r1-ptzopticsg2-backlight-query | R1 | PtzOpticsG2 | BacklightInquiry | 09 04 33 | 02=on;03=off | §7.10; command inquiry list |
| r14-ptzopticsg3-backlight-query | R14 | PtzOpticsG3 | BacklightInquiry | 09 04 33 | 02=on;03=off | Queries page; current model scope |
| r1-ptzoptics30x-backlight-query | R1 | PtzOptics30X | BacklightInquiry | 09 04 33 | 02=on;03=off | §7.10; command inquiry list |
| r7-sonyfr7-backlight-query | R7 | SonyFr7 | BacklightInquiry | 09 04 33 | 02=on;03=off | FR7 inquiry command list |
| r11-sonybrch900-backlight-query | R11 | SonyBrcH900 | BacklightInquiry | 09 04 33 | 02=on;03=off | Version 2.20 pp.20–22 |
| r8-sonyevih100-backlight-query | R8 | SonyEviH100 | BacklightInquiry | 09 04 33 | 02=on;03=off | Technical Manual inquiry list |
| r12-sonybrc300-backlight-query | R12 | SonyBrc300 | BacklightInquiry | 09 04 33 | 02=on;03=off | Technical Manual inquiry list |
| r21-nearusbrc300-backlight-query | R21 | NearusBrc300 | BacklightInquiry | 09 04 33 | 02=on;03=off | Command List inquiry list |
| r8-genericvisca-backlight-query | R8 | GenericVisca | BacklightInquiry | 09 04 33 | 02=on;03=off | Technical Manual inquiry list |
| r12-genericvisca-backlight-query | R12 | GenericVisca | BacklightInquiry | 09 04 33 | 02=on;03=off | Technical Manual inquiry list |
| r21-genericvisca-backlight-query | R21 | GenericVisca | BacklightInquiry | 09 04 33 | 02=on;03=off | Command List inquiry list |
| r1-ptzopticsg2-imageflip-query | R1 | PtzOpticsG2 | ImageFlipInquiry | 09 04 66 | 02=on;03=off | §7.10; command inquiry list |
| r14-ptzopticsg3-imageflip-query | R14 | PtzOpticsG3 | ImageFlipInquiry | 09 04 66 | 02=on;03=off | Queries page; current model scope |
| r1-ptzoptics30x-imageflip-query | R1 | PtzOptics30X | ImageFlipInquiry | 09 04 66 | 02=on;03=off | §7.10; command inquiry list |
| r12-sonybrc300-imageflip-query | R12 | SonyBrc300 | ImageFlipInquiry | 09 04 66 | 02=on;03=off | Technical Manual inquiry list |
| r21-nearusbrc300-imageflip-query | R21 | NearusBrc300 | ImageFlipInquiry | 09 04 66 | 02=on;03=off | Command List inquiry list |
| r1-ptzopticsg2-flipstate-query | R1 | PtzOpticsG2 | FlipStateInquiry | 09 04 A4 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-flipstate-query | R14 | PtzOpticsG3 | FlipStateInquiry | 09 04 A4 | - | Queries page; current model scope |
| r1-ptzoptics30x-flipstate-query | R1 | PtzOptics30X | FlipStateInquiry | 09 04 A4 | - | §7.10; command inquiry list |
| r1-ptzopticsg2-noisereduction2dmode-query | R1 | PtzOpticsG2 | NoiseReduction2DModeInquiry | 09 04 50 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-noisereduction2dmode-query | R14 | PtzOpticsG3 | NoiseReduction2DModeInquiry | 09 04 50 | - | Queries page; current model scope |
| r1-ptzoptics30x-noisereduction2dmode-query | R1 | PtzOptics30X | NoiseReduction2DModeInquiry | 09 04 50 | - | §7.10; command inquiry list |
| r1-ptzopticsg2-noisereduction2d-query | R1 | PtzOpticsG2 | NoiseReduction2DInquiry | 09 04 53 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-noisereduction2d-query | R14 | PtzOpticsG3 | NoiseReduction2DInquiry | 09 04 53 | - | Queries page; current model scope |
| r1-ptzoptics30x-noisereduction2d-query | R1 | PtzOptics30X | NoiseReduction2DInquiry | 09 04 53 | - | §7.10; command inquiry list |
| r8-sonyevih100-noisereduction2d-query | R8 | SonyEviH100 | NoiseReduction2DInquiry | 09 04 53 | - | Technical Manual inquiry list |
| r1-ptzopticsg2-noisereduction3d-query | R1 | PtzOpticsG2 | NoiseReduction3DInquiry | 09 04 54 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-noisereduction3d-query | R14 | PtzOpticsG3 | NoiseReduction3DInquiry | 09 04 54 | - | Queries page; current model scope |
| r1-ptzoptics30x-noisereduction3d-query | R1 | PtzOptics30X | NoiseReduction3DInquiry | 09 04 54 | - | §7.10; command inquiry list |
| b1-ptzopticsg2-focuszone-query | B1 | PtzOpticsG2 | FocusZoneInquiry | 09 04 AA | - | §7.13 read/set/readback |
| b1-ptzoptics30x-focuszone-query | B1 | PtzOptics30X | FocusZoneInquiry | 09 04 AA | - | §7.13 read/set/readback |
| r1-ptzopticsg2-autofocussensitivity-query | R1 | PtzOpticsG2 | AutoFocusSensitivityInquiry | 09 04 58 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-autofocussensitivity-query | R14 | PtzOpticsG3 | AutoFocusSensitivityInquiry | 09 04 58 | - | Queries page; current model scope |
| r1-ptzoptics30x-autofocussensitivity-query | R1 | PtzOptics30X | AutoFocusSensitivityInquiry | 09 04 58 | - | §7.10; command inquiry list |
| r8-sonyevih100-autofocussensitivity-query | R8 | SonyEviH100 | AutoFocusSensitivityInquiry | 09 04 58 | - | Technical Manual inquiry list |
| r8-sonyevih100-focusnearlimit-query | R8 | SonyEviH100 | FocusNearLimitInquiry | 09 04 28 | - | Technical Manual inquiry list |
| r1-ptzopticsg2-focusmode-query | R1 | PtzOpticsG2 | FocusModeInquiry | 09 04 38 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-focusmode-query | R14 | PtzOpticsG3 | FocusModeInquiry | 09 04 38 | - | Queries page; current model scope |
| r1-ptzoptics30x-focusmode-query | R1 | PtzOptics30X | FocusModeInquiry | 09 04 38 | - | §7.10; command inquiry list |
| r7-sonyfr7-focusmode-query | R7 | SonyFr7 | FocusModeInquiry | 09 04 38 | - | FR7 inquiry command list |
| r11-sonybrch900-focusmode-query | R11 | SonyBrcH900 | FocusModeInquiry | 09 04 38 | - | Version 2.20 pp.20–22 |
| r8-sonyevih100-focusmode-query | R8 | SonyEviH100 | FocusModeInquiry | 09 04 38 | - | Technical Manual inquiry list |
| r12-sonybrc300-focusmode-query | R12 | SonyBrc300 | FocusModeInquiry | 09 04 38 | - | Technical Manual inquiry list |
| r21-nearusbrc300-focusmode-query | R21 | NearusBrc300 | FocusModeInquiry | 09 04 38 | - | Command List inquiry list |
| r8-genericvisca-focusmode-query | R8 | GenericVisca | FocusModeInquiry | 09 04 38 | - | Technical Manual inquiry list |
| r12-genericvisca-focusmode-query | R12 | GenericVisca | FocusModeInquiry | 09 04 38 | - | Technical Manual inquiry list |
| r21-genericvisca-focusmode-query | R21 | GenericVisca | FocusModeInquiry | 09 04 38 | - | Command List inquiry list |
| r1-ptzopticsg2-menuopenclose-query | R1 | PtzOpticsG2 | MenuOpenCloseInquiry | 09 06 06 | 02=on;03=off | §7.10; command inquiry list |
| r14-ptzopticsg3-menuopenclose-query | R14 | PtzOpticsG3 | MenuOpenCloseInquiry | 09 06 06 | 02=on;03=off | Queries page; current model scope |
| r1-ptzoptics30x-menuopenclose-query | R1 | PtzOptics30X | MenuOpenCloseInquiry | 09 06 06 | 02=on;03=off | §7.10; command inquiry list |
| r7-sonyfr7-menuopenclose-query | R7 | SonyFr7 | MenuOpenCloseInquiry | 09 06 06 | 02=on;03=off | FR7 inquiry command list |
| r7-sonyfr7-ndfilter-query | R7 | SonyFr7 | NdFilterInquiry | 09 7E 04 42 | - | FR7 inquiry command list |
| r1-ptzopticsg2-pictureeffect-query | R1 | PtzOpticsG2 | PictureEffectInquiry | 09 04 63 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-pictureeffect-query | R14 | PtzOpticsG3 | PictureEffectInquiry | 09 04 63 | - | Queries page; current model scope |
| r1-ptzoptics30x-pictureeffect-query | R1 | PtzOptics30X | PictureEffectInquiry | 09 04 63 | - | §7.10; command inquiry list |
| r1-ptzopticsg2-autowhitebalancesensitivity-query | R1 | PtzOpticsG2 | AutoWhiteBalanceSensitivityInquiry | 09 04 A9 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-autowhitebalancesensitivity-query | R14 | PtzOpticsG3 | AutoWhiteBalanceSensitivityInquiry | 09 04 A9 | - | Queries page; current model scope |
| r1-ptzoptics30x-autowhitebalancesensitivity-query | R1 | PtzOptics30X | AutoWhiteBalanceSensitivityInquiry | 09 04 A9 | - | §7.10; command inquiry list |
| r1-ptzopticsg2-exposurecompensationposition-query | R1 | PtzOpticsG2 | ExposureCompensationPositionInquiry | 09 04 4E | - | §7.10; command inquiry list |
| r14-ptzopticsg3-exposurecompensationposition-query | R14 | PtzOpticsG3 | ExposureCompensationPositionInquiry | 09 04 4E | - | Queries page; current model scope |
| r1-ptzoptics30x-exposurecompensationposition-query | R1 | PtzOptics30X | ExposureCompensationPositionInquiry | 09 04 4E | - | §7.10; command inquiry list |
| r8-sonyevih100-exposurecompensationposition-query | R8 | SonyEviH100 | ExposureCompensationPositionInquiry | 09 04 4E | - | Technical Manual inquiry list |
| r12-sonybrc300-exposurecompensationposition-query | R12 | SonyBrc300 | ExposureCompensationPositionInquiry | 09 04 4E | - | Technical Manual inquiry list |
| r21-nearusbrc300-exposurecompensationposition-query | R21 | NearusBrc300 | ExposureCompensationPositionInquiry | 09 04 4E | - | Command List inquiry list |
| r8-genericvisca-exposurecompensationposition-query | R8 | GenericVisca | ExposureCompensationPositionInquiry | 09 04 4E | - | Technical Manual inquiry list |
| r12-genericvisca-exposurecompensationposition-query | R12 | GenericVisca | ExposureCompensationPositionInquiry | 09 04 4E | - | Technical Manual inquiry list |
| r21-genericvisca-exposurecompensationposition-query | R21 | GenericVisca | ExposureCompensationPositionInquiry | 09 04 4E | - | Command List inquiry list |
| r1-ptzopticsg2-redtuning-query | R1 | PtzOpticsG2 | RedTuningInquiry | 09 04 43 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-redtuning-query | R14 | PtzOpticsG3 | RedTuningInquiry | 09 04 43 | - | Queries page; current model scope |
| r1-ptzoptics30x-redtuning-query | R1 | PtzOptics30X | RedTuningInquiry | 09 04 43 | - | §7.10; command inquiry list |
| r1-ptzopticsg2-bluetuning-query | R1 | PtzOpticsG2 | BlueTuningInquiry | 09 04 44 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-bluetuning-query | R14 | PtzOpticsG3 | BlueTuningInquiry | 09 04 44 | - | Queries page; current model scope |
| r1-ptzoptics30x-bluetuning-query | R1 | PtzOptics30X | BlueTuningInquiry | 09 04 44 | - | §7.10; command inquiry list |
| r1-ptzopticsg2-gamma-query | R1 | PtzOpticsG2 | GammaInquiry | 09 04 5B | - | §7.10; command inquiry list |
| r14-ptzopticsg3-gamma-query | R14 | PtzOpticsG3 | GammaInquiry | 09 04 5B | - | Queries page; current model scope |
| r1-ptzoptics30x-gamma-query | R1 | PtzOptics30X | GammaInquiry | 09 04 5B | - | §7.10; command inquiry list |
| r8-sonyevih100-gamma-query | R8 | SonyEviH100 | GammaInquiry | 09 04 5B | - | Technical Manual inquiry list |
| r1-ptzopticsg2-sharpnessposition-query | R1 | PtzOpticsG2 | SharpnessPositionInquiry | 09 04 42 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-sharpnessposition-query | R14 | PtzOpticsG3 | SharpnessPositionInquiry | 09 04 42 | - | Queries page; current model scope |
| r1-ptzoptics30x-sharpnessposition-query | R1 | PtzOptics30X | SharpnessPositionInquiry | 09 04 42 | - | §7.10; command inquiry list |
| r7-sonyfr7-sharpnessposition-query | R7 | SonyFr7 | SharpnessPositionInquiry | 09 04 42 | - | FR7 inquiry command list |
| r8-sonyevih100-sharpnessposition-query | R8 | SonyEviH100 | SharpnessPositionInquiry | 09 04 42 | - | Technical Manual inquiry list |
| r1-ptzopticsg2-usbaudio-query | R1 | PtzOpticsG2 | UsbAudioInquiry | 2A 02 A0 04 | 02=on;03=off | §7.10; command inquiry list |
| r1-ptzoptics30x-usbaudio-query | R1 | PtzOptics30X | UsbAudioInquiry | 2A 02 A0 04 | 02=on;03=off | §7.10; command inquiry list |
| r7-sonyfr7-ndfilterpreset-query | R7 | SonyFr7 | NdFilterPresetInquiry | 09 7E 01 53 | - | FR7 inquiry command list |
| r7-sonyfr7-tallyred-query | R7 | SonyFr7 | TallyRedInquiry | 09 7E 01 0A | 02=on;03=off | FR7 inquiry command list |
| r7-sonyfr7-tallygreen-query | R7 | SonyFr7 | TallyGreenInquiry | 09 7E 04 1A | 02=on;03=off | FR7 inquiry command list |
| r1-ptzopticsg2-flickermode-query | R1 | PtzOpticsG2 | FlickerModeInquiry | 09 04 55 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-flickermode-query | R14 | PtzOpticsG3 | FlickerModeInquiry | 09 04 55 | - | Queries page; current model scope |
| r1-ptzoptics30x-flickermode-query | R1 | PtzOptics30X | FlickerModeInquiry | 09 04 55 | - | §7.10; command inquiry list |
| r1-ptzopticsg2-contrast-query | R1 | PtzOpticsG2 | ContrastInquiry | 09 04 A2 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-contrast-query | R14 | PtzOpticsG3 | ContrastInquiry | 09 04 A2 | - | Queries page; current model scope |
| r1-ptzoptics30x-contrast-query | R1 | PtzOptics30X | ContrastInquiry | 09 04 A2 | - | §7.10; command inquiry list |
| r1-ptzopticsg2-luminance-query | R1 | PtzOpticsG2 | LuminanceInquiry | 09 04 A1 | - | §7.10; command inquiry list |
| r14-ptzopticsg3-luminance-query | R14 | PtzOpticsG3 | LuminanceInquiry | 09 04 A1 | - | Queries page; current model scope |
| r1-ptzoptics30x-luminance-query | R1 | PtzOptics30X | LuminanceInquiry | 09 04 A1 | - | §7.10; command inquiry list |

| r11-sonybrch900-zoomposition-query | R11 | SonyBrcH900 | ZoomPositionInquiry | 09 04 47 | - | Version 2.20 p. 23 |
| r11-sonybrch900-focusposition-query | R11 | SonyBrcH900 | FocusPositionInquiry | 09 04 48 | - | Version 2.20 p. 23 |
| r11-sonybrch900-menuopenclose-query | R11 | SonyBrcH900 | MenuOpenCloseInquiry | 09 06 06 | 02=on;03=off | Version 2.20 p. 22 |

| r1-ptzopticsg2-ptzopticsantiflicker-grant | R1 | PtzOpticsG2 | grant.PtzOpticsAntiFlicker | 01 04 23 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-ptzopticssettingssave-grant | R1 | PtzOpticsG2 | grant.PtzOpticsSettingsSave | 01 04 A5 10 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-ptzopticspresetrecallspeed-grant | R1 | PtzOpticsG2 | grant.PtzOpticsPresetRecallSpeed | 01 06 01 pp | - | §7 command/inquiry tables |
| r1-ptzopticsg2-ptzopticsmulticaststreaming-grant | R1 | PtzOpticsG2 | grant.PtzOpticsMulticastStreaming | 0B 01 23 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-ptzopticsndiquality-grant | R1 | PtzOpticsG2 | grant.PtzOpticsNdiQuality | 0B 01 01 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-exposuremode-grant | R1 | PtzOpticsG2 | grant.ExposureMode | 01 04 39;09 04 39 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-exposurecompensation-grant | R1 | PtzOpticsG2 | grant.ExposureCompensation | 01 04 3E;01 04 0E;01 04 4E;09 04 3E;09 04 4E | - | §7 command/inquiry tables |
| r1-ptzopticsg2-brightnesscontrol-grant | R1 | PtzOpticsG2 | grant.BrightnessControl | 01 04 0D;01 04 4D;09 04 4D | - | §7 command/inquiry tables |
| r1-ptzopticsg2-focuslock-grant | R1 | PtzOpticsG2 | grant.FocusLock | 0A 04 68 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-directzoom-grant | R1 | PtzOpticsG2 | grant.DirectZoom | 01 04 47;09 04 47 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-iriscontrol-grant | R1 | PtzOpticsG2 | grant.IrisControl | 01 04 0B;01 04 4B;09 04 4B | - | §7 command/inquiry tables |
| b1-ptzopticsg2-focuszone-grant | B1 | PtzOpticsG2 | grant.FocusZone | 01 04 AA | - | §7.13 model/firmware read-set-readback |
| b1-ptzopticsg2-focuszoneinquiry-grant | B1 | PtzOpticsG2 | grant.FocusZoneInquiry | 09 04 AA | - | §7.13 model/firmware read-set-readback |
| r1-ptzopticsg2-usbaudio-grant | R1 | PtzOpticsG2 | grant.UsbAudio | 0A 02 A0;2A 02 A0 04 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-backlightcompensation-grant | R1 | PtzOpticsG2 | grant.BacklightCompensation | 01 04 33;09 04 33 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-widedynamicrange-grant | R1 | PtzOpticsG2 | grant.WideDynamicRange | 01 04 25 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-colortemperature-grant | R1 | PtzOpticsG2 | grant.ColorTemperature | 01 04 20;01 04 35 20 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-colortemperatureinquiry-grant | R1 | PtzOpticsG2 | grant.ColorTemperatureInquiry | 09 04 20 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-rgbgain-grant | R1 | PtzOpticsG2 | grant.RgbGain | 01 04 03;01 04 04;01 04 43;01 04 44;09 04 43;09 04 44 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-rgbtuning-grant | R1 | PtzOpticsG2 | grant.RgbTuning | 01 04 43 tuning;01 04 44 tuning;09 04 43;09 04 44 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-onepushwhitebalance-grant | R1 | PtzOpticsG2 | grant.OnePushWhiteBalance | 01 04 35 03;01 04 10 05 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-autowhitebalancesensitivity-grant | R1 | PtzOpticsG2 | grant.AutoWhiteBalanceSensitivity | 01 04 A9;09 04 A9 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-imageflip-grant | R1 | PtzOpticsG2 | grant.ImageFlip | 01 04 66;09 04 66 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-imagemirror-grant | R1 | PtzOpticsG2 | grant.ImageMirror | 01 04 61 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-combinedimageflip-grant | R1 | PtzOpticsG2 | grant.CombinedImageFlip | 01 04 A4;09 04 A4 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-contrastcontrol-grant | R1 | PtzOpticsG2 | grant.ContrastControl | 01 04 A2;09 04 A2 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-sharpnesscontrol-grant | R1 | PtzOpticsG2 | grant.SharpnessControl | 01 04 02;01 04 42;09 04 42 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-saturationcontrol-grant | R1 | PtzOpticsG2 | grant.SaturationControl | 01 04 49;09 04 49 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-huecontrol-grant | R1 | PtzOpticsG2 | grant.HueControl | 01 04 4F;09 04 4F | - | §7 command/inquiry tables |
| r1-ptzopticsg2-luminancecontrol-grant | R1 | PtzOpticsG2 | grant.LuminanceControl | 01 04 A1;09 04 A1 | - | §7 command/inquiry tables |
| r1-ptzopticsg2-gammacontrol-grant | R1 | PtzOpticsG2 | grant.GammaControl | 01 04 5B;09 04 5B | - | §7 command/inquiry tables |
| r1-ptzopticsg2-noisereduction2d-grant | R1 | PtzOpticsG2 | grant.NoiseReduction2D | 09 04 53 | - | §7 command/inquiry tables |
| r14-ptzopticsg2-noisereduction2dmode-grant | R14 | PtzOpticsG2 | grant.NoiseReduction2DMode | 01 04 50;09 04 50 | - | Current Developer Portal command and query pages |
| r1-ptzopticsg2-noisereduction3d-grant | R1 | PtzOpticsG2 | grant.NoiseReduction3D | 09 04 54 | - | §7 command/inquiry tables |
| r14-ptzopticsg2-noisereduction2dcontrol-grant | R14 | PtzOpticsG2 | grant.NoiseReduction2DControl | 01 04 53 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg2-noisereduction3dcontrol-grant | R14 | PtzOpticsG2 | grant.NoiseReduction3DControl | 01 04 54 | - | Current Developer Portal command and query pages |
| r1-ptzopticsg2-pictureeffect-grant | R1 | PtzOpticsG2 | grant.PictureEffect | 01 04 63;09 04 63 | - | §7 command/inquiry tables |
| r10-ptzopticsg3-ptzopticsantiflicker-grant | R10 | PtzOpticsG3 | grant.PtzOpticsAntiFlicker | 01 04 23 | - | Move 4K G3 command/inquiry list |
| r10-ptzopticsg3-ptzopticssettingssave-grant | R10 | PtzOpticsG3 | grant.PtzOpticsSettingsSave | 01 04 A5 10 | - | Move 4K G3 command/inquiry list |
| r10-ptzopticsg3-ptzopticspresetrecallspeed-grant | R10 | PtzOpticsG3 | grant.PtzOpticsPresetRecallSpeed | 01 06 01 pp | - | Move 4K G3 command/inquiry list |
| r10-ptzopticsg3-ptzopticsmulticaststreaming-grant | R10 | PtzOpticsG3 | grant.PtzOpticsMulticastStreaming | 0B 01 23 | - | Move 4K G3 command/inquiry list |
| r10-ptzopticsg3-ptzopticsndiquality-grant | R10 | PtzOpticsG3 | grant.PtzOpticsNdiQuality | 0B 01 01 | - | Move 4K G3 command/inquiry list |
| r14-ptzopticsg3-exposuremode-grant | R14 | PtzOpticsG3 | grant.ExposureMode | 01 04 39;09 04 39 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-exposurecompensation-grant | R14 | PtzOpticsG3 | grant.ExposureCompensation | 01 04 3E;01 04 0E;01 04 4E;09 04 3E;09 04 4E | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-brightnesscontrol-grant | R14 | PtzOpticsG3 | grant.BrightnessControl | 01 04 0D;01 04 4D;09 04 4D | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-focuslock-grant | R14 | PtzOpticsG3 | grant.FocusLock | 0A 04 68 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-directzoom-grant | R14 | PtzOpticsG3 | grant.DirectZoom | 01 04 47;09 04 47 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-iriscontrol-grant | R14 | PtzOpticsG3 | grant.IrisControl | 01 04 0B;01 04 4B;09 04 4B | - | Current Developer Portal command and query pages |
| r20-ptzopticsg3-focuszone-grant | R20 | PtzOpticsG3 | grant.FocusZone | 01 04 AA | - | CAM_AFZone selection rows |
| r14-ptzopticsg3-backlightcompensation-grant | R14 | PtzOpticsG3 | grant.BacklightCompensation | 01 04 33;09 04 33 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-widedynamicrange-grant | R14 | PtzOpticsG3 | grant.WideDynamicRange | 01 04 25 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-colortemperature-grant | R14 | PtzOpticsG3 | grant.ColorTemperature | 01 04 20;01 04 35 20 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-rgbgain-grant | R14 | PtzOpticsG3 | grant.RgbGain | 01 04 03;01 04 04;01 04 43;01 04 44;09 04 43;09 04 44 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-rgbtuning-grant | R14 | PtzOpticsG3 | grant.RgbTuning | 01 04 43 tuning;01 04 44 tuning;09 04 43;09 04 44 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-onepushwhitebalance-grant | R14 | PtzOpticsG3 | grant.OnePushWhiteBalance | 01 04 35 03;01 04 10 05 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-autowhitebalancesensitivity-grant | R14 | PtzOpticsG3 | grant.AutoWhiteBalanceSensitivity | 01 04 A9;09 04 A9 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-imageflip-grant | R14 | PtzOpticsG3 | grant.ImageFlip | 01 04 66;09 04 66 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-imagemirror-grant | R14 | PtzOpticsG3 | grant.ImageMirror | 01 04 61 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-combinedimageflip-grant | R14 | PtzOpticsG3 | grant.CombinedImageFlip | 01 04 A4;09 04 A4 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-contrastcontrol-grant | R14 | PtzOpticsG3 | grant.ContrastControl | 01 04 A2;09 04 A2 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-sharpnesscontrol-grant | R14 | PtzOpticsG3 | grant.SharpnessControl | 01 04 02;01 04 42;09 04 42 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-saturationcontrol-grant | R14 | PtzOpticsG3 | grant.SaturationControl | 01 04 49;09 04 49 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-huecontrol-grant | R14 | PtzOpticsG3 | grant.HueControl | 01 04 4F;09 04 4F | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-luminancecontrol-grant | R14 | PtzOpticsG3 | grant.LuminanceControl | 01 04 A1;09 04 A1 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-gammacontrol-grant | R14 | PtzOpticsG3 | grant.GammaControl | 01 04 5B;09 04 5B | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-noisereduction2d-grant | R14 | PtzOpticsG3 | grant.NoiseReduction2D | 09 04 53 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-noisereduction2dmode-grant | R14 | PtzOpticsG3 | grant.NoiseReduction2DMode | 01 04 50;09 04 50 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-noisereduction3d-grant | R14 | PtzOpticsG3 | grant.NoiseReduction3D | 09 04 54 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-noisereduction2dcontrol-grant | R14 | PtzOpticsG3 | grant.NoiseReduction2DControl | 01 04 53 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-noisereduction3dcontrol-grant | R14 | PtzOpticsG3 | grant.NoiseReduction3DControl | 01 04 54 | - | Current Developer Portal command and query pages |
| r14-ptzopticsg3-pictureeffect-grant | R14 | PtzOpticsG3 | grant.PictureEffect | 01 04 63;09 04 63 | - | Current Developer Portal command and query pages |
| r1-ptzoptics30x-ptzopticsantiflicker-grant | R1 | PtzOptics30X | grant.PtzOpticsAntiFlicker | 01 04 23 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-ptzopticssettingssave-grant | R1 | PtzOptics30X | grant.PtzOpticsSettingsSave | 01 04 A5 10 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-ptzopticspresetrecallspeed-grant | R1 | PtzOptics30X | grant.PtzOpticsPresetRecallSpeed | 01 06 01 pp | - | §7 command/inquiry tables |
| r1-ptzoptics30x-ptzopticsmulticaststreaming-grant | R1 | PtzOptics30X | grant.PtzOpticsMulticastStreaming | 0B 01 23 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-ptzopticsndiquality-grant | R1 | PtzOptics30X | grant.PtzOpticsNdiQuality | 0B 01 01 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-exposuremode-grant | R1 | PtzOptics30X | grant.ExposureMode | 01 04 39;09 04 39 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-exposurecompensation-grant | R1 | PtzOptics30X | grant.ExposureCompensation | 01 04 3E;01 04 0E;01 04 4E;09 04 3E;09 04 4E | - | §7 command/inquiry tables |
| r1-ptzoptics30x-brightnesscontrol-grant | R1 | PtzOptics30X | grant.BrightnessControl | 01 04 0D;01 04 4D;09 04 4D | - | §7 command/inquiry tables |
| r1-ptzoptics30x-focuslock-grant | R1 | PtzOptics30X | grant.FocusLock | 0A 04 68 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-directzoom-grant | R1 | PtzOptics30X | grant.DirectZoom | 01 04 47;09 04 47 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-iriscontrol-grant | R1 | PtzOptics30X | grant.IrisControl | 01 04 0B;01 04 4B;09 04 4B | - | §7 command/inquiry tables |
| b1-ptzoptics30x-focuszone-grant | B1 | PtzOptics30X | grant.FocusZone | 01 04 AA | - | §7.13 model/firmware read-set-readback |
| b1-ptzoptics30x-focuszoneinquiry-grant | B1 | PtzOptics30X | grant.FocusZoneInquiry | 09 04 AA | - | §7.13 model/firmware read-set-readback |
| r1-ptzoptics30x-usbaudio-grant | R1 | PtzOptics30X | grant.UsbAudio | 0A 02 A0;2A 02 A0 04 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-backlightcompensation-grant | R1 | PtzOptics30X | grant.BacklightCompensation | 01 04 33;09 04 33 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-widedynamicrange-grant | R1 | PtzOptics30X | grant.WideDynamicRange | 01 04 25 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-colortemperature-grant | R1 | PtzOptics30X | grant.ColorTemperature | 01 04 20;01 04 35 20 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-colortemperatureinquiry-grant | R1 | PtzOptics30X | grant.ColorTemperatureInquiry | 09 04 20 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-rgbgain-grant | R1 | PtzOptics30X | grant.RgbGain | 01 04 03;01 04 04;01 04 43;01 04 44;09 04 43;09 04 44 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-rgbtuning-grant | R1 | PtzOptics30X | grant.RgbTuning | 01 04 43 tuning;01 04 44 tuning;09 04 43;09 04 44 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-onepushwhitebalance-grant | R1 | PtzOptics30X | grant.OnePushWhiteBalance | 01 04 35 03;01 04 10 05 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-autowhitebalancesensitivity-grant | R1 | PtzOptics30X | grant.AutoWhiteBalanceSensitivity | 01 04 A9;09 04 A9 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-imageflip-grant | R1 | PtzOptics30X | grant.ImageFlip | 01 04 66;09 04 66 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-imagemirror-grant | R1 | PtzOptics30X | grant.ImageMirror | 01 04 61 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-combinedimageflip-grant | R1 | PtzOptics30X | grant.CombinedImageFlip | 01 04 A4;09 04 A4 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-contrastcontrol-grant | R1 | PtzOptics30X | grant.ContrastControl | 01 04 A2;09 04 A2 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-sharpnesscontrol-grant | R1 | PtzOptics30X | grant.SharpnessControl | 01 04 02;01 04 42;09 04 42 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-saturationcontrol-grant | R1 | PtzOptics30X | grant.SaturationControl | 01 04 49;09 04 49 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-huecontrol-grant | R1 | PtzOptics30X | grant.HueControl | 01 04 4F;09 04 4F | - | §7 command/inquiry tables |
| r1-ptzoptics30x-luminancecontrol-grant | R1 | PtzOptics30X | grant.LuminanceControl | 01 04 A1;09 04 A1 | - | §7 command/inquiry tables |
| r1-ptzoptics30x-gammacontrol-grant | R1 | PtzOptics30X | grant.GammaControl | 01 04 5B;09 04 5B | - | §7 command/inquiry tables |
| r1-ptzoptics30x-noisereduction2d-grant | R1 | PtzOptics30X | grant.NoiseReduction2D | 09 04 53 | - | §7 command/inquiry tables |
| r14-ptzoptics30x-noisereduction2dmode-grant | R14 | PtzOptics30X | grant.NoiseReduction2DMode | 01 04 50;09 04 50 | - | Current Developer Portal command and query pages |
| r1-ptzoptics30x-noisereduction3d-grant | R1 | PtzOptics30X | grant.NoiseReduction3D | 09 04 54 | - | §7 command/inquiry tables |
| r14-ptzoptics30x-noisereduction2dcontrol-grant | R14 | PtzOptics30X | grant.NoiseReduction2DControl | 01 04 53 | - | Current Developer Portal command and query pages |
| r14-ptzoptics30x-noisereduction3dcontrol-grant | R14 | PtzOptics30X | grant.NoiseReduction3DControl | 01 04 54 | - | Current Developer Portal command and query pages |
| r1-ptzoptics30x-pictureeffect-grant | R1 | PtzOptics30X | grant.PictureEffect | 01 04 63;09 04 63 | - | §7 command/inquiry tables |
| r7-sonyfr7-versioninquiry-grant | R7 | SonyFr7 | grant.VersionInquiry | 09 00 02 | - | FR7 v4 pp.14–23 |
| r7-sonyfr7-sonyspotlight-grant | R7 | SonyFr7 | grant.SonySpotlight | 01 04 3A 02;01 04 3A 03 | - | FR7 v4 pp.14–23 |
| r7-sonyfr7-pushautofocus-grant | R7 | SonyFr7 | grant.PushAutoFocus | 01 7E 04 58 | - | FR7 v4 pp.14–23 |
| r7-sonyfr7-directzoom-grant | R7 | SonyFr7 | grant.DirectZoom | 01 04 47;09 04 47 | - | FR7 v4 pp.14–23 |
| r7-sonyfr7-backlightcompensation-grant | R7 | SonyFr7 | grant.BacklightCompensation | 01 04 33;09 04 33 | - | FR7 v4 pp.14–23 |
| r7-sonyfr7-autotrackingwhitebalance-grant | R7 | SonyFr7 | grant.AutoTrackingWhiteBalance | 01 04 35 04 | - | FR7 v4 pp.14–23 |
| r7-sonyfr7-sharpnesscontrol-grant | R7 | SonyFr7 | grant.SharpnessControl | 01 04 02;01 04 42;09 04 42 | - | FR7 v4 pp.14–23 |
| r7-sonyfr7-tally-grant | R7 | SonyFr7 | grant.Tally | 01 7E 01 0A;01 7E 04 1A;09 7E 01 0A;09 7E 04 1A | - | FR7 v4 pp.14–23 |
| r7-sonyfr7-directmenu-grant | R7 | SonyFr7 | grant.DirectMenu | 01 7E 04 72 | - | FR7 v4 pp.14–23 |
| r7-sonyfr7-ndfilter-grant | R7 | SonyFr7 | grant.NdFilter | 01 7E 04 52;01 7E 04 12;01 7E 04 53;01 7E 04 42;09 7E 01 53;09 7E 04 42 | - | FR7 v4 pp.14–23 |
| r7-sonyfr7-variablespeed-grant | R7 | SonyFr7 | grant.VariableSpeed | 01 06 45 | - | FR7 v4 pp.14–23 |
| r11-sonybrch900-versioninquiry-grant | R11 | SonyBrcH900 | grant.VersionInquiry | 09 00 02 | - | BRC-H900 v2.20 pp.14,20,22 |
| r11-sonybrch900-exposuremode-grant | R11 | SonyBrcH900 | grant.ExposureMode | 01 04 39;09 04 39 | - | BRC-H900 v2.20 pp.14,20,22 |
| r11-sonybrch900-iriscontrol-grant | R11 | SonyBrcH900 | grant.IrisControl | 01 04 0B;01 04 4B;09 04 4B | - | BRC-H900 v2.20 pp.14,20,22 |
| r8-sonyevih100-versioninquiry-grant | R8 | SonyEviH100 | grant.VersionInquiry | 09 00 02 | - | EVI-H100 Technical Manual command/inquiry lists |
| r8-sonyevih100-sonyautoslowshutter-grant | R8 | SonyEviH100 | grant.SonyAutoSlowShutter | 01 04 5A 02;01 04 5A 03 | - | EVI-H100 Technical Manual command/inquiry lists |
| r8-sonyevih100-exposuremode-grant | R8 | SonyEviH100 | grant.ExposureMode | 01 04 39;09 04 39 | - | EVI-H100 Technical Manual command/inquiry lists |
| r8-sonyevih100-directzoom-grant | R8 | SonyEviH100 | grant.DirectZoom | 01 04 47;09 04 47 | - | EVI-H100 Technical Manual command/inquiry lists |
| r8-sonyevih100-digitalzoomtoggle-grant | R8 | SonyEviH100 | grant.DigitalZoomToggle | 01 04 06;09 04 06 | - | EVI-H100 Technical Manual command/inquiry lists |
| r8-sonyevih100-digitalzoomrange-grant | R8 | SonyEviH100 | grant.DigitalZoomRange | 01 04 47 digital positions | - | EVI-H100 Technical Manual command/inquiry lists |
| r8-sonyevih100-onepushfocus-grant | R8 | SonyEviH100 | grant.OnePushFocus | 01 04 18 01 | - | EVI-H100 Technical Manual command/inquiry lists |
| r8-sonyevih100-iriscontrol-grant | R8 | SonyEviH100 | grant.IrisControl | 01 04 0B;01 04 4B;09 04 4B | - | EVI-H100 Technical Manual command/inquiry lists |
| r8-sonyevih100-focusnearlimitinquiry-grant | R8 | SonyEviH100 | grant.FocusNearLimitInquiry | 09 04 28 | - | EVI-H100 Technical Manual command/inquiry lists |
| r8-sonyevih100-brightnesscontrol-grant | R8 | SonyEviH100 | grant.BrightnessControl | 01 04 0D;01 04 4D;09 04 4D | - | EVI-H100 Technical Manual command/inquiry lists |
| r8-sonyevih100-exposurecompensation-grant | R8 | SonyEviH100 | grant.ExposureCompensation | 01 04 3E;01 04 0E;01 04 4E;09 04 3E;09 04 4E | - | EVI-H100 Technical Manual command/inquiry lists |
| r8-sonyevih100-backlightcompensation-grant | R8 | SonyEviH100 | grant.BacklightCompensation | 01 04 33;09 04 33 | - | EVI-H100 Technical Manual command/inquiry lists |
| r8-sonyevih100-rgbgain-grant | R8 | SonyEviH100 | grant.RgbGain | 01 04 03;01 04 04;01 04 43;01 04 44;09 04 43;09 04 44 | - | EVI-H100 Technical Manual command/inquiry lists |
| r8-sonyevih100-onepushwhitebalance-grant | R8 | SonyEviH100 | grant.OnePushWhiteBalance | 01 04 35 03;01 04 10 05 | - | EVI-H100 Technical Manual command/inquiry lists |
| r8-sonyevih100-saturationcontrol-grant | R8 | SonyEviH100 | grant.SaturationControl | 01 04 49;09 04 49 | - | EVI-H100 Technical Manual command/inquiry lists |
| r8-sonyevih100-huecontrol-grant | R8 | SonyEviH100 | grant.HueControl | 01 04 4F;09 04 4F | - | EVI-H100 Technical Manual command/inquiry lists |
| r8-sonyevih100-gammacontrol-grant | R8 | SonyEviH100 | grant.GammaControl | 01 04 5B;09 04 5B | - | EVI-H100 Technical Manual command/inquiry lists |
| r8-sonyevih100-noisereduction2d-grant | R8 | SonyEviH100 | grant.NoiseReduction2D | 09 04 53 | - | EVI-H100 Technical Manual command/inquiry lists |
| r8-sonyevih100-noisereduction2dcontrol-grant | R8 | SonyEviH100 | grant.NoiseReduction2DControl | 01 04 53 | - | EVI-H100 Technical Manual command/inquiry lists |
| r12-sonybrc300-versioninquiry-grant | R12 | SonyBrc300 | grant.VersionInquiry | 09 00 02 | - | BRC-300 Technical Manual command/inquiry lists |
| r12-sonybrc300-sonyautoslowshutter-grant | R12 | SonyBrc300 | grant.SonyAutoSlowShutter | 01 04 5A 02;01 04 5A 03 | - | BRC-300 Technical Manual command/inquiry lists |
| r12-sonybrc300-exposuremode-grant | R12 | SonyBrc300 | grant.ExposureMode | 01 04 39;09 04 39 | - | BRC-300 Technical Manual command/inquiry lists |
| r12-sonybrc300-directzoom-grant | R12 | SonyBrc300 | grant.DirectZoom | 01 04 47;09 04 47 | - | BRC-300 Technical Manual command/inquiry lists |
| r12-sonybrc300-digitalzoomtoggle-grant | R12 | SonyBrc300 | grant.DigitalZoomToggle | 01 04 06;09 04 06 | - | BRC-300 Technical Manual command/inquiry lists |
| r12-sonybrc300-digitalzoomrange-grant | R12 | SonyBrc300 | grant.DigitalZoomRange | 01 04 47 digital positions | - | BRC-300 Technical Manual command/inquiry lists |
| r12-sonybrc300-onepushfocus-grant | R12 | SonyBrc300 | grant.OnePushFocus | 01 04 18 01 | - | BRC-300 Technical Manual command/inquiry lists |
| r12-sonybrc300-iriscontrol-grant | R12 | SonyBrc300 | grant.IrisControl | 01 04 0B;01 04 4B;09 04 4B | - | BRC-300 Technical Manual command/inquiry lists |
| r12-sonybrc300-brightnesscontrol-grant | R12 | SonyBrc300 | grant.BrightnessControl | 01 04 0D;01 04 4D;09 04 4D | - | BRC-300 Technical Manual command/inquiry lists |
| r12-sonybrc300-exposurecompensation-grant | R12 | SonyBrc300 | grant.ExposureCompensation | 01 04 3E;01 04 0E;01 04 4E;09 04 3E;09 04 4E | - | BRC-300 Technical Manual command/inquiry lists |
| r12-sonybrc300-backlightcompensation-grant | R12 | SonyBrc300 | grant.BacklightCompensation | 01 04 33;09 04 33 | - | BRC-300 Technical Manual command/inquiry lists |
| r12-sonybrc300-rgbgain-grant | R12 | SonyBrc300 | grant.RgbGain | 01 04 03;01 04 04;01 04 43;01 04 44;09 04 43;09 04 44 | - | BRC-300 Technical Manual command/inquiry lists |
| r12-sonybrc300-imageflip-grant | R12 | SonyBrc300 | grant.ImageFlip | 01 04 66;09 04 66 | - | BRC-300 Technical Manual command/inquiry lists |
| r12-sonybrc300-onepushwhitebalance-grant | R12 | SonyBrc300 | grant.OnePushWhiteBalance | 01 04 35 03;01 04 10 05 | - | BRC-300 Technical Manual command/inquiry lists |
| r21-nearusbrc300-versioninquiry-grant | R21 | NearusBrc300 | grant.VersionInquiry | 09 00 02 | - | BRC-300/300P Command List |
| r21-nearusbrc300-sonyautoslowshutter-grant | R21 | NearusBrc300 | grant.SonyAutoSlowShutter | 01 04 5A 02;01 04 5A 03 | - | BRC-300/300P Command List |
| r21-nearusbrc300-exposuremode-grant | R21 | NearusBrc300 | grant.ExposureMode | 01 04 39;09 04 39 | - | BRC-300/300P Command List |
| r21-nearusbrc300-directzoom-grant | R21 | NearusBrc300 | grant.DirectZoom | 01 04 47;09 04 47 | - | BRC-300/300P Command List |
| r21-nearusbrc300-digitalzoomtoggle-grant | R21 | NearusBrc300 | grant.DigitalZoomToggle | 01 04 06;09 04 06 | - | BRC-300/300P Command List |
| r21-nearusbrc300-digitalzoomrange-grant | R21 | NearusBrc300 | grant.DigitalZoomRange | 01 04 47 digital positions | - | BRC-300/300P Command List |
| r21-nearusbrc300-onepushfocus-grant | R21 | NearusBrc300 | grant.OnePushFocus | 01 04 18 01 | - | BRC-300/300P Command List |
| r21-nearusbrc300-iriscontrol-grant | R21 | NearusBrc300 | grant.IrisControl | 01 04 0B;01 04 4B;09 04 4B | - | BRC-300/300P Command List |
| r21-nearusbrc300-brightnesscontrol-grant | R21 | NearusBrc300 | grant.BrightnessControl | 01 04 0D;01 04 4D;09 04 4D | - | BRC-300/300P Command List |
| r21-nearusbrc300-exposurecompensation-grant | R21 | NearusBrc300 | grant.ExposureCompensation | 01 04 3E;01 04 0E;01 04 4E;09 04 3E;09 04 4E | - | BRC-300/300P Command List |
| r21-nearusbrc300-backlightcompensation-grant | R21 | NearusBrc300 | grant.BacklightCompensation | 01 04 33;09 04 33 | - | BRC-300/300P Command List |
| r21-nearusbrc300-rgbgain-grant | R21 | NearusBrc300 | grant.RgbGain | 01 04 03;01 04 04;01 04 43;01 04 44;09 04 43;09 04 44 | - | BRC-300/300P Command List |
| r21-nearusbrc300-imageflip-grant | R21 | NearusBrc300 | grant.ImageFlip | 01 04 66;09 04 66 | - | BRC-300/300P Command List |
| r21-nearusbrc300-onepushwhitebalance-grant | R21 | NearusBrc300 | grant.OnePushWhiteBalance | 01 04 35 03;01 04 10 05 | - | BRC-300/300P Command List |
| r8-genericvisca-versioninquiry-grant | R8 | GenericVisca | grant.VersionInquiry | 09 00 02 | - | EVI-H100 Technical Manual command/inquiry lists |
| r12-genericvisca-versioninquiry-grant | R12 | GenericVisca | grant.VersionInquiry | 09 00 02 | - | BRC-300 Technical Manual command/inquiry lists |
| r21-genericvisca-versioninquiry-grant | R21 | GenericVisca | grant.VersionInquiry | 09 00 02 | - | BRC-300/300P Command List |
| r8-genericvisca-exposuremode-grant | R8 | GenericVisca | grant.ExposureMode | 01 04 39;09 04 39 | - | EVI-H100 Technical Manual command/inquiry lists |
| r12-genericvisca-exposuremode-grant | R12 | GenericVisca | grant.ExposureMode | 01 04 39;09 04 39 | - | BRC-300 Technical Manual command/inquiry lists |
| r21-genericvisca-exposuremode-grant | R21 | GenericVisca | grant.ExposureMode | 01 04 39;09 04 39 | - | BRC-300/300P Command List |
| r8-genericvisca-iriscontrol-grant | R8 | GenericVisca | grant.IrisControl | 01 04 0B;01 04 4B;09 04 4B | - | EVI-H100 Technical Manual command/inquiry lists |
| r12-genericvisca-iriscontrol-grant | R12 | GenericVisca | grant.IrisControl | 01 04 0B;01 04 4B;09 04 4B | - | BRC-300 Technical Manual command/inquiry lists |
| r21-genericvisca-iriscontrol-grant | R21 | GenericVisca | grant.IrisControl | 01 04 0B;01 04 4B;09 04 4B | - | BRC-300/300P Command List |
| r8-genericvisca-onepushfocus-grant | R8 | GenericVisca | grant.OnePushFocus | 01 04 18 01 | - | EVI-H100 Technical Manual command/inquiry lists |
| r12-genericvisca-onepushfocus-grant | R12 | GenericVisca | grant.OnePushFocus | 01 04 18 01 | - | BRC-300 Technical Manual command/inquiry lists |
| r21-genericvisca-onepushfocus-grant | R21 | GenericVisca | grant.OnePushFocus | 01 04 18 01 | - | BRC-300/300P Command List |
| r8-genericvisca-onepushwhitebalance-grant | R8 | GenericVisca | grant.OnePushWhiteBalance | 01 04 35 03;01 04 10 05 | - | EVI-H100 Technical Manual command/inquiry lists |
| r12-genericvisca-onepushwhitebalance-grant | R12 | GenericVisca | grant.OnePushWhiteBalance | 01 04 35 03;01 04 10 05 | - | BRC-300 Technical Manual command/inquiry lists |
| r21-genericvisca-onepushwhitebalance-grant | R21 | GenericVisca | grant.OnePushWhiteBalance | 01 04 35 03;01 04 10 05 | - | BRC-300/300P Command List |

| r1-pantilthome-command | R1 | PtzOpticsG2, PtzOptics30X | command.PanTiltHome | 01 06 04 | - | §7 command list |
| r14-pantilthome-command | R14 | PtzOpticsG3 | command.PanTiltHome | 01 06 04 | - | Current Developer Portal command pages |
| r7-pantilthome-command | R7 | SonyFr7 | command.PanTiltHome | 01 06 04 | - | FR7 v4 command list pp.14–20 |
| r11-pantilthome-command | R11 | SonyBrcH900 | command.PanTiltHome | 01 06 04 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-pantilthome-command | R8 | SonyEviH100, GenericVisca | command.PanTiltHome | 01 06 04 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-pantilthome-command | R12 | SonyBrc300, GenericVisca | command.PanTiltHome | 01 06 04 | - | BRC-300 Command List pp.9–12 |
| r21-pantilthome-command | R21 | NearusBrc300, GenericVisca | command.PanTiltHome | 01 06 04 | - | BRC-300/300P Command List pp.9–12 |
| r1-pantiltreset-command | R1 | PtzOpticsG2, PtzOptics30X | command.PanTiltReset | 01 06 05 | - | §7 command list |
| r14-pantiltreset-command | R14 | PtzOpticsG3 | command.PanTiltReset | 01 06 05 | - | Current Developer Portal command pages |
| r7-pantiltreset-command | R7 | SonyFr7 | command.PanTiltReset | 01 06 05 | - | FR7 v4 command list pp.14–20 |
| r11-pantiltreset-command | R11 | SonyBrcH900 | command.PanTiltReset | 01 06 05 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-pantiltreset-command | R8 | SonyEviH100, GenericVisca | command.PanTiltReset | 01 06 05 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-pantiltreset-command | R12 | SonyBrc300, GenericVisca | command.PanTiltReset | 01 06 05 | - | BRC-300 Command List pp.9–12 |
| r21-pantiltreset-command | R21 | NearusBrc300, GenericVisca | command.PanTiltReset | 01 06 05 | - | BRC-300/300P Command List pp.9–12 |
| r1-pantiltdrive-command | R1 | PtzOpticsG2, PtzOptics30X | command.PanTiltDrive | 01 06 01 | - | §7 command list |
| r14-pantiltdrive-command | R14 | PtzOpticsG3 | command.PanTiltDrive | 01 06 01 | - | Current Developer Portal command pages |
| r7-pantiltdrive-command | R7 | SonyFr7 | command.PanTiltDrive | 01 06 01 | - | FR7 v4 command list pp.14–20 |
| r11-pantiltdrive-command | R11 | SonyBrcH900 | command.PanTiltDrive | 01 06 01 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-pantiltdrive-command | R8 | SonyEviH100, GenericVisca | command.PanTiltDrive | 01 06 01 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-pantiltdrive-command | R12 | SonyBrc300, GenericVisca | command.PanTiltDrive | 01 06 01 | - | BRC-300 Command List pp.9–12 |
| r21-pantiltdrive-command | R21 | NearusBrc300, GenericVisca | command.PanTiltDrive | 01 06 01 | - | BRC-300/300P Command List pp.9–12 |
| r1-pantiltstop-command | R1 | PtzOpticsG2, PtzOptics30X | command.PanTiltStop | 01 06 01 | - | §7 command list |
| r14-pantiltstop-command | R14 | PtzOpticsG3 | command.PanTiltStop | 01 06 01 | - | Current Developer Portal command pages |
| r7-pantiltstop-command | R7 | SonyFr7 | command.PanTiltStop | 01 06 01 | - | FR7 v4 command list pp.14–20 |
| r11-pantiltstop-command | R11 | SonyBrcH900 | command.PanTiltStop | 01 06 01 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-pantiltstop-command | R8 | SonyEviH100, GenericVisca | command.PanTiltStop | 01 06 01 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-pantiltstop-command | R12 | SonyBrc300, GenericVisca | command.PanTiltStop | 01 06 01 | - | BRC-300 Command List pp.9–12 |
| r21-pantiltstop-command | R21 | NearusBrc300, GenericVisca | command.PanTiltStop | 01 06 01 | - | BRC-300/300P Command List pp.9–12 |
| r1-pantiltabsolute-command | R1 | PtzOpticsG2, PtzOptics30X | command.PanTiltAbsolute | 01 06 02 | - | §7 command list |
| r14-pantiltabsolute-command | R14 | PtzOpticsG3 | command.PanTiltAbsolute | 01 06 02 | - | Current Developer Portal command pages |
| r7-pantiltabsolute-command | R7 | SonyFr7 | command.PanTiltAbsolute | 01 06 02 | - | FR7 v4 command list pp.14–20 |
| r11-pantiltabsolute-command | R11 | SonyBrcH900 | command.PanTiltAbsolute | 01 06 02 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-pantiltabsolute-command | R8 | SonyEviH100, GenericVisca | command.PanTiltAbsolute | 01 06 02 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-pantiltabsolute-command | R12 | SonyBrc300, GenericVisca | command.PanTiltAbsolute | 01 06 02 | - | BRC-300 Command List pp.9–12 |
| r21-pantiltabsolute-command | R21 | NearusBrc300, GenericVisca | command.PanTiltAbsolute | 01 06 02 | - | BRC-300/300P Command List pp.9–12 |
| r1-pantiltrelative-command | R1 | PtzOpticsG2, PtzOptics30X | command.PanTiltRelative | 01 06 03 | - | §7 command list |
| r14-pantiltrelative-command | R14 | PtzOpticsG3 | command.PanTiltRelative | 01 06 03 | - | Current Developer Portal command pages |
| r7-pantiltrelative-command | R7 | SonyFr7 | command.PanTiltRelative | 01 06 03 | - | FR7 v4 command list pp.14–20 |
| r11-pantiltrelative-command | R11 | SonyBrcH900 | command.PanTiltRelative | 01 06 03 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-pantiltrelative-command | R8 | SonyEviH100, GenericVisca | command.PanTiltRelative | 01 06 03 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-pantiltrelative-command | R12 | SonyBrc300, GenericVisca | command.PanTiltRelative | 01 06 03 | - | BRC-300 Command List pp.9–12 |
| r21-pantiltrelative-command | R21 | NearusBrc300, GenericVisca | command.PanTiltRelative | 01 06 03 | - | BRC-300/300P Command List pp.9–12 |
| r1-pantiltlimitset-command | R1 | PtzOpticsG2, PtzOptics30X | command.PanTiltLimitSet | 01 06 07 00 | - | §7 command list |
| r14-pantiltlimitset-command | R14 | PtzOpticsG3 | command.PanTiltLimitSet | 01 06 07 00 | - | Current Developer Portal command pages |
| r7-pantiltlimitset-command | R7 | SonyFr7 | command.PanTiltLimitSet | 01 06 07 00 | - | FR7 v4 command list pp.14–20 |
| r11-pantiltlimitset-command | R11 | SonyBrcH900 | command.PanTiltLimitSet | 01 06 07 00 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-pantiltlimitset-command | R8 | SonyEviH100, GenericVisca | command.PanTiltLimitSet | 01 06 07 00 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-pantiltlimitset-command | R12 | SonyBrc300, GenericVisca | command.PanTiltLimitSet | 01 06 07 00 | - | BRC-300 Command List pp.9–12 |
| r21-pantiltlimitset-command | R21 | NearusBrc300, GenericVisca | command.PanTiltLimitSet | 01 06 07 00 | - | BRC-300/300P Command List pp.9–12 |
| r1-pantiltlimitclear-command | R1 | PtzOpticsG2, PtzOptics30X | command.PanTiltLimitClear | 01 06 07 01 | - | §7 command list |
| r14-pantiltlimitclear-command | R14 | PtzOpticsG3 | command.PanTiltLimitClear | 01 06 07 01 | - | Current Developer Portal command pages |
| r7-pantiltlimitclear-command | R7 | SonyFr7 | command.PanTiltLimitClear | 01 06 07 01 | - | FR7 v4 command list pp.14–20 |
| r11-pantiltlimitclear-command | R11 | SonyBrcH900 | command.PanTiltLimitClear | 01 06 07 01 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-pantiltlimitclear-command | R8 | SonyEviH100, GenericVisca | command.PanTiltLimitClear | 01 06 07 01 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-pantiltlimitclear-command | R12 | SonyBrc300, GenericVisca | command.PanTiltLimitClear | 01 06 07 01 | - | BRC-300 Command List pp.9–12 |
| r21-pantiltlimitclear-command | R21 | NearusBrc300, GenericVisca | command.PanTiltLimitClear | 01 06 07 01 | - | BRC-300/300P Command List pp.9–12 |
| r1-zoomstop-command | R1 | PtzOpticsG2, PtzOptics30X | command.ZoomStop | 01 04 07 00 | - | §7 command list |
| r14-zoomstop-command | R14 | PtzOpticsG3 | command.ZoomStop | 01 04 07 00 | - | Current Developer Portal command pages |
| r7-zoomstop-command | R7 | SonyFr7 | command.ZoomStop | 01 04 07 00 | - | FR7 v4 command list pp.14–20 |
| r11-zoomstop-command | R11 | SonyBrcH900 | command.ZoomStop | 01 04 07 00 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-zoomstop-command | R8 | SonyEviH100, GenericVisca | command.ZoomStop | 01 04 07 00 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-zoomstop-command | R12 | SonyBrc300, GenericVisca | command.ZoomStop | 01 04 07 00 | - | BRC-300 Command List pp.9–12 |
| r21-zoomstop-command | R21 | NearusBrc300, GenericVisca | command.ZoomStop | 01 04 07 00 | - | BRC-300/300P Command List pp.9–12 |
| r1-zoomtele-command | R1 | PtzOpticsG2, PtzOptics30X | command.ZoomTele | 01 04 07 02 | - | §7 command list |
| r14-zoomtele-command | R14 | PtzOpticsG3 | command.ZoomTele | 01 04 07 02 | - | Current Developer Portal command pages |
| r7-zoomtele-command | R7 | SonyFr7 | command.ZoomTele | 01 04 07 02 | - | FR7 v4 command list pp.14–20 |
| r11-zoomtele-command | R11 | SonyBrcH900 | command.ZoomTele | 01 04 07 02 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-zoomtele-command | R8 | SonyEviH100, GenericVisca | command.ZoomTele | 01 04 07 02 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-zoomtele-command | R12 | SonyBrc300, GenericVisca | command.ZoomTele | 01 04 07 02 | - | BRC-300 Command List pp.9–12 |
| r21-zoomtele-command | R21 | NearusBrc300, GenericVisca | command.ZoomTele | 01 04 07 02 | - | BRC-300/300P Command List pp.9–12 |
| r1-zoomwide-command | R1 | PtzOpticsG2, PtzOptics30X | command.ZoomWide | 01 04 07 03 | - | §7 command list |
| r14-zoomwide-command | R14 | PtzOpticsG3 | command.ZoomWide | 01 04 07 03 | - | Current Developer Portal command pages |
| r7-zoomwide-command | R7 | SonyFr7 | command.ZoomWide | 01 04 07 03 | - | FR7 v4 command list pp.14–20 |
| r11-zoomwide-command | R11 | SonyBrcH900 | command.ZoomWide | 01 04 07 03 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-zoomwide-command | R8 | SonyEviH100, GenericVisca | command.ZoomWide | 01 04 07 03 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-zoomwide-command | R12 | SonyBrc300, GenericVisca | command.ZoomWide | 01 04 07 03 | - | BRC-300 Command List pp.9–12 |
| r21-zoomwide-command | R21 | NearusBrc300, GenericVisca | command.ZoomWide | 01 04 07 03 | - | BRC-300/300P Command List pp.9–12 |
| r1-zoomtelevariable-command | R1 | PtzOpticsG2, PtzOptics30X | command.ZoomTeleVariable | 01 04 07 | - | §7 command list |
| r14-zoomtelevariable-command | R14 | PtzOpticsG3 | command.ZoomTeleVariable | 01 04 07 | - | Current Developer Portal command pages |
| r7-zoomtelevariable-command | R7 | SonyFr7 | command.ZoomTeleVariable | 01 04 07 | - | FR7 v4 command list pp.14–20 |
| r11-zoomtelevariable-command | R11 | SonyBrcH900 | command.ZoomTeleVariable | 01 04 07 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-zoomtelevariable-command | R8 | SonyEviH100, GenericVisca | command.ZoomTeleVariable | 01 04 07 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-zoomtelevariable-command | R12 | SonyBrc300, GenericVisca | command.ZoomTeleVariable | 01 04 07 | - | BRC-300 Command List pp.9–12 |
| r21-zoomtelevariable-command | R21 | NearusBrc300, GenericVisca | command.ZoomTeleVariable | 01 04 07 | - | BRC-300/300P Command List pp.9–12 |
| r1-zoomwidevariable-command | R1 | PtzOpticsG2, PtzOptics30X | command.ZoomWideVariable | 01 04 07 | - | §7 command list |
| r14-zoomwidevariable-command | R14 | PtzOpticsG3 | command.ZoomWideVariable | 01 04 07 | - | Current Developer Portal command pages |
| r7-zoomwidevariable-command | R7 | SonyFr7 | command.ZoomWideVariable | 01 04 07 | - | FR7 v4 command list pp.14–20 |
| r11-zoomwidevariable-command | R11 | SonyBrcH900 | command.ZoomWideVariable | 01 04 07 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-zoomwidevariable-command | R8 | SonyEviH100, GenericVisca | command.ZoomWideVariable | 01 04 07 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-zoomwidevariable-command | R12 | SonyBrc300, GenericVisca | command.ZoomWideVariable | 01 04 07 | - | BRC-300 Command List pp.9–12 |
| r21-zoomwidevariable-command | R21 | NearusBrc300, GenericVisca | command.ZoomWideVariable | 01 04 07 | - | BRC-300/300P Command List pp.9–12 |
| r1-zoomposition-command | R1 | PtzOpticsG2, PtzOptics30X | command.ZoomPosition | 01 04 47 | - | §7 command list |
| r14-zoomposition-command | R14 | PtzOpticsG3 | command.ZoomPosition | 01 04 47 | - | Current Developer Portal command pages |
| r7-zoomposition-command | R7 | SonyFr7 | command.ZoomPosition | 01 04 47 | - | FR7 v4 command list pp.14–20 |
| r11-zoomposition-command | R11 | SonyBrcH900 | command.ZoomPosition | 01 04 47 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-zoomposition-command | R8 | SonyEviH100, GenericVisca | command.ZoomPosition | 01 04 47 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-zoomposition-command | R12 | SonyBrc300, GenericVisca | command.ZoomPosition | 01 04 47 | - | BRC-300 Command List pp.9–12 |
| r21-zoomposition-command | R21 | NearusBrc300, GenericVisca | command.ZoomPosition | 01 04 47 | - | BRC-300/300P Command List pp.9–12 |
| r8-digitalzoom-command | R8 | SonyEviH100, GenericVisca | command.DigitalZoom | 01 04 06 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-digitalzoom-command | R12 | SonyBrc300, GenericVisca | command.DigitalZoom | 01 04 06 | - | BRC-300 Command List pp.9–12 |
| r21-digitalzoom-command | R21 | NearusBrc300, GenericVisca | command.DigitalZoom | 01 04 06 | - | BRC-300/300P Command List pp.9–12 |
| r1-focusstop-command | R1 | PtzOpticsG2, PtzOptics30X | command.FocusStop | 01 04 08 00 | - | §7 command list |
| r14-focusstop-command | R14 | PtzOpticsG3 | command.FocusStop | 01 04 08 00 | - | Current Developer Portal command pages |
| r7-focusstop-command | R7 | SonyFr7 | command.FocusStop | 01 04 08 00 | - | FR7 v4 command list pp.14–20 |
| r11-focusstop-command | R11 | SonyBrcH900 | command.FocusStop | 01 04 08 00 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-focusstop-command | R8 | SonyEviH100, GenericVisca | command.FocusStop | 01 04 08 00 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-focusstop-command | R12 | SonyBrc300, GenericVisca | command.FocusStop | 01 04 08 00 | - | BRC-300 Command List pp.9–12 |
| r21-focusstop-command | R21 | NearusBrc300, GenericVisca | command.FocusStop | 01 04 08 00 | - | BRC-300/300P Command List pp.9–12 |
| r1-focusfar-command | R1 | PtzOpticsG2, PtzOptics30X | command.FocusFar | 01 04 08 02 | - | §7 command list |
| r14-focusfar-command | R14 | PtzOpticsG3 | command.FocusFar | 01 04 08 02 | - | Current Developer Portal command pages |
| r7-focusfar-command | R7 | SonyFr7 | command.FocusFar | 01 04 08 02 | - | FR7 v4 command list pp.14–20 |
| r11-focusfar-command | R11 | SonyBrcH900 | command.FocusFar | 01 04 08 02 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-focusfar-command | R8 | SonyEviH100, GenericVisca | command.FocusFar | 01 04 08 02 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-focusfar-command | R12 | SonyBrc300, GenericVisca | command.FocusFar | 01 04 08 02 | - | BRC-300 Command List pp.9–12 |
| r21-focusfar-command | R21 | NearusBrc300, GenericVisca | command.FocusFar | 01 04 08 02 | - | BRC-300/300P Command List pp.9–12 |
| r1-focusnear-command | R1 | PtzOpticsG2, PtzOptics30X | command.FocusNear | 01 04 08 03 | - | §7 command list |
| r14-focusnear-command | R14 | PtzOpticsG3 | command.FocusNear | 01 04 08 03 | - | Current Developer Portal command pages |
| r7-focusnear-command | R7 | SonyFr7 | command.FocusNear | 01 04 08 03 | - | FR7 v4 command list pp.14–20 |
| r11-focusnear-command | R11 | SonyBrcH900 | command.FocusNear | 01 04 08 03 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-focusnear-command | R8 | SonyEviH100, GenericVisca | command.FocusNear | 01 04 08 03 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-focusnear-command | R12 | SonyBrc300, GenericVisca | command.FocusNear | 01 04 08 03 | - | BRC-300 Command List pp.9–12 |
| r21-focusnear-command | R21 | NearusBrc300, GenericVisca | command.FocusNear | 01 04 08 03 | - | BRC-300/300P Command List pp.9–12 |
| r1-focusfarvariable-command | R1 | PtzOpticsG2, PtzOptics30X | command.FocusFarVariable | 01 04 08 | - | §7 command list |
| r14-focusfarvariable-command | R14 | PtzOpticsG3 | command.FocusFarVariable | 01 04 08 | - | Current Developer Portal command pages |
| r7-focusfarvariable-command | R7 | SonyFr7 | command.FocusFarVariable | 01 04 08 | - | FR7 v4 command list pp.14–20 |
| r11-focusfarvariable-command | R11 | SonyBrcH900 | command.FocusFarVariable | 01 04 08 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-focusfarvariable-command | R8 | SonyEviH100, GenericVisca | command.FocusFarVariable | 01 04 08 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-focusfarvariable-command | R12 | SonyBrc300, GenericVisca | command.FocusFarVariable | 01 04 08 | - | BRC-300 Command List pp.9–12 |
| r21-focusfarvariable-command | R21 | NearusBrc300, GenericVisca | command.FocusFarVariable | 01 04 08 | - | BRC-300/300P Command List pp.9–12 |
| r1-focusnearvariable-command | R1 | PtzOpticsG2, PtzOptics30X | command.FocusNearVariable | 01 04 08 | - | §7 command list |
| r14-focusnearvariable-command | R14 | PtzOpticsG3 | command.FocusNearVariable | 01 04 08 | - | Current Developer Portal command pages |
| r7-focusnearvariable-command | R7 | SonyFr7 | command.FocusNearVariable | 01 04 08 | - | FR7 v4 command list pp.14–20 |
| r11-focusnearvariable-command | R11 | SonyBrcH900 | command.FocusNearVariable | 01 04 08 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-focusnearvariable-command | R8 | SonyEviH100, GenericVisca | command.FocusNearVariable | 01 04 08 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-focusnearvariable-command | R12 | SonyBrc300, GenericVisca | command.FocusNearVariable | 01 04 08 | - | BRC-300 Command List pp.9–12 |
| r21-focusnearvariable-command | R21 | NearusBrc300, GenericVisca | command.FocusNearVariable | 01 04 08 | - | BRC-300/300P Command List pp.9–12 |
| r1-focusposition-command | R1 | PtzOpticsG2, PtzOptics30X | command.FocusPosition | 01 04 48 | - | §7 command list |
| r14-focusposition-command | R14 | PtzOpticsG3 | command.FocusPosition | 01 04 48 | - | Current Developer Portal command pages |
| r7-focusposition-command | R7 | SonyFr7 | command.FocusPosition | 01 04 48 | - | FR7 v4 command list pp.14–20 |
| r11-focusposition-command | R11 | SonyBrcH900 | command.FocusPosition | 01 04 48 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-focusposition-command | R8 | SonyEviH100, GenericVisca | command.FocusPosition | 01 04 48 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-focusposition-command | R12 | SonyBrc300, GenericVisca | command.FocusPosition | 01 04 48 | - | BRC-300 Command List pp.9–12 |
| r21-focusposition-command | R21 | NearusBrc300, GenericVisca | command.FocusPosition | 01 04 48 | - | BRC-300/300P Command List pp.9–12 |
| r1-focusauto-command | R1 | PtzOpticsG2, PtzOptics30X | command.FocusAuto | 01 04 38 02 | - | §7 command list |
| r14-focusauto-command | R14 | PtzOpticsG3 | command.FocusAuto | 01 04 38 02 | - | Current Developer Portal command pages |
| r7-focusauto-command | R7 | SonyFr7 | command.FocusAuto | 01 04 38 02 | - | FR7 v4 command list pp.14–20 |
| r11-focusauto-command | R11 | SonyBrcH900 | command.FocusAuto | 01 04 38 02 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-focusauto-command | R8 | SonyEviH100, GenericVisca | command.FocusAuto | 01 04 38 02 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-focusauto-command | R12 | SonyBrc300, GenericVisca | command.FocusAuto | 01 04 38 02 | - | BRC-300 Command List pp.9–12 |
| r21-focusauto-command | R21 | NearusBrc300, GenericVisca | command.FocusAuto | 01 04 38 02 | - | BRC-300/300P Command List pp.9–12 |
| r1-focusmanual-command | R1 | PtzOpticsG2, PtzOptics30X | command.FocusManual | 01 04 38 03 | - | §7 command list |
| r14-focusmanual-command | R14 | PtzOpticsG3 | command.FocusManual | 01 04 38 03 | - | Current Developer Portal command pages |
| r7-focusmanual-command | R7 | SonyFr7 | command.FocusManual | 01 04 38 03 | - | FR7 v4 command list pp.14–20 |
| r11-focusmanual-command | R11 | SonyBrcH900 | command.FocusManual | 01 04 38 03 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-focusmanual-command | R8 | SonyEviH100, GenericVisca | command.FocusManual | 01 04 38 03 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-focusmanual-command | R12 | SonyBrc300, GenericVisca | command.FocusManual | 01 04 38 03 | - | BRC-300 Command List pp.9–12 |
| r21-focusmanual-command | R21 | NearusBrc300, GenericVisca | command.FocusManual | 01 04 38 03 | - | BRC-300/300P Command List pp.9–12 |
| r8-focusonepush-command | R8 | SonyEviH100, GenericVisca | command.FocusOnePush | 01 04 18 01 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-focusonepush-command | R12 | SonyBrc300, GenericVisca | command.FocusOnePush | 01 04 18 01 | - | BRC-300 Command List pp.9–12 |
| r21-focusonepush-command | R21 | NearusBrc300, GenericVisca | command.FocusOnePush | 01 04 18 01 | - | BRC-300/300P Command List pp.9–12 |
| r11-focusonepush-command | R11 | SonyBrcH900 | command.FocusOnePush | 01 04 18 01 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-focusinfinity-command | R8 | SonyEviH100, GenericVisca | command.FocusInfinity | 01 04 18 02 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-focusinfinity-command | R12 | SonyBrc300, GenericVisca | command.FocusInfinity | 01 04 18 02 | - | BRC-300 Command List pp.9–12 |
| r21-focusinfinity-command | R21 | NearusBrc300, GenericVisca | command.FocusInfinity | 01 04 18 02 | - | BRC-300/300P Command List pp.9–12 |
| r11-focusinfinity-command | R11 | SonyBrcH900 | command.FocusInfinity | 01 04 18 02 | - | BRC-H900 v2.20 command list pp.14–18 |
| r1-focustoggle-command | R1 | PtzOpticsG2, PtzOptics30X | command.FocusToggle | 01 04 38 10 | - | §7 command list |
| r14-focustoggle-command | R14 | PtzOpticsG3 | command.FocusToggle | 01 04 38 10 | - | Current Developer Portal command pages |
| r7-focustoggle-command | R7 | SonyFr7 | command.FocusToggle | 01 04 38 10 | - | FR7 v4 command list pp.14–20 |
| r11-focustoggle-command | R11 | SonyBrcH900 | command.FocusToggle | 01 04 38 10 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-focustoggle-command | R8 | SonyEviH100, GenericVisca | command.FocusToggle | 01 04 38 10 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-focustoggle-command | R12 | SonyBrc300, GenericVisca | command.FocusToggle | 01 04 38 10 | - | BRC-300 Command List pp.9–12 |
| r21-focustoggle-command | R21 | NearusBrc300, GenericVisca | command.FocusToggle | 01 04 38 10 | - | BRC-300/300P Command List pp.9–12 |
| r20-focuszone-command | R20 | PtzOpticsG2, PtzOpticsG3, PtzOptics30X | command.FocusZone | 01 04 AA | - | Focus selection rows |
| r1-focusautosensitivity-command | R1 | PtzOpticsG2, PtzOptics30X | command.FocusAutoSensitivity | 01 04 58 | - | §7 command list |
| r14-focusautosensitivity-command | R14 | PtzOpticsG3 | command.FocusAutoSensitivity | 01 04 58 | - | Current Developer Portal command pages |
| r8-focusautosensitivity-command | R8 | SonyEviH100 | command.FocusAutoSensitivity | 01 04 58 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r8-focusnearlimit-command | R8 | SonyEviH100 | command.FocusNearLimit | 01 04 28 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r1-focuslock-command | R1 | PtzOpticsG2, PtzOptics30X | command.FocusLock | 0A 04 68 | - | §7 command list |
| r14-focuslock-command | R14 | PtzOpticsG3 | command.FocusLock | 0A 04 68 | - | Current Developer Portal command pages |
| r7-pushafpress-command | R7 | SonyFr7 | command.PushAfPress | 01 7E 04 58 | - | FR7 v4 command list pp.14–20 |
| r7-pushafrelease-command | R7 | SonyFr7 | command.PushAfRelease | 01 7E 04 58 | - | FR7 v4 command list pp.14–20 |
| r1-presetrecall-command | R1 | PtzOpticsG2, PtzOptics30X | command.PresetRecall | 01 04 3F 02 | - | §7 command list |
| r14-presetrecall-command | R14 | PtzOpticsG3 | command.PresetRecall | 01 04 3F 02 | - | Current Developer Portal command pages |
| r7-presetrecall-command | R7 | SonyFr7 | command.PresetRecall | 01 04 3F 02 | - | FR7 v4 command list pp.14–20 |
| r11-presetrecall-command | R11 | SonyBrcH900 | command.PresetRecall | 01 04 3F 02 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-presetrecall-command | R8 | SonyEviH100, GenericVisca | command.PresetRecall | 01 04 3F 02 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-presetrecall-command | R12 | SonyBrc300, GenericVisca | command.PresetRecall | 01 04 3F 02 | - | BRC-300 Command List pp.9–12 |
| r21-presetrecall-command | R21 | NearusBrc300, GenericVisca | command.PresetRecall | 01 04 3F 02 | - | BRC-300/300P Command List pp.9–12 |
| r1-presetrecallspeed-command | R1 | PtzOpticsG2, PtzOptics30X | command.PresetRecallSpeed | 01 06 01 | - | §7 command list |
| r10-presetrecallspeed-command | R10 | PtzOpticsG3 | command.PresetRecallSpeed | 01 06 01 | - | Move 4K G3 command list |
| r1-presetset-command | R1 | PtzOpticsG2, PtzOptics30X | command.PresetSet | 01 04 3F 01 | - | §7 command list |
| r14-presetset-command | R14 | PtzOpticsG3 | command.PresetSet | 01 04 3F 01 | - | Current Developer Portal command pages |
| r7-presetset-command | R7 | SonyFr7 | command.PresetSet | 01 04 3F 01 | - | FR7 v4 command list pp.14–20 |
| r8-presetset-command | R8 | SonyEviH100, GenericVisca | command.PresetSet | 01 04 3F 01 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-presetset-command | R12 | SonyBrc300, GenericVisca | command.PresetSet | 01 04 3F 01 | - | BRC-300 Command List pp.9–12 |
| r21-presetset-command | R21 | NearusBrc300, GenericVisca | command.PresetSet | 01 04 3F 01 | - | BRC-300/300P Command List pp.9–12 |
| r1-presetreset-command | R1 | PtzOpticsG2, PtzOptics30X | command.PresetReset | 01 04 3F 00 | - | §7 command list |
| r14-presetreset-command | R14 | PtzOpticsG3 | command.PresetReset | 01 04 3F 00 | - | Current Developer Portal command pages |
| r7-presetreset-command | R7 | SonyFr7 | command.PresetReset | 01 04 3F 00 | - | FR7 v4 command list pp.14–20 |
| r8-presetreset-command | R8 | SonyEviH100, GenericVisca | command.PresetReset | 01 04 3F 00 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-presetreset-command | R12 | SonyBrc300, GenericVisca | command.PresetReset | 01 04 3F 00 | - | BRC-300 Command List pp.9–12 |
| r21-presetreset-command | R21 | NearusBrc300, GenericVisca | command.PresetReset | 01 04 3F 00 | - | BRC-300/300P Command List pp.9–12 |
| r1-poweron-command | R1 | PtzOpticsG2, PtzOptics30X | command.PowerOn | 01 04 00 02 | - | §7 command list |
| r14-poweron-command | R14 | PtzOpticsG3 | command.PowerOn | 01 04 00 02 | - | Current Developer Portal command pages |
| r7-poweron-command | R7 | SonyFr7 | command.PowerOn | 01 04 00 02 | - | FR7 v4 command list pp.14–20 |
| r11-poweron-command | R11 | SonyBrcH900 | command.PowerOn | 01 04 00 02 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-poweron-command | R8 | SonyEviH100, GenericVisca | command.PowerOn | 01 04 00 02 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-poweron-command | R12 | SonyBrc300, GenericVisca | command.PowerOn | 01 04 00 02 | - | BRC-300 Command List pp.9–12 |
| r21-poweron-command | R21 | NearusBrc300, GenericVisca | command.PowerOn | 01 04 00 02 | - | BRC-300/300P Command List pp.9–12 |
| r1-powerstandby-command | R1 | PtzOpticsG2, PtzOptics30X | command.PowerStandby | 01 04 00 03 | - | §7 command list |
| r14-powerstandby-command | R14 | PtzOpticsG3 | command.PowerStandby | 01 04 00 03 | - | Current Developer Portal command pages |
| r7-powerstandby-command | R7 | SonyFr7 | command.PowerStandby | 01 04 00 03 | - | FR7 v4 command list pp.14–20 |
| r11-powerstandby-command | R11 | SonyBrcH900 | command.PowerStandby | 01 04 00 03 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-powerstandby-command | R8 | SonyEviH100, GenericVisca | command.PowerStandby | 01 04 00 03 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-powerstandby-command | R12 | SonyBrc300, GenericVisca | command.PowerStandby | 01 04 00 03 | - | BRC-300 Command List pp.9–12 |
| r21-powerstandby-command | R21 | NearusBrc300, GenericVisca | command.PowerStandby | 01 04 00 03 | - | BRC-300/300P Command List pp.9–12 |
| r1-exposuremode-command | R1 | PtzOpticsG2, PtzOptics30X | command.ExposureMode | 01 04 39 | - | §7 command list |
| r14-exposuremode-command | R14 | PtzOpticsG3 | command.ExposureMode | 01 04 39 | - | Current Developer Portal command pages |
| r11-exposuremode-command | R11 | SonyBrcH900 | command.ExposureMode | 01 04 39 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-exposuremode-command | R8 | SonyEviH100, GenericVisca | command.ExposureMode | 01 04 39 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-exposuremode-command | R12 | SonyBrc300, GenericVisca | command.ExposureMode | 01 04 39 | - | BRC-300 Command List pp.9–12 |
| r21-exposuremode-command | R21 | NearusBrc300, GenericVisca | command.ExposureMode | 01 04 39 | - | BRC-300/300P Command List pp.9–12 |
| r1-exposurecompensationon-command | R1 | PtzOpticsG2, PtzOptics30X | command.ExposureCompensationOn | 01 04 3E 02 | - | §7 command list |
| r14-exposurecompensationon-command | R14 | PtzOpticsG3 | command.ExposureCompensationOn | 01 04 3E 02 | - | Current Developer Portal command pages |
| r8-exposurecompensationon-command | R8 | SonyEviH100, GenericVisca | command.ExposureCompensationOn | 01 04 3E 02 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-exposurecompensationon-command | R12 | SonyBrc300, GenericVisca | command.ExposureCompensationOn | 01 04 3E 02 | - | BRC-300 Command List pp.9–12 |
| r21-exposurecompensationon-command | R21 | NearusBrc300, GenericVisca | command.ExposureCompensationOn | 01 04 3E 02 | - | BRC-300/300P Command List pp.9–12 |
| r1-exposurecompensationoff-command | R1 | PtzOpticsG2, PtzOptics30X | command.ExposureCompensationOff | 01 04 3E 03 | - | §7 command list |
| r14-exposurecompensationoff-command | R14 | PtzOpticsG3 | command.ExposureCompensationOff | 01 04 3E 03 | - | Current Developer Portal command pages |
| r8-exposurecompensationoff-command | R8 | SonyEviH100, GenericVisca | command.ExposureCompensationOff | 01 04 3E 03 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-exposurecompensationoff-command | R12 | SonyBrc300, GenericVisca | command.ExposureCompensationOff | 01 04 3E 03 | - | BRC-300 Command List pp.9–12 |
| r21-exposurecompensationoff-command | R21 | NearusBrc300, GenericVisca | command.ExposureCompensationOff | 01 04 3E 03 | - | BRC-300/300P Command List pp.9–12 |
| r1-exposurecompensationreset-command | R1 | PtzOpticsG2, PtzOptics30X | command.ExposureCompensationReset | 01 04 0E 00 | - | §7 command list |
| r14-exposurecompensationreset-command | R14 | PtzOpticsG3 | command.ExposureCompensationReset | 01 04 0E 00 | - | Current Developer Portal command pages |
| r8-exposurecompensationreset-command | R8 | SonyEviH100, GenericVisca | command.ExposureCompensationReset | 01 04 0E 00 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-exposurecompensationreset-command | R12 | SonyBrc300, GenericVisca | command.ExposureCompensationReset | 01 04 0E 00 | - | BRC-300 Command List pp.9–12 |
| r21-exposurecompensationreset-command | R21 | NearusBrc300, GenericVisca | command.ExposureCompensationReset | 01 04 0E 00 | - | BRC-300/300P Command List pp.9–12 |
| r1-exposurecompensationup-command | R1 | PtzOpticsG2, PtzOptics30X | command.ExposureCompensationUp | 01 04 0E 02 | - | §7 command list |
| r14-exposurecompensationup-command | R14 | PtzOpticsG3 | command.ExposureCompensationUp | 01 04 0E 02 | - | Current Developer Portal command pages |
| r8-exposurecompensationup-command | R8 | SonyEviH100, GenericVisca | command.ExposureCompensationUp | 01 04 0E 02 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-exposurecompensationup-command | R12 | SonyBrc300, GenericVisca | command.ExposureCompensationUp | 01 04 0E 02 | - | BRC-300 Command List pp.9–12 |
| r21-exposurecompensationup-command | R21 | NearusBrc300, GenericVisca | command.ExposureCompensationUp | 01 04 0E 02 | - | BRC-300/300P Command List pp.9–12 |
| r1-exposurecompensationdown-command | R1 | PtzOpticsG2, PtzOptics30X | command.ExposureCompensationDown | 01 04 0E 03 | - | §7 command list |
| r14-exposurecompensationdown-command | R14 | PtzOpticsG3 | command.ExposureCompensationDown | 01 04 0E 03 | - | Current Developer Portal command pages |
| r8-exposurecompensationdown-command | R8 | SonyEviH100, GenericVisca | command.ExposureCompensationDown | 01 04 0E 03 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-exposurecompensationdown-command | R12 | SonyBrc300, GenericVisca | command.ExposureCompensationDown | 01 04 0E 03 | - | BRC-300 Command List pp.9–12 |
| r21-exposurecompensationdown-command | R21 | NearusBrc300, GenericVisca | command.ExposureCompensationDown | 01 04 0E 03 | - | BRC-300/300P Command List pp.9–12 |
| r1-exposurecompensationdirect-command | R1 | PtzOpticsG2, PtzOptics30X | command.ExposureCompensationDirect | 01 04 4E | - | §7 command list |
| r14-exposurecompensationdirect-command | R14 | PtzOpticsG3 | command.ExposureCompensationDirect | 01 04 4E | - | Current Developer Portal command pages |
| r8-exposurecompensationdirect-command | R8 | SonyEviH100, GenericVisca | command.ExposureCompensationDirect | 01 04 4E | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-exposurecompensationdirect-command | R12 | SonyBrc300, GenericVisca | command.ExposureCompensationDirect | 01 04 4E | - | BRC-300 Command List pp.9–12 |
| r21-exposurecompensationdirect-command | R21 | NearusBrc300, GenericVisca | command.ExposureCompensationDirect | 01 04 4E | - | BRC-300/300P Command List pp.9–12 |
| r1-dynamicrange-command | R1 | PtzOpticsG2, PtzOptics30X | command.DynamicRange | 01 04 25 | - | §7 command list |
| r14-dynamicrange-command | R14 | PtzOpticsG3 | command.DynamicRange | 01 04 25 | - | Current Developer Portal command pages |
| r1-irisreset-command | R1 | PtzOpticsG2, PtzOptics30X | command.IrisReset | 01 04 0B 00 | - | §7 command list |
| r14-irisreset-command | R14 | PtzOpticsG3 | command.IrisReset | 01 04 0B 00 | - | Current Developer Portal command pages |
| r11-irisreset-command | R11 | SonyBrcH900 | command.IrisReset | 01 04 0B 00 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-irisreset-command | R8 | SonyEviH100, GenericVisca | command.IrisReset | 01 04 0B 00 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-irisreset-command | R12 | SonyBrc300, GenericVisca | command.IrisReset | 01 04 0B 00 | - | BRC-300 Command List pp.9–12 |
| r21-irisreset-command | R21 | NearusBrc300, GenericVisca | command.IrisReset | 01 04 0B 00 | - | BRC-300/300P Command List pp.9–12 |
| r1-irisup-command | R1 | PtzOpticsG2, PtzOptics30X | command.IrisUp | 01 04 0B 02 | - | §7 command list |
| r14-irisup-command | R14 | PtzOpticsG3 | command.IrisUp | 01 04 0B 02 | - | Current Developer Portal command pages |
| r11-irisup-command | R11 | SonyBrcH900 | command.IrisUp | 01 04 0B 02 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-irisup-command | R8 | SonyEviH100, GenericVisca | command.IrisUp | 01 04 0B 02 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-irisup-command | R12 | SonyBrc300, GenericVisca | command.IrisUp | 01 04 0B 02 | - | BRC-300 Command List pp.9–12 |
| r21-irisup-command | R21 | NearusBrc300, GenericVisca | command.IrisUp | 01 04 0B 02 | - | BRC-300/300P Command List pp.9–12 |
| r1-irisdown-command | R1 | PtzOpticsG2, PtzOptics30X | command.IrisDown | 01 04 0B 03 | - | §7 command list |
| r14-irisdown-command | R14 | PtzOpticsG3 | command.IrisDown | 01 04 0B 03 | - | Current Developer Portal command pages |
| r11-irisdown-command | R11 | SonyBrcH900 | command.IrisDown | 01 04 0B 03 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-irisdown-command | R8 | SonyEviH100, GenericVisca | command.IrisDown | 01 04 0B 03 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-irisdown-command | R12 | SonyBrc300, GenericVisca | command.IrisDown | 01 04 0B 03 | - | BRC-300 Command List pp.9–12 |
| r21-irisdown-command | R21 | NearusBrc300, GenericVisca | command.IrisDown | 01 04 0B 03 | - | BRC-300/300P Command List pp.9–12 |
| r1-irisdirect-command | R1 | PtzOpticsG2, PtzOptics30X | command.IrisDirect | 01 04 4B | - | §7 command list |
| r14-irisdirect-command | R14 | PtzOpticsG3 | command.IrisDirect | 01 04 4B | - | Current Developer Portal command pages |
| r11-irisdirect-command | R11 | SonyBrcH900 | command.IrisDirect | 01 04 4B | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-irisdirect-command | R8 | SonyEviH100, GenericVisca | command.IrisDirect | 01 04 4B | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-irisdirect-command | R12 | SonyBrc300, GenericVisca | command.IrisDirect | 01 04 4B | - | BRC-300 Command List pp.9–12 |
| r21-irisdirect-command | R21 | NearusBrc300, GenericVisca | command.IrisDirect | 01 04 4B | - | BRC-300/300P Command List pp.9–12 |
| r1-shutterreset-command | R1 | PtzOpticsG2, PtzOptics30X | command.ShutterReset | 01 04 0A 00 | - | §7 command list |
| r14-shutterreset-command | R14 | PtzOpticsG3 | command.ShutterReset | 01 04 0A 00 | - | Current Developer Portal command pages |
| r11-shutterreset-command | R11 | SonyBrcH900 | command.ShutterReset | 01 04 0A 00 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-shutterreset-command | R8 | SonyEviH100, GenericVisca | command.ShutterReset | 01 04 0A 00 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-shutterreset-command | R12 | SonyBrc300, GenericVisca | command.ShutterReset | 01 04 0A 00 | - | BRC-300 Command List pp.9–12 |
| r21-shutterreset-command | R21 | NearusBrc300, GenericVisca | command.ShutterReset | 01 04 0A 00 | - | BRC-300/300P Command List pp.9–12 |
| r1-shutterup-command | R1 | PtzOpticsG2, PtzOptics30X | command.ShutterUp | 01 04 0A 02 | - | §7 command list |
| r14-shutterup-command | R14 | PtzOpticsG3 | command.ShutterUp | 01 04 0A 02 | - | Current Developer Portal command pages |
| r11-shutterup-command | R11 | SonyBrcH900 | command.ShutterUp | 01 04 0A 02 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-shutterup-command | R8 | SonyEviH100, GenericVisca | command.ShutterUp | 01 04 0A 02 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-shutterup-command | R12 | SonyBrc300, GenericVisca | command.ShutterUp | 01 04 0A 02 | - | BRC-300 Command List pp.9–12 |
| r21-shutterup-command | R21 | NearusBrc300, GenericVisca | command.ShutterUp | 01 04 0A 02 | - | BRC-300/300P Command List pp.9–12 |
| r1-shutterdown-command | R1 | PtzOpticsG2, PtzOptics30X | command.ShutterDown | 01 04 0A 03 | - | §7 command list |
| r14-shutterdown-command | R14 | PtzOpticsG3 | command.ShutterDown | 01 04 0A 03 | - | Current Developer Portal command pages |
| r11-shutterdown-command | R11 | SonyBrcH900 | command.ShutterDown | 01 04 0A 03 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-shutterdown-command | R8 | SonyEviH100, GenericVisca | command.ShutterDown | 01 04 0A 03 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-shutterdown-command | R12 | SonyBrc300, GenericVisca | command.ShutterDown | 01 04 0A 03 | - | BRC-300 Command List pp.9–12 |
| r21-shutterdown-command | R21 | NearusBrc300, GenericVisca | command.ShutterDown | 01 04 0A 03 | - | BRC-300/300P Command List pp.9–12 |
| r1-shutterdirect-command | R1 | PtzOpticsG2, PtzOptics30X | command.ShutterDirect | 01 04 4A | - | §7 command list |
| r14-shutterdirect-command | R14 | PtzOpticsG3 | command.ShutterDirect | 01 04 4A | - | Current Developer Portal command pages |
| r11-shutterdirect-command | R11 | SonyBrcH900 | command.ShutterDirect | 01 04 4A | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-shutterdirect-command | R8 | SonyEviH100, GenericVisca | command.ShutterDirect | 01 04 4A | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-shutterdirect-command | R12 | SonyBrc300, GenericVisca | command.ShutterDirect | 01 04 4A | - | BRC-300 Command List pp.9–12 |
| r21-shutterdirect-command | R21 | NearusBrc300, GenericVisca | command.ShutterDirect | 01 04 4A | - | BRC-300/300P Command List pp.9–12 |
| r1-brightnessreset-command | R1 | PtzOpticsG2, PtzOptics30X | command.BrightnessReset | 01 04 0D 00 | - | §7 command list |
| r14-brightnessreset-command | R14 | PtzOpticsG3 | command.BrightnessReset | 01 04 0D 00 | - | Current Developer Portal command pages |
| r8-brightnessreset-command | R8 | SonyEviH100, GenericVisca | command.BrightnessReset | 01 04 0D 00 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-brightnessreset-command | R12 | SonyBrc300, GenericVisca | command.BrightnessReset | 01 04 0D 00 | - | BRC-300 Command List pp.9–12 |
| r21-brightnessreset-command | R21 | NearusBrc300, GenericVisca | command.BrightnessReset | 01 04 0D 00 | - | BRC-300/300P Command List pp.9–12 |
| r1-brightnessup-command | R1 | PtzOpticsG2, PtzOptics30X | command.BrightnessUp | 01 04 0D 02 | - | §7 command list |
| r14-brightnessup-command | R14 | PtzOpticsG3 | command.BrightnessUp | 01 04 0D 02 | - | Current Developer Portal command pages |
| r8-brightnessup-command | R8 | SonyEviH100, GenericVisca | command.BrightnessUp | 01 04 0D 02 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-brightnessup-command | R12 | SonyBrc300, GenericVisca | command.BrightnessUp | 01 04 0D 02 | - | BRC-300 Command List pp.9–12 |
| r21-brightnessup-command | R21 | NearusBrc300, GenericVisca | command.BrightnessUp | 01 04 0D 02 | - | BRC-300/300P Command List pp.9–12 |
| r1-brightnessdown-command | R1 | PtzOpticsG2, PtzOptics30X | command.BrightnessDown | 01 04 0D 03 | - | §7 command list |
| r14-brightnessdown-command | R14 | PtzOpticsG3 | command.BrightnessDown | 01 04 0D 03 | - | Current Developer Portal command pages |
| r8-brightnessdown-command | R8 | SonyEviH100, GenericVisca | command.BrightnessDown | 01 04 0D 03 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-brightnessdown-command | R12 | SonyBrc300, GenericVisca | command.BrightnessDown | 01 04 0D 03 | - | BRC-300 Command List pp.9–12 |
| r21-brightnessdown-command | R21 | NearusBrc300, GenericVisca | command.BrightnessDown | 01 04 0D 03 | - | BRC-300/300P Command List pp.9–12 |
| r1-brightnessset-command | R1 | PtzOpticsG2, PtzOptics30X | command.BrightnessSet | 01 04 4D | - | §7 command list |
| r14-brightnessset-command | R14 | PtzOpticsG3 | command.BrightnessSet | 01 04 4D | - | Current Developer Portal command pages |
| r8-brightnessset-command | R8 | SonyEviH100, GenericVisca | command.BrightnessSet | 01 04 4D | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-brightnessset-command | R12 | SonyBrc300, GenericVisca | command.BrightnessSet | 01 04 4D | - | BRC-300 Command List pp.9–12 |
| r21-brightnessset-command | R21 | NearusBrc300, GenericVisca | command.BrightnessSet | 01 04 4D | - | BRC-300/300P Command List pp.9–12 |
| r1-antiflicker-command | R1 | PtzOpticsG2, PtzOptics30X | command.AntiFlicker | 01 04 23 | - | §7 command list |
| r10-antiflicker-command | R10 | PtzOpticsG3 | command.AntiFlicker | 01 04 23 | - | Move 4K G3 command list |
| r7-spotlighton-command | R7 | SonyFr7 | command.SpotlightOn | 01 04 3A 02 | - | FR7 v4 command list pp.14–20 |
| r11-spotlighton-command | R11 | SonyBrcH900 | command.SpotlightOn | 01 04 3A 02 | - | BRC-H900 v2.20 command list pp.14–18 |
| r7-spotlightoff-command | R7 | SonyFr7 | command.SpotlightOff | 01 04 3A 03 | - | FR7 v4 command list pp.14–20 |
| r8-autoslowshutteron-command | R8 | SonyEviH100, GenericVisca | command.AutoSlowShutterOn | 01 04 5A 02 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-autoslowshutteron-command | R12 | SonyBrc300, GenericVisca | command.AutoSlowShutterOn | 01 04 5A 02 | - | BRC-300 Command List pp.9–12 |
| r21-autoslowshutteron-command | R21 | NearusBrc300, GenericVisca | command.AutoSlowShutterOn | 01 04 5A 02 | - | BRC-300/300P Command List pp.9–12 |
| r8-autoslowshutteroff-command | R8 | SonyEviH100, GenericVisca | command.AutoSlowShutterOff | 01 04 5A 03 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-autoslowshutteroff-command | R12 | SonyBrc300, GenericVisca | command.AutoSlowShutterOff | 01 04 5A 03 | - | BRC-300 Command List pp.9–12 |
| r21-autoslowshutteroff-command | R21 | NearusBrc300, GenericVisca | command.AutoSlowShutterOff | 01 04 5A 03 | - | BRC-300/300P Command List pp.9–12 |
| r1-gainreset-command | R1 | PtzOpticsG2, PtzOptics30X | command.GainReset | 01 04 0C 00 | - | §7 command list |
| r14-gainreset-command | R14 | PtzOpticsG3 | command.GainReset | 01 04 0C 00 | - | Current Developer Portal command pages |
| r11-gainreset-command | R11 | SonyBrcH900 | command.GainReset | 01 04 0C 00 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-gainreset-command | R8 | SonyEviH100, GenericVisca | command.GainReset | 01 04 0C 00 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-gainreset-command | R12 | SonyBrc300, GenericVisca | command.GainReset | 01 04 0C 00 | - | BRC-300 Command List pp.9–12 |
| r21-gainreset-command | R21 | NearusBrc300, GenericVisca | command.GainReset | 01 04 0C 00 | - | BRC-300/300P Command List pp.9–12 |
| r1-gainup-command | R1 | PtzOpticsG2, PtzOptics30X | command.GainUp | 01 04 0C 02 | - | §7 command list |
| r14-gainup-command | R14 | PtzOpticsG3 | command.GainUp | 01 04 0C 02 | - | Current Developer Portal command pages |
| r11-gainup-command | R11 | SonyBrcH900 | command.GainUp | 01 04 0C 02 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-gainup-command | R8 | SonyEviH100, GenericVisca | command.GainUp | 01 04 0C 02 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-gainup-command | R12 | SonyBrc300, GenericVisca | command.GainUp | 01 04 0C 02 | - | BRC-300 Command List pp.9–12 |
| r21-gainup-command | R21 | NearusBrc300, GenericVisca | command.GainUp | 01 04 0C 02 | - | BRC-300/300P Command List pp.9–12 |
| r1-gaindown-command | R1 | PtzOpticsG2, PtzOptics30X | command.GainDown | 01 04 0C 03 | - | §7 command list |
| r14-gaindown-command | R14 | PtzOpticsG3 | command.GainDown | 01 04 0C 03 | - | Current Developer Portal command pages |
| r11-gaindown-command | R11 | SonyBrcH900 | command.GainDown | 01 04 0C 03 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-gaindown-command | R8 | SonyEviH100, GenericVisca | command.GainDown | 01 04 0C 03 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-gaindown-command | R12 | SonyBrc300, GenericVisca | command.GainDown | 01 04 0C 03 | - | BRC-300 Command List pp.9–12 |
| r21-gaindown-command | R21 | NearusBrc300, GenericVisca | command.GainDown | 01 04 0C 03 | - | BRC-300/300P Command List pp.9–12 |
| r1-gaindirect-command | R1 | PtzOpticsG2, PtzOptics30X | command.GainDirect | 01 04 4C | - | §7 command list |
| r14-gaindirect-command | R14 | PtzOpticsG3 | command.GainDirect | 01 04 4C | - | Current Developer Portal command pages |
| r11-gaindirect-command | R11 | SonyBrcH900 | command.GainDirect | 01 04 4C | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-gaindirect-command | R8 | SonyEviH100, GenericVisca | command.GainDirect | 01 04 4C | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-gaindirect-command | R12 | SonyBrc300, GenericVisca | command.GainDirect | 01 04 4C | - | BRC-300 Command List pp.9–12 |
| r21-gaindirect-command | R21 | NearusBrc300, GenericVisca | command.GainDirect | 01 04 4C | - | BRC-300/300P Command List pp.9–12 |
| r1-gainlimit-command | R1 | PtzOpticsG2, PtzOptics30X | command.GainLimit | 01 04 2C | - | §7 command list |
| r14-gainlimit-command | R14 | PtzOpticsG3 | command.GainLimit | 01 04 2C | - | Current Developer Portal command pages |
| r1-whitebalanceauto-command | R1 | PtzOpticsG2, PtzOptics30X | command.WhiteBalanceAuto | 01 04 35 00 | - | §7 command list |
| r14-whitebalanceauto-command | R14 | PtzOpticsG3 | command.WhiteBalanceAuto | 01 04 35 00 | - | Current Developer Portal command pages |
| r11-whitebalanceauto-command | R11 | SonyBrcH900 | command.WhiteBalanceAuto | 01 04 35 00 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-whitebalanceauto-command | R8 | SonyEviH100, GenericVisca | command.WhiteBalanceAuto | 01 04 35 00 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-whitebalanceauto-command | R12 | SonyBrc300, GenericVisca | command.WhiteBalanceAuto | 01 04 35 00 | - | BRC-300 Command List pp.9–12 |
| r21-whitebalanceauto-command | R21 | NearusBrc300, GenericVisca | command.WhiteBalanceAuto | 01 04 35 00 | - | BRC-300/300P Command List pp.9–12 |
| r1-whitebalanceindoor-command | R1 | PtzOpticsG2, PtzOptics30X | command.WhiteBalanceIndoor | 01 04 35 01 | - | §7 command list |
| r14-whitebalanceindoor-command | R14 | PtzOpticsG3 | command.WhiteBalanceIndoor | 01 04 35 01 | - | Current Developer Portal command pages |
| r11-whitebalanceindoor-command | R11 | SonyBrcH900 | command.WhiteBalanceIndoor | 01 04 35 01 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-whitebalanceindoor-command | R8 | SonyEviH100, GenericVisca | command.WhiteBalanceIndoor | 01 04 35 01 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-whitebalanceindoor-command | R12 | SonyBrc300, GenericVisca | command.WhiteBalanceIndoor | 01 04 35 01 | - | BRC-300 Command List pp.9–12 |
| r21-whitebalanceindoor-command | R21 | NearusBrc300, GenericVisca | command.WhiteBalanceIndoor | 01 04 35 01 | - | BRC-300/300P Command List pp.9–12 |
| r1-whitebalanceoutdoor-command | R1 | PtzOpticsG2, PtzOptics30X | command.WhiteBalanceOutdoor | 01 04 35 02 | - | §7 command list |
| r14-whitebalanceoutdoor-command | R14 | PtzOpticsG3 | command.WhiteBalanceOutdoor | 01 04 35 02 | - | Current Developer Portal command pages |
| r11-whitebalanceoutdoor-command | R11 | SonyBrcH900 | command.WhiteBalanceOutdoor | 01 04 35 02 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-whitebalanceoutdoor-command | R8 | SonyEviH100, GenericVisca | command.WhiteBalanceOutdoor | 01 04 35 02 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-whitebalanceoutdoor-command | R12 | SonyBrc300, GenericVisca | command.WhiteBalanceOutdoor | 01 04 35 02 | - | BRC-300 Command List pp.9–12 |
| r21-whitebalanceoutdoor-command | R21 | NearusBrc300, GenericVisca | command.WhiteBalanceOutdoor | 01 04 35 02 | - | BRC-300/300P Command List pp.9–12 |
| r1-whitebalanceonepush-command | R1 | PtzOpticsG2, PtzOptics30X | command.WhiteBalanceOnePush | 01 04 35 03 | - | §7 command list |
| r14-whitebalanceonepush-command | R14 | PtzOpticsG3 | command.WhiteBalanceOnePush | 01 04 35 03 | - | Current Developer Portal command pages |
| r11-whitebalanceonepush-command | R11 | SonyBrcH900 | command.WhiteBalanceOnePush | 01 04 35 03 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-whitebalanceonepush-command | R8 | SonyEviH100, GenericVisca | command.WhiteBalanceOnePush | 01 04 35 03 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-whitebalanceonepush-command | R12 | SonyBrc300, GenericVisca | command.WhiteBalanceOnePush | 01 04 35 03 | - | BRC-300 Command List pp.9–12 |
| r21-whitebalanceonepush-command | R21 | NearusBrc300, GenericVisca | command.WhiteBalanceOnePush | 01 04 35 03 | - | BRC-300/300P Command List pp.9–12 |
| r7-whitebalanceautotracking-command | R7 | SonyFr7 | command.WhiteBalanceAutoTracking | 01 04 35 04 | - | FR7 v4 command list pp.14–20 |
| r1-whitebalancemanual-command | R1 | PtzOpticsG2, PtzOptics30X | command.WhiteBalanceManual | 01 04 35 05 | - | §7 command list |
| r14-whitebalancemanual-command | R14 | PtzOpticsG3 | command.WhiteBalanceManual | 01 04 35 05 | - | Current Developer Portal command pages |
| r11-whitebalancemanual-command | R11 | SonyBrcH900 | command.WhiteBalanceManual | 01 04 35 05 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-whitebalancemanual-command | R8 | SonyEviH100, GenericVisca | command.WhiteBalanceManual | 01 04 35 05 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-whitebalancemanual-command | R12 | SonyBrc300, GenericVisca | command.WhiteBalanceManual | 01 04 35 05 | - | BRC-300 Command List pp.9–12 |
| r21-whitebalancemanual-command | R21 | NearusBrc300, GenericVisca | command.WhiteBalanceManual | 01 04 35 05 | - | BRC-300/300P Command List pp.9–12 |
| r1-whitebalancecolortemperature-command | R1 | PtzOpticsG2, PtzOptics30X | command.WhiteBalanceColorTemperature | 01 04 35 20 | - | §7 command list |
| r14-whitebalancecolortemperature-command | R14 | PtzOpticsG3 | command.WhiteBalanceColorTemperature | 01 04 35 20 | - | Current Developer Portal command pages |
| r1-autowhitebalancesensitivity-command | R1 | PtzOpticsG2, PtzOptics30X | command.AutoWhiteBalanceSensitivity | 01 04 A9 | - | §7 command list |
| r14-autowhitebalancesensitivity-command | R14 | PtzOpticsG3 | command.AutoWhiteBalanceSensitivity | 01 04 A9 | - | Current Developer Portal command pages |
| r1-onepushwhitebalancetrigger-command | R1 | PtzOpticsG2, PtzOptics30X | command.OnePushWhiteBalanceTrigger | 01 04 10 05 | - | §7 command list |
| r14-onepushwhitebalancetrigger-command | R14 | PtzOpticsG3 | command.OnePushWhiteBalanceTrigger | 01 04 10 05 | - | Current Developer Portal command pages |
| r8-onepushwhitebalancetrigger-command | R8 | SonyEviH100, GenericVisca | command.OnePushWhiteBalanceTrigger | 01 04 10 05 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-onepushwhitebalancetrigger-command | R12 | SonyBrc300, GenericVisca | command.OnePushWhiteBalanceTrigger | 01 04 10 05 | - | BRC-300 Command List pp.9–12 |
| r21-onepushwhitebalancetrigger-command | R21 | NearusBrc300, GenericVisca | command.OnePushWhiteBalanceTrigger | 01 04 10 05 | - | BRC-300/300P Command List pp.9–12 |
| r1-redtuning-command | R1 | PtzOpticsG2, PtzOptics30X | command.RedTuning | 01 04 43 | - | §7 command list |
| r14-redtuning-command | R14 | PtzOpticsG3 | command.RedTuning | 01 04 43 | - | Current Developer Portal command pages |
| r1-bluetuning-command | R1 | PtzOpticsG2, PtzOptics30X | command.BlueTuning | 01 04 44 | - | §7 command list |
| r14-bluetuning-command | R14 | PtzOpticsG3 | command.BlueTuning | 01 04 44 | - | Current Developer Portal command pages |
| r1-saturation-command | R1 | PtzOpticsG2, PtzOptics30X | command.Saturation | 01 04 49 | - | §7 command list |
| r14-saturation-command | R14 | PtzOpticsG3 | command.Saturation | 01 04 49 | - | Current Developer Portal command pages |
| r8-saturation-command | R8 | SonyEviH100 | command.Saturation | 01 04 49 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r1-hue-command | R1 | PtzOpticsG2, PtzOptics30X | command.Hue | 01 04 4F | - | §7 command list |
| r14-hue-command | R14 | PtzOpticsG3 | command.Hue | 01 04 4F | - | Current Developer Portal command pages |
| r8-hue-command | R8 | SonyEviH100 | command.Hue | 01 04 4F | - | EVI-H100 Technical Manual command list pp.31–34 |
| r1-colortemperaturereset-command | R1 | PtzOpticsG2, PtzOptics30X | command.ColorTemperatureReset | 01 04 20 | - | §7 command list |
| r14-colortemperaturereset-command | R14 | PtzOpticsG3 | command.ColorTemperatureReset | 01 04 20 | - | Current Developer Portal command pages |
| r1-colortemperatureup-command | R1 | PtzOpticsG2, PtzOptics30X | command.ColorTemperatureUp | 01 04 20 | - | §7 command list |
| r14-colortemperatureup-command | R14 | PtzOpticsG3 | command.ColorTemperatureUp | 01 04 20 | - | Current Developer Portal command pages |
| r1-colortemperaturedown-command | R1 | PtzOpticsG2, PtzOptics30X | command.ColorTemperatureDown | 01 04 20 | - | §7 command list |
| r14-colortemperaturedown-command | R14 | PtzOpticsG3 | command.ColorTemperatureDown | 01 04 20 | - | Current Developer Portal command pages |
| r1-colortemperaturedirect-command | R1 | PtzOpticsG2, PtzOptics30X | command.ColorTemperatureDirect | 01 04 20 | - | §7 command list |
| r14-colortemperaturedirect-command | R14 | PtzOpticsG3 | command.ColorTemperatureDirect | 01 04 20 | - | Current Developer Portal command pages |
| r1-redgainreset-command | R1 | PtzOpticsG2, PtzOptics30X | command.RedGainReset | 01 04 03 00 | - | §7 command list |
| r14-redgainreset-command | R14 | PtzOpticsG3 | command.RedGainReset | 01 04 03 00 | - | Current Developer Portal command pages |
| r8-redgainreset-command | R8 | SonyEviH100, GenericVisca | command.RedGainReset | 01 04 03 00 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-redgainreset-command | R12 | SonyBrc300, GenericVisca | command.RedGainReset | 01 04 03 00 | - | BRC-300 Command List pp.9–12 |
| r21-redgainreset-command | R21 | NearusBrc300, GenericVisca | command.RedGainReset | 01 04 03 00 | - | BRC-300/300P Command List pp.9–12 |
| r1-redgainup-command | R1 | PtzOpticsG2, PtzOptics30X | command.RedGainUp | 01 04 03 02 | - | §7 command list |
| r14-redgainup-command | R14 | PtzOpticsG3 | command.RedGainUp | 01 04 03 02 | - | Current Developer Portal command pages |
| r8-redgainup-command | R8 | SonyEviH100, GenericVisca | command.RedGainUp | 01 04 03 02 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-redgainup-command | R12 | SonyBrc300, GenericVisca | command.RedGainUp | 01 04 03 02 | - | BRC-300 Command List pp.9–12 |
| r21-redgainup-command | R21 | NearusBrc300, GenericVisca | command.RedGainUp | 01 04 03 02 | - | BRC-300/300P Command List pp.9–12 |
| r1-redgaindown-command | R1 | PtzOpticsG2, PtzOptics30X | command.RedGainDown | 01 04 03 03 | - | §7 command list |
| r14-redgaindown-command | R14 | PtzOpticsG3 | command.RedGainDown | 01 04 03 03 | - | Current Developer Portal command pages |
| r8-redgaindown-command | R8 | SonyEviH100, GenericVisca | command.RedGainDown | 01 04 03 03 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-redgaindown-command | R12 | SonyBrc300, GenericVisca | command.RedGainDown | 01 04 03 03 | - | BRC-300 Command List pp.9–12 |
| r21-redgaindown-command | R21 | NearusBrc300, GenericVisca | command.RedGainDown | 01 04 03 03 | - | BRC-300/300P Command List pp.9–12 |
| r1-redgaindirect-command | R1 | PtzOpticsG2, PtzOptics30X | command.RedGainDirect | 01 04 43 | - | §7 command list |
| r14-redgaindirect-command | R14 | PtzOpticsG3 | command.RedGainDirect | 01 04 43 | - | Current Developer Portal command pages |
| r8-redgaindirect-command | R8 | SonyEviH100, GenericVisca | command.RedGainDirect | 01 04 43 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-redgaindirect-command | R12 | SonyBrc300, GenericVisca | command.RedGainDirect | 01 04 43 | - | BRC-300 Command List pp.9–12 |
| r21-redgaindirect-command | R21 | NearusBrc300, GenericVisca | command.RedGainDirect | 01 04 43 | - | BRC-300/300P Command List pp.9–12 |
| r1-bluegainreset-command | R1 | PtzOpticsG2, PtzOptics30X | command.BlueGainReset | 01 04 04 00 | - | §7 command list |
| r14-bluegainreset-command | R14 | PtzOpticsG3 | command.BlueGainReset | 01 04 04 00 | - | Current Developer Portal command pages |
| r8-bluegainreset-command | R8 | SonyEviH100, GenericVisca | command.BlueGainReset | 01 04 04 00 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-bluegainreset-command | R12 | SonyBrc300, GenericVisca | command.BlueGainReset | 01 04 04 00 | - | BRC-300 Command List pp.9–12 |
| r21-bluegainreset-command | R21 | NearusBrc300, GenericVisca | command.BlueGainReset | 01 04 04 00 | - | BRC-300/300P Command List pp.9–12 |
| r1-bluegainup-command | R1 | PtzOpticsG2, PtzOptics30X | command.BlueGainUp | 01 04 04 02 | - | §7 command list |
| r14-bluegainup-command | R14 | PtzOpticsG3 | command.BlueGainUp | 01 04 04 02 | - | Current Developer Portal command pages |
| r8-bluegainup-command | R8 | SonyEviH100, GenericVisca | command.BlueGainUp | 01 04 04 02 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-bluegainup-command | R12 | SonyBrc300, GenericVisca | command.BlueGainUp | 01 04 04 02 | - | BRC-300 Command List pp.9–12 |
| r21-bluegainup-command | R21 | NearusBrc300, GenericVisca | command.BlueGainUp | 01 04 04 02 | - | BRC-300/300P Command List pp.9–12 |
| r1-bluegaindown-command | R1 | PtzOpticsG2, PtzOptics30X | command.BlueGainDown | 01 04 04 03 | - | §7 command list |
| r14-bluegaindown-command | R14 | PtzOpticsG3 | command.BlueGainDown | 01 04 04 03 | - | Current Developer Portal command pages |
| r8-bluegaindown-command | R8 | SonyEviH100, GenericVisca | command.BlueGainDown | 01 04 04 03 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-bluegaindown-command | R12 | SonyBrc300, GenericVisca | command.BlueGainDown | 01 04 04 03 | - | BRC-300 Command List pp.9–12 |
| r21-bluegaindown-command | R21 | NearusBrc300, GenericVisca | command.BlueGainDown | 01 04 04 03 | - | BRC-300/300P Command List pp.9–12 |
| r1-bluegaindirect-command | R1 | PtzOpticsG2, PtzOptics30X | command.BlueGainDirect | 01 04 44 | - | §7 command list |
| r14-bluegaindirect-command | R14 | PtzOpticsG3 | command.BlueGainDirect | 01 04 44 | - | Current Developer Portal command pages |
| r8-bluegaindirect-command | R8 | SonyEviH100, GenericVisca | command.BlueGainDirect | 01 04 44 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-bluegaindirect-command | R12 | SonyBrc300, GenericVisca | command.BlueGainDirect | 01 04 44 | - | BRC-300 Command List pp.9–12 |
| r21-bluegaindirect-command | R21 | NearusBrc300, GenericVisca | command.BlueGainDirect | 01 04 44 | - | BRC-300/300P Command List pp.9–12 |
| r1-sharpnessmode-command | R1 | PtzOpticsG2, PtzOptics30X | command.SharpnessMode | 01 04 05 | - | §7 command list |
| r14-sharpnessmode-command | R14 | PtzOpticsG3 | command.SharpnessMode | 01 04 05 | - | Current Developer Portal command pages |
| r1-sharpnessreset-command | R1 | PtzOpticsG2, PtzOptics30X | command.SharpnessReset | 01 04 02 00 | - | §7 command list |
| r14-sharpnessreset-command | R14 | PtzOpticsG3 | command.SharpnessReset | 01 04 02 00 | - | Current Developer Portal command pages |
| r7-sharpnessreset-command | R7 | SonyFr7 | command.SharpnessReset | 01 04 02 00 | - | FR7 v4 command list pp.14–20 |
| r8-sharpnessreset-command | R8 | SonyEviH100 | command.SharpnessReset | 01 04 02 00 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r1-sharpnessup-command | R1 | PtzOpticsG2, PtzOptics30X | command.SharpnessUp | 01 04 02 02 | - | §7 command list |
| r14-sharpnessup-command | R14 | PtzOpticsG3 | command.SharpnessUp | 01 04 02 02 | - | Current Developer Portal command pages |
| r7-sharpnessup-command | R7 | SonyFr7 | command.SharpnessUp | 01 04 02 02 | - | FR7 v4 command list pp.14–20 |
| r8-sharpnessup-command | R8 | SonyEviH100 | command.SharpnessUp | 01 04 02 02 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r1-sharpnessdown-command | R1 | PtzOpticsG2, PtzOptics30X | command.SharpnessDown | 01 04 02 03 | - | §7 command list |
| r14-sharpnessdown-command | R14 | PtzOpticsG3 | command.SharpnessDown | 01 04 02 03 | - | Current Developer Portal command pages |
| r7-sharpnessdown-command | R7 | SonyFr7 | command.SharpnessDown | 01 04 02 03 | - | FR7 v4 command list pp.14–20 |
| r8-sharpnessdown-command | R8 | SonyEviH100 | command.SharpnessDown | 01 04 02 03 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r1-sharpnessdirect-command | R1 | PtzOpticsG2, PtzOptics30X | command.SharpnessDirect | 01 04 42 | - | §7 command list |
| r14-sharpnessdirect-command | R14 | PtzOpticsG3 | command.SharpnessDirect | 01 04 42 | - | Current Developer Portal command pages |
| r7-sharpnessdirect-command | R7 | SonyFr7 | command.SharpnessDirect | 01 04 42 | - | FR7 v4 command list pp.14–20 |
| r8-sharpnessdirect-command | R8 | SonyEviH100 | command.SharpnessDirect | 01 04 42 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r1-luminance-command | R1 | PtzOpticsG2, PtzOptics30X | command.Luminance | 01 04 A1 | - | §7 command list |
| r14-luminance-command | R14 | PtzOpticsG3 | command.Luminance | 01 04 A1 | - | Current Developer Portal command pages |
| r1-contrast-command | R1 | PtzOpticsG2, PtzOptics30X | command.Contrast | 01 04 A2 | - | §7 command list |
| r14-contrast-command | R14 | PtzOpticsG3 | command.Contrast | 01 04 A2 | - | Current Developer Portal command pages |
| r1-gamma-command | R1 | PtzOpticsG2, PtzOptics30X | command.Gamma | 01 04 5B | - | §7 command list |
| r14-gamma-command | R14 | PtzOpticsG3 | command.Gamma | 01 04 5B | - | Current Developer Portal command pages |
| r8-gamma-command | R8 | SonyEviH100 | command.Gamma | 01 04 5B | - | EVI-H100 Technical Manual command list pp.31–34 |
| r1-backlight-command | R1 | PtzOpticsG2, PtzOptics30X | command.Backlight | 01 04 33 | - | §7 command list |
| r14-backlight-command | R14 | PtzOpticsG3 | command.Backlight | 01 04 33 | - | Current Developer Portal command pages |
| r7-backlight-command | R7 | SonyFr7 | command.Backlight | 01 04 33 | - | FR7 v4 command list pp.14–20 |
| r8-backlight-command | R8 | SonyEviH100, GenericVisca | command.Backlight | 01 04 33 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-backlight-command | R12 | SonyBrc300, GenericVisca | command.Backlight | 01 04 33 | - | BRC-300 Command List pp.9–12 |
| r21-backlight-command | R21 | NearusBrc300, GenericVisca | command.Backlight | 01 04 33 | - | BRC-300/300P Command List pp.9–12 |
| r14-noisereduction2dmode-command | R14 | PtzOpticsG2, PtzOpticsG3, PtzOptics30X | command.NoiseReduction2dMode | 01 04 50 | - | Current Developer Portal command pages |
| r14-noisereduction2d-command | R14 | PtzOpticsG2, PtzOpticsG3, PtzOptics30X | command.NoiseReduction2d | 01 04 53 | - | Current Developer Portal command pages |
| r8-noisereduction2d-command | R8 | SonyEviH100 | command.NoiseReduction2d | 01 04 53 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r14-noisereduction2doff-command | R14 | PtzOpticsG2, PtzOpticsG3, PtzOptics30X | command.NoiseReduction2dOff | 01 04 53 | - | Current Developer Portal command pages |
| r8-noisereduction2doff-command | R8 | SonyEviH100 | command.NoiseReduction2dOff | 01 04 53 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r14-noisereduction3d-command | R14 | PtzOpticsG2, PtzOpticsG3, PtzOptics30X | command.NoiseReduction3d | 01 04 54 | - | Current Developer Portal command pages |
| r14-noisereduction3doff-command | R14 | PtzOpticsG2, PtzOpticsG3, PtzOptics30X | command.NoiseReduction3dOff | 01 04 54 | - | Current Developer Portal command pages |
| r1-imageflipoff-command | R1 | PtzOpticsG2, PtzOptics30X | command.ImageFlipOff | 01 04 66 | - | §7 command list |
| r14-imageflipoff-command | R14 | PtzOpticsG3 | command.ImageFlipOff | 01 04 66 | - | Current Developer Portal command pages |
| r12-imageflipoff-command | R12 | SonyBrc300 | command.ImageFlipOff | 01 04 66 | - | BRC-300 Command List pp.9–12 |
| r21-imageflipoff-command | R21 | NearusBrc300 | command.ImageFlipOff | 01 04 66 | - | BRC-300/300P Command List pp.9–12 |
| r1-imagefliphorizontal-command | R1 | PtzOpticsG2, PtzOptics30X | command.ImageFlipHorizontal | 01 04 61 | - | §7 command list |
| r14-imagefliphorizontal-command | R14 | PtzOpticsG3 | command.ImageFlipHorizontal | 01 04 61 | - | Current Developer Portal command pages |
| r1-imagefliphorizontaloff-command | R1 | PtzOpticsG2, PtzOptics30X | command.ImageFlipHorizontalOff | 01 04 61 | - | §7 command list |
| r14-imagefliphorizontaloff-command | R14 | PtzOpticsG3 | command.ImageFlipHorizontalOff | 01 04 61 | - | Current Developer Portal command pages |
| r1-imageflipvertical-command | R1 | PtzOpticsG2, PtzOptics30X | command.ImageFlipVertical | 01 04 66 | - | §7 command list |
| r14-imageflipvertical-command | R14 | PtzOpticsG3 | command.ImageFlipVertical | 01 04 66 | - | Current Developer Portal command pages |
| r12-imageflipvertical-command | R12 | SonyBrc300 | command.ImageFlipVertical | 01 04 66 | - | BRC-300 Command List pp.9–12 |
| r21-imageflipvertical-command | R21 | NearusBrc300 | command.ImageFlipVertical | 01 04 66 | - | BRC-300/300P Command List pp.9–12 |
| r1-imageflipboth-command | R1 | PtzOpticsG2, PtzOptics30X | command.ImageFlipBoth | 01 04 A4 | - | §7 command list |
| r14-imageflipboth-command | R14 | PtzOpticsG3 | command.ImageFlipBoth | 01 04 A4 | - | Current Developer Portal command pages |
| r1-imageflipcombined-command | R1 | PtzOpticsG2, PtzOptics30X | command.ImageFlipCombined | 01 04 A4 | - | §7 command list |
| r14-imageflipcombined-command | R14 | PtzOpticsG3 | command.ImageFlipCombined | 01 04 A4 | - | Current Developer Portal command pages |
| r1-pictureeffect-command | R1 | PtzOpticsG2, PtzOptics30X | command.PictureEffect | 01 04 63 | - | §7 command list |
| r14-pictureeffect-command | R14 | PtzOpticsG3 | command.PictureEffect | 01 04 63 | - | Current Developer Portal command pages |
| r7-ndfiltermode-command | R7 | SonyFr7 | command.NdFilterMode | 01 7E 04 52 | - | FR7 v4 command list pp.14–20 |
| r7-ndfilterdirect-command | R7 | SonyFr7 | command.NdFilterDirect | 01 7E 04 42 | - | FR7 v4 command list pp.14–20 |
| r7-ndfilterstepup-command | R7 | SonyFr7 | command.NdFilterStepUp | 01 7E 04 12 02 | - | FR7 v4 command list pp.14–20 |
| r7-ndfilterstepdown-command | R7 | SonyFr7 | command.NdFilterStepDown | 01 7E 04 12 03 | - | FR7 v4 command list pp.14–20 |
| r7-ndfilterautoon-command | R7 | SonyFr7 | command.NdFilterAutoOn | 01 7E 04 53 02 | - | FR7 v4 command list pp.14–20 |
| r7-ndfilterautooff-command | R7 | SonyFr7 | command.NdFilterAutoOff | 01 7E 04 53 03 | - | FR7 v4 command list pp.14–20 |
| r7-tallyredon-command | R7 | SonyFr7 | command.TallyRedOn | 01 7E 01 0A 00 02 | - | FR7 v4 command list pp.14–20 |
| r7-tallyredoff-command | R7 | SonyFr7 | command.TallyRedOff | 01 7E 01 0A 00 03 | - | FR7 v4 command list pp.14–20 |
| r7-tallygreenon-command | R7 | SonyFr7 | command.TallyGreenOn | 01 7E 04 1A 00 02 | - | FR7 v4 command list pp.14–20 |
| r7-tallygreenoff-command | R7 | SonyFr7 | command.TallyGreenOff | 01 7E 04 1A 00 03 | - | FR7 v4 command list pp.14–20 |
| r7-menudisplay-command | R7 | SonyFr7 | command.MenuDisplay | 01 06 06 | - | FR7 v4 command list pp.14–20 |
| r1-menunavigate-command | R1 | PtzOpticsG2, PtzOptics30X | command.MenuNavigate | 01 06 01 | - | §7 command list |
| r14-menunavigate-command | R14 | PtzOpticsG3 | command.MenuNavigate | 01 06 01 | - | Current Developer Portal command pages |
| r7-menunavigate-command | R7 | SonyFr7 | command.MenuNavigate | 01 06 01 | - | FR7 v4 command list pp.14–20 |
| r11-menunavigate-command | R11 | SonyBrcH900 | command.MenuNavigate | 01 06 01 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-menunavigate-command | R8 | SonyEviH100, GenericVisca | command.MenuNavigate | 01 06 01 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-menunavigate-command | R12 | SonyBrc300, GenericVisca | command.MenuNavigate | 01 06 01 | - | BRC-300 Command List pp.9–12 |
| r21-menunavigate-command | R21 | NearusBrc300, GenericVisca | command.MenuNavigate | 01 06 01 | - | BRC-300/300P Command List pp.9–12 |
| r1-menuselect-command | R1 | PtzOpticsG2, PtzOptics30X | command.MenuSelect | 01 06 06 | - | §7 command list |
| r14-menuselect-command | R14 | PtzOpticsG3 | command.MenuSelect | 01 06 06 | - | Current Developer Portal command pages |
| r1-menucancel-command | R1 | PtzOpticsG2, PtzOptics30X | command.MenuCancel | 01 06 06 | - | §7 command list |
| r14-menucancel-command | R14 | PtzOpticsG3 | command.MenuCancel | 01 06 06 | - | Current Developer Portal command pages |
| r7-directmenu-command | R7 | SonyFr7 | command.DirectMenu | 01 7E 04 72 | - | FR7 v4 command list pp.14–20 |
| r1-multicaststreamingon-command | R1 | PtzOpticsG2, PtzOptics30X | command.MulticastStreamingOn | 0B 01 23 | - | §7 command list |
| r10-multicaststreamingon-command | R10 | PtzOpticsG3 | command.MulticastStreamingOn | 0B 01 23 | - | Move 4K G3 command list |
| r1-multicaststreamingoff-command | R1 | PtzOpticsG2, PtzOptics30X | command.MulticastStreamingOff | 0B 01 23 | - | §7 command list |
| r10-multicaststreamingoff-command | R10 | PtzOpticsG3 | command.MulticastStreamingOff | 0B 01 23 | - | Move 4K G3 command list |
| r1-ndiquality-command | R1 | PtzOpticsG2, PtzOptics30X | command.NdiQuality | 0B 01 01 | - | §7 command list |
| r10-ndiquality-command | R10 | PtzOpticsG3 | command.NdiQuality | 0B 01 01 | - | Move 4K G3 command list |
| r1-usbaudioon-command | R1 | PtzOpticsG2, PtzOptics30X | command.UsbAudioOn | 2A 02 A0 04 | - | §7 command list |
| r1-usbaudiooff-command | R1 | PtzOpticsG2, PtzOptics30X | command.UsbAudioOff | 2A 02 A0 04 | - | §7 command list |
| r8-addressset-command | R8 | SonyEviH100, GenericVisca | command.AddressSet | 30 01 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-addressset-command | R12 | SonyBrc300, GenericVisca | command.AddressSet | 30 01 | - | BRC-300 Command List pp.9–12 |
| r21-addressset-command | R21 | NearusBrc300, GenericVisca | command.AddressSet | 30 01 | - | BRC-300/300P Command List pp.9–12 |
| r11-addressset-command | R11 | SonyBrcH900 | command.AddressSet | 30 01 | - | BRC-H900 v2.20 command list pp.14–18 |
| r1-interfaceclear-command | R1 | PtzOpticsG2, PtzOptics30X | command.InterfaceClear | 01 00 01 | - | §7 command list |
| r14-interfaceclear-command | R14 | PtzOpticsG3 | command.InterfaceClear | 01 00 01 | - | Current Developer Portal command pages |
| r7-interfaceclear-command | R7 | SonyFr7 | command.InterfaceClear | 01 00 01 | - | FR7 v4 command list pp.14–20 |
| r11-interfaceclear-command | R11 | SonyBrcH900 | command.InterfaceClear | 01 00 01 | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-interfaceclear-command | R8 | SonyEviH100, GenericVisca | command.InterfaceClear | 01 00 01 | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-interfaceclear-command | R12 | SonyBrc300, GenericVisca | command.InterfaceClear | 01 00 01 | - | BRC-300 Command List pp.9–12 |
| r21-interfaceclear-command | R21 | NearusBrc300, GenericVisca | command.InterfaceClear | 01 00 01 | - | BRC-300/300P Command List pp.9–12 |
| r1-commandcancel-command | R1 | PtzOpticsG2, PtzOptics30X | command.CommandCancel | 2p | - | §7 command list |
| r14-commandcancel-command | R14 | PtzOpticsG3 | command.CommandCancel | 2p | - | Current Developer Portal command pages |
| r7-commandcancel-command | R7 | SonyFr7 | command.CommandCancel | 2p | - | FR7 v4 command list pp.14–20 |
| r11-commandcancel-command | R11 | SonyBrcH900 | command.CommandCancel | 2p | - | BRC-H900 v2.20 command list pp.14–18 |
| r8-commandcancel-command | R8 | SonyEviH100, GenericVisca | command.CommandCancel | 2p | - | EVI-H100 Technical Manual command list pp.31–34 |
| r12-commandcancel-command | R12 | SonyBrc300, GenericVisca | command.CommandCancel | 2p | - | BRC-300 Command List pp.9–12 |
| r21-commandcancel-command | R21 | NearusBrc300, GenericVisca | command.CommandCancel | 2p | - | BRC-300/300P Command List pp.9–12 |
| r1-settingssave-command | R1 | PtzOpticsG2, PtzOptics30X | command.SettingsSave | 01 04 A5 10 | - | §7 command list |
| r10-settingssave-command | R10 | PtzOpticsG3 | command.SettingsSave | 01 04 A5 10 | - | Move 4K G3 command list |
| r7-variablespeedmode-command | R7 | SonyFr7 | command.VariableSpeedMode | 01 06 45 | - | FR7 v4 command list pp.14–20 |

<!-- END CHECKED SOURCE EVIDENCE -->
