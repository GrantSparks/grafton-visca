# 2.0 hardware release checklist

This checklist records targeted representative physical-camera evidence. Under
the proportional policy authorized by #753 (superseding #738/D8 in #732), an RC
may be published while every row remains `Pending (Not run)`, provided its
release notes state plainly: **"No physical hardware validation has been
performed, and no hardware support has been verified for this release
candidate."** A stable release with major version 2 or higher requires a
representative pass on all five rows. Software tests and packet fixtures do not
constitute a hardware pass.

## Recording rules

Before each run, record the camera model, exact firmware revision, transport,
profile type, camera address, host OS, adapter/runtime, power state, and
operator. Attach the raw command transcript or packet/serial capture and link
any failure issue. Stop immediately if a movement, power, or network test
behaves unexpectedly.

Status vocabulary:

- `Pending (Not run)` means no physical evidence has been collected.
- `Blocked` means the required camera, firmware, cable, or safe bench is not
  available; record the blocker and owner.
- `Pass` or `Fail` is allowed only with the evidence fields completed.

The current `2.0.0-rc.1` status is intentionally unverified: no physical
hardware validation has been performed, and no hardware support has been
verified for this release candidate.

Hardware-tested commit: Pending

A stable hardware sign-off requires the full 40-character commit SHA above.

Hardware operator/date: Pending

## Targeted representative scenarios

| ID | Required scenario | Status | Firmware / bench | Transcript / evidence |
| --- | --- | --- | --- | --- |
| HW-01 | Representative Raw TCP and UDP command, inquiry, and motion-safe-stop flow | Pending (Not run) | Pending | Pending |
| HW-02 | Blocking and Tokio Raw serial startup and timeout behavior, including attribution with at least two registered addresses | Pending (Not run) | Pending | Pending |
| HW-03 | Representative Sony UDP envelope and sequence-numbered command and inquiry flow | Pending (Not run) | Pending | Pending |
| HW-04 | Movement cancellation and emergency stop, plus disconnect and recovery, on representative Raw and Sony paths | Pending (Not run) | Pending | Pending |
| HW-05 | Representative high-risk corrected and profile-specific wire rows, with exact firmware and a complete transcript | Pending (Not run) | Pending | Pending |

## Stable-release sign-off

Before a stable hardware pass, finalize the Rust sources, Cargo manifests,
and dependency metadata at the commit recorded above. The five rows must be
`Pass`, with exact firmware and transcript or capture evidence. Tests,
documentation, workflows, and release records may change afterward, but shipped
Rust sources and Cargo manifests may not. Release automation compares those
paths between `Hardware-tested commit:` and release `HEAD`; a difference
requires another targeted pass.

For an RC, leave the rows and provenance fields `Pending` when hardware has not
been run. Its release notes must state that no physical hardware validation has
been performed and no hardware support has been verified for that release
candidate. Do not describe hardware support as verified based on software CI,
the release tag, or an unrecorded manual observation.

Evidence index: Pending

Final sign-off: Pending
