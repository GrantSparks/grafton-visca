# Consolidated VISCA / PTZOptics Gen‑2 / Axis Reference

**Document status:** Final consolidated reference - gap recommendations implemented
**Date:** 2026-05-15
**Primary scope:** PTZOptics Gen‑2 NDI®\|HX cameras (`PT12X‑NDI`, `PT20X‑NDI`, `PT30X‑NDI`), Axis VISCA Interface API behavior, and only the Sony VISCA references needed to resolve protocol/opcode conflicts.

This is the repository's canonical protocol reference. It replaces the former
repo-local PTZOptics, Axis, and unified VISCA protocol notes; those source names
may still appear in the appendices only as provenance for the consolidated
material.

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
Byte 8+:  VISCA payload, 1 to 16 bytes, ending in FF
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
`AwaitingAck`, `AwaitingCompletion`, and `AwaitingLateAck`. The
`AwaitingCompletion` phase is the completion-only shape: it holds the target
channel exclusively and never earns a socket. For an ACK-bearing command,
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
owner (#721). The old request moves to unkeyed ambiguity quarantine and the new
request owns the named socket. A socketless ACK may select the first free
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
| Direct focus | `81 01 04 48 0p 0q 0r 0s FF` |
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
AE/iris rows identified below. Entries without their own G3 source row remain
out of G3 scope.

| Inquiry | Packet | Reply / values |
|---|---|---|
| Power | `81 09 04 00 FF` | `90 50 02 FF` on; `90 50 03 FF` standby; `90 50 04 FF` internal power circuit error. |
| Zoom position | `81 09 04 47 FF` | `90 50 0p 0q 0r 0s FF`; `pqrs = Zoom Position`. |
| Focus AF mode | `81 09 04 38 FF` | `90 50 02 FF` auto; `90 50 03 FF` manual. |
| Focus position | `81 09 04 48 FF` | `90 50 0p 0q 0r 0s FF`; `pqrs = Focus Position`. |
| WB mode | `81 09 04 35 FF` | `00` auto, `01` indoor, `02` outdoor, `03` one-push, `05` manual, `20` color-temperature mode. |
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
| AF zone | `81 09 04 AA FF` | `90 50 00 FF` top, `01` center, `02` bottom. |
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
establish those typed or targeted-inquiry guarantees for FR7 or the other Sony,
EVI, or Nearus profiles.

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
- Payload length `1–16` bytes.
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
| Nearus BRC-300 | No independent model source | None | Both families |

For PTZOptics Gen-2, the checked sources do not establish the same
automatic-slow-shutter inquiry, so this table is not a generic-VISCA grant.

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
from a later phase. If an active raw retry reaches budget expiry while in
`Sending`, `AwaitingAck`, `AwaitingCompletion`, or `Executing`, the default is a per-request
`UnsequencedCommandUnconfirmed` result with its correlation quarantined; a
ready/backoff raw retry may finish with its retained last error. The session is
poisoned only when `strict_unconfirmed_poison` is enabled, which reports
`StreamPoisoned`. Cancellation quarantine is separate and is never shortened
by budget expiry.
Empty UDP datagrams are discarded while receiving and do not reset or extend
the one overall receive deadline; the async adapter yields cooperatively before
polling again.

| Transport | Recommended retry behavior |
|---|---|
| Raw UDP PTZOptics | Do not automatically replay a successfully sent command after an ACK/completion/cancellation ambiguity or active retry-budget expiry in `Sending`, `AwaitingAck`, `AwaitingCompletion`, or `Executing`: without a sequence, retry is indistinguishable from a new physical action. An intrinsic `Urgent` stop may cross one open candidate; ACK/error evidence observed with both open binds to neither (#714). A receive fault while awaiting ACK likewise never authorizes a replay, but by default leaves an *uncancelled* command in `AwaitingAck`; only a later ACK deadline without an ACK yields the per-request `UnsequencedCommandUnconfirmed` outcome and correlation quarantine. `strict_unconfirmed_poison` poisons immediately on that fault only with no recorded cancel; a recorded cancel follows cancellation-driven late-ACK resolution and poisons only if its deadline remains unconfirmed. A conclusive camera rejection may be retried under policy. Prefer TCP `5678` for high-reliability control. |
| Raw TCP PTZOptics | TCP handles byte delivery/order but does not prove camera execution. Serialize the ordinary one-command pre-ACK window per target and retain socket concurrency after ACK; an intrinsic `Urgent` stop may cross one candidate, making ACK/error evidence ambiguous and attributable to neither (#714). Treat an ACK/completion/cancellation ambiguity or active retry-budget expiry in `Sending`, `AwaitingAck`, `AwaitingCompletion`, or `Executing` as the default per-request `UnsequencedCommandUnconfirmed` outcome (the session survives), never a blind replay. A receive fault while awaiting ACK also forbids replay but leaves an *uncancelled* command in `AwaitingAck` by default; only its later ACK deadline without an ACK produces that outcome. `strict_unconfirmed_poison` poisons immediately on that fault only with no recorded cancel; a recorded cancel follows cancellation-driven late-ACK resolution and poisons only if its deadline remains unconfirmed. A conclusive camera rejection may be retried under policy. |
| Sony encapsulated UDP | Use the Sony sequence field to correlate replies. This document adopts the Sony-manual correction in §5.3: timeout recovery should retransmit the timed-out message with the same sequence number, rather than blindly issuing a new logical command. |
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
- Sony ILME-FR7 VISCA Command List, encapsulated VISCA-over-IP format: https://pro.sony/s3/2022/09/14131603/VISCA-Command-List-Version-2.00.pdf

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

**Narrow BRC-300 coordinate exception:** R12's pan/tilt value table maps
positive signed raw pan to left (`08A58`) and positive signed raw tilt to up
(`493D`); the negative endpoints (`F75A8` and `E796`) are right and down. The
library's BRC-300 profile therefore uses a negative signed degree-to-unit scale
for both axes while retaining the documented signed wire fields. This
profile-specific polarity must not be generalized to other VISCA profiles.

**Why it remains open:** The uploaded unified guide explicitly says the non-PTZOptics and non-Axis sections were not re-validated in the prior patch set. This final document used Sony manuals only to resolve transport and opcode semantics, not to validate every model-family capability.

**Validation needed:** For each target model family, validate against the current primary model manual/command list:

- Sony ILME‑FR7 / FR7K
- Sony BRC-H900 / BRC-X1000 / BRC-X400 / SRG families
- Sony EVI-H100S/H100V and older EVI models
- Nearus BRC‑300 / Sony BRC‑300 equivalents
- Marshall / AVer / Avonic / other VISCA-compatible brands

**Helpful sources:**

- Sony ILME‑FR7 VISCA Command List: https://pro.sony/s3/2022/09/14131603/VISCA-Command-List-Version-2.00.pdf
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
| R14 | PTZOptics Developer Portal, API v1.0 (portal dated 2026-08-31) | https://docs.ptzoptics.com/articles/miscellaneous/misc-cameras/pt-limits-packet-sender/; https://docs.ptzoptics.com/dev/visca-api/exposure/; https://docs.ptzoptics.com/dev/visca-api/queries/ | Official current command portal described by PTZOptics as the full VISCA list for G2 and G3. Used for G2/G3 AE, iris, query, image, and NR rows; it documents `04 39`, `04 0B`/`04 4B`, and `09 04 39`/`09 04 4B`, but not `09 04 2B`. Its Exposure page documents NR command inputs `04 50` Auto/Manual, `04 53` off/`1–5`, and `04 54` off/`1–8`; its Queries page separately documents current `09 04 50` and `09 04 53`/`54` results of `0–5`. |
| R15 | PTZOptics G2/Legacy camera index | https://docs.ptzoptics.com/docs/cameras/g2-legacy/ | Official legacy-scope index. It explicitly lists the 12X, 20X, and 30X SDI/NDI G2 models used to constrain the legacy `PtzOptics30X` profile; it does not generalize support to newer 30X, Move, or Link models. |

## C.2 Official Axis source

| Ref | Source | Link | Used for |
|---|---|---|---|
| R6 | Axis VISCA Interface API Description, User Manual M1.6, March 2021 | https://www.axis.com/dam/public/70/3d/31/visca-interface-api-description-en-US-266656.pdf | Axis zoom/focus/pan/tilt ranges, Axis inquiries, Axis menu display inquiry, Axis tally command. |

## C.3 Official Sony sources

| Ref | Source | Link | Used for |
|---|---|---|---|
| R7 | Sony ILME‑FR7 / FR7K VISCA Command List, Version 2.00 | https://pro.sony/s3/2022/09/14131603/VISCA-Command-List-Version-2.00.pdf | Sony VISCA-over-IP UDP `52381`, 8-byte header, payload types, sequence number, socket behavior, errors, retransmission guidance; FR7 vendor-relative `7E 04 4B` iris Up/Down, `05 34` Auto Iris inquiry, variable-ND controls/inquiries, and fixed `04 3A` spotlight controls. It does not establish the shared `04 39` AE-mode command/inquiry family, absolute iris direct control, or a `09 04 4B` position inquiry. |
| R8 | Sony EVI‑H100S/H100V Technical Manual | https://www.sony.com/electronics/support/res/manuals/AE4U/AE4U1001M.pdf | Bright Direct `04 4D`, Gamma `04 5B`, digital zoom inquiry, fixed `04 5A` automatic slow-shutter commands, 240 ms post-preset caveat. |
| R9 | Sony EVI‑H100S support/manuals page | https://www.sony.com.au/electronics/support/network-camera-systems-ptz-cameras/evi-h100s/manuals | Official support page that links the Technical Manual. |
| R11 | Sony BRC-H900 VISCA Command List | https://pro.sony/s3/cms-static-content/uploadfile/59/1237493025759.pdf | BRC-H900 fixed `04 3A` spotlight commands; it does not establish the fixed `04 5A` automatic-slow-shutter family. |
| R12 | Sony BRC-300 Technical Manual | https://www.sony.jp/aii/contents/smojsdmk/b2b_index/manual_pdf/remote_camera/AC1Y100131.pdf | BRC-300 fixed `04 5A` automatic-slow-shutter commands; its five-nibble signed pan/four-nibble signed tilt position commands, limits, inquiries, and endpoints; it does not establish the fixed `04 3A` spotlight family. |

## C.4 Supplemental explanatory source

| Ref | Source | Link | Used for |
|---|---|---|---|
| R13 | Jon Skeet, “Variations in the VISCA protocol” | https://codeblog.jonskeet.uk/2023/11/25/variations-in-the-visca-protocol/ | Secondary explanatory context for raw vs encapsulated VISCA-over-IP differences. Not used as a primary authority where manufacturer manuals exist. |

## C.5 Uploaded source bundle

| Ref | Uploaded file | Used for |
|---|---|---|
| U0 | `visca_reference.md` | Original final consolidated draft used as the structural base for this completed reference. |
| U1 | `PTZOptics-G2-VISCA-over-IP-Command-List(2).md` | Patched PTZOptics command reference; complete PTZOptics command/inquiry/block-inquiry rows; Pelco source tables; hardware-test notes for Gamma, Luminance/Contrast width, Bright Direct erratum, and raw transport. |
| U2 | `PTZOptics-NDI-HX-Gen2-Specs(1).md` | Consolidated PT12X/PT20X/PT30X specs, streaming tables, interface details, dimensions, package contents, and cross-document PTZOptics validation notes. |
| U3 | `axis_visca-interface-api-description(2).md` | Patched Axis API text including corrected absolute zoom command, Axis-only ranges, focus-near-limit table, red/blue gain, exposure/shutter rows, shutter-speed table, and Axis inquiries. |
| U4 | `visca_unified_reference(2).md` | Broad unified guide; used only as background except where cross-validated. Non-PTZOptics/non-Axis sections remain unvalidated unless supported by primary sources. |
