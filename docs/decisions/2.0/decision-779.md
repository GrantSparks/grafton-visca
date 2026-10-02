# Decision record: Owner-level halt with prompt STOP dispatch and per-axis results (#779)

Status: Proposed — awaiting maintainer ratification

Decision number: assigned at ratification. Does not reverse #567
(detach-on-drop) or D10; cancellation stays distinct from STOP.

## Context

`stop_all_motion()` is a facade composition. The async version submits the
pan/tilt STOP and awaits its `applied()` before submitting the zoom STOP, then
focus (`src/async_session.rs:572-603`); the blocking version is identical in
shape (`src/blocking.rs:903-937`). Both attempt every supported axis after an
earlier failure but keep only the first error (`retain_first_error`). The
public signature is `Result<()>` on every facade (`src/blocking_nouns.rs:430`,
`src/dynapi/nouns.rs:428`, `src/dynapi/blocking_projection.rs:140`;
`api/2.0.0-rc.1/all-features.txt:341`). Engine requests carry no motion-axis
metadata (`RuntimeRequest` in `src/runtime/engine/types.rs:387-400`;
`RequestContext` at `src/runtime/engine/types.rs:277-287`), so the owner cannot
currently identify which queued or retrying requests affect a given axis.
Nothing prevents older queued motion from being dispatched after the STOPs.

## Recommendation

**Defer to 2.x (owner-level halt as a new named operation); in 2.0, land only
the submit-before-observe ordering and documentation for `stop_all_motion()`.**

What lands in 2.0:

1. Async and dyn `stop_all_motion()` submit every supported STOP before
   awaiting any of them, then await all and keep the existing first-error
   result. Signature unchanged.
2. Blocking `stop_all_motion()` keeps sequential submit-then-observe (see
   rationale), with the difference documented as a latency difference, not a
   semantic one.
3. Documentation states that `stop_all_motion()` is a composition of typed
   STOPs, is not atomic, does not suppress older queued or retrying motion,
   and returns only the first error.

Deferred to 2.x (additive): a new owner action (name illustrative:
`motion().halt(axes)`) that establishes an ordering point at the owner,
supersedes unsent pre-halt motion and suppresses its retries for the selected
axes, dispatches eligible STOPs independently, and returns a per-axis report
under one end-to-end deadline. `stop_all_motion()` may later delegate to it
while keeping its `Result<()>` contract.

## Rationale

The full proposal needs immutable axis/effect metadata carried from
preparation into the engine, suppression rules for queued and retrying
entries, a per-target ordering generation, a new report type, and a defined
policy for raw/custom commands with undeclared effects. That is engine work
of L-XL size with physical-safety implications that should be validated on
hardware, and it can be added in 2.x as a new method without breaking 2.0.
The ordering improvement in async is small, stays within the current
contract, and removes the main latency problem (one axis's missing completion
delaying the request to stop another axis). Blocking submission includes an
immediate first write that returns `TransportBusy` when it cannot win first
dispatch (`src/blocking.rs:72-76`); issuing three urgent stops back to back
may exceed socket capacity and convert a delay into a failure, so blocking
waits for #780.

## Breaking-change impact

The 2.0 slice is non-breaking (same signature, same error contract). The 2.x
halt is additive (new method, new report type; make the report and its enums
`#[non_exhaustive]` from the start). Changing `stop_all_motion()` to return a
report would be breaking; this record avoids that by adding a new method
instead. Nothing here forces a 3.0.

## Rough effort

2.0 slice: S (`src/async_session.rs` stop loop, doc comments in
`src/blocking.rs`, `src/blocking_nouns.rs`, dyn nouns; one test with pan/tilt
STOP ACKed but completion withheld proving zoom/focus STOPs are written before
the pan/tilt observation resolves; existing `stop_all_motion_*` tests in
`src/async_session.rs:1007-1080` stay green).
Full halt: L-XL (preparation metadata in `src/prepared.rs`, engine types and
suppression in `src/runtime/engine/{types.rs,mod.rs}`, both owner shells,
report types, three facades, generated noun projections, deterministic tests
for concurrent submitters, retries, partial failure, shutdown; hardware
re-validation of HW-04).

## User-visible limitation if rejected or deferred

An application cannot ask the library to "halt these axes and ensure nothing
queued earlier moves them again". Motion submitted before a stop may still be
dispatched after it. Per-axis stop failures are not individually visible from
`stop_all_motion()`; applications that need them must submit `pan_tilt().stop()`,
`zoom().stop()`, `focus().stop()` themselves and observe each. To prevent
older motion, applications must cancel their own outstanding operations
(which requires keeping their handles; see #777) before stopping. In blocking
mode, a slow pan/tilt completion still delays the zoom/focus STOP.

## Dependencies

- #778 (control admission reserve) before the 2.x halt; the 2.0 async slice
  also benefits from it (three stops need three admissions under load).
- #777 (borrowing waits) simplifies per-axis result collection.
- #776 is the natural place for the suppression/ordering-generation rules.
- #780 needed for the blocking facade to dispatch stops concurrently.
- #782's motion generations and #779's halt generations should share one
  owner-ordered generation concept.

## Alternatives considered

- Make `stop_all_motion()` itself the owner halt in 2.0 with a report return
  type: breaking signature and large engine work under freeze.
- Persistent inhibit after halt (requiring explicit resume): stronger but adds
  a new session state; the issue prefers a one-time fence. Leave to the 2.x
  design.
- Leave the facade loop as is: keeps avoidable cross-axis latency in async.

## Open questions for the maintainer

1. Accept the 2.0 async/dyn submit-before-observe change with blocking left
   sequential (a documented latency difference between facades)?
2. For 2.x: new method (`halt`) versus redefining `stop_all_motion()`?
3. One-time fence versus persistent inhibit for post-halt motion?
4. Contract for raw/custom commands with undeclared effects (never suppressed,
   or suppressed only when the caller declares axes)?
