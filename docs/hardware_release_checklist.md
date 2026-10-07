# 2.0 hardware release checklist

This checklist records targeted representative physical-camera evidence. Under
the proportional policy authorized by #753 (superseding #738/D8 in #732), an RC
may be published while every row remains `Pending (Not run)`, provided its
release notes state plainly: **"No physical hardware validation has been
performed, and no hardware support has been verified for this release
candidate."** Software tests and packet fixtures do not constitute a hardware
pass.

For a stable release with major version 2 or higher, #753 required a
representative pass on all five rows. The maintainer decision of 2026-10-07
supersedes that requirement: a stable release requires `Pass` on HW-01, HW-04
and HW-05. HW-02 (Raw serial) and HW-03 (Sony UDP) may instead carry the status
`Not verified — accepted known limitation (<reason>)` with a dated maintainer
decision in their evidence cell. Until the hardware is available they remain
documented known limitations: implemented and covered by software tests, but
not verified on a physical camera.

## Recording rules

Before each run, record the camera model, exact firmware revision, transport,
profile type, camera address, host OS, adapter/runtime, power state, and
operator. Attach the raw command transcript or packet/serial capture and link
any failure issue. Stop immediately if a movement, power, or network test
behaves unexpectedly. Camera addresses and raw packet captures stay in the
private run record; published evidence names each camera by label, model and
firmware and carries no network address or device serial number.

Status vocabulary:

- `Pending (Not run)` means no physical evidence has been collected.
- `Blocked` means the required camera, firmware, cable, or safe bench is not
  available; record the blocker and owner.
- `Pass` or `Fail` is allowed only with the evidence fields completed.
- `Not verified — accepted known limitation (<reason>)` is allowed only for
  HW-02 and HW-03, with the dated maintainer decision in the evidence cell.

Current status: HW-01, HW-04 and HW-05 passed on the PTZOptics G2 bench on
2026-10-07 at the hardware-tested commit below. HW-02 and HW-03 were not run
and are accepted known limitations. No Raw serial or Sony UDP path has been
verified on hardware.

Hardware-tested commit: cc1ab8f1f752ee6c946bed11365e60b8d321ab36

A stable hardware sign-off requires the full 40-character commit SHA above.
One HW-04 run, the e5 stale-reply contract re-run, used
`6eba1696fd07e70312318897d9318a68c41b8298`, which differs from the commit above
only in `tests/hardware_experiments_test.rs` (a corrected test assertion).

Hardware operator/date: Grant Sparks (maintainer-observed run) — 2026-10-07 UTC

## Bench

PTZOptics G2 bench, 2026-10-07. Raw VISCA over TCP port 5678 and UDP port 1259
through the blocking and Tokio APIs, on a Linux x86_64 host with
`rustc 1.95.0`. Each camera was restored to preset 1 after motion.

| Camera | Model | Firmware (SOC / ARM) | Profile |
| --- | --- | --- | --- |
| cam1 | PT30X-NDI G2 | 6.3.32 / 6.3.51THI | `PtzOptics30X` in the HW-05 wire-row runs, `PtzOpticsG2` elsewhere |
| cam2, cam3 | PT20X-NDI G2 | 6.3.22 / 6.3.76THI | `PtzOpticsG2` |
| cam4, cam5 | PT12X-NDI G2 | 6.3.62 / 6.4.18SHI | `PtzOpticsG2` |

## Targeted representative scenarios

| ID | Required scenario | Status | Firmware / bench | Transcript / evidence |
| --- | --- | --- | --- | --- |
| HW-01 | Representative Raw TCP and UDP command, inquiry, and motion-safe-stop flow | Pass | PTZOptics G2 bench, all five cameras: PT30X-NDI G2 6.3.32/6.3.51THI; PT20X-NDI G2 6.3.22/6.3.76THI; PT12X-NDI G2 6.3.62/6.4.18SHI | 27/27 baseline inquiries on each camera; TCP pan/tilt drive and stop and UDP zoom drive and stop 10/10 ([P0 and P1 runs](../.github/hardware-evidence/2026-10-07-g2-bench/README.md#run-index)) |
| HW-02 | Blocking and Tokio Raw serial startup and timeout behavior, including attribution with at least two registered addresses | Not verified — accepted known limitation (no Raw serial adapter or serial camera on the bench) | No serial hardware available | Maintainer decision 2026-10-07 (#753): accepted known limitation for 2.0; covered by software tests only |
| HW-03 | Representative Sony UDP envelope and sequence-numbered command and inquiry flow | Not verified — accepted known limitation (no Sony camera on the bench) | No Sony camera available | Maintainer decision 2026-10-07 (#753): accepted known limitation for 2.0; covered by software tests only |
| HW-04 | Movement cancellation and owner halt: queued/retry fencing, independently dispatched STOPs and partial reports, then physical-rest observation, disconnect and recovery on representative Raw paths (the Sony path belongs to HW-03) | Pass | PTZOptics G2 bench, all five cameras (firmware as HW-01); disconnect, e1 and e5 runs on cam4 (PT12X-NDI G2 6.3.62/6.4.18SHI) | Halt idle report, halt fences queued motion, async G2 cancel refused then STOP 15/15; recovery after TCP reset and after silent drop; e1 manual-focus halt; e5 stale-reply contract at `6eba1696` (27 correlation refusals, 0 wrong values, then 8 correct answers on the same session) ([P3 runs](../.github/hardware-evidence/2026-10-07-g2-bench/README.md#run-index)) |
| HW-05 | Representative high-risk corrected and profile-specific wire rows, with exact firmware and a complete transcript | Pass | PTZOptics G2 bench, all five cameras (firmware as HW-01); cam1 with `PtzOptics30X`, cam2-cam5 with `PtzOpticsG2` | Corrected inquiries, NR level zero, focus zone, shutter round trip, direct zoom positions, absolute pan/tilt small offset 30/30; tilt polarity 5/5, with a visible ~9° UP move confirmed physically UP by the operator on all five ([P2 runs](../.github/hardware-evidence/2026-10-07-g2-bench/README.md#run-index)) |

## Concurrent multi-camera scenarios

These runs drive all five bench cameras at once from one process. They are
recorded evidence, not a separate release gate.

| Scenario | Result | Evidence |
| --- | --- | --- |
| hwc01: concurrent inquiries, Tokio | Pass | [run](../.github/hardware-evidence/2026-10-07-g2-bench/p4-hwc01_concurrent_inquiries_tokio/stdout.txt) |
| hwc02: concurrent inquiries, blocking threads | Pass | [run](../.github/hardware-evidence/2026-10-07-g2-bench/p4-hwc02_concurrent_inquiries_blocking_threads/stdout.txt) |
| hwc03: mixed TCP and UDP transports | Pass | [run](../.github/hardware-evidence/2026-10-07-g2-bench/p4-hwc03_mixed_transports/stdout.txt) |
| hwc04: concurrent drive and stop | Pass | [run](../.github/hardware-evidence/2026-10-07-g2-bench/p4-hwc04_concurrent_drive_and_stop/stdout.txt) |
| hwc05: one camera faulted does not stall the others | Pass: unfaulted cameras p50 134-149 ms, max < 255 ms; cam4 recovered on the same session | [run](../.github/hardware-evidence/2026-10-07-g2-bench/p4-hwc05_one_camera_fault_does_not_stall_others/stdout.txt) |

The final-state inquiries (P5) equal the P0 baseline except that cam1 and cam2
now rest at preset 1 (maintainer-approved) and the autofocus focus positions
drifted.

## Stable-release sign-off

For a stable release, HW-01, HW-04 and HW-05
must be `Pass`, with exact firmware and transcript or capture evidence; HW-02
and HW-03 must be `Pass` or an accepted known limitation as described above.
The recorded commit identifies the code the bench exercised; later changes do
not invalidate the pass by themselves. Run another targeted pass when a change
alters wire encoding, transport, correlation, or motion behaviour on a verified
path.

For an RC, leave the rows and provenance fields `Pending` when hardware has not
been run. Its release notes must state that no physical hardware validation has
been performed and no hardware support has been verified for that release
candidate. Do not describe hardware support as verified based on software CI,
the release tag, or an unrecorded manual observation. Release notes for a
release whose rows carry an accepted known limitation must name that
limitation.

Evidence index: [.github/hardware-evidence/2026-10-07-g2-bench/README.md](../.github/hardware-evidence/2026-10-07-g2-bench/README.md)

Final sign-off: Pending
