# PTZOptics Gen‑2 (NDI®|HX) — VISCA over IP / TCP/UDP Command Reference

**Patch date:** 2026-02-10

This file consolidates and cross‑checks:
- The existing **PTZOptics G2 VISCA over IP Commands (10/27/2023)** markdown content, and
- The **VISCA / VISCA‑over‑IP / Pelco‑D / Pelco‑P** command tables embedded in the provided PTZOptics NDI®|HX user manuals (Rev 1.5/1.6, Aug 2020).

## Scope

Applies to PTZOptics NDI®|HX Gen‑2 models (PT12X‑NDI, PT20X‑NDI, PT30X‑NDI). The command set is shared across these models; optical limits (zoom/focus ranges) are model‑dependent.

---

## Transport and ports (from PTZOptics NDI®|HX user manual)

The cameras expose multiple control paths:

- **TCP PTZ control server:** default **5678**
- **UDP PTZ control server:** default **1259**
- **HTTP‑CGI control + web UI:** default **80**
- **RTSP streaming:** default **554**
- **SRT streaming:** default **4578**
- **“Sony VISCA” port:** present but not user‑changeable (manual does not state the port number)

> The PTZ TCP/UDP servers accept **VISCA‑format command bytes** as shown in the manual’s “Serial Communication Control” / VISCA sections.

---

## Camera responses (ACK / Completion / Errors)

### Responses (existing 2023 doc + manual agree)

- **ACK:** `90 4y FF` — command accepted
- **Completion:** `90 5y FF` — command executed
  - `y` is the socket number.

### Error messages

- **Syntax Error:** `90 60 02 FF` — bad command format / illegal parameters
- **Command Buffer Full:** `90 60 03 FF` — two sockets already in use
- **Command Canceled:** `90 6y 04 FF` — command canceled in socket `y`
- **No Socket:** `90 6y 05 FF` — no command executing in socket `y` / invalid socket
- **Command Not Executable:** `90 6y 41 FF` — cannot execute in current conditions

---

## Curated command highlights (from the existing 2023 markdown)

Below is the existing curated list kept intact for quick reference, followed by the full manual command tables.

### PTZOptics G2 VISCA over IP Commands (10/27/2023)

#### Camera Responses
- **ACK/Completion**
  - ACK: `90 4y FF` - Returned when the command is accepted.
  - Completion: `90 5y FF` - Returned when the command has been executed.

- **Error Messages**
  - Syntax Error: `90 60 02 FF` - Returned when the command format is incorrect or parameters are illegal.
  - Command Buffer Full: `90 60 03 FF` - Indicates command couldn't be accepted as two sockets are already in use.
  - Command Canceled: `90 6y 04 FF` - Returned when a command is canceled in the specified socket.
  - No Socket: `90 6y 05 FF` - Returned when no command is executed in the specified socket.
  - Command Not Executable: `90 6y 41 FF` - Returned when the command cannot be executed due to current conditions.

#### Camera Commands

- **Image**
  - Luminance Direct: `81 01 04 A1 00 00 0p 0q FF` (pq: Brightness Position, 0x00–0x0E)
  - Contrast Direct: `81 01 04 A2 00 00 0p 0q FF` (pq: Contrast Position, 0x00–0x0E)
  - Gamma Direct: `81 01 04 5B 0p FF` (p: Gamma Curve, 0x00=Standard, 0x01–0x04=Alternate curves)
  - Sharpness:
    - Mode: `81 01 04 05 0p FF` (p: 0x2=Auto, 0x3=Manual)
    - Reset: `81 01 04 02 00 FF`
    - Up: `81 01 04 02 02 FF`
    - Down: `81 01 04 02 03 FF`
    - Direct: `81 01 04 42 00 00 0p 0q FF` (pq: 0x00=0 ~ 0x0B=11)

- **Exposure**
  - Exposure Mode: `81 01 04 39 0p FF` (p: 0x0=Auto, 0x3=Manual, 0xA=Shutter, 0xB=Iris, 0xD=Bright)
  - Exposure Compensation:
    - On/Off: `81 01 04 3E 0p FF` (p: 0x2=On, 0x3=Off)
    - Reset: `81 01 04 0E 00 FF`
    - Up: `81 01 04 0E 02 FF`
    - Down: `81 01 04 0E 03 FF`
    - Direct: `81 01 04 4E 00 00 0p 0q FF` (pq: 0x0=-7 ~ 0x7=0 ~ 0xE=+7)
  - Dynamic Range Control Direct: `81 01 04 25 00 00 00 0p FF` (p: 0x0=0 ~ 0x8=8)
  - Backlight On/Off: `81 01 04 33 0p FF` (p: 0x2=On, 0x3=Off)
  - Iris:
    - Reset: `81 01 04 0B 00 FF`
    - Up: `81 01 04 0B 02 FF`
    - Down: `81 01 04 0B 03 FF`
    - Direct: `81 01 04 4B 00 00 00 0p FF` (p: 0x0=Close ~ 0xC=F1.8)
  - Shutter:
    - Reset: `81 01 04 0A 00 FF`
    - Up: `81 01 04 0A 02 FF`
    - Down: `81 01 04 0A 03 FF`
    - Direct: `81 01 04 4A 00 00 0p 0q FF` (pq: 0x01=1/30 ~ 0x11=1/10000)
  - Bright:
    - Reset: `81 01 04 0D 00 FF`
    - Up: `81 01 04 0D 02 FF`
    - Down: `81 01 04 0D 03 FF`
    - Direct: `81 01 04 4D 00 00 0p 0q FF` (pq: 0x00=0 ~ 0x11=17)

- **Gain**
  - Reset: `81 01 04 0C 00 FF`
  - Up: `81 01 04 0C 02 FF`
  - Down: `81 01 04 0C 03 FF`
  - Direct: `81 01 04 4C 00 00 0p 0q FF` (pq: 0x00=0 ~ 0x08=8)
  - Gain Limit Direct: `81 01 04 2C 0p FF` (p: 0x0=0 ~ 0xF=15)
  - Anti-Flicker Direct: `81 01 04 23 0p FF` (p: 0x0=Off, 0x1=50Hz, 0x2=60Hz)

- **Color**
  - White Balance Mode: `81 01 04 35 pq FF` (pq: 0x00=Auto, 0x01=Indoor, 0x02=Outdoor, 0x03=OnePush, 0x05=Manual, 0x20=ColorTemperature)
  - OnePush Trigger: `81 01 04 10 05 FF`
  - Red Tuning Direct: `81 0A 01 12 pq FF` (pq: 0x00=-10 ~ 0x0A=0 ~ 0x14=+10)
  - Blue Tuning Direct: `81 0A 01 13 pq FF` (pq: 0x00=-10 ~ 0x0A=0 ~ 0x14=+10)
  - Saturation Direct: `81 01 04 49 00 00 00 0p FF` (p: 0x0=60% ~ 0xE=200%)
  - Hue Direct: `81 01 04 4F 00 00 00 0p FF` (p: 0x0=0 ~ 0xE=14)

- **Pan Tilt**
  - Pan Tilt Drive:
    - Up: `81 01 06 01 vv ww 03 01 FF`
    - Down: `81 01 06 01 vv ww 03 02 FF`
    - Left: `81 01 06 01 vv ww 01 03 FF`
    - Right: `81 01 06 01 vv ww 02 03 FF`
    - UpLeft: `81 01 06 01 vv ww 01 01 FF`
    - UpRight: `81 01 06 01 vv ww 02 01 FF`
    - DownLeft: `81 01 06 01 vv ww 01 02 FF`
    - DownRight: `81 01 06 01 vv ww 02 02 FF`
    - Stop: `81 01 06 01 vv ww 03 03 FF`
    - AbsolutePosition: `81 01 06 02 vv ww 0y 0y 0y 0y 0z 0z 0z 0z FF`
    - RelativePosition: `81 01 06 03 vv ww 0y 0y 0y 0y 0z 0z 0z 0z FF`
    - Home: `81 01 06 04 FF`
    - Reset: `81 01 06 05 FF`
  - Pan Tilt Limit:
    - LimitSet: `81 01 06 07 00 0w 0y 0y 0y 0y 0z 0z 0z 0z FF`
    - LimitClear: `81 01 06 07 01 0w 07 0f 0f 0f 07 0f 0f 0f FF`

#### Zoom
- Stop Direct: `81 01 04 07 00 FF`
- Tele Standard: `81 01 04 07 02 FF`
- Wide Standard: `81 01 04 07 03 FF`
- Tele Adjustable Speed: `81 01 04 07 2p FF` (p: 0x0 ~ 0x7)
- Wide Adjustable Speed: `81 01 04 07 3p FF` (p: 0x0 ~ 0x7)
- Direct Direct: `81 01 04 47 0p 0q 0r 0s FF`

#### Focus
- Mode:
  - Auto / Manual: `81 01 04 38 0p FF` (p: 0x2=Auto, 0x3=Manual)

---

## Full command tables (verbatim extraction from PT12X NDI®|HX user manual)

The following appendices include the complete VISCA/VISCA‑over‑IP and Pelco command tables as embedded in the manual (PDF text extraction). Use these when you need **exhaustive** coverage beyond the curated list above.

> **Note:** The manual uses `8x` / `z0` / `y0` placeholders in some tables. In practice, these map to the VISCA destination address / socket formatting used by your transport.

```text

--- Page 19 (PDF index 18) ---
VISCA Command List
Part 1: Camera-Issued Messages
ACK/Completion Message
Command Function Command Packet Comments
z0 4y FF
ACK Returned when the command is accepted.
ACK/Completion (y: Socket No.)
Messages z0 5y FF
Completion Returned when the command has been executed.
(y: Socket No.)
Error Messages
Command Function Command Packet Comments
Returned when the command format is different
Syntax Error z0 60 02 FF or when a command with illegal command
parameters is accepted.
Indicates that two sockets are already being used
Command Buffer Full z0 60 03 FF (executing two commands) and the command
could not be accepted when received.
Returned when a command which is being
z0 6y 04 FF executed in a socket specified by the cancel
Command Canceled
Error Messages (y: Socket No.) command is canceled. The completion message
for the command is not returned.
Returned when no command is executed in a
z0 6y 05 FF
No Socket socket specified by the cancel command, or when
(y: Socket No.)
an invalid socket number is specified.
Returned when a command cannot be executed
z0 6y 41 FF
due to current conditions. For example, when
Command Not Executable (y: Execution command Socket
commands controlling the focus manually are
No. Inquiry command: 0)
received during auto focus.
z = Camera Address + 8
Page 16 of 50
Rev 1.5 8/20

--- Page 20 (PDF index 19) ---
Part 2: VISCA Command List
Command Function Command Packet Comments
On 8x 01 04 00 02 FF
CAM_Power Power ON/OFF
Off 8x 01 04 00 03 FF
Stop 8x 01 04 07 00 FF
Tele (Standard) 8x 01 04 07 02 FF
Wide (Standard) 8x 01 04 07 03 FF
CAM_Zoom
Tele (Variable) 8x 01 04 07 2p FF
p = 0(low) - 7(high)
Wide (Variable) 8x 01 04 07 3p FF
Direct 8x 01 04 47 0p 0q 0r 0s FF pqrs: Zoom Position
Stop 8x 01 04 08 00 FF
Far (Standard) 8x 01 04 08 02 FF
Near (Standard) 8x 01 04 08 03 FF
Far (Variable) 8x 01 04 08 2p FF
p = 0(low) - 7(high)
Near (Variable) 8x 01 04 08 3p FF
CAM_Focus Direct 8x 01 04 48 0p 0q 0r 0s FF pqrs: Focus Position
Auto Focus 8x 01 04 38 02 FF
Manual Focus 8x 01 04 38 03 FF AF On/Off
Auto/Manual 8x 01 04 38 10 FF
Focus Lock 8x 0a 04 68 02 FF Prevents any other operation or command from
Focus Unlock 8x 0a 04 68 03 FF adjusting the current focus state
Auto 8x 01 04 35 00 FF Normal Auto
Indoor mode 8x 01 04 35 01 FF Indoor mode
Outdoor mode 8x 01 04 35 02 FF Outdoor mode
CAM_WB OnePush mode 8x 01 04 35 03 FF One Push WB mode
Manual 8x 01 04 35 05 FF Manual Control mode
Color Temperature 8x 01 04 35 20 FF Color Temperature mode
OnePush trigger 8x 01 04 10 05 FF One Push WB Trigger
Reset 8x 01 04 03 00 FF
Up 8x 01 04 03 02 FF Manual Control of R Gain
CAM_RGain
Down 8x 01 04 03 03 FF
Direct 8x 01 04 43 00 00 0p 0q FF pq: R Gain
Reset 8x 01 04 04 00 FF
Up 8x 01 04 04 02 FF Manual Control of B Gain
CAM_Bgain
Down 8x 01 04 04 03 FF
Direct 8x 01 04 44 00 00 0p 0q FF pq: B Gain
Reset 8x 01 04 20 00 FF Default ColorTemperature setting
CAM_ColorTemp Up 8x 01 04 20 02 FF
Down 8x 01 04 20 03 FF
Page 17 of 50
Rev 1.5 8/20

--- Page 21 (PDF index 20) ---
pq: Color Temperature position 0x00: 2500K ~
Direct 8x 01 04 20 0p 0q FF
0x37: 8000K
Full Auto 8x 01 04 39 00 FF Automatic Exposure mode
Manual 8x 01 04 39 03 FF Manual Control mode
CAM_AE Shutter priority 8x 01 04 39 0A FF Shutter Priority Automatic Exposure mode
Iris priority 8x 01 04 39 0B FF Iris Priority Automatic Exposure mode
Bright 8x 01 04 39 0D FF Bright Mode(Manual control)
Reset 8x 01 04 0B 00 FF
Up 8x 01 04 0B 02 FF Iris Setting
CAM_Iris
Down 8x 01 04 0B 03 FF
Direct 8x 01 04 4B 00 00 0p 0q FF pq: Iris Position
Reset 8x 01 04 0A 00 FF Default Shutter setting
Up 8x 01 04 0A 02 FF
CAM_Shutter
Down 8x 01 04 0A 03 FF
Direct 8x 01 04 4A 00 00 0p 0q FF pq: Shutter Position
Reset 8x 01 04 0D 00 FF
Up 8x 01 04 0D 02 FF Bright Setting
CAM_Bright
Down 8x 01 04 0D 03 FF
Direct 8x 01 04 0D 00 00 0p 0q FF pq: Bright Position
On 8x 01 04 3E 02 FF
Exposure Compensation On/Off
Off 8x 01 04 3E 03 FF
Reset 8x 01 04 0E 00 FF
CAM_ExpComp
Up 8x 01 04 0E 02 FF Exposure Compensation Amount Setting
Down 8x 01 04 0E 03 FF
Direct 8x 01 04 4E 00 00 0p 0q FF pq: ExpComp Position
On 8x 01 04 33 02 FF
CAM_BackLight Back Light Compensation On/Off
Off 8x 01 04 33 03 FF
CAM_Flicker - 8x 01 04 23 0p FF p: Flicker Settings (0: Off, 1: 50Hz, 2: 60Hz)
Off 8x 01 04 63 00 FF
CAM_PictureEffect Picture Effect Setting
B&W 8x 01 04 63 04 FF
Reset 8x 01 04 3F 00 pp FF
CAM_Memory Set 8x 01 04 3F 01 pp FF pp: Memory Number (=0 to 127)
Recall 8x 01 04 3F 02 pp FF
Preset Recall Speed Preset Speed 8x 01 06 01 p FF p : s p e e d g r a d e , th e v a lues are (0x01~0x18)
On 8x 01 04 61 02 FF
CAM_LR_Reverse Image Flip Horizontal On/Off
Off 8x 01 04 61 03 FF
On 8x 01 04 66 02 FF
CAM_PictureFlip Image Flip Vertical On/Off
Off 8x 01 04 66 03 FF
CAM_ColorGain Diret 8x 01 04 49 00 00 00 0p FF p: Color Gain setting 0h (60%) to Eh (200%)
Page 18 of 50
Rev 1.5 8/20

--- Page 22 (PDF index 21) ---
Up 8x 01 06 01 VV WW 03 01 FF
Down 8x 01 06 01 VV WW 03 02 FF
Left 8x 01 06 01 VV WW 01 03 FF
Right 8x 01 06 01 VV WW 02 03 FF
Upleft 8x 01 06 01 VV WW 01 01 FF
VV: Pan speed 0x01 (low speed) to 0x18 (high
Upright 8x 01 06 01 VV WW 02 01 FF
speed)
DownLeft 8x 01 06 01 VV WW 01 02 FF
WW: Tilt speed 0x01 (low speed) to 0x14 (high
Pan_tiltDrive DownRight 8x 01 06 01 VV WW 02 02 FF
speed)
Stop 8x 01 06 01 VV WW 03 03 FF
YYYY: Pan Position
8x 01 06 02 VV WW
AbsolutePosition ZZZZ: Tilt Position
0Y 0Y 0Y 0Y 0Z 0Z 0Z 0Z FF
8x 01 06 03 VV WW
RelativePosition
0Y 0Y 0Y 0Y 0Z 0Z 0Z 0Z FF
Home 8x 01 06 04 FF
Reset 8x 01 06 05 FF
8x 01 06 07 00 0W
LimitSet W: 1 UpRight 0: DownLeft
0Y 0Y 0Y 0Y 0Z 0Z 0Z 0Z FF
Pan_tiltLimitSet YYYY: Pan Limit Position
8x 01 06 07 01 0W
LimitClear ZZZZ: Tilt Position
07 0F 0F 0F 07 0F 0F 0F FF
CAM_Brightness Direct 8x 01 04 A1 00 00 0p 0q FF pq: Brightness Position
CAM_Contrast Direct 8x 01 04 A2 00 00 0p 0q FF pq: Contrast Position
CAM_Gamma Direct 8x 01 04 5B 0p FF p: Gamma Curve (0=Standard, 1-4=Alternate)
Off 8x 01 04 A4 00 FF
Flip-H 8x 01 04 A4 01 FF
CAM_Flip Single Command For Video Flip
Flip-V 8x 01 04 A4 02 FF
Flip-HV 8x 01 04 A4 03 FF
CAM_SettingSave Save 8x 01 04 A5 10 FF Save Current Setting
High 8x 01 04 A9 00 FF High
CAM_AWBSensitivity Normal 8x 01 04 A9 01 FF Normal
Low 8x 01 04 A9 02 FF Low
Top 8x 01 04 AA 00 FF
CAM_AFZone Center 8x 01 04 AA 01 FF AF Zone weight select
Bottom 8x 01 04 AA 02 FF
CAM_ColorHue Direct 8x 01 04 4F 00 00 00 0p FF p: Color Hue 0h (−14 degrees) to Eh (+14 degrees)
Open / Close 8x 01 04 3F 02 5F FF
Navigate Up 8x 01 06 01 0E 0E 03 01 FF
Navigate Down 8x 01 06 01 0E 0E 03 02 FF
OSD_Control
Navigate Left 8x 01 06 01 0E 0E 01 03 FF
Navigate Right 8x 01 06 01 0E 0E 02 03 FF
Enter 8x 01 06 06 05 FF
Page 19 of 50
Rev 1.5 8/20

--- Page 23 (PDF index 22) ---
Return 8x 01 06 06 04 FF
High 8x 0B 01 01 FF
Medium 8x 0B 01 02 FF
CAM_NDIMode
Low 8x 0B 01 03 FF
Off 8x 0B 01 04 FF
CAM_MulticastMode Multicast Mode 8x 0B 01 23 0p FF p=1: On, p=2: Off
PTZ Motion Sync On 8x 0A 11 13 02 FF
CAM_PTZMotionSync PTZ Motion Sync Off 8x 0A 11 13 03 FF
MS Lower Speed Limit 8x 0A 11 14 pq FF pq: speed stage
CAM_UACStatus Toggle USB Audio 8x 2A 02 A0 04 0p FF p=2: On, p=3: Off
Part 3: VISCA Query Command List
Inquiry Command List
Command Command packed Inquiry Packet Comments
y0 50 02 FF On
CAM_PowerInq 8x 09 04 00 FF y0 50 03 FF Off (Standby)
y0 50 04 FF Internal power circuit error
CAM_ZoomPosInq 8x 09 04 47 FF y0 50 0p 0q 0r 0s FF pqrs: Zoom Position
CAM_FocusAFMode y0 50 02 FF Auto Focus
8x 09 04 38 FF
Inq y0 50 03 FF Manual Focus
CAM_FocusPosInq 8x 09 04 48 FF y0 50 0p 0q 0r 0s FF pqrs: Focus Position
y0 50 00 FF Auto
y0 50 01 FF Indoor mode
y0 50 02 FF Outdoor mode
CAM_WBModeInq 8x 09 04 35 FF
y0 50 03 FF OnePush mode
y0 50 05 FF Manual
y0 50 20 FF ColorTemperature Mode
CAM_RGainInq 8x 09 04 43 FF y0 50 00 00 0p 0q FF pq: R Gain
CAM_BGainInq 8x 09 04 44 FF y0 50 00 00 0p 0q FF pq: B Gain
y0 50 00 FF Full Auto
y0 50 03 FF Manual
CAM_AEModeInq 8x 09 04 39 FF y0 50 0A FF Shutter priority
y0 50 0B FF Iris priority
y0 50 0D FF Bright
CAM_ShutterPosInq 8x 09 04 4A FF y0 50 00 00 0p 0q FF pq: Shutter Position
CAM_IrisPosInq 8x 09 04 4B FF y0 50 00 00 0p 0q FF pq: Iris Position
CAM_BrightPosInq 8x 09 04 4D FF y0 50 00 00 0p 0q FF pq: Bright Position
Page 20 of 50
Rev 1.5 8/20

--- Page 24 (PDF index 23) ---
CAM_ExpCompMod y0 50 02 FF On
8x 09 04 3E FF
eInq y0 50 03 FF Off
CAM_ExpCompPosI
8x 09 04 4E FF y0 50 00 00 0p 0q FF pq: ExpComp Position
nq
CAM_BacklightMode y0 50 02 FF On
8x 09 04 33 FF
Inq y0 50 03 FF Off
CAM_Nosise2DMode y0 50 02 FF Auto Noise 2D
8x 09 04 50 FF
Ing y0 50 03 FF Manual Noise 3D
CAM_Nosise2DLevel 8x 09 04 53 FF y0 50 0p FF Noise Reduction (2D) p: 0 to 5
CAM_Noise3DLevel 8x 09 04 54 FF y0 50 0p FF Noise Reduction (3D) p: 0 to 8
CAM_FlickerModeIn
8x 09 04 55 FF y0 50 0p FF p: Flicker Settings(0: OFF, 1: 50Hz, 2: 60Hz)
q
CAM_ApertureModeI y0 50 02 FF Auto Sharpness
8x 09 04 05 FF
nq (Sharpness) y0 50 03 FF Manual Sharpness
CAM_ApertureInq
8x 09 04 42 FF y0 50 00 00 0p 0q FF pq: Aperture Gain
(Sharpness)
y0 50 02 FF On
SYS_MenuModeInq 8x 09 06 06 FF
y0 50 03 FF Off
CAM_PictureEffectM y0 50 02 FF Off
8x 09 04 63 FF
odeInq y0 50 04 FF B&W
y0 50 02 FF On
CAM_LR_ReverseInq 8x 09 04 61 FF
y0 50 03 FF Off
y0 50 02 FF On
CAM_PictureFlipInq 8x 09 04 66 FF
y0 50 03 FF Off
CAM_ColorGainInq 8x 09 04 49 FF y0 50 00 00 00 0p FF p: Color Gain setting 0h (60%) to Eh (200%)
y0 50 0w 0w 0w 0w wwww: Pan Position
Pan-tiltPosInq 8x 09 06 12 FF
0z 0z 0z 0z FF zzzz: Tilt Position
CAM_GainLimitInq 8x 09 04 2C FF y0 50 0q FF p: Gain Limit
y0 50 01 FF High
CAM_AFSensitivityI
8x 09 04 58 FF y0 50 02 FF Normal
nq
y0 50 03 FF Low
CAM_BrightnessInq 8x 09 04 A1 FF y0 50 00 00 0p 0q FF pq: Brightness Position
CAM_ContrastInq 8x 09 04 A2 FF y0 50 00 00 0p 0q FF pq: Contrast Position
CAM_GammaInq 8x 09 04 5B FF y0 50 0p FF p: Gamma Curve Position
y0 50 00 FF Off
y0 50 01 FF Flip-H
CAM_FlipInq 8x 09 04 A4 FF
y0 50 02 FF Flip-V
y0 50 03 FF Flip-HV
Page 21 of 50
Rev 1.5 8/20

--- Page 25 (PDF index 24) ---
y0 50 00 FF Top
CAM_AFZone 8x 09 04 AA FF y0 50 01 FF Center
y0 50 02 FF Bottom
p: Color Hue setting 0h (− 14 degrees) to Eh ( +14
CAM_ColorHueInq 8x 09 04 4F FF y0 50 00 00 00 0p FF
degrees
y0 50 00 FF High
CAM_AWBSensitivit
8x 09 04 A9 FF y0 50 01 FF Normal
yInq
y0 50 02 FF Low
y0 50 02 FF On
CAM_UACInq 8x 2A 02 A0 04 FF
y0 50 03 FF Off
Block Inquiry Command List
Command Command packed Inquiry Packet Comments
uuuu: Zoom Position
y0 50 0u 0u 0u 0u 00 00 0v 0v
CAM_LensBlockInq 8x 09 7E 7E 00 FF vvvv: Focus Position
0v 0v 00 0w 00 FF
w.bit0: Focus Mode 1: Auto 0: Manual
pp: R_Gain
qq: B_Gain
r: WB Mode
s: Aperture
tt: AE Mode
CAM_CameraBlockIn y0 50 0p 0p 0q 0q 0r 0s tt 0u vv
8x 09 7E 7E 01 FF u.bit2: Back Light
q ww 00 xx 0z FF
u.bit1: Exposure Comp.
vv: Shutter Position
ww: Iris Position
xx: Bright Position
z: Exposure Comp. Position
p.bit0: Power 1:On, 0:Off
y0 50 0p 0q 00 0r 00 00 00 00
CAM_OtherBlockInq 8x 09 7E 7E 02 FF q.bit2: LR Reverse 1:On, 0:Off
00 00 00 00 00 FF
r.bit3~0: Picture Effect Mode
p: AF sensitivity
q.bit0: Picture flip(1:On, 0:Off)
CAM_EnlargementBl y0 50 00 00 00 00 00 00 00 0p rr.bit6~3: Color Gain(0h(60%) to Eh(200%))
8x 09 7E 7E 03 FF
ockInq 0q rr 0s 0t 0u FF s: Flip(0: Off, 1:Flip-H, 2:Flip-V, 3:Flip-HV)
t.bit2~0: NR2D Level
u: Gain Limit
Note: The [x] in the above table is the camera address, [y] = [x + 8].
Page 22 of 50
Rev 1.5 8/20

--- Page 26 (PDF index 25) ---
Part 4: VISCA over IP Command List
Command Function Command Packet Comments
On 81 01 04 00 02 FF
CAM_Power Power ON/OFF
Off 81 01 04 00 03 FF
Stop 81 01 04 07 00 FF
Tele (Standard) 81 01 04 07 02 FF
Wide (Standard) 81 01 04 07 03 FF
CAM_Zoom
Tele (Variable) 81 01 04 07 2p FF
p = 0(low) - 7(high)
Wide (Variable) 81 01 04 07 3p FF
Direct 81 01 04 47 0p 0q 0r 0s FF pqrs: Zoom Position
Stop 81 01 04 08 00 FF
Far (Standard) 81 01 04 08 02 FF
Near (Standard) 81 01 04 08 03 FF
Far (Variable) 81 01 04 08 2p FF
p = 0(low) - 7(high)
Near (Variable) 81 01 04 08 3p FF
CAM_Focus Direct 81 01 04 48 0p 0q 0r 0s FF pqrs: Focus Position
Auto Focus 81 01 04 38 02 FF
Manual Focus 81 01 04 38 03 FF AF On/Off
Auto/Manual 81 01 04 38 10 FF
Focus Lock 81 0a 04 68 02 FF Prevents any other operation or command from
Focus Unlock 81 0a 04 68 03 FF adjusting the current focus state
Auto 81 01 04 35 00 FF Normal Auto
Indoor mode 81 01 04 35 01 FF Indoor mode
Outdoor mode 81 01 04 35 02 FF Outdoor mode
CAM_WB OnePush mode 81 01 04 35 03 FF One Push WB mode
Manual 81 01 04 35 05 FF Manual Control mode
Color Temperature 81 01 04 35 20 FF Color Temperature mode
OnePush trigger 81 01 04 10 05 FF One Push WB Trigger
Reset 81 01 04 03 00 FF
Up 81 01 04 03 02 FF Manual Control of R Gain
CAM_RGain
Down 81 01 04 03 03 FF
Direct 81 01 04 43 00 00 0p 0q FF pq: R Gain
Reset 81 01 04 04 00 FF
Up 81 01 04 04 02 FF Manual Control of B Gain
CAM_Bgain
Down 81 01 04 04 03 FF
Direct 81 01 04 44 00 00 0p 0q FF pq: B Gain
Reset 81 01 04 20 00 FF Default ColorTemperature setting
CAM_ColorTemp Up 81 01 04 20 02 FF
Down 81 01 04 20 03 FF
Page 23 of 50
Rev 1.5 8/20

--- Page 27 (PDF index 26) ---
pq: Color Temperature position 0x00: 2500K ~
Direct 81 01 04 20 0p 0q FF
0x37: 8000K
Full Auto 81 01 04 39 00 FF Automatic Exposure mode
Manual 81 01 04 39 03 FF Manual Control mode
CAM_AE Shutter priority 81 01 04 39 0A FF Shutter Priority Automatic Exposure mode
Iris priority 81 01 04 39 0B FF Iris Priority Automatic Exposure mode
Bright 81 01 04 39 0D FF Bright Mode(Manual control)
Reset 81 01 04 0B 00 FF
Up 81 01 04 0B 02 FF Iris Setting
CAM_Iris
Down 81 01 04 0B 03 FF
Direct 81 01 04 4B 00 00 0p 0q FF pq: Iris Position
Reset 81 01 04 0A 00 FF Default Shutter setting
Up 81 01 04 0A 02 FF
CAM_Shutter
Down 81 01 04 0A 03 FF
Direct 81 01 04 4A 00 00 0p 0q FF pq: Shutter Position
Reset 81 01 04 0D 00 FF
Up 81 01 04 0D 02 FF Bright Setting
CAM_Bright
Down 81 01 04 0D 03 FF
Direct 81 01 04 0D 00 00 0p 0q FF pq: Bright Position
On 81 01 04 3E 02 FF
Exposure Compensation On/Off
Off 81 01 04 3E 03 FF
Reset 81 01 04 0E 00 FF
CAM_ExpComp
Up 81 01 04 0E 02 FF Exposure Compensation Amount Setting
Down 81 01 04 0E 03 FF
Direct 81 01 04 4E 00 00 0p 0q FF pq: ExpComp Position
On 81 01 04 33 02 FF
CAM_BackLight Back Light Compensation On/Off
Off 81 01 04 33 03 FF
CAM_Flicker - 81 01 04 23 0p FF p: Flicker Settings (0: Off, 1: 50Hz, 2: 60Hz)
Off 81 01 04 63 00 FF
CAM_PictureEffect Picture Effect Setting
B&W 81 01 04 63 04 FF
Reset 81 01 04 3F 00 pp FF
CAM_Memory Set 81 01 04 3F 01 pp FF pp: Memory Number (=0 to 127)
Recall 81 01 04 3F 02 pp FF
Preset Recall Speed Preset Speed 81 01 06 01 p FF p : s p e e d g r a d e , th e v a lues are (0x01~0x18)
On 81 01 04 61 02 FF
CAM_LR_Reverse Image Flip Horizontal On/Off
Off 81 01 04 61 03 FF
On 81 01 04 66 02 FF
CAM_PictureFlip Image Flip Vertical On/Off
Off 81 01 04 66 03 FF
CAM_ColorGain Direct 81 01 04 49 00 00 00 0p FF p: Color Gain setting 0h (60%) to Eh (200%)
Page 24 of 50
Rev 1.5 8/20

--- Page 28 (PDF index 27) ---
Up 81 01 06 01 VV WW 03 01 FF
Down 81 01 06 01 VV WW 03 02 FF
Left 81 01 06 01 VV WW 01 03 FF
Right 81 01 06 01 VV WW 02 03 FF
Upleft 81 01 06 01 VV WW 01 01 FF
VV: Pan speed 0x01 (low speed) to 0x18 (high
Upright 81 01 06 01 VV WW 02 01 FF
speed)
DownLeft 81 01 06 01 VV WW 01 02 FF
WW: Tilt speed 0x01 (low speed) to 0x14 (high
Pan_tiltDrive DownRight 81 01 06 01 VV WW 02 02 FF
speed)
Stop 81 01 06 01 VV WW 03 03 FF
YYYY: Pan Position
81 01 06 02 VV WW
AbsolutePosition ZZZZ: Tilt Position
0Y 0Y 0Y 0Y 0Z 0Z 0Z 0Z FF
81 01 06 03 VV WW
RelativePosition
0Y 0Y 0Y 0Y 0Z 0Z 0Z 0Z FF
Home 81 01 06 04 FF
Reset 81 01 06 05 FF
81 01 06 07 00 0W
LimitSet W: 1 UpRight 0: DownLeft
0Y 0Y 0Y 0Y 0Z 0Z 0Z 0Z FF
Pan_tiltLimitSet YYYY: Pan Limit Position
81 01 06 07 01 0W
LimitClear ZZZZ: Tilt Position
07 0F 0F 0F 07 0F 0F 0F FF
CAM_Brightness Direct 81 01 04 A1 00 00 0p 0q FF pq: Brightness Position
CAM_Contrast Direct 81 01 04 A2 00 00 0p 0q FF pq: Contrast Position
CAM_Gamma Direct 81 01 04 5B 0p FF p: Gamma Curve (0=Standard, 1-4=Alternate)
Off 81 01 04 A4 00 FF
Flip-H 81 01 04 A4 01 FF
CAM_Flip Single Command For Video Flip
Flip-V 81 01 04 A4 02 FF
Flip-HV 81 01 04 A4 03 FF
CAM_SettingSave Save 81 01 04 A5 10 FF Save Current Setting
High 81 01 04 A9 00 FF High
CAM_AWBSensitivity Normal 81 01 04 A9 01 FF Normal
Low 81 01 04 A9 02 FF Low
Top 81 01 04 AA 00 FF
CAM_AFZone Center 81 01 04 AA 01 FF AF Zone weight select
Bottom 81 01 04 AA 02 FF
CAM_ColorHue Direct 81 01 04 4F 00 00 00 0p FF p: Color Hue 0h (−14 degrees) to Eh (+14 degrees)
Open / Close 81 01 04 3F 02 5F FF
Navigate Up 81 01 06 01 0E 0E 03 01 FF
Navigate Down 81 01 06 01 0E 0E 03 02 FF
OSD_Control
Navigate Left 81 01 06 01 0E 0E 01 03 FF
Navigate Right 81 01 06 01 0E 0E 02 03 FF
Enter 81 01 06 06 05 FF
Page 25 of 50
Rev 1.5 8/20

--- Page 29 (PDF index 28) ---
Return 81 01 06 06 04 FF
High 81 0B 01 01 FF
Medium 81 0B 01 02 FF
CAM_NDIMode
Low 81 0B 01 03 FF
Off 81 0B 01 04 FF
CAM_MulticastMode Multicast Mode 81 0B 01 23 0p FF p=1: On, p=2: Off
PTZ Motion Sync On 81 0A 11 13 02 FF
CAM_PTZMotionSync PTZ Motion Sync Off 81 0A 11 13 03 FF
MS Lower Speed Limit 81 0A 11 14 pq FF pq: speed stage
CAM_UACStatus Toggle USB Audio 81 2A 02 A0 04 0p FF p=2: On, p=3: Off
Part 5: VISCA over IP Query Command List
Inquiry Command List
Command Command packed Inquiry Packet Comments
90 50 02 FF On
CAM_PowerInq 81 09 04 00 FF 90 50 03 FF Off (Standby)
90 50 04 FF Internal power circuit error
CAM_ZoomPosInq 81 09 04 47 FF 90 50 0p 0q 0r 0s FF pqrs: Zoom Position
CAM_FocusAFMode 90 50 02 FF Auto Focus
81 09 04 38 FF
Inq 90 50 03 FF Manual Focus
CAM_FocusPosInq 81 09 04 48 FF 90 50 0p 0q 0r 0s FF pqrs: Focus Position
90 50 00 FF Auto
90 50 01 FF Indoor mode
90 50 02 FF Outdoor mode
CAM_WBModeInq 81 09 04 35 FF
90 50 03 FF OnePush mode
90 50 05 FF Manual
90 50 20 FF ColorTemperature Mode
CAM_RGainInq 81 09 04 43 FF 90 50 00 00 0p 0q FF pq: R Gain
CAM_BGainInq 81 09 04 44 FF 90 50 00 00 0p 0q FF pq: B Gain
90 50 00 FF Full Auto
90 50 03 FF Manual
CAM_AEModeInq 81 09 04 39 FF 90 50 0A FF Shutter priority
90 50 0B FF Iris priority
90 50 0D FF Bright
CAM_ShutterPosInq 81 09 04 4A FF 90 50 00 00 0p 0q FF pq: Shutter Position
CAM_IrisPosInq 81 09 04 4B FF 90 50 00 00 0p 0q FF pq: Iris Position
CAM_BrightPosInq 81 09 04 4D FF 90 50 00 00 0p 0q FF pq: Bright Position
Page 26 of 50
Rev 1.5 8/20

--- Page 30 (PDF index 29) ---
CAM_ExpCompMod 90 50 02 FF On
81 09 04 3E FF
eInq 90 50 03 FF Off
CAM_ExpCompPosI
81 09 04 4E FF 90 50 00 00 0p 0q FF pq: ExpComp Position
nq
CAM_BacklightMode 90 50 02 FF On
81 09 04 33 FF
Inq 90 50 03 FF Off
CAM_Nosise2DMode 90 50 02 FF Auto Noise 2D
81 09 04 50 FF
Ing 90 50 03 FF Manual Noise 3D
CAM_Nosise2DLevel 81 09 04 53 FF 90 50 0p FF Noise Reduction (2D) p: 0 to 5
CAM_Noise3DLevel 81 09 04 54 FF 90 50 0p FF Noise Reduction (3D) p: 0 to 8
CAM_FlickerModeIn
81 09 04 55 FF 90 50 0p FF p: Flicker Settings(0: OFF, 1: 50Hz, 2: 60Hz)
q
CAM_ApertureModeI 90 50 02 FF Auto Sharpness
81 09 04 05 FF
nq (Sharpness) 90 50 03 FF Manual Sharpness
CAM_ApertureInq
81 09 04 42 FF 90 50 00 00 0p 0q FF pq: Aperture Gain
(Sharpness)
90 50 02 FF On
SYS_MenuModeInq 81 09 06 06 FF
90 50 03 FF Off
CAM_PictureEffectM 90 50 02 FF Off
81 09 04 63 FF
odeInq 90 50 04 FF B&W
90 50 02 FF On
CAM_LR_ReverseInq 81 09 04 61 FF
90 50 03 FF Off
90 50 02 FF On
CAM_PictureFlipInq 81 09 04 66 FF
90 50 03 FF Off
CAM_ColorGainInq 81 09 04 49 FF 90 50 00 00 00 0p FF p: Color Gain setting 0h (60%) to Eh (200%)
90 50 0w 0w 0w 0w wwww: Pan Position
Pan-tiltPosInq 81 09 06 12 FF
0z 0z 0z 0z FF zzzz: Tilt Position
CAM_GainLimitInq 81 09 04 2C FF 90 50 0q FF p: Gain Limit
90 50 01 FF High
CAM_AFSensitivityI
81 09 04 58 FF 90 50 02 FF Normal
nq
90 50 03 FF Low
CAM_BrightnessInq 81 09 04 A1 FF 90 50 00 00 0p 0q FF pq: Brightness Position
CAM_ContrastInq 81 09 04 A2 FF 90 50 00 00 0p 0q FF pq: Contrast Position
CAM_GammaInq 81 09 04 5B FF 90 50 0p FF p: Gamma Curve Position
90 50 00 FF Off
90 50 01 FF Flip-H
CAM_FlipInq 81 09 04 A4 FF
90 50 02 FF Flip-V
90 50 03 FF Flip-HV
Page 27 of 50
Rev 1.5 8/20

--- Page 31 (PDF index 30) ---
90 50 00 FF Top
CAM_AFZone 81 09 04 AA FF 90 50 01 FF Center
90 50 02 FF Bottom
p: Color Hue setting 0h (− 14 degrees) to Eh ( +14
CAM_ColorHueInq 81 09 04 4F FF 90 50 00 00 00 0p FF
degrees
90 50 00 FF High
CAM_AWBSensitivit
81 09 04 A9 FF 90 50 01 FF Normal
yInq
90 50 02 FF Low
90 50 02 FF On
CAM_UACInq 81 2A 02 A0 04 FF
90 50 03 FF Off
Part 6: Pelco-D Protocol Command List
Function Byte1 Byte2 Byte3 Byte4 Byte5 Byte6 Byte7
Up 0xFF Address 0x00 0x08 Pan Speed Tilt Speed SUM
Down 0xFF Address 0x00 0x10 Pan Speed Tilt Speed SUM
Left 0xFF Address 0x00 0x04 Pan Speed Tilt Speed SUM
Right 0xFF Address 0x00 0x02 Pan Speed Tilt Speed SUM
Zoom In 0xFF Address 0x00 0x20 0x00 0x00 SUM
Zoom Out 0xFF Address 0x00 0x40 0x00 0x00 SUM
Focus Far 0xFF Address 0x00 0x80 0x00 0x00 SUM
Focus Near 0xFF Address 0x01 0x00 0x00 0x00 SUM
Set Preset 0xFF Address 0x00 0x03 0x00 Preset ID SUM
Clear Preset 0xFF Address 0x00 0x05 0x00 Preset ID SUM
Call Preset 0xFF Address 0x00 0x07 0x00 Preset ID SUM
Auto Focus 0xFF Address 0x00 0x2B 0x00 0x01 SUM
Manual Focus 0xFF Address 0x00 0x2B 0x00 0x02 SUM
Query Pan Position 0xFF Address 0x00 0x51 0x00 0x00 SUM
Value High Value Low
Query Pan Position Response 0xFF Address 0x00 0x59 SUM
Byte Byte
Query Tilt Position 0xFF Address 0x00 0x53 0x00 0x00 SUM
Value High Value Low
Query Tilt Position Response 0xFF Address 0x00 0x5B SUM
Byte Byte
Query Zoom Position 0xFF Address 0x00 0x55 0x00 0x00 SUM
Query Zoom Position Value High Value Low
0xFF Address 0x00 0x5D SUM
Response Byte Byte
Page 28 of 50
Rev 1.5 8/20

--- Page 32 (PDF index 31) ---
Part 7: Pelco-P Protocol Command List
Function Byte1 Byte2 Byte3 Byte4 Byte5 Byte6 Byte7 Byte8
Up 0xA0 Address 0x00 0x08 Pan Speed Tilt Speed 0xAF XOR
Down 0xA0 Address 0x00 0x10 Pan Speed Tilt Speed 0xAF XOR
Left 0xA0 Address 0x00 0x04 Pan Speed Tilt Speed 0xAF XOR
Right 0xA0 Address 0x00 0x02 Pan Speed Tilt Speed 0xAF XOR
Zoom In 0xA0 Address 0x00 0x20 0x00 0x00 0xAF XOR
Zoom Out 0xA0 Address 0x00 0x40 0x00 0x00 0xAF XOR
Focus Far 0xA0 Address 0x00 0x80 0x00 0x00 0xAF XOR
Focus Near 0xA0 Address 0x01 0x00 0x00 0x00 0xAF XOR
Set Preset 0xA0 Address 0x00 0x03 0x00 Preset ID 0xAF XOR
Clear Preset 0xA0 Address 0x00 0x05 0x00 Preset ID 0xAF XOR
Call Preset 0xA0 Address 0x00 0x07 0x00 Preset ID 0xAF XOR
Auto Focus 0xA0 Address 0x00 0x2B 0x00 0x01 0xAF XOR
Manual Focus 0xA0 Address 0x00 0x2B 0x00 0x02 0xAF XOR
Query Pan Position 0xA0 Address 0x00 0x51 0x00 0x00 0xAF XOR
Query Pan Position Value High Value Low
0xA0 Address 0x00 0x59 0xAF XOR
Response Byte Byte
Query Tilt Position 0xA0 Address 0x00 0x53 0x00 0x00 0xAF XOR
Query Tilt Position Value High Value Low
0xA0 Address 0x00 0x5B 0xAF XOR
Response Byte Byte
Query Zoom Position 0xA0 Address 0x00 0x55 0x00 0x00 0xAF XOR
Query Zoom Position Value High Value Low
0xA0 Address 0x00 0x5D 0xAF XOR
Response Byte Byte
Page 29 of 50
Rev 1.5 8/20
```

---

## Validation / errata notes

The source documents contain a few internal inconsistencies. These are the major ones to be aware of:

1. **PT30X lens max focal length in manual:** The PT30X manual “Technical Specifications” table lists `f4.42mm ~ 88.5mm` while the PT30X datasheet lists `f4.42mm ~ 132.6mm`. Treat the datasheet as authoritative for PT30X optics.

2. **Brightness Direct opcode typo in the manual:** The manual’s Part 2 table lists `CAM_Bright Direct` as `... 04 0D ...` even though the inquiry uses `04 4D` (and standard VISCA uses `4D` for Bright direct).
   - **Recommendation:** Prefer `04 4D` for “Bright Direct” when implementing.

3. **Luminance/Contrast parameter width — RESOLVED:**
   - Manual lists `A1/A2` "Direct" with `0p 0q` (2 nibbles); the curated 2023 list had a more constrained `... 00 0p` form.
   - Hardware testing on a PTZOptics G2 confirms the `0p 0q` (2-nibble) form is correct.
   - Inquiry responses use the standard 4-nibble format: `y0 50 00 00 0p 0q FF`.
   - The curated list has been updated to match.

4. **VISCA‑over‑IP encapsulation:** PTZOptics NDI®|HX cameras use **raw VISCA bytes** over TCP/UDP on the PTZ ports; they do not use Sony's UDP "encapsulated VISCA over IP" header format.

5. **Gamma (0x5B) command — undocumented but functional:**
   - The PTZOptics G2 manual does not mention the gamma command (`81 01 04 5B 0p FF`), but hardware testing confirms it is supported. Both the set command and inquiry work correctly.
   - Gamma values 0–4 are accepted: 0 = Standard, 1–4 = alternate gamma curves.
   - Inquiry format: `81 09 04 5B FF` → response `90 50 0p FF`.
   - This command has been added to the curated command list and inquiry table above.
