
# Unified VISCA Protocol Implementation Guide  
*(PTZOptics Gen‑2/NDI, Sony ILME‑FR7, Sony BRC‑series, Sony EVI‑H100 S/V, Nearus BRC‑300)*  

---

## 1  Scope & Intent  

This document captures VISCA protocol and implementation details for some popular PTZ cameras from multiple manufacturers.

---

## 2  Terminology  

| Term | Meaning |
|------|---------|
| **Controller** | VISCA command originator (serial address `0`; IP `Src = 0x00`). |
| **Peripheral / Camera** | VISCA responder (serial `1‑7`; IP **always `1`**). |
| **Socket / Buffer** | One of two concurrent command slots in every Sony‑derived PTZ. |
| **ACK** | `90 4y FF` – command accepted, socket `y (1 or 2)`. |
| **Completion** | `90 5y FF` – command finished, socket `y`. |
| **Error** | `90 60/6y .. FF` (`02` syntax, `03` buffer full, `41` not executable …). |

---

## 3  Physical & Transport Layers  

### 3.1 Serial RS‑232/RS‑422  

| Connector | Baud | Frame | Addressing |
|-----------|------|-------|------------|
| MiniDIN‑8 (Sony) or DE‑9 (PTZOptics) | 9 600 / 38 400 bps, 8‑N‑1 | `8x … FF` (1–14 payload bytes) | Controller `0`, Cameras `1‑7` |

### 3.2 VISCA‑over‑IP  

#### 3.2.1 **Two incompatible flavours**  

| Camera family | Envelope required? | Sequence number? | Default port(s) |
|---------------|-------------------|------------------|-----------------|
| **Sony** BRC/SRG/FR series (and clones) | **Yes** – 8‑byte header | 32‑bit counter (wraps) | UDP 52381 |
| **PTZOptics** (all firmware revs to 2025‑06) | **No** – *raw VISCA bytes* | *Not used* | UDP 1259, TCP 5678 |
| Hybrids claiming “encapsulated” mode (Marshall, AVer etc.) | Yes | Yes | UDP 52381 |

> **Short answer – no.** The *PTZOptics VISCA‑over‑IP Command Sheet* (Rev 1.2, 24 Aug 2020) never mentions a header or sequence field; wire‑captures confirm the camera consumes the canonical serial byte‑string directly. Supplying a Sony‑style header causes the packet to be ignored.  
> **Implementation tip:** expose a switch *“Sony‑style” vs “Raw”* and wrap only when needed.

#### 3.2.2 Sony encapsulated header (UDP 52381)  

```
┌─────────────── 8 bytes ───────────────┐┌───── VISCA frame (≤16 B) ─────┐
│ Payload‑type │ Length │ Sequence‑No. ││   8x …payload… FF             │
└───────────────────────────────────────┘└────────────────────────────────┘
```

*  `Payload‑type` `0x0000` Command, `0x0001` Inquiry, `0x0002` Reply  
*  `Sequence‑No.` 16‑bit (BRC/H900) or 32‑bit (FR7) counter; host increments, camera echoes.

#### 3.2.3 Retransmission (Sony only)  

If no **ACK** within 2 video frames, resend the identical packet **with a new sequence‑ID**. Sony’s FR7 matrix (Manual v2.0 p. 10) defines when the camera will discard duplicates vs process them.

---

## 4  Transaction & Timing Model  

| Phase | Reply | Notes |
|-------|-------|-------|
| **ACK** | `90 4y FF` | ≤ 1 V‑sync (≈16.7 ms NTSC). |
| **Completion** | `90 5y FF` | Frees socket `y`. |
| **Error** | `90 60 02` Syntax<br>`90 60 03` Buffer Full<br>`90 6y 41` Not Executable | |

*Two‑socket rule* – never queue a **third** in‑flight command.  
*EVI‑H100 / BRC‑H900*: extra 240 ms “busy” after **Preset Recall** completion; expect transient `…41`.

---

## 5  Baseline Command Set (universally recognised)  

| Category | Opcode | Purpose |
|----------|--------|---------|
| Power | `81 01 04 00 02/03 FF` | On / Standby |
| Zoom | `81 01 04 07 .. FF` | Std / Var / Direct |
| Focus | `81 01 04 08 .. FF` | Std / Var / Direct |
| Pan‑Tilt | `81 01 06 01 VV WW DD DD FF` | Continuous move. |
| Memory | `81 01 04 3F 00/01/02 pp FF` | Reset / Set / Recall preset |

---

## 6  Cross‑Model Capability Matrix  

| Feature | PTZOptics Gen‑2 | Sony ILME‑FR7 | Sony BRC‑H900 | Sony EVI‑H100 | Nearus BRC‑300 |
|---------|-----------------|---------------|---------------|--------------|----------------|
| Transport (IP) | **Raw UDP/TCP** | Encaps. UDP | Encaps. UDP* | — | — |
| Preset slots | **128** (fw ≥2.2) | 100 | 16 | 6 | 6 |
| Pan‑spd steps | 1‑24 | **1‑50** | 1‑24 | 1‑24 | 1‑24 |
| Focus Lock cmd | ✔ | — | — | — | — |
| ND Filter ctrl | — | ✔ | — | — | — |
| Dual Tally | Single cmd (flash/on/off) | **Red+Green** | Red (Hi/Lo) | — | — |
| Bright AE mode | ✔ | ✖ | ✔ | ✔ | ✔ |
| ATW WB | — | ✔ | — | — | — |
| Colour‑Temp WB | ✔ | — | ✔ | ✔ | — |

\* BRC‑H900 requires optional IP‑card; otherwise serial only.  

---

## 7  Parameter Ranges & Scaling  

### 7.1 Zoom  

| Model | Optical range | Digital |
|-------|---------------|---------|
| PTZOptics 20× | `0x0000` → `0x4000` | to `0x7800` |
| BRC‑300 | 1–12× (`0x0000`→`0x4000`) | 4× |
| EVI‑H100 30× | (`0x0000`→`0x7AC0`) | 12× |

### 7.2 Pan / Tilt scales  

* **Legacy unsigned** (BRC‑300/Nearus): 0x0000 = far‑left, 0xFFFF = far‑right.  
* **Signed‑centre** (all others): 0x0000 = centre; negative = left, positive = right.

| Model | Pan ° | Tilt ° (desktop) | Hex ends |
|-------|-------|------------------|----------|
| PTZOptics | ±170 | –30 → +90 | ≈ `E1E5`↔`1E1B` |
| FR7 | ±170 | –30 → +195 | 20‑bit clip |

### 7.3 Exposure numeric domains  

| Control | Range | Note |
|---------|-------|------|
| Shutter | 24 steps 1/1–1/10000 | All models |
| Iris | 16 steps | FR7 lens‑dependent |
| Gain | 0–15 (≈0–33 dB) | |
| EV shift | 0x00–0x0E (others) / **0x00–0x12 FR7** | |

---

## 8  Model‑Specific Command Extensions  

### 8.1 PTZOptics  

* Focus Lock `0A 04 68 02/03`  
* H‑Flip `01 04 61 02/03`, V‑Flip `01 04 66 02/03`  
* AWB Sensitivity `01 04 A9 00/01/02`  
* Multicast `0B 01 23 02/03`  

### 8.2 Sony ILME‑FR7  

* ND Mode `7E 04 52 0p`, ND Level `7E 04 42 00 0p 0q`  
* Auto ND `7E 04 53 02/03`  
* Tally Red `7E 01 0A 00 02/03`; Tally Green `7E 04 1A 00 02/03`  
* Speed‑Step select `7E 04 1B 01/02` (24 vs 50)  
* Direct‑Menu press `7E 04 72 pp 0q` – ND, Iris, ISO, Shutter …  

### 8.3 Sony BRC‑H900  

* Image Freeze `01 04 62 02/03`  
* Tally brightness `7E 01 0A 01 04/05`  
* IP‑card discover: send `"ENQ:network"` to UDP 52380.  

### 8.4 Sony EVI‑H100  

* Gamma 0‑4, Wide‑D/HDR block, NR 0‑5.  
* 240 ms busy window post‑preset.  

---

## 9  Behavioural Edge‑Cases & Recommended Tests  

| Scenario | Expectation |
|----------|-------------|
| Third in‑flight command | `…60 03` Buffer Full |
| Manual focus while AF On | `…6y 41` Not Executable |
| PTZOptics Focus‑Lock active → manual move | `41` until unlocked |
| FR7 ND level cmd in Preset mode | `41` |
| FR7 Tally auto‑off | Lamp goes dark after 15 s no refresh |
| EVI‑H100 preset recall + <240 ms next cmd | One‑off `41`, retry succeeds |
| Send Sony‑header packet to PTZOptics | **Ignored** (no ACK) |

---

## 10  Implementation Checklist  

1. Pluggable **transport adapter** (raw vs encapsulated).  
2. Two‑socket state machine; gate ≥3rd command.  
3. Axis scale abstraction (`UnsignedEnd16` vs `SignedCentre16`).  
4. Firmware‑quirk registry (PTZ preset ≤32, EVI 240 ms, FR7 CT mode TBD).  
5. IP‑card discovery (BRC‑H900).  
6. Retry on UDP loss with new sequence‑ID (Sony only).  
7. Auto‑close OSD (`06 06 03`) before operational moves.  

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
