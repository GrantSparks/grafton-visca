# PTZOptics G2 bench evidence, 2026-10-07

Raw run evidence for rows HW-01, HW-04 and HW-05 and for the concurrent
multi-camera scenarios of the
[2.0 hardware release checklist](../../../docs/hardware_release_checklist.md).
HW-02 and HW-03 were not run on this bench; the checklist records their status.

## Provenance

- Hardware-tested commit: `cc1ab8f1f752ee6c946bed11365e60b8d321ab36` for every run except
  `p3-e5-stale-reply-contract-cam4`, which ran at `6eba1696fd07e70312318897d9318a68c41b8298`. That commit
  differs from `cc1ab8f1` only in `tests/hardware_experiments_test.rs`
  (corrected e5 assertion); shipped sources and manifests are identical.
- Date: 2026-10-07 (UTC times in each `meta.txt`).
- Operator: maintainer-supervised automated run (Grant Sparks). The maintainer
  watched the run and visually confirmed the tilt direction
  ([confirmation](p2-visible-tilt-operator-confirmation.txt)).
- Host: Linux 6.8.0 x86_64, Debian GNU/Linux 12; toolchain `rustc 1.95.0`
  (59807616e 2026-04-14); release-profile test binaries run with
  `--ignored --nocapture --test-threads=1`. Every run's tree was clean.
- Transports: Raw VISCA over TCP port 5678 and UDP port 1259, through the
  blocking and Tokio APIs.

## Cameras

| Label | Model | devtype | Firmware (SOC / ARM) | Profile used | Identity |
| --- | --- | --- | --- | --- | --- |
| cam1 | PT30X-NDI G2 | V63C | 6.3.32 / 6.3.51THI | `PtzOptics30X` in the HW-05 wire-row runs; `PtzOpticsG2` elsewhere | [cam1.txt](p0-identity/cam1.txt) |
| cam2 | PT20X-NDI G2 | V60 | 6.3.22 / 6.3.76THI | `PtzOpticsG2` | [cam2.txt](p0-identity/cam2.txt) |
| cam3 | PT20X-NDI G2 | V60 | 6.3.22 / 6.3.76THI | `PtzOpticsG2` | [cam3.txt](p0-identity/cam3.txt) |
| cam4 | PT12X-NDI G2 | V61 | 6.3.62 / 6.4.18SHI | `PtzOpticsG2` | [cam4.txt](p0-identity/cam4.txt) |
| cam5 | PT12X-NDI G2 | V61 | 6.3.62 / 6.4.18SHI | `PtzOpticsG2` | [cam5.txt](p0-identity/cam5.txt) |

Cameras are identified by model and firmware only. The identity dumps keep
`devtype`, `versioninfo` and `device_model`; device-unique fields are removed.

## Files

Each run folder holds:

- `meta.txt`: run id, camera label, tested commit, clean-tree flag, host,
  toolchain, operator, start/end time, command and exit code.
- `stdout.txt`: the test's `HW|` observation lines and the libtest result.
- `transcript.txt`: every VISCA frame on the wire, one per line, with
  timestamp, camera label, transport, ports and direction.
- `fault.log` (fault-injection runs only): `FAULT_ON`/`FAULT_OFF` times of a
  host-side packet-filter rule on cam4's TCP port 5678 (TCP reset or silent
  drop), followed by the filter policy after the rule was removed.

Packet captures (`capture.pcap`, one per run) are retained privately by the
maintainer and are not published, because their packet headers carry LAN
addresses. Each `transcript.txt` is decoded from that run's capture by the
bench's pcap-to-VISCA decoder. Camera addresses are replaced by `cam1`..`cam5`;
`target/` paths are the release test binaries, and `bench/` paths name the
bench's own fault-flag files and the visible-tilt script, which are not part
of this repository. The visible-tilt runs print their own frame log to
`stdout.txt`.

## Results

| Phase | What ran | Result |
| --- | --- | --- |
| P0 | Baseline inquiries | 27/27 on all five cameras |
| P1 HW-01 | TCP pan/tilt drive and stop; UDP zoom drive and stop | 10/10 Pass |
| P2 HW-05 | Corrected inquiries, NR level zero, focus zone, shutter round trip, direct zoom positions, absolute pan/tilt small offset | 30/30 Pass |
| P2 HW-05 | Tilt polarity | 5/5 Pass; a visible ~9° (128-130 unit) UP move was confirmed physically UP on all five by the operator |
| P3 HW-04 | Halt idle report, halt fences queued motion, async G2 cancel refused then STOP | 15/15 Pass |
| P3 HW-04 | Recover after disconnect, TCP reset and silent drop (cam4) | Pass |
| P3 HW-04 | e1 manual-focus halt (cam4) | Pass |
| P3 HW-04 | e5 stale-reply contract (cam4, `6eba1696`) | Pass: 27 correlation refusals, 0 wrong values, then 8 correct answers on the same session |
| P4 | Concurrent hwc01-hwc05 (all five cameras, `PtzOpticsG2`) | Pass; in hwc05 the unfaulted cameras kept p50 134-149 ms and max < 255 ms while cam4 was faulted, and cam4 recovered on the same session |
| P5 | Final-state inquiries | 27/27 on all five; equal to P0 except that cam1 and cam2 now rest at preset 1 (maintainer-approved) and autofocus focus positions drifted |

## Run index

| Run | Row | Scenario | Camera | Commit | Packets | Result | Files |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `p0-inquiry-cam1` | P0 baseline (HW-01 inquiry) | 27 baseline inquiries | cam1 | `cc1ab8f1` | 361 | Pass (27/27) | [meta](p0-inquiry-cam1/meta.txt) [stdout](p0-inquiry-cam1/stdout.txt) [transcript](p0-inquiry-cam1/transcript.txt) |
| `p0-inquiry-cam2` | P0 baseline (HW-01 inquiry) | 27 baseline inquiries | cam2 | `cc1ab8f1` | 371 | Pass (27/27) | [meta](p0-inquiry-cam2/meta.txt) [stdout](p0-inquiry-cam2/stdout.txt) [transcript](p0-inquiry-cam2/transcript.txt) |
| `p0-inquiry-cam3` | P0 baseline (HW-01 inquiry) | 27 baseline inquiries | cam3 | `cc1ab8f1` | 375 | Pass (27/27) | [meta](p0-inquiry-cam3/meta.txt) [stdout](p0-inquiry-cam3/stdout.txt) [transcript](p0-inquiry-cam3/transcript.txt) |
| `p0-inquiry-cam4` | P0 baseline (HW-01 inquiry) | 27 baseline inquiries | cam4 | `cc1ab8f1` | 377 | Pass (27/27) | [meta](p0-inquiry-cam4/meta.txt) [stdout](p0-inquiry-cam4/stdout.txt) [transcript](p0-inquiry-cam4/transcript.txt) |
| `p0-inquiry-cam5` | P0 baseline (HW-01 inquiry) | 27 baseline inquiries | cam5 | `cc1ab8f1` | 366 | Pass (27/27) | [meta](p0-inquiry-cam5/meta.txt) [stdout](p0-inquiry-cam5/stdout.txt) [transcript](p0-inquiry-cam5/transcript.txt) |
| `p1-hw01_tcp_pan_tilt_drive_and_stop-cam1` | HW-01 | hw01_tcp_pan_tilt_drive_and_stop | cam1 | `cc1ab8f1` | 69 | Pass (1/1) | [meta](p1-hw01_tcp_pan_tilt_drive_and_stop-cam1/meta.txt) [stdout](p1-hw01_tcp_pan_tilt_drive_and_stop-cam1/stdout.txt) [transcript](p1-hw01_tcp_pan_tilt_drive_and_stop-cam1/transcript.txt) |
| `p1-hw01_tcp_pan_tilt_drive_and_stop-cam2` | HW-01 | hw01_tcp_pan_tilt_drive_and_stop | cam2 | `cc1ab8f1` | 70 | Pass (1/1) | [meta](p1-hw01_tcp_pan_tilt_drive_and_stop-cam2/meta.txt) [stdout](p1-hw01_tcp_pan_tilt_drive_and_stop-cam2/stdout.txt) [transcript](p1-hw01_tcp_pan_tilt_drive_and_stop-cam2/transcript.txt) |
| `p1-hw01_tcp_pan_tilt_drive_and_stop-cam3` | HW-01 | hw01_tcp_pan_tilt_drive_and_stop | cam3 | `cc1ab8f1` | 68 | Pass (1/1) | [meta](p1-hw01_tcp_pan_tilt_drive_and_stop-cam3/meta.txt) [stdout](p1-hw01_tcp_pan_tilt_drive_and_stop-cam3/stdout.txt) [transcript](p1-hw01_tcp_pan_tilt_drive_and_stop-cam3/transcript.txt) |
| `p1-hw01_tcp_pan_tilt_drive_and_stop-cam4` | HW-01 | hw01_tcp_pan_tilt_drive_and_stop | cam4 | `cc1ab8f1` | 67 | Pass (1/1) | [meta](p1-hw01_tcp_pan_tilt_drive_and_stop-cam4/meta.txt) [stdout](p1-hw01_tcp_pan_tilt_drive_and_stop-cam4/stdout.txt) [transcript](p1-hw01_tcp_pan_tilt_drive_and_stop-cam4/transcript.txt) |
| `p1-hw01_tcp_pan_tilt_drive_and_stop-cam5` | HW-01 | hw01_tcp_pan_tilt_drive_and_stop | cam5 | `cc1ab8f1` | 69 | Pass (1/1) | [meta](p1-hw01_tcp_pan_tilt_drive_and_stop-cam5/meta.txt) [stdout](p1-hw01_tcp_pan_tilt_drive_and_stop-cam5/stdout.txt) [transcript](p1-hw01_tcp_pan_tilt_drive_and_stop-cam5/transcript.txt) |
| `p1-hw01_udp_zoom_drive_and_stop-cam1` | HW-01 | hw01_udp_zoom_drive_and_stop | cam1 | `cc1ab8f1` | 31 | Pass (1/1) | [meta](p1-hw01_udp_zoom_drive_and_stop-cam1/meta.txt) [stdout](p1-hw01_udp_zoom_drive_and_stop-cam1/stdout.txt) [transcript](p1-hw01_udp_zoom_drive_and_stop-cam1/transcript.txt) |
| `p1-hw01_udp_zoom_drive_and_stop-cam2` | HW-01 | hw01_udp_zoom_drive_and_stop | cam2 | `cc1ab8f1` | 31 | Pass (1/1) | [meta](p1-hw01_udp_zoom_drive_and_stop-cam2/meta.txt) [stdout](p1-hw01_udp_zoom_drive_and_stop-cam2/stdout.txt) [transcript](p1-hw01_udp_zoom_drive_and_stop-cam2/transcript.txt) |
| `p1-hw01_udp_zoom_drive_and_stop-cam3` | HW-01 | hw01_udp_zoom_drive_and_stop | cam3 | `cc1ab8f1` | 31 | Pass (1/1) | [meta](p1-hw01_udp_zoom_drive_and_stop-cam3/meta.txt) [stdout](p1-hw01_udp_zoom_drive_and_stop-cam3/stdout.txt) [transcript](p1-hw01_udp_zoom_drive_and_stop-cam3/transcript.txt) |
| `p1-hw01_udp_zoom_drive_and_stop-cam4` | HW-01 | hw01_udp_zoom_drive_and_stop | cam4 | `cc1ab8f1` | 33 | Pass (1/1) | [meta](p1-hw01_udp_zoom_drive_and_stop-cam4/meta.txt) [stdout](p1-hw01_udp_zoom_drive_and_stop-cam4/stdout.txt) [transcript](p1-hw01_udp_zoom_drive_and_stop-cam4/transcript.txt) |
| `p1-hw01_udp_zoom_drive_and_stop-cam5` | HW-01 | hw01_udp_zoom_drive_and_stop | cam5 | `cc1ab8f1` | 31 | Pass (1/1) | [meta](p1-hw01_udp_zoom_drive_and_stop-cam5/meta.txt) [stdout](p1-hw01_udp_zoom_drive_and_stop-cam5/stdout.txt) [transcript](p1-hw01_udp_zoom_drive_and_stop-cam5/transcript.txt) |
| `p2-hw05_absolute_pan_tilt_small_offset-cam1` | HW-05 | hw05_absolute_pan_tilt_small_offset | cam1 | `cc1ab8f1` | 41 | Pass (1/1) | [meta](p2-hw05_absolute_pan_tilt_small_offset-cam1/meta.txt) [stdout](p2-hw05_absolute_pan_tilt_small_offset-cam1/stdout.txt) [transcript](p2-hw05_absolute_pan_tilt_small_offset-cam1/transcript.txt) |
| `p2-hw05_absolute_pan_tilt_small_offset-cam2` | HW-05 | hw05_absolute_pan_tilt_small_offset | cam2 | `cc1ab8f1` | 43 | Pass (1/1) | [meta](p2-hw05_absolute_pan_tilt_small_offset-cam2/meta.txt) [stdout](p2-hw05_absolute_pan_tilt_small_offset-cam2/stdout.txt) [transcript](p2-hw05_absolute_pan_tilt_small_offset-cam2/transcript.txt) |
| `p2-hw05_absolute_pan_tilt_small_offset-cam3` | HW-05 | hw05_absolute_pan_tilt_small_offset | cam3 | `cc1ab8f1` | 41 | Pass (1/1) | [meta](p2-hw05_absolute_pan_tilt_small_offset-cam3/meta.txt) [stdout](p2-hw05_absolute_pan_tilt_small_offset-cam3/stdout.txt) [transcript](p2-hw05_absolute_pan_tilt_small_offset-cam3/transcript.txt) |
| `p2-hw05_absolute_pan_tilt_small_offset-cam4` | HW-05 | hw05_absolute_pan_tilt_small_offset | cam4 | `cc1ab8f1` | 39 | Pass (1/1) | [meta](p2-hw05_absolute_pan_tilt_small_offset-cam4/meta.txt) [stdout](p2-hw05_absolute_pan_tilt_small_offset-cam4/stdout.txt) [transcript](p2-hw05_absolute_pan_tilt_small_offset-cam4/transcript.txt) |
| `p2-hw05_absolute_pan_tilt_small_offset-cam5` | HW-05 | hw05_absolute_pan_tilt_small_offset | cam5 | `cc1ab8f1` | 39 | Pass (1/1) | [meta](p2-hw05_absolute_pan_tilt_small_offset-cam5/meta.txt) [stdout](p2-hw05_absolute_pan_tilt_small_offset-cam5/stdout.txt) [transcript](p2-hw05_absolute_pan_tilt_small_offset-cam5/transcript.txt) |
| `p2-hw05_corrected_inquiries-cam1` | HW-05 | hw05_corrected_inquiries | cam1 | `cc1ab8f1` | 23 | Pass (1/1) | [meta](p2-hw05_corrected_inquiries-cam1/meta.txt) [stdout](p2-hw05_corrected_inquiries-cam1/stdout.txt) [transcript](p2-hw05_corrected_inquiries-cam1/transcript.txt) |
| `p2-hw05_corrected_inquiries-cam2` | HW-05 | hw05_corrected_inquiries | cam2 | `cc1ab8f1` | 24 | Pass (1/1) | [meta](p2-hw05_corrected_inquiries-cam2/meta.txt) [stdout](p2-hw05_corrected_inquiries-cam2/stdout.txt) [transcript](p2-hw05_corrected_inquiries-cam2/transcript.txt) |
| `p2-hw05_corrected_inquiries-cam3` | HW-05 | hw05_corrected_inquiries | cam3 | `cc1ab8f1` | 26 | Pass (1/1) | [meta](p2-hw05_corrected_inquiries-cam3/meta.txt) [stdout](p2-hw05_corrected_inquiries-cam3/stdout.txt) [transcript](p2-hw05_corrected_inquiries-cam3/transcript.txt) |
| `p2-hw05_corrected_inquiries-cam4` | HW-05 | hw05_corrected_inquiries | cam4 | `cc1ab8f1` | 22 | Pass (1/1) | [meta](p2-hw05_corrected_inquiries-cam4/meta.txt) [stdout](p2-hw05_corrected_inquiries-cam4/stdout.txt) [transcript](p2-hw05_corrected_inquiries-cam4/transcript.txt) |
| `p2-hw05_corrected_inquiries-cam5` | HW-05 | hw05_corrected_inquiries | cam5 | `cc1ab8f1` | 23 | Pass (1/1) | [meta](p2-hw05_corrected_inquiries-cam5/meta.txt) [stdout](p2-hw05_corrected_inquiries-cam5/stdout.txt) [transcript](p2-hw05_corrected_inquiries-cam5/transcript.txt) |
| `p2-hw05_direct_zoom_positions-cam1` | HW-05 | hw05_direct_zoom_positions | cam1 | `cc1ab8f1` | 47 | Pass (1/1) | [meta](p2-hw05_direct_zoom_positions-cam1/meta.txt) [stdout](p2-hw05_direct_zoom_positions-cam1/stdout.txt) [transcript](p2-hw05_direct_zoom_positions-cam1/transcript.txt) |
| `p2-hw05_direct_zoom_positions-cam2` | HW-05 | hw05_direct_zoom_positions | cam2 | `cc1ab8f1` | 49 | Pass (1/1) | [meta](p2-hw05_direct_zoom_positions-cam2/meta.txt) [stdout](p2-hw05_direct_zoom_positions-cam2/stdout.txt) [transcript](p2-hw05_direct_zoom_positions-cam2/transcript.txt) |
| `p2-hw05_direct_zoom_positions-cam3` | HW-05 | hw05_direct_zoom_positions | cam3 | `cc1ab8f1` | 47 | Pass (1/1) | [meta](p2-hw05_direct_zoom_positions-cam3/meta.txt) [stdout](p2-hw05_direct_zoom_positions-cam3/stdout.txt) [transcript](p2-hw05_direct_zoom_positions-cam3/transcript.txt) |
| `p2-hw05_direct_zoom_positions-cam4` | HW-05 | hw05_direct_zoom_positions | cam4 | `cc1ab8f1` | 47 | Pass (1/1) | [meta](p2-hw05_direct_zoom_positions-cam4/meta.txt) [stdout](p2-hw05_direct_zoom_positions-cam4/stdout.txt) [transcript](p2-hw05_direct_zoom_positions-cam4/transcript.txt) |
| `p2-hw05_direct_zoom_positions-cam5` | HW-05 | hw05_direct_zoom_positions | cam5 | `cc1ab8f1` | 48 | Pass (1/1) | [meta](p2-hw05_direct_zoom_positions-cam5/meta.txt) [stdout](p2-hw05_direct_zoom_positions-cam5/stdout.txt) [transcript](p2-hw05_direct_zoom_positions-cam5/transcript.txt) |
| `p2-hw05_focus_zone_round_trip-cam1` | HW-05 | hw05_focus_zone_round_trip | cam1 | `cc1ab8f1` | 27 | Pass (1/1) | [meta](p2-hw05_focus_zone_round_trip-cam1/meta.txt) [stdout](p2-hw05_focus_zone_round_trip-cam1/stdout.txt) [transcript](p2-hw05_focus_zone_round_trip-cam1/transcript.txt) |
| `p2-hw05_focus_zone_round_trip-cam2` | HW-05 | hw05_focus_zone_round_trip | cam2 | `cc1ab8f1` | 28 | Pass (1/1) | [meta](p2-hw05_focus_zone_round_trip-cam2/meta.txt) [stdout](p2-hw05_focus_zone_round_trip-cam2/stdout.txt) [transcript](p2-hw05_focus_zone_round_trip-cam2/transcript.txt) |
| `p2-hw05_focus_zone_round_trip-cam3` | HW-05 | hw05_focus_zone_round_trip | cam3 | `cc1ab8f1` | 26 | Pass (1/1) | [meta](p2-hw05_focus_zone_round_trip-cam3/meta.txt) [stdout](p2-hw05_focus_zone_round_trip-cam3/stdout.txt) [transcript](p2-hw05_focus_zone_round_trip-cam3/transcript.txt) |
| `p2-hw05_focus_zone_round_trip-cam4` | HW-05 | hw05_focus_zone_round_trip | cam4 | `cc1ab8f1` | 29 | Pass (1/1) | [meta](p2-hw05_focus_zone_round_trip-cam4/meta.txt) [stdout](p2-hw05_focus_zone_round_trip-cam4/stdout.txt) [transcript](p2-hw05_focus_zone_round_trip-cam4/transcript.txt) |
| `p2-hw05_focus_zone_round_trip-cam5` | HW-05 | hw05_focus_zone_round_trip | cam5 | `cc1ab8f1` | 28 | Pass (1/1) | [meta](p2-hw05_focus_zone_round_trip-cam5/meta.txt) [stdout](p2-hw05_focus_zone_round_trip-cam5/stdout.txt) [transcript](p2-hw05_focus_zone_round_trip-cam5/transcript.txt) |
| `p2-hw05_nr_level_zero_round_trip-cam1` | HW-05 | hw05_nr_level_zero_round_trip | cam1 | `cc1ab8f1` | 61 | Pass (1/1) | [meta](p2-hw05_nr_level_zero_round_trip-cam1/meta.txt) [stdout](p2-hw05_nr_level_zero_round_trip-cam1/stdout.txt) [transcript](p2-hw05_nr_level_zero_round_trip-cam1/transcript.txt) |
| `p2-hw05_nr_level_zero_round_trip-cam2` | HW-05 | hw05_nr_level_zero_round_trip | cam2 | `cc1ab8f1` | 67 | Pass (1/1) | [meta](p2-hw05_nr_level_zero_round_trip-cam2/meta.txt) [stdout](p2-hw05_nr_level_zero_round_trip-cam2/stdout.txt) [transcript](p2-hw05_nr_level_zero_round_trip-cam2/transcript.txt) |
| `p2-hw05_nr_level_zero_round_trip-cam3` | HW-05 | hw05_nr_level_zero_round_trip | cam3 | `cc1ab8f1` | 62 | Pass (1/1) | [meta](p2-hw05_nr_level_zero_round_trip-cam3/meta.txt) [stdout](p2-hw05_nr_level_zero_round_trip-cam3/stdout.txt) [transcript](p2-hw05_nr_level_zero_round_trip-cam3/transcript.txt) |
| `p2-hw05_nr_level_zero_round_trip-cam4` | HW-05 | hw05_nr_level_zero_round_trip | cam4 | `cc1ab8f1` | 59 | Pass (1/1) | [meta](p2-hw05_nr_level_zero_round_trip-cam4/meta.txt) [stdout](p2-hw05_nr_level_zero_round_trip-cam4/stdout.txt) [transcript](p2-hw05_nr_level_zero_round_trip-cam4/transcript.txt) |
| `p2-hw05_nr_level_zero_round_trip-cam5` | HW-05 | hw05_nr_level_zero_round_trip | cam5 | `cc1ab8f1` | 64 | Pass (1/1) | [meta](p2-hw05_nr_level_zero_round_trip-cam5/meta.txt) [stdout](p2-hw05_nr_level_zero_round_trip-cam5/stdout.txt) [transcript](p2-hw05_nr_level_zero_round_trip-cam5/transcript.txt) |
| `p2-hw05_shutter_round_trip-cam1` | HW-05 | hw05_shutter_round_trip | cam1 | `cc1ab8f1` | 89 | Pass (1/1) | [meta](p2-hw05_shutter_round_trip-cam1/meta.txt) [stdout](p2-hw05_shutter_round_trip-cam1/stdout.txt) [transcript](p2-hw05_shutter_round_trip-cam1/transcript.txt) |
| `p2-hw05_shutter_round_trip-cam2` | HW-05 | hw05_shutter_round_trip | cam2 | `cc1ab8f1` | 97 | Pass (1/1) | [meta](p2-hw05_shutter_round_trip-cam2/meta.txt) [stdout](p2-hw05_shutter_round_trip-cam2/stdout.txt) [transcript](p2-hw05_shutter_round_trip-cam2/transcript.txt) |
| `p2-hw05_shutter_round_trip-cam3` | HW-05 | hw05_shutter_round_trip | cam3 | `cc1ab8f1` | 96 | Pass (1/1) | [meta](p2-hw05_shutter_round_trip-cam3/meta.txt) [stdout](p2-hw05_shutter_round_trip-cam3/stdout.txt) [transcript](p2-hw05_shutter_round_trip-cam3/transcript.txt) |
| `p2-hw05_shutter_round_trip-cam4` | HW-05 | hw05_shutter_round_trip | cam4 | `cc1ab8f1` | 93 | Pass (1/1) | [meta](p2-hw05_shutter_round_trip-cam4/meta.txt) [stdout](p2-hw05_shutter_round_trip-cam4/stdout.txt) [transcript](p2-hw05_shutter_round_trip-cam4/transcript.txt) |
| `p2-hw05_shutter_round_trip-cam5` | HW-05 | hw05_shutter_round_trip | cam5 | `cc1ab8f1` | 90 | Pass (1/1) | [meta](p2-hw05_shutter_round_trip-cam5/meta.txt) [stdout](p2-hw05_shutter_round_trip-cam5/stdout.txt) [transcript](p2-hw05_shutter_round_trip-cam5/transcript.txt) |
| `p2-hw05_tilt_polarity-cam1` | HW-05 | hw05_tilt_polarity | cam1 | `cc1ab8f1` | 118 | Pass (1/1) | [meta](p2-hw05_tilt_polarity-cam1/meta.txt) [stdout](p2-hw05_tilt_polarity-cam1/stdout.txt) [transcript](p2-hw05_tilt_polarity-cam1/transcript.txt) |
| `p2-hw05_tilt_polarity-cam2` | HW-05 | hw05_tilt_polarity | cam2 | `cc1ab8f1` | 117 | Pass (1/1) | [meta](p2-hw05_tilt_polarity-cam2/meta.txt) [stdout](p2-hw05_tilt_polarity-cam2/stdout.txt) [transcript](p2-hw05_tilt_polarity-cam2/transcript.txt) |
| `p2-hw05_tilt_polarity-cam3` | HW-05 | hw05_tilt_polarity | cam3 | `cc1ab8f1` | 119 | Pass (1/1) | [meta](p2-hw05_tilt_polarity-cam3/meta.txt) [stdout](p2-hw05_tilt_polarity-cam3/stdout.txt) [transcript](p2-hw05_tilt_polarity-cam3/transcript.txt) |
| `p2-hw05_tilt_polarity-cam4` | HW-05 | hw05_tilt_polarity | cam4 | `cc1ab8f1` | 117 | Pass (1/1) | [meta](p2-hw05_tilt_polarity-cam4/meta.txt) [stdout](p2-hw05_tilt_polarity-cam4/stdout.txt) [transcript](p2-hw05_tilt_polarity-cam4/transcript.txt) |
| `p2-hw05_tilt_polarity-cam5` | HW-05 | hw05_tilt_polarity | cam5 | `cc1ab8f1` | 119 | Pass (1/1) | [meta](p2-hw05_tilt_polarity-cam5/meta.txt) [stdout](p2-hw05_tilt_polarity-cam5/stdout.txt) [transcript](p2-hw05_tilt_polarity-cam5/transcript.txt) |
| `p2-visible-tilt-cam1` | HW-05 | visible ~9° tilt UP then DOWN, operator-observed | cam1 | `cc1ab8f1` | 49 | Pass: UP +128 tilt units, operator saw UP | [meta](p2-visible-tilt-cam1/meta.txt) [stdout](p2-visible-tilt-cam1/stdout.txt) [transcript](p2-visible-tilt-cam1/transcript.txt) |
| `p2-visible-tilt-cam2` | HW-05 | visible ~9° tilt UP then DOWN, operator-observed | cam2 | `cc1ab8f1` | 49 | Pass: UP +128 tilt units, operator saw UP | [meta](p2-visible-tilt-cam2/meta.txt) [stdout](p2-visible-tilt-cam2/stdout.txt) [transcript](p2-visible-tilt-cam2/transcript.txt) |
| `p2-visible-tilt-cam3` | HW-05 | visible ~9° tilt UP then DOWN, operator-observed | cam3 | `cc1ab8f1` | 49 | Pass: UP +128 tilt units, operator saw UP | [meta](p2-visible-tilt-cam3/meta.txt) [stdout](p2-visible-tilt-cam3/stdout.txt) [transcript](p2-visible-tilt-cam3/transcript.txt) |
| `p2-visible-tilt-cam4` | HW-05 | visible ~9° tilt UP then DOWN, operator-observed | cam4 | `cc1ab8f1` | 48 | Pass: UP +130 tilt units, operator saw UP | [meta](p2-visible-tilt-cam4/meta.txt) [stdout](p2-visible-tilt-cam4/stdout.txt) [transcript](p2-visible-tilt-cam4/transcript.txt) |
| `p2-visible-tilt-cam5` | HW-05 | visible ~9° tilt UP then DOWN, operator-observed | cam5 | `cc1ab8f1` | 49 | Pass: UP +128 tilt units, operator saw UP | [meta](p2-visible-tilt-cam5/meta.txt) [stdout](p2-visible-tilt-cam5/stdout.txt) [transcript](p2-visible-tilt-cam5/transcript.txt) |
| `p3-e1-manual-focus-halt-cam4` | HW-04 | e1_manual_focus_halt | cam4 | `cc1ab8f1` | 40 | Pass (1/1) | [meta](p3-e1-manual-focus-halt-cam4/meta.txt) [stdout](p3-e1-manual-focus-halt-cam4/stdout.txt) [transcript](p3-e1-manual-focus-halt-cam4/transcript.txt) |
| `p3-e5-stale-reply-contract-cam4` | HW-04 | e5_stale_stream_reply_binding (#795 contract) | cam4 | `6eba1696` | 49 | Pass (1/1) | [meta](p3-e5-stale-reply-contract-cam4/meta.txt) [stdout](p3-e5-stale-reply-contract-cam4/stdout.txt) [transcript](p3-e5-stale-reply-contract-cam4/transcript.txt) [fault](p3-e5-stale-reply-contract-cam4/fault.log) |
| `p3-hw04-recover-drop-cam4` | HW-04 | hw04_recover_after_disconnect (silent drop) | cam4 | `cc1ab8f1` | 20 | Pass (1/1) | [meta](p3-hw04-recover-drop-cam4/meta.txt) [stdout](p3-hw04-recover-drop-cam4/stdout.txt) [transcript](p3-hw04-recover-drop-cam4/transcript.txt) [fault](p3-hw04-recover-drop-cam4/fault.log) |
| `p3-hw04-recover-reset-cam4` | HW-04 | hw04_recover_after_disconnect (TCP reset) | cam4 | `cc1ab8f1` | 17 | Pass (1/1) | [meta](p3-hw04-recover-reset-cam4/meta.txt) [stdout](p3-hw04-recover-reset-cam4/stdout.txt) [transcript](p3-hw04-recover-reset-cam4/transcript.txt) [fault](p3-hw04-recover-reset-cam4/fault.log) |
| `p3-hw04_async_g2_cancel_refused_then_stop-cam1` | HW-04 | hw04_async_g2_cancel_refused_then_stop | cam1 | `cc1ab8f1` | 42 | Pass (1/1) | [meta](p3-hw04_async_g2_cancel_refused_then_stop-cam1/meta.txt) [stdout](p3-hw04_async_g2_cancel_refused_then_stop-cam1/stdout.txt) [transcript](p3-hw04_async_g2_cancel_refused_then_stop-cam1/transcript.txt) |
| `p3-hw04_async_g2_cancel_refused_then_stop-cam2` | HW-04 | hw04_async_g2_cancel_refused_then_stop | cam2 | `cc1ab8f1` | 45 | Pass (1/1) | [meta](p3-hw04_async_g2_cancel_refused_then_stop-cam2/meta.txt) [stdout](p3-hw04_async_g2_cancel_refused_then_stop-cam2/stdout.txt) [transcript](p3-hw04_async_g2_cancel_refused_then_stop-cam2/transcript.txt) |
| `p3-hw04_async_g2_cancel_refused_then_stop-cam3` | HW-04 | hw04_async_g2_cancel_refused_then_stop | cam3 | `cc1ab8f1` | 42 | Pass (1/1) | [meta](p3-hw04_async_g2_cancel_refused_then_stop-cam3/meta.txt) [stdout](p3-hw04_async_g2_cancel_refused_then_stop-cam3/stdout.txt) [transcript](p3-hw04_async_g2_cancel_refused_then_stop-cam3/transcript.txt) |
| `p3-hw04_async_g2_cancel_refused_then_stop-cam4` | HW-04 | hw04_async_g2_cancel_refused_then_stop | cam4 | `cc1ab8f1` | 46 | Pass (1/1) | [meta](p3-hw04_async_g2_cancel_refused_then_stop-cam4/meta.txt) [stdout](p3-hw04_async_g2_cancel_refused_then_stop-cam4/stdout.txt) [transcript](p3-hw04_async_g2_cancel_refused_then_stop-cam4/transcript.txt) |
| `p3-hw04_async_g2_cancel_refused_then_stop-cam5` | HW-04 | hw04_async_g2_cancel_refused_then_stop | cam5 | `cc1ab8f1` | 42 | Pass (1/1) | [meta](p3-hw04_async_g2_cancel_refused_then_stop-cam5/meta.txt) [stdout](p3-hw04_async_g2_cancel_refused_then_stop-cam5/stdout.txt) [transcript](p3-hw04_async_g2_cancel_refused_then_stop-cam5/transcript.txt) |
| `p3-hw04_halt_fences_queued_motion-cam1` | HW-04 | hw04_halt_fences_queued_motion | cam1 | `cc1ab8f1` | 108 | Pass (1/1) | [meta](p3-hw04_halt_fences_queued_motion-cam1/meta.txt) [stdout](p3-hw04_halt_fences_queued_motion-cam1/stdout.txt) [transcript](p3-hw04_halt_fences_queued_motion-cam1/transcript.txt) |
| `p3-hw04_halt_fences_queued_motion-cam2` | HW-04 | hw04_halt_fences_queued_motion | cam2 | `cc1ab8f1` | 105 | Pass (1/1) | [meta](p3-hw04_halt_fences_queued_motion-cam2/meta.txt) [stdout](p3-hw04_halt_fences_queued_motion-cam2/stdout.txt) [transcript](p3-hw04_halt_fences_queued_motion-cam2/transcript.txt) |
| `p3-hw04_halt_fences_queued_motion-cam3` | HW-04 | hw04_halt_fences_queued_motion | cam3 | `cc1ab8f1` | 108 | Pass (1/1) | [meta](p3-hw04_halt_fences_queued_motion-cam3/meta.txt) [stdout](p3-hw04_halt_fences_queued_motion-cam3/stdout.txt) [transcript](p3-hw04_halt_fences_queued_motion-cam3/transcript.txt) |
| `p3-hw04_halt_fences_queued_motion-cam4` | HW-04 | hw04_halt_fences_queued_motion | cam4 | `cc1ab8f1` | 107 | Pass (1/1) | [meta](p3-hw04_halt_fences_queued_motion-cam4/meta.txt) [stdout](p3-hw04_halt_fences_queued_motion-cam4/stdout.txt) [transcript](p3-hw04_halt_fences_queued_motion-cam4/transcript.txt) |
| `p3-hw04_halt_fences_queued_motion-cam5` | HW-04 | hw04_halt_fences_queued_motion | cam5 | `cc1ab8f1` | 105 | Pass (1/1) | [meta](p3-hw04_halt_fences_queued_motion-cam5/meta.txt) [stdout](p3-hw04_halt_fences_queued_motion-cam5/stdout.txt) [transcript](p3-hw04_halt_fences_queued_motion-cam5/transcript.txt) |
| `p3-hw04_halt_idle_report-cam1` | HW-04 | hw04_halt_idle_report | cam1 | `cc1ab8f1` | 23 | Pass (1/1) | [meta](p3-hw04_halt_idle_report-cam1/meta.txt) [stdout](p3-hw04_halt_idle_report-cam1/stdout.txt) [transcript](p3-hw04_halt_idle_report-cam1/transcript.txt) |
| `p3-hw04_halt_idle_report-cam2` | HW-04 | hw04_halt_idle_report | cam2 | `cc1ab8f1` | 23 | Pass (1/1) | [meta](p3-hw04_halt_idle_report-cam2/meta.txt) [stdout](p3-hw04_halt_idle_report-cam2/stdout.txt) [transcript](p3-hw04_halt_idle_report-cam2/transcript.txt) |
| `p3-hw04_halt_idle_report-cam3` | HW-04 | hw04_halt_idle_report | cam3 | `cc1ab8f1` | 23 | Pass (1/1) | [meta](p3-hw04_halt_idle_report-cam3/meta.txt) [stdout](p3-hw04_halt_idle_report-cam3/stdout.txt) [transcript](p3-hw04_halt_idle_report-cam3/transcript.txt) |
| `p3-hw04_halt_idle_report-cam4` | HW-04 | hw04_halt_idle_report | cam4 | `cc1ab8f1` | 23 | Pass (1/1) | [meta](p3-hw04_halt_idle_report-cam4/meta.txt) [stdout](p3-hw04_halt_idle_report-cam4/stdout.txt) [transcript](p3-hw04_halt_idle_report-cam4/transcript.txt) |
| `p3-hw04_halt_idle_report-cam5` | HW-04 | hw04_halt_idle_report | cam5 | `cc1ab8f1` | 23 | Pass (1/1) | [meta](p3-hw04_halt_idle_report-cam5/meta.txt) [stdout](p3-hw04_halt_idle_report-cam5/stdout.txt) [transcript](p3-hw04_halt_idle_report-cam5/transcript.txt) |
| `p4-hwc01_concurrent_inquiries_tokio` | Concurrent | hwc01_concurrent_inquiries_tokio | cam1-cam5 | `cc1ab8f1` | 2094 | Pass (1/1) | [meta](p4-hwc01_concurrent_inquiries_tokio/meta.txt) [stdout](p4-hwc01_concurrent_inquiries_tokio/stdout.txt) [transcript](p4-hwc01_concurrent_inquiries_tokio/transcript.txt) |
| `p4-hwc02_concurrent_inquiries_blocking_threads` | Concurrent | hwc02_concurrent_inquiries_blocking_threads | cam1-cam5 | `cc1ab8f1` | 2080 | Pass (1/1) | [meta](p4-hwc02_concurrent_inquiries_blocking_threads/meta.txt) [stdout](p4-hwc02_concurrent_inquiries_blocking_threads/stdout.txt) [transcript](p4-hwc02_concurrent_inquiries_blocking_threads/transcript.txt) |
| `p4-hwc03_mixed_transports` | Concurrent | hwc03_mixed_transports | cam1-cam5 | `cc1ab8f1` | 1722 | Pass (1/1) | [meta](p4-hwc03_mixed_transports/meta.txt) [stdout](p4-hwc03_mixed_transports/stdout.txt) [transcript](p4-hwc03_mixed_transports/transcript.txt) |
| `p4-hwc04_concurrent_drive_and_stop` | Concurrent | hwc04_concurrent_drive_and_stop | cam1-cam5 | `cc1ab8f1` | 601 | Pass (1/1) | [meta](p4-hwc04_concurrent_drive_and_stop/meta.txt) [stdout](p4-hwc04_concurrent_drive_and_stop/stdout.txt) [transcript](p4-hwc04_concurrent_drive_and_stop/transcript.txt) |
| `p4-hwc05_one_camera_fault_does_not_stall_others` | Concurrent | hwc05_one_camera_fault_does_not_stall_others | cam1-cam5 | `cc1ab8f1` | 1892 | Pass (1/1) | [meta](p4-hwc05_one_camera_fault_does_not_stall_others/meta.txt) [stdout](p4-hwc05_one_camera_fault_does_not_stall_others/stdout.txt) [transcript](p4-hwc05_one_camera_fault_does_not_stall_others/transcript.txt) [fault](p4-hwc05_one_camera_fault_does_not_stall_others/fault.log) |
| `p5-final-inquiry-cam1` | P5 final state | 27 final-state inquiries | cam1 | `cc1ab8f1` | 365 | Pass (27/27) | [meta](p5-final-inquiry-cam1/meta.txt) [stdout](p5-final-inquiry-cam1/stdout.txt) [transcript](p5-final-inquiry-cam1/transcript.txt) |
| `p5-final-inquiry-cam2` | P5 final state | 27 final-state inquiries | cam2 | `cc1ab8f1` | 359 | Pass (27/27) | [meta](p5-final-inquiry-cam2/meta.txt) [stdout](p5-final-inquiry-cam2/stdout.txt) [transcript](p5-final-inquiry-cam2/transcript.txt) |
| `p5-final-inquiry-cam3` | P5 final state | 27 final-state inquiries | cam3 | `cc1ab8f1` | 372 | Pass (27/27) | [meta](p5-final-inquiry-cam3/meta.txt) [stdout](p5-final-inquiry-cam3/stdout.txt) [transcript](p5-final-inquiry-cam3/transcript.txt) |
| `p5-final-inquiry-cam4` | P5 final state | 27 final-state inquiries | cam4 | `cc1ab8f1` | 366 | Pass (27/27) | [meta](p5-final-inquiry-cam4/meta.txt) [stdout](p5-final-inquiry-cam4/stdout.txt) [transcript](p5-final-inquiry-cam4/transcript.txt) |
| `p5-final-inquiry-cam5` | P5 final state | 27 final-state inquiries | cam5 | `cc1ab8f1` | 366 | Pass (27/27) | [meta](p5-final-inquiry-cam5/meta.txt) [stdout](p5-final-inquiry-cam5/stdout.txt) [transcript](p5-final-inquiry-cam5/transcript.txt) |

## Superseded runs

Published for completeness; they are not counted in the results above.

| Run | Camera | Commit | Recorded result | Reason | Files |
| --- | --- | --- | --- | --- | --- |
| `superseded/p3-e5-stale-reply-cam4` | cam4 | `cc1ab8f1` | Fail (0/1) | Failed (exit 101) only because the experiment's old assertion counted the documented `InquiryCorrelationLost` refusals as wrong values: all 8 post-fault inquiries were refused and none returned a value. The assertion was corrected in `6eba1696` and the run repeated as `p3-e5-stale-reply-contract-cam4`. | [meta](superseded/p3-e5-stale-reply-cam4/meta.txt) [stdout](superseded/p3-e5-stale-reply-cam4/stdout.txt) [transcript](superseded/p3-e5-stale-reply-cam4/transcript.txt) [fault](superseded/p3-e5-stale-reply-cam4/fault.log) |
| `superseded/p4-hwc01_concurrent_inquiries_tokio` | cam1-cam5 | `cc1ab8f1` | Pass (1/1) | First concurrent run. It passed, but its packet capture never started (the capture host filter was invalid), so it has no transcript. Repeated with capture as `p4-hwc01_concurrent_inquiries_tokio`. | [meta](superseded/p4-hwc01_concurrent_inquiries_tokio/meta.txt) [stdout](superseded/p4-hwc01_concurrent_inquiries_tokio/stdout.txt) |
| `superseded/p4-hwc02_concurrent_inquiries_blocking_threads` | cam1-cam5 | `cc1ab8f1` | Pass (1/1) | First concurrent run. It passed, but its packet capture never started (the capture host filter was invalid), so it has no transcript. Repeated with capture as `p4-hwc02_concurrent_inquiries_blocking_threads`. | [meta](superseded/p4-hwc02_concurrent_inquiries_blocking_threads/meta.txt) [stdout](superseded/p4-hwc02_concurrent_inquiries_blocking_threads/stdout.txt) |
| `superseded/p4-hwc03_mixed_transports` | cam1-cam5 | `cc1ab8f1` | Pass (1/1) | First concurrent run. It passed, but its packet capture never started (the capture host filter was invalid), so it has no transcript. Repeated with capture as `p4-hwc03_mixed_transports`. | [meta](superseded/p4-hwc03_mixed_transports/meta.txt) [stdout](superseded/p4-hwc03_mixed_transports/stdout.txt) |
| `superseded/p4-hwc04_concurrent_drive_and_stop` | cam1-cam5 | `cc1ab8f1` | Pass (1/1) | First concurrent run. It passed, but its packet capture never started (the capture host filter was invalid), so it has no transcript. Repeated with capture as `p4-hwc04_concurrent_drive_and_stop`. | [meta](superseded/p4-hwc04_concurrent_drive_and_stop/meta.txt) [stdout](superseded/p4-hwc04_concurrent_drive_and_stop/stdout.txt) |
