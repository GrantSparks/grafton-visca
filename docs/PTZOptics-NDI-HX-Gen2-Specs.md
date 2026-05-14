# PTZOptics NDI®|HX Gen‑2 (PT12X / PT20X / PT30X) — Specs + Control + Network Reference

**Patch date:** 2026-02-10

This document consolidates (and cross‑checks) the key technical data found in the provided **PT12X/PT20X/PT30X NDI®|HX user manuals** and **datasheets**.

## Source documents used

- PT12X: PT12X-NDI-xx User Manual.pdf (Rev 1.5 8/20); PT12X-NDI-xx Data Sheet.pdf (2 pages)
- PT20X: PT20X-NDI-xx User Manual.pdf (Rev 1.6 8/20); PT20X-NDI-xx Data Sheet.pdf (3 pages)
- PT30X: PT30X-NDI-xx User Manual.pdf (Rev 1.6 8/20); PT30X-NDI-xx Data Sheet.pdf (2 pages)

## Quick compare

| Model | Optical zoom | Lens / focal length | HFOV (tele → wide) | VFOV (tele → wide) | Min lux | Weight | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| PT12X-NDI | 12x | f3.5mm ~ 42.3mm, F1.8 ~ F2.8 | 6.9° (tele) ~ 72.5° (wide) | 3.9° (tele) ~ 44.8° (wide) | 0.05 Lux (@F1.8, AGC ON) | 3.20 lbs (1.45 kg) | Datasheet notes 720p‑120 is available only over NDI|HX / IP streaming.; Manual/datasheet include BETA note for broadcast frame rates (59.94/29.97). |
| PT20X-NDI | 20x | f4.42mm ~ 88.5mm, F1.8 ~ F2.8 | 3.36° (tele) ~ 60.7° (wide) | 1.89° (tele) ~ 34.1° (wide) | 0.05 Lux (@F1.8, AGC ON) | 3.00 lbs (1.36 kg) | Datasheet includes a boxed/package dimension line; the printed metric line is truncated in the provided PDF export (inch dims are intact).; Manual shows max height in mm as 168mm but also states 7.8" max height; 7.8" ≈ 198mm, matching the datasheet. |
| PT30X-NDI | 30x | f4.42mm ~ 132.6mm, F1.8 ~ F2.8 (datasheet); user manual table shows 88.5mm max, which appears to be a copy/paste error | 2.28° (tele) ~ 60.7° (wide) | 1.28° (tele) ~ 34.1° (wide) | 0.05 Lux (@F1.8, AGC ON) | 3.05 lbs (1.39 kg) | Limitation note appears in both datasheet and manual: cannot perform 1080p60 over IP stream & SDI/HDMI simultaneously.; User manual 'Lens' max focal length is inconsistent with 30x zoom and datasheet; datasheet value (132.6mm) aligns with the listed 2.28° tele HFOV. |


## Common capabilities (all 3 models)

### Video outputs and interfaces

- **Simultaneous outputs:** NDI®|HX + HDMI + 3G‑SDI (and IP streaming) are called out across datasheets; all list **CVBS** as SD output.
- **HDMI:** Version 1.3
- **3G‑SDI:** BNC, 800mVp‑p, 75Ω, SMPTE 424M
- **Network:** RJ‑45 10/100/1000 Ethernet
- **Audio input:** 3.5mm Line‑In (unbalanced stereo) — used for **NDI/HDMI/IP stream**
- **USB:** USB 2.0 Type‑A (manual notes “future use” on datasheets)

### IP streaming (manual spec tables)

| Item | Details |
|---|---|
| Video compression | NDI®|HX / H.264 / H.265 / M‑JPEG |
| IP video streams | Two (2) IP video output streams available |
| First stream resolutions | 1920x1080, 1280x720, 1024x576, 960x540, 640x480, 640x360 |
| Second stream resolutions | 1280x720, 1024x576, 720x480, 720x408, 640x360, 480x270, 320x240, 320x180 |
| Video bitrate | 32 Kbps ~ 102400 Kbps |
| Bitrate type | Variable Rate, Fixed Rate |
| Frame rate | 50Hz: 1 FPS ~ 50 FPS; 60Hz: 1 FPS ~ 60 FPS |
| Audio compression | AAC |
| Audio bitrate | 96 Kbps, 128 Kbps, 256 Kbps |
| Supported protocols | TCP/IP, HTTP, RTSP, RTMP, DHCP, Multicast, etc. |


### PTZ mechanics and presets

- **Pan range:** ±170°
- **Tilt range:** −30° to +90°
- **Pan speed range:** 1.7°/s ~ 100°/s
- **Tilt speed range:** 1.7°/s ~ 69.9°/s
- **Presets:** 255 via serial/IP; IR remote supports 10 presets (0–9)
- **Preset accuracy:** 0.1°

### Capability boundaries

- The manual/datasheet capability tables do not list camera one-push focus or
  PTZ Motion Sync as supported Gen-2 model capabilities.
- The VISCA command-list reference may contain raw opcodes for firmware-specific
  commands. Treat those as raw command references unless the model capability
  specs above explicitly establish support.

### Power and environment

- **Input voltage:** DC 12V (10.8–13.0V) or PoE (802.3af)
- **Max power:** 12W
- **Max current:** 1.0A
- **Operating temperature:** −10°C to 40°C (14°F to 104°F)
- **Storage temperature:** −40°C to 60°C (−40°F to 140°F)
- **Operating humidity:** 10% to 80%
- **MTBF:** >30000h

> **Note:** PT12X datasheet explicitly warns **not** to power via **DC and PoE simultaneously**.

## Network ports (defaults) and control methods

### Default ports (from PT12X manual web interface description)

| Item | Details |
|---|---|
| HTTP port | 80 (HTTP‑CGI control + web UI) |
| RTSP port | 554 |
| PTZ port (TCP) | 5678 (TCP/IP control protocol) |
| UDP port | 1259 (UDP control protocol) |
| Sony VISCA port | Fixed / not user‑changeable (port number not stated in manual) |
| SRT port | 4578 |
| SRT encryption | Off, AES‑128, AES‑192, AES‑256 |
| SRT password default | 1234567891 |


### Supported control methods

- IR remote control
- RS‑232 (VISCA / Pelco‑D / Pelco‑P)
- RS‑485 (VISCA / Pelco‑D / Pelco‑P)
- TCP control server (default port 5678)
- UDP control server (default port 1259)
- HTTP‑CGI control endpoints (e.g., `/cgi-bin/ptzctrl.cgi?ptzcmd&...`)
- NDI® control (NDI®|HX)

### HTTP‑CGI examples (from manual)

- **Pan/Tilt:** `http://<camera_ip>/cgi-bin/ptzctrl.cgi?ptzcmd&<ACTION>&<PAN_SPEED>&<TILT_SPEED>`
  - ACTION: `UP`, `DOWN`, `LEFT`, `RIGHT`, `LEFTUP`, `RIGHTUP`, `LEFTDOWN`, `RIGHTDOWN`, `PTZSTOP`
  - Pan speed: 1–24, Tilt speed: 1–20

- **Zoom:** `http://<camera_ip>/cgi-bin/ptzctrl.cgi?ptzcmd&<ACTION>&<ZOOM_SPEED>`
  - ACTION: `ZOOMIN`, `ZOOMOUT`, `ZOOMSTOP`
  - Zoom speed: 1–7

## Per‑model details

### PT12X-NDI

**Models:** PT12X-NDI-GY (Gray), PT12X-NDI-WH (White)

**Datasheet:** PT12X-NDI-xx Data Sheet.pdf (2 pages)
**User manual:** PT12X-NDI-xx User Manual.pdf (Rev 1.5 8/20)

#### Camera (manual spec table)

| Item | Value |
|---|---|
| Type | PTZOptics NDI®|HX HD 1080p Color Video Camera |
| Video system | 1080p‑60/50/30/25/59.94*/29.97*, 1080i‑60/50/59.94*, 720p‑60/50/59.94*; CVBS: 576i, 480i. (*Broadcast frame rates are BETA.) |
| Sensor | 1/2.7" CMOS, effective pixel 2.07M |
| Scanning mode | Progressive |
| White balance | Auto, Indoor, Outdoor, One Push, Manual, VAR |
| Backlight compensation | Support |
| Digital noise reduction | 2D & 3D Digital Noise Reduction |
| Image flip | Support |
| Image mirror | Support |
| Image freeze | Support |
| PoE | Support (802.3af) |
| Face detection | Not Supported |
| Local storage | Not Supported |
| Lens | 12x, f3.5mm ~ 42.3mm, F1.8 ~ F2.8 |
| Digital zoom | 16x |
| Minimal illumination | 0.05 Lux (@F1.8, AGC ON) |
| Shutter | 1/30s ~ 1/10000s |
| Video S/N | ≥55 dB |
| Horizontal angle of view | 6.9° (tele) ~ 72.5° (wide) |
| Vertical angle of view | 3.9° (tele) ~ 44.8° (wide) |
| Horizontal rotation range | ±170° |
| Vertical rotation range | ‑30° ~ +90° (Up 90°, Down 30° in datasheet wording) |
| Pan speed range | 1.7° ~ 100°/s |
| Tilt speed range | 1.7° ~ 69.9°/s |
| Number of preset | 255 |
| Preset accuracy | 0.1° |


#### Input/Output interface (manual spec table)

| Item | Value |
|---|---|
| HD output (HDMI) | 1 × HDMI, Version 1.3 |
| HD output (3G‑SDI) | 1 × 3G‑SDI (BNC), 800mVp‑p, 75Ω, SMPTE 424M |
| HD output (IP/NDI) | 1 × RJ45 NDI®|HX / IP Network streaming, 10/100/1000 Ethernet |
| SD output | 1 × CVBS (RCA), 1Vp‑p, 75Ω |
| Network interface | 1 × RJ45 10/100/1000M adaptive Ethernet ports |
| Audio input | 1‑ch 3.5mm Line‑In (unbalanced stereo) (manual: NDI®|HX & IP Network stream only) |
| USB | 1 × USB 2.0 Type‑A |
| RS‑232 IN | 8‑pin Mini‑DIN, max distance 30m, protocols: VISCA / Pelco‑D / Pelco‑P |
| RS‑232 OUT | 8‑pin Mini‑DIN, max distance 30m, protocol: VISCA network use only |
| RS‑485 | 2‑pin Phoenix port, max distance 1200m, protocols: VISCA / Pelco‑D / Pelco‑P |
| Power jack | JEITA type (DC IN 12V) |


#### Generic specification (manual spec table)

| Item | Value |
|---|---|
| Input voltage | DC 12V / PoE (802.3af) (optional) |
| Current consumption | 1.0A (Max) |
| Operating temperature | -10°C ~ 40°C (14°F ~ 104°F) |
| Storage temperature | -40°C ~ 60°C (-40°F ~ 140°F) |
| Operating humidity | 10% ~ 80% |
| Power consumption | 12W (Max) |
| MTBF | >30000h |
| Size (in) (W×D×H) | 5.6" W x 6.7" D x 6.5" H (manual also lists 7.88" max height w/ tilt) |
| Size (mm) (W×D×H) | 142mm W x 169mm D x 164mm H (manual lists 189mm max height w/ tilt) |
| Camera weight | 3.20 lbs (1.45 kg) |
| Box weight | 5.4 lbs (2.45 kg) |



**Datasheet notes (extras / deltas vs manual):**
- Datasheet additionally lists 720p‑120** and 720p‑30/25, and repeats the BETA note for 59.94/29.97. (**720p‑120 only over NDI|HX / IP streaming.)
- Datasheet notes 720p‑120 is available only over NDI|HX / IP streaming.
- Manual/datasheet include BETA note for broadcast frame rates (59.94/29.97).
- Manual vs datasheet dimension conversions are inconsistent for the 'max height w/ tilt' value; see validation notes.

**What’s in the box (datasheet):**
- 12X Zoom NDI|HX Camera
- Power Adapter + Cord
- IR Remote Control
- RS‑232C Cable
- Quick Start Guide
- (2) AAA Batteries


---

### PT20X-NDI

**Models:** PT20X-NDI-GY (Gray), PT20X-NDI-WH (White)

**Datasheet:** PT20X-NDI-xx Data Sheet.pdf (3 pages)
**User manual:** PT20X-NDI-xx User Manual.pdf (Rev 1.6 8/20)

#### Camera (manual spec table)

| Item | Value |
|---|---|
| Type | PTZOptics NDI®|HX HD 1080p Color Video Camera |
| Video system | 1080p‑60/50/30/25/59.94*/29.97*, 1080i‑60/50/59.94*, 720p‑60/50/59.94*; CVBS: 576i, 480i. (*Broadcast frame rates are BETA.) |
| Sensor | 1/2.7" CMOS, effective pixel 2.07M |
| Scanning mode | Progressive |
| White balance | Auto, Indoor, Outdoor, One Push, Manual, VAR |
| Backlight compensation | Support |
| Digital noise reduction | 2D & 3D Digital Noise Reduction |
| Image flip | Support |
| Image mirror | Support |
| Image freeze | Support |
| PoE | Support (802.3af) |
| Face detection | Not Supported |
| Local storage | Not Supported |
| Lens | 20x, f4.42mm ~ 88.5mm, F1.8 ~ F2.8 |
| Digital zoom | 16x |
| Minimal illumination | 0.05 Lux (@F1.8, AGC ON) |
| Shutter | 1/30s ~ 1/10000s |
| Video S/N | ≥55 dB |
| Horizontal angle of view | 3.36° (tele) ~ 60.7° (wide) |
| Vertical angle of view | 1.89° (tele) ~ 34.1° (wide) |
| Horizontal rotation range | ±170° |
| Vertical rotation range | ‑30° ~ +90° |
| Pan speed range | 1.7° ~ 100°/s |
| Tilt speed range | 1.7° ~ 69.9°/s |
| Number of preset | 255 |
| Preset accuracy | 0.1° |


#### Input/Output interface (manual spec table)

| Item | Value |
|---|---|
| HD output (HDMI) | 1 × HDMI, Version 1.3 |
| HD output (3G‑SDI) | 1 × 3G‑SDI (BNC), 800mVp‑p, 75Ω, SMPTE 424M |
| HD output (IP/NDI) | 1 × RJ45 NDI®|HX / IP Network streaming, 10/100/1000 Ethernet |
| SD output | 1 × CVBS (RCA), 1Vp‑p, 75Ω |
| Network interface | 1 × RJ45 10/100/1000M adaptive Ethernet ports |
| Audio input | 1‑ch 3.5mm Line‑In (unbalanced stereo) (manual: NDI®|HX & IP Network stream only) |
| USB | 1 × USB 2.0 Type‑A |
| RS‑232 IN | 8‑pin Mini‑DIN, max distance 30m, protocols: VISCA / Pelco‑D / Pelco‑P |
| RS‑232 OUT | 8‑pin Mini‑DIN, max distance 30m, protocol: VISCA network use only |
| RS‑485 | 2‑pin Phoenix port, max distance 1200m, protocols: VISCA / Pelco‑D / Pelco‑P |
| Power jack | JEITA type (DC IN 12V) |


#### Generic specification (manual spec table)

| Item | Value |
|---|---|
| Input voltage | DC 12V / PoE (802.3af) (optional) |
| Current consumption | 1.0A (Max) |
| Operating temperature | -10°C ~ 40°C (14°F ~ 104°F) |
| Storage temperature | -40°C ~ 60°C (-40°F ~ 140°F) |
| Operating humidity | 10% ~ 80% |
| Power consumption | 12W (Max) |
| MTBF | >30000h |
| Size (in) (W×D×H) | 5.6" W x 6.7" D x 6.5" H (7.8" H w/ max tilt per manual/datasheet) |
| Size (mm) (W×D×H) | 142mm W x 169mm D x 164mm H (datasheet lists ~198mm max height w/ tilt; manual shows 168mm which does not match 7.8") |
| Camera weight | 3.00 lbs (1.36 kg) |
| Box weight | 5.4 lbs (2.45 kg) |



**Datasheet notes (extras / deltas vs manual):**
- Datasheet lists 720p frame rates as 60/50/30/25 (no 59.94), and repeats 1080p/1080i sets.
- Datasheet includes a boxed/package dimension line; the printed metric line is truncated in the provided PDF export (inch dims are intact).
- Manual shows max height in mm as 168mm but also states 7.8" max height; 7.8" ≈ 198mm, matching the datasheet.

**What’s in the box (datasheet):**
- 20X Zoom NDI|HX Camera
- Power Adapter + Cord
- IR Remote Control
- RS‑232C Cable
- Quick Start Guide
- (2) AAA Batteries


---

### PT30X-NDI

**Models:** PT30X-NDI-GY (Gray), PT30X-NDI-WH (White)

**Datasheet:** PT30X-NDI-xx Data Sheet.pdf (2 pages)
**User manual:** PT30X-NDI-xx User Manual.pdf (Rev 1.6 8/20)

#### Camera (manual spec table)

| Item | Value |
|---|---|
| Type | PTZOptics NDI®|HX HD 1080p Color Video Camera |
| Video system | 1080p‑60/50/30/25/59.94*/29.97*, 1080i‑60/50/59.94*, 720p‑60/50/59.94*; CVBS: 576i, 480i. (*Broadcast frame rates are BETA.) |
| Sensor | 1/2.7" CMOS, effective pixel 2.07M |
| Scanning mode | Progressive |
| White balance | Auto, Indoor, Outdoor, One Push, Manual, VAR |
| Backlight compensation | Support |
| Digital noise reduction | 2D & 3D Digital Noise Reduction |
| Image flip | Support |
| Image mirror | Support |
| Image freeze | Support |
| PoE | Support (802.3af) |
| Face detection | Not Supported |
| Local storage | Not Supported |
| Lens | 30x, f4.42mm ~ 132.6mm, F1.8 ~ F2.8 (datasheet); user manual table shows 88.5mm max, which appears to be a copy/paste error |
| Digital zoom | 16x |
| Minimal illumination | 0.05 Lux (@F1.8, AGC ON) |
| Shutter | 1/30s ~ 1/10000s |
| Video S/N | ≥55 dB |
| Horizontal angle of view | 2.28° (tele) ~ 60.7° (wide) |
| Vertical angle of view | 1.28° (tele) ~ 34.1° (wide) |
| Horizontal rotation range | ±170° |
| Vertical rotation range | ‑30° ~ +90° |
| Pan speed range | 1.7° ~ 100°/s |
| Tilt speed range | 1.7° ~ 69.9°/s |
| Number of preset | 255 |
| Preset accuracy | 0.1° |


#### Input/Output interface (manual spec table)

| Item | Value |
|---|---|
| HD output (HDMI) | 1 × HDMI, Version 1.3 |
| HD output (3G‑SDI) | 1 × 3G‑SDI (BNC), 800mVp‑p, 75Ω, SMPTE 424M |
| HD output (IP/NDI) | 1 × RJ45 NDI®|HX / IP Network streaming, 10/100/1000 Ethernet |
| SD output | 1 × CVBS (RCA), 1Vp‑p, 75Ω |
| Network interface | 1 × RJ45 10/100/1000M adaptive Ethernet ports |
| Audio input | 1‑ch 3.5mm Line‑In (unbalanced stereo) (manual: NDI®|HX & IP Network stream only) |
| USB | 1 × USB 2.0 Type‑A |
| RS‑232 IN | 8‑pin Mini‑DIN, max distance 30m, protocols: VISCA / Pelco‑D / Pelco‑P |
| RS‑232 OUT | 8‑pin Mini‑DIN, max distance 30m, protocol: VISCA network use only |
| RS‑485 | 2‑pin Phoenix port, max distance 1200m, protocols: VISCA / Pelco‑D / Pelco‑P |
| Power jack | JEITA type (DC IN 12V) |


#### Generic specification (manual spec table)

| Item | Value |
|---|---|
| Input voltage | DC 12V / PoE (802.3af) (optional) |
| Current consumption | 1.0A (Max) |
| Operating temperature | -10°C ~ 40°C (14°F ~ 104°F) |
| Storage temperature | -40°C ~ 60°C (-40°F ~ 140°F) |
| Operating humidity | 10% ~ 80% |
| Power consumption | 12W (Max) |
| MTBF | >30000h |
| Size (in) (W×D×H) | 5.6" W x 6.7" D x 6.5" H (7.8" H w/ max tilt per manual/datasheet) |
| Size (mm) (W×D×H) | 142mm W x 169mm D x 164mm H (datasheet lists 198mm max height w/ tilt; manual shows 168mm which does not match 7.8") |
| Camera weight | 3.05 lbs (1.39 kg) |
| Box weight | 5.4 lbs (2.45 kg) |



**Datasheet notes (extras / deltas vs manual):**
- Datasheet lists 720p frame rates as 60/50/30/25, and includes a limitation note about 1080p60 over IP + SDI/HDMI simultaneously.
- Limitation note appears in both datasheet and manual: cannot perform 1080p60 over IP stream & SDI/HDMI simultaneously.
- User manual 'Lens' max focal length is inconsistent with 30x zoom and datasheet; datasheet value (132.6mm) aligns with the listed 2.28° tele HFOV.

**What’s in the box (datasheet):**
- 30X Zoom NDI|HX Camera
- Power Adapter + Cord
- IR Remote Control
- RS‑232C Cable
- Quick Start Guide
- (2) AAA Batteries


## Validation notes (cross‑document mismatches)

- **PT30X lens max focal length:** PT30X manual table lists `f4.42mm ~ 88.5mm` while the PT30X datasheet lists `f4.42mm ~ 132.6mm`. Given the model is explicitly 30× and lists a narrower tele HFOV (2.28°), the **datasheet focal length** is the consistent value.
- **Max height “with tilt” unit conversion:** PT20X/PT30X manuals show `7.8" H w/ max tilt` but list `168mm H w/ max tilt`, which does not match the inch value. Both PT20X/PT30X datasheets show `198mm` max height, which matches `7.8"`.
- **PT12X dimensions (datasheet vs manual):** PT12X datasheet mixes an inch value (`7.88"`) with mm dimensions that correspond to `6.7"`. The PT12X manual includes detailed dimensional drawings in mm; treat those drawings and the base dimensions (`142×169×164mm`) as authoritative for mechanical fit.
- **720p‑120 on PT12X:** PT12X datasheet calls out `720p‑120` availability over NDI|HX/IP streaming, but PT12X manual’s video system table does not mention `720p‑120`. Treat this as a **feature that may be firmware/stream‑path dependent** (per the datasheet note).
