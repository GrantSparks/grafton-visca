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
  - Luminance Direct: `81 01 04 A1 00 00 00 0p FF` (p: 0x0=0 ~ E=14)
  - Contrast Direct: `81 01 04 A2 00 00 00 0p FF` (p: 0x0=0 ~ E=14)
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
    - Direct: `81 01 04 0D 00 00 0p 0q FF` (pq: 0x00=0 ~ 0x11=17)

- **Gain**
  - Reset: `81 01 04 0C 00 FF`
  - Up: `81 01 04 0C 02 FF`
  - Down: `81 01 04 0C 03 FF`
  - Direct: `81 01 04 0C 00 00 0p 0q FF` (pq: 0x00=0 ~ 0x07=7)
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


