# VISCA Interface API Description

**User Manual**
Version: M1.6
Date: March 2021
© Axis Communications AB, 2021
Part No. T10162575

---

## Command List: Camera

| Command Set | Command | Command Packet | Comments |
|-------------|---------|----------------|----------|
| **continuous zoom** | stop | `8x 01 04 07 00 FF` | - |
| | tele fixed speed | `8x 01 04 07 02 FF` | - |
| | wide fixed speed | `8x 01 04 07 03 FF` | - |
| | tele controllable speed | `8x 01 04 07 2p FF` | p = speed, min 0x0, max 0x7 |
| | wide control speed | `8x 01 04 07 3p FF` | p = speed, min 0x0, max 0x7 |
| **absolute zoom** | - | `8x 01 04 07 02 FF` | pqrs = position<br>0x0000 to 0x4000 optical range<br>0x4001 to 0x7AC0 digital range |
| **continuous focus** | stop | `8x 01 04 08 00 FF` | - |
| | far fixed speed | `8x 01 04 08 02 FF` | - |
| | near fixed speed | `8x 01 04 08 03 FF` | - |
| | far controllable speed | `8x 01 04 08 2p FF` | p = speed, min 0x0, max 0x7 |
| | near controllable speed | `8x 01 04 08 3p FF` | p = speed, min 0x0, max 0x7 |
| **absolute focus** | - | `8x 01 04 48 0p 0q 0r 0s FF` | pqrs = position, min 0x1000, max 0xF000 |
| **auto focus** | - | `8x 01 04 38 pq FF` | pq = mode<br>0x10 = toggle<br>0x02 = on<br>0x03 = off |
| **set focus near limit** | - | `8x 01 04 28 00 00 00 0p FF` | p = distance<br>0x2 = 20.00m<br>0x3 = 10.00m<br>0x4 = 6.00m<br>0x5 = 4.20m<br>0x6 = 3.10m<br>0x7 = 2.50m<br>0x8 = 2.00m<br>0x9 = 1.65m<br>0xA = 1.40m<br>0xB = 1.20m<br>0xC = 0.80m<br>0xD = 0.30m<br>0xE = 0.11m |
| **white balance** | - | `8x 01 04 35 0p FF` | p = mode<br>0x0 = auto<br>0x1 = fixed indoor<br>0x2 = fixed outdoor<br>0x4 = auto outdoor<br>0x5 = manual |
| **white balance one push** | trigger | `8x 01 04 10 05 FF` | - |
| **relative red gain** | - | `8x 01 04 03 0p FF` | p = mode<br>0x0 = reset, set to 50%<br>0x2 = up, more<br>0x3 = down, less |
| **absolute red gain** | - | `8x 01 04 43 00 00 0p 0q FF` | pq = gain, min 0x00, max 0xFF |
| **relative blue gain** | - | `8x 01 04 04 0p FF` | p = mode<br>0x0 = reset, set to 50%<br>0x2 = up, more<br>0x3 = down, less |
| **absolute blue gain** | - | `8x 01 04 44 00 00 0p 0q FF` | pq = gain, min 0x00, max 0xFF |
| **auto exposure** | - | `8x 01 04 39 0p FF` | p = mode<br>0x0 = full auto<br>0x3 = full manual<br>0xA = shutter priority<br>0xB = iris priority<br>0xD = bright mode |
| **relative shutter** | - | `8x 01 04 0A 0p FF` | p = mode<br>0x0 = reset<br>0x2 = up, shorter<br>0x3 = down, longer |
| **absolute shutter** | - | `8x 01 04 4A 00 00 0p 0q FF` | pq = time (see shutter speed table below) |

### Absolute Shutter Speed Values

| Value | 50Hz Mode | 60Hz Mode |
|-------|-----------|-----------|
| 0x00 | 1/1s | 1/1s |
| 0x01 | 1/2s | 1/2s |
| 0x02 | 1/3s | 1/4s |
| 0x03 | 1/6s | 1/8s |
| 0x04 | 1/12s | 1/15s |
| 0x05 | 1/25s | 1/30s |
| 0x06 | 1/50s | 1/60s |
| 0x07 | 1/75s | 1/90s |
| 0x08 | 1/100s | 1/100s |
| 0x09 | 1/120s | 1/125s |
| 0x0A | 1/150s | 1/180s |
| 0x0B | 1/215s | 1/250s |
| 0x0C | 1/300s | 1/350s |
| 0x0D | 1/425s | 1/500s |
| 0x0E | 1/600s | 1/725s |
| 0x0F | 1/1000s | 1/1000s |
| 0x10 | 1/1250s | 1/1500s |
| 0x11 | 1/1750s | 1/2000s |
| 0x12 | 1/2500s | 1/3000s |
| 0x13 | 1/3500s | 1/4000s |
| 0x14 | 1/6000s | 1/6000s |
| 0x15 | 1/10000s | 1/10000s |

---

## Command List: Pan/Tilt

| Command Set | Command | Command Packet | Comments |
|-------------|---------|----------------|----------|
| **continuous pan tilt** | move up | `8x 01 06 01 pp tt 03 01 FF` | pp = pan speed, min 0x01, max 0x18<br>tt = tilt speed, min 0x01, max 0x17 |
| | move down | `8x 01 06 01 pp tt 03 02 FF` | |
| | move left | `8x 01 06 01 pp tt 01 03 FF` | |
| | move right | `8x 01 06 01 pp tt 02 03 FF` | |
| | move up-left | `8x 01 06 01 pp tt 01 01 FF` | |
| | move down-left | `8x 01 06 01 pp tt 01 02 FF` | |
| | move up-right | `8x 01 06 01 pp tt 02 01 FF` | |
| | move down-right | `8x 01 06 01 pp tt 02 02 FF` | |
| | stop movement | `8x 01 06 01 pp tt 03 03 FF` | |
| **absolute pan tilt** | move directly to target position | `8x 01 06 02 pp tt 0g 0h 0i 0j 0k 0l 0m 0n FF` | pp = pan speed, min 0x01, max 0x18<br>tt = tilt speed, min 0x01, max 0x17<br>ghij = pan position, min 0xDE00 (-0x2200), max 0x2200<br>klmn = tilt position, min 0xFC00 (-0x0400), max 0x2200 |
| **relative pan tilt** | move requested distance | `8x 01 06 03 pp tt 0g 0h 0i 0j 0k 0l 0m 0n FF` | pp = pan speed, min 0x01, max 0x18<br>tt = tilt speed, min 0x01, max 0x17<br>ghij = pan change, min 0xBC00 (-0x4400), max 0x4400<br>klmn = tilt change, min 0xEA00 (-0x1600), max 0x1600 |
| **goto home** | move to home position | `8x 01 06 04 FF` | - |
| **reset pan tilt** | resets pan tilt driver | `8x 01 06 05 FF` | - |
| **set pan tilt limits** | - | `8x 01 06 07 00 0p 0g 0h 0i 0j 0k 0l 0m 0n FF` | p = corner, down-left 0x0, up-right 0x1<br>ghij = pan position, min 0xDE00, max 0x2200<br>klmn = tilt position, min 0xFC00 (-0x0400), max 0x1200 |

---

## Command List: Extended

| Command Set | Command | Command Packet | Comments |
|-------------|---------|----------------|----------|
| **tally control** | control tally light | `8x 01 7E 01 0A 00 0p FF` | p = mode<br>0x2 = on<br>0x3 = off |

---

## Inquiry List: Device

| Command Set | Command Packet | Inquiry Packet | Comments |
|-------------|----------------|----------------|----------|
| **Version** | `X 09 00 02 FF` | `9y 50 41 58 56 pq rs 00 02 FF` | pqrs = product number<br>Example: pqrs = 5925 for AXIS V5925 |

---

## Inquiry List: Camera

| Command Set | Command Packet | Inquiry Packet | Comments |
|-------------|----------------|----------------|----------|
| **camera power** | `8x 09 04 00 FF` | Fixed to on: `y0 50 02 FF` | - |
| **auto focus** | `8x 09 04 38 FF` | `y0 50 0p FF` | p = mode<br>0x2 = on<br>0x3 = off |
| **white balance** | `8x 09 04 35 FF` | `y0 50 0p FF` | p = mode<br>0x0 = auto<br>0x1 = fixed indoor<br>0x2 = fixed outdoor<br>0x4 = auto outdoor<br>0x5 = manual |
| **auto exposure** | `8x 09 04 39 FF` | `y0 50 0p FF` | p = mode<br>0x0 = auto<br>0x3 = full manual<br>0xA = shutter priority<br>0xB = iris priority<br>0xD = bright mode |
| **back light compensation** | `8x 09 04 33 FF` | `y0 50 0p FF` | p = mode<br>0x2 = on<br>0x3 = off |
| **spot light mode** | `8x 09 04 3A FF` | Fixed to off: `y0 50 03 FF` | - |
| **exposure compensation** | `8x 09 04 3E FF` | Fixed to off: `y0 50 03 FF` | - |
| **zoom position** | `8x 09 04 47 FF` | `y0 50 0p 0q 0r 0s FF` | pqrs = position<br>0x0000 to 0x4000 optical range<br>0x4001 to 0x7AC0 digital range |

---

## Inquiry List: Pan/Tilt

| Command Set | Command Packet | Inquiry Packet | Comments |
|-------------|----------------|----------------|----------|
| **pan tilt position** | `8x 09 06 12 FF` | `y0 50 0g 0h 0i 0j 0k 0l 0m 0n FF` | ghij = pan position, min 0xDE00 (-0x2200), max 0x2200<br>klmn = tilt position, min 0xFC00 (-0x0400), max 0x1200 |
| **menu display** | `8x 09 06 06 FF` | Fixed to off: `y0 50 03 FF` | - |

---

## Notes

- `x` in command packets represents the device address (typically 1 for a single camera)
- `y` in inquiry packets represents the response address
- All packets are terminated with `FF`
- Hexadecimal values are indicated with `0x` prefix
- Negative values are represented in two's complement notation
