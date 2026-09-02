# 2.0.0-rc.1 hardware release checklist

This checklist separates software contract evidence from physical-camera
evidence. Every row is intentionally `Pending (Not run)` for this release
candidate until a named owner records the bench, firmware, date, command
transcript, and artifact link. A passing software test is not a hardware pass,
and no row may be marked complete from an unrecorded manual observation.

## Recording rules

Before a hardware run, record the camera model, exact firmware revision,
transport, profile type, camera address, host OS, adapter/runtime, power state,
and test operator. Attach the raw transcript or capture, the sanitized
diagnostic/metrics export, and any issue number for a failure. Stop immediately
if a movement, power, or network test behaves unexpectedly.

Status vocabulary:

- `Pending (Not run)` means no evidence has been collected.
- `Blocked` means the required camera, firmware, cable, or safe bench is not
  available; record the blocker and owner.
- `Pass` or `Fail` is allowed only with the evidence fields completed.

The release candidate has no hardware claim until the rows below are updated.

## Profile and transport matrix

Default ports are included so a run can verify both endpoint selection and wire
framing. `Raw` means unencapsulated VISCA; `Sony` means Sony encapsulated VISCA.

| ID | Profile | Transport / envelope | Default | Owner | Status | Firmware / bench | Evidence artifact / notes |
| --- | --- | --- | ---: | --- | --- | --- | --- |
| PT-01 | `GenericVisca` | TCP / Raw | 5678 | Transport QA | Pending (Not run) | Pending | Pending — transcript and packet capture |
| PT-02 | `GenericVisca` | UDP / Raw | 1259 | Transport QA | Pending (Not run) | Pending | Pending — transcript and packet capture |
| PT-03 | `GenericVisca` | Blocking serial / Raw | — | Serial QA | Pending (Not run) | Pending | Pending — serial capture and wiring |
| PT-04 | `PtzOpticsG2` | TCP / Raw | 5678 | PTZOptics QA | Pending (Not run) | Pending | Pending — transcript and packet capture |
| PT-05 | `PtzOpticsG2` | UDP / Raw | 1259 | PTZOptics QA | Pending (Not run) | Pending | Pending — transcript and packet capture |
| PT-06 | `PtzOpticsG2` | Blocking serial / Raw | — | Serial QA | Pending (Not run) | Pending | Pending — serial capture and wiring |
| PT-07 | `PtzOpticsG3` | TCP / Raw | 5678 | PTZOptics QA | Pending (Not run) | Pending | Pending — transcript and packet capture |
| PT-08 | `PtzOpticsG3` | UDP / Raw | 1259 | PTZOptics QA | Pending (Not run) | Pending | Pending — transcript and packet capture |
| PT-09 | `PtzOpticsG3` | Blocking serial / Raw | — | Serial QA | Pending (Not run) | Pending | Pending — serial capture and wiring |
| PT-10 | `PtzOptics30X` | TCP / Raw | 5678 | PTZOptics QA | Pending (Not run) | Pending | Pending — transcript and packet capture |
| PT-11 | `PtzOptics30X` | UDP / Raw | 1259 | PTZOptics QA | Pending (Not run) | Pending | Pending — transcript and packet capture |
| PT-12 | `PtzOptics30X` | Blocking serial / Raw | — | Serial QA | Pending (Not run) | Pending | Pending — serial capture and wiring |
| PT-13 | `SonyEVIH100` | TCP / Raw | 5678 | Sony QA | Pending (Not run) | Pending | Pending — transcript and packet capture |
| PT-14 | `SonyEVIH100` | UDP / Raw | 1259 | Sony QA | Pending (Not run) | Pending | Pending — transcript and packet capture |
| PT-15 | `SonyEVIH100` | Blocking serial / Raw | — | Serial QA | Pending (Not run) | Pending | Pending — serial capture and wiring |
| PT-16 | `SonyBRC300` | TCP / Raw | 5678 | Sony QA | Pending (Not run) | Pending | Pending — transcript and packet capture |
| PT-17 | `SonyBRC300` | UDP / Raw | 1259 | Sony QA | Pending (Not run) | Pending | Pending — transcript and packet capture |
| PT-18 | `SonyBRC300` | Blocking serial / Raw | — | Serial QA | Pending (Not run) | Pending | Pending — serial capture and wiring |
| PT-19 | `NearusBRC300` | TCP / Raw | 5678 | Nearus QA | Pending (Not run) | Pending | Pending — transcript and packet capture |
| PT-20 | `NearusBRC300` | UDP / Raw | 1259 | Nearus QA | Pending (Not run) | Pending | Pending — transcript and packet capture |
| PT-21 | `NearusBRC300` | Blocking serial / Raw | — | Serial QA | Pending (Not run) | Pending | Pending — serial capture and wiring |
| PT-22 | `SonyFR7` | UDP / Sony | 52381 | Sony QA | Pending (Not run) | Pending | Pending — encapsulated packet capture |
| PT-23 | `SonyBRCH900` | UDP / Sony | 52381 | Sony QA | Pending (Not run) | Pending | Pending — encapsulated packet capture |

## Firmware and profile behavior

At least one exact firmware revision must be recorded for each profile family.
If a row cannot be exercised, mark it `Blocked` and preserve the reason rather
than treating a neighboring model or firmware as equivalent.

| ID | Profile family / firmware revision | Required checks | Owner | Status | Firmware / bench | Evidence artifact / notes |
| --- | --- | --- | --- | --- | --- | --- |
| FW-01 | `GenericVisca` / exact revision | Profile facts, shared `04 39` exposure and `04 4B` iris control/inquiries, safe stop | Profile QA | Pending (Not run) | Pending | Pending |
| FW-02 | `PtzOpticsG2` / exact revision | Profile limits, shared exposure/iris, movement, inquiries, safe stop | Profile QA | Pending (Not run) | Pending | Pending |
| FW-03 | `PtzOpticsG3` / exact revision | Profile limits, shared exposure/iris, movement, inquiries, safe stop | Profile QA | Pending (Not run) | Pending | Pending |
| FW-04 | `PtzOptics30X` / exact revision | Profile limits, shared exposure/iris, movement, inquiries, safe stop | Profile QA | Pending (Not run) | Pending | Pending |
| FW-05 | `SonyEVIH100` / exact revision | Raw framing, shared exposure/iris (including the pending R8 line-item audit), profile limits, safe stop | Profile QA | Pending (Not run) | Pending | Pending |
| FW-06 | `SonyBRC300` / exact revision | Raw framing, shared exposure/iris, profile limits, safe stop | Profile QA | Pending (Not run) | Pending | Pending |
| FW-07 | `NearusBRC300` / exact revision | Raw framing, BRC-300-compatible shared exposure/iris, profile limits, safe stop | Profile QA | Pending (Not run) | Pending | Pending |
| FW-08 | `SonyFR7` / exact revision | Sony envelope, sequence correlation, reject shared exposure/iris before write, validate vendor-relative iris via raw escape hatch, safe stop | Profile QA | Pending (Not run) | Pending | Pending |
| FW-09 | `SonyBRCH900` / exact revision | Sony envelope, sequence correlation, shared exposure/iris, safe stop | Profile QA | Pending (Not run) | Pending | Pending |

## Wire-correction verification

These rows correspond one-for-one with the source-backed wire families changed
from the pinned 1.2 oracle in #715. The exhaustive software golden inventory is
necessary evidence, but does not complete a hardware row.

| ID | Profile / changed family | Required checks | Owner | Status | Firmware / bench | Evidence artifact / notes |
| --- | --- | --- | --- | --- | --- | --- |
| WC-01 | PTZOptics legacy profile / autofocus sensitivity | Send Low, Normal, High; capture `04 58 03/02/01`; inquire each value and verify semantic round trip | Protocol QA | Pending (Not run) | Pending | Pending — command/reply capture |
| WC-02 | `SonyEVIH100` and one supporting PTZOptics profile / Bright Direct | Capture `04 4D` for two distinct direct values and confirm the camera changes/reads back brightness; verify reset/up/down remain `04 0D` | Protocol QA | Pending (Not run) | Pending | Pending — R8/R1 comparison and capture |
| WC-03 | One standard VISCA profile and `SonyBRC300` / pan-tilt limit corner | Set and clear both corners; capture `W=00` DownLeft and `W=01` UpRight, then safely exercise the limits | Motion QA | Pending (Not run) | Pending | Pending — safe-bench video plus capture |
| WC-04 | Supporting PTZOptics legacy profile / focus zone inquiry | Send `09 04 AA`; select Top/Center/Bottom and confirm each returned value | Protocol QA | Pending (Not run) | Pending | Pending — command/reply capture |
| WC-05 | Supporting PTZOptics profile / picture effect | Send/inquire Off and BlackAndWhite with `04/09 04 63`; confirm `09 04 63` is not interpreted as resolution | Image QA | Pending (Not run) | Pending | Pending — image evidence plus capture |
| WC-06 | Supporting PTZOptics legacy profile / USB audio | Capture command/inquiry family `2A 02 A0 04`; verify reply `02` means on and `03` means off | Protocol QA | Pending (Not run) | Pending | Pending — USB audio observation plus capture |
| WC-07 | Profiles supporting preset 255 and Direct Menu / terminal data `FF` | Send preset set/recall/reset 255 and Direct Menu values ending in `FF`; prove the final data byte and separate terminator are both present; prove `FF 81` is rejected before I/O | Protocol QA | Pending (Not run) | Pending | Pending — exact frame and zero-I/O rejection trace |
| WC-08 | `SonyBRC300` plus PTZOptics control / extended tilt-speed domain | Exercise BRC-300 `VV=15` and `VV=18`; confirm PTZOptics still rejects tilt speed above `14` before I/O | Motion QA | Pending (Not run) | Pending | Pending — safe-bench capture and zero-I/O trace |
| WC-09 | `PtzOpticsG2`, `PtzOpticsG3`, and legacy `PtzOptics30X` / NR zero | Send 2D and 3D level `0`; confirm it disables NR, then set nonzero endpoints and query the documented output range | Image QA | Pending (Not run) | Pending | Pending — before/after image and command/reply capture |
| WC-10 | `SonyFR7` Sony envelope / payload-type table | Capture or inject device-setting, control-command, and control-reply headers and verify `01 20`, `02 00`, and `02 01`; reject the former 1.x values | Protocol QA | Pending (Not run) | Pending | Pending — Sony-header packet capture |
| WC-11 | `SonyBRC300` / position grammar | Capture absolute and relative commands with equal `VV`; verify fixed `00`, five signed pan nibbles, four signed tilt nibbles, documented polarity/endpoints, and local rejection of unequal pan/tilt speeds | Motion QA | Pending (Not run) | Pending | Pending — R12 pp. 12/22 comparison, safe-bench video, and capture |

## Cancellation, retry, and recovery

Run the cancellation and retry rows on every applicable profile/transport row
above, or link each result to the corresponding `PT-*` and `FW-*` evidence.
Never claim that a cancellation stopped physical motion without observing the
camera and recording the result.

| ID | Scope | Required checks | Owner | Status | Firmware / bench | Evidence artifact / notes |
| --- | --- | --- | --- | --- | --- | --- |
| CR-01 | Raw TCP profiles | Urgent queued cancellation ahead of ordinary work, shared-spacing compliance, supported/unsupported sent cancellation, closed-owner outcome | Lifecycle QA | Pending (Not run) | Pending | Pending |
| CR-02 | Raw UDP profiles | No replay after sent-command ACK/completion/cancellation ambiguity, receive fault while awaiting ACK, or active retry-budget expiry; conclusive-rejection retry handling; urgent cancellation obeys shared spacing; cancellation and late-reply handling | Lifecycle QA | Pending (Not run) | Pending | Pending |
| CR-03 | Sony UDP profiles | Same-sequence retry, urgent cancellation obeying shared spacing, cancellation result, and late-reply handling | Lifecycle QA | Pending (Not run) | Pending | Pending |
| CR-04 | Blocking serial profiles | Raw ambiguity/no-replay boundary, timeout, cancellation boundary, shared spacing, explicit STOP | Lifecycle QA | Pending (Not run) | Pending | Pending |
| CR-05 | Tokio serial profile paths | Raw ambiguity/no-replay boundary, timeout, cancellation boundary, shared spacing, explicit STOP | Lifecycle QA | Pending (Not run) | Pending | Pending |
| CR-06 | Every applicable profile | `UnsequencedCommandUnconfirmed` is per-request by default (#671) — the session keeps running, reconcile the one command, no automatic replay; a fresh replacement session is required only after transport close or under the `strict_unconfirmed_poison` opt-in; cache starts unknown | Recovery QA | Pending (Not run) | Pending | Pending |
| CR-07 | PTZOptics NDI TCP (and any TCP profile) (#544, #719) | Whether an idle control session is closed by the camera with default keepalive on (idle 10 s / interval 10 s; OS probe count applies); the idle interval before the close measured from the last VISCA operation; whether an application heartbeat inquiry below that interval prevents it; silent-open behavior and `received_frames` around the heartbeat; that FIN/RST or keepalive `TimedOut` surfaces as `ConnectionClosed`/`requires_new_session()` and drives a clean supervisor rebuild | Recovery QA | Pending (Not run) | Pending | Pending — packet capture with FIN/RST direction, idle interval, heartbeat on/off, and a black-holed peer through keepalive exhaustion |
| RT-01 | Raw TCP profiles | Ordinary one-command pre-ACK gate per target, lost-ACK ordinary release at the ambiguity deadline, `Urgent` stop write through one open candidate with ambiguous ACK binding neither (#714), post-ACK socket concurrency, exact fixed-frame lengths, no replay on ambiguity or active retry-budget expiry, conclusive-rejection retry, total retry budget/cause | Runtime QA | Pending (Not run) | Pending | Pending |
| RT-02 | Raw UDP profiles | Ordinary one-command pre-ACK gate per target, lost-ACK ordinary release at the ambiguity deadline, `Urgent` stop write through one open candidate with ambiguous ACK binding neither (#714), post-ACK socket concurrency, exact fixed-frame lengths, empty-datagram discard under one deadline with async cooperative yield, no replay on ambiguity or active retry-budget expiry, conclusive-rejection retry, total retry budget/cause | Runtime QA | Pending (Not run) | Pending | Pending |
| RT-03 | Sony UDP profiles | Pre-ACK pipeline, exact sequence correlation, same-sequence ACK/completion retry, active retry-budget expiry handling, retry limit/backoff and total budget/cause | Runtime QA | Pending (Not run) | Pending | Pending |
| RT-04 | Serial profiles | Read/write timeout, raw ambiguity/no-replay boundary including active retry-budget expiry, retry limit/backoff and total budget/cause, buffer sizing | Runtime QA | Pending (Not run) | Pending | Pending |

## Multi-camera and addressing

These rows verify that target registration and response attribution remain safe;
they do not authorize sending to broadcast as a session target.

| ID | Scenario | Required checks | Owner | Status | Firmware / bench | Evidence artifact / notes |
| --- | --- | --- | --- | --- | --- | --- |
| MC-01 | Raw serial bus with IDs 1–7 | Heterogeneous compatible raw profiles, target-local inquiries/cache, no cross-target replies | Multi-camera QA | Pending (Not run) | Pending | Pending — serial capture and target log |
| MC-02 | IP session with two registered targets | Registration succeeds only where response attribution is safe; `camera_for` selects the target | Multi-camera QA | Pending (Not run) | Pending | Pending — config and packet evidence |
| MC-03 | Duplicate, broadcast, zero, or eighth target | Preflight rejects before device open, socket creation, or protocol I/O | API QA | Pending (Not run) | Pending | Pending — zero-I/O trace |
| MC-04 | Mixed raw/Sony envelope registry | Preflight rejects incompatible shared transport before I/O | API QA | Pending (Not run) | Pending | Pending — zero-I/O trace |
| MC-05 | Reconnect after one target/transport failure | Fresh owner and target views do not inherit stale cache or lifecycle state | Recovery QA | Pending (Not run) | Pending | Pending — diagnostics/cache export |

## Release evidence and sign-off

| Gate | Owner | Status | Evidence / blocker |
| --- | --- | --- | --- |
| Every applicable `PT-*` and `FW-*` row has an exact firmware revision | Release owner | Pending (Not run) | Pending |
| Every `WC-*` wire-correction row has a command/reply capture or an explicit blocker | Release owner | Pending (Not run) | Pending |
| Every cancellation/retry row has a transcript and sanitized diagnostics | Lifecycle QA | Pending (Not run) | Pending |
| Every multi-camera row has target-attribution evidence | Multi-camera QA | Pending (Not run) | Pending |
| No open safety issue or unexplained physical behavior | Release owner | Pending (Not run) | Pending |
| Software feature/API/allocation gates are recorded separately | CI owner | Pending (Not run) | Pending |

Do not publish or describe hardware support as verified until the release owner
reviews the completed evidence and records the date and sign-off here.
