# Decision record: Bounded admission reserved for stop/halt control (#778)

Status: Accepted — 2026-10-02

Decision number: D26 (`docs/issue_542_design_review.md`). This extends D10 ("intrinsically
urgent cancellation bypasses queued ordinary work but never bypasses physical
command pacing") and D1 (urgency is request-owned metadata) in
`docs/issue_542_design_review.md`; it reverses neither. It changes the
documented meaning of `SessionConfig::admission_capacity`, which needs a
`### Changed` CHANGELOG entry and a migration row.

## Decision

As recommended below:

1. `admission_capacity` bounds ordinary requests. The total bound is
   `capacity + reserve`.
2. Each registered target has a reserve sized to its profile's supported
   typed STOP paths (at most three). Only `ControlClass::Urgent` typed stops
   may use it.
3. Exhausting the reserve returns a distinct error. It joins the D20 matrix
   update.
4. `MetricsSnapshot` becomes `#[non_exhaustive]` (also covered by D27) and
   gains control-reserve counters.
5. No coalescing. The halt (D21) admits its STOPs through this reserve.
   With the blocking worker (D24), the reserve also protects blocking stops.

## Context

Urgency is applied only after admission. The async owner takes a permit from
one shared pool before anything else and returns `RuntimeQueueFull` when none
is left (`src/runtime/owner/async_actor.rs:1642-1652`); the blocking owner
probes the same pool (`src/runtime/owner/blocking.rs:1469-1480`); the pool is
a single counter covering boundary work and the whole engine lifecycle
(`src/runtime/owner/mod.rs:609-669`); and the engine separately rejects when
`entries.len() >= policy.capacity` (`src/runtime/engine/mod.rs:759-766`).
`SessionConfig::admission_capacity` is documented as a total bound: "once this
many requests are pending or active, a new submission returns
`RuntimeQueueFull`" (`src/session_config.rs:151-161`; default 64,
`src/session_config.rs:24`). With enough ordinary work outstanding, a typed
STOP is rejected locally before its urgent dispatch class matters. The
`MetricsSnapshot` struct has public fields and is not `#[non_exhaustive]`
(`src/observability.rs:36-37`).

## Recommendation

**Accept for 2.0 (narrow reserve for intrinsically Urgent typed stops).**

What lands in 2.0:

1. `admission_capacity` is redefined as the ordinary-request bound. A small
   fixed control reserve exists in addition: per registered target, enough
   slots for that profile's supported typed STOP paths (at most three:
   pan/tilt, zoom, focus). The total bound is documented as
   `capacity + reserve`.
2. Only requests whose prepared metadata carries `ControlClass::Urgent` may use
   the reserve (no wire inspection; raw `Urgent` is already refused at
   construction per D1/#679).
3. The reserve is accounted at the permit pool, the async admission channel,
   the owner record, and the engine entry cap; per-target accounting gives
   inter-target fairness for free.
4. Exhausting the reserve returns a distinct error (new `Error` variant; `Error`
   is non-exhaustive) rather than `RuntimeQueueFull`.
5. Add `#[non_exhaustive]` to `MetricsSnapshot` and add counters for
   control-reserve admissions and rejections.

No coalescing of repeated halt intents in 2.0 (bounded by the reserve
instead). No change to pacing, socket limits, raw correlation holds, or
in-progress writes: an admitted stop still waits for protocol eligibility.

## Rationale

A stop that is rejected because the application queued too much ordinary work
is the failure mode an operator least expects, and HW-04 in
`docs/hardware_release_checklist.md` exercises emergency stop on hardware;
changing admission semantics after that pass would require repeating it. The
change is local (permit pool, one engine check, blocking probe, error and
metrics) and does not require the halt redesign (#779). Redefining
`capacity` is a semantic change that is cheap now and confusing after 2.0.0.
A per-target reserve sized by supported stop paths is simpler to reason about
than coalescing and still bounded.

## Breaking-change impact

Semantic: the total number of live requests can exceed `admission_capacity` by
the reserve; a new error variant appears; `MetricsSnapshot` struct literals
outside the crate stop compiling once it is `#[non_exhaustive]` (functional
update with `..Default::default()` is also disallowed for non-exhaustive
structs). Must be decided before 2.0.0 final. If deferred, the minimum to keep
2.x open is: document `capacity` as the ordinary bound "plus any reserve the
library defines for urgent control", and mark `MetricsSnapshot`
`#[non_exhaustive]`.

## Rough effort

M. `src/runtime/owner/mod.rs` (pool with ordinary and per-target reserve
counters), `src/runtime/owner/async_actor.rs` and `src/runtime/owner/blocking.rs`
(class-aware acquire/probe), `src/runtime/engine/mod.rs` (class-aware entry
cap), `src/session_config.rs` docs, `src/error.rs`, `src/observability.rs`.
Tests: saturate at capacity 1 and 64 with queued and active ordinary work on
both facades and dyn; flood urgent stops and assert bounded permits/entries
and the distinct error; one noisy target cannot use another target's reserve;
existing #714/#744/#746 urgent-dispatch tests stay green. Snapshot refresh,
migration row.

## User-visible limitation if rejected or deferred

Under ordinary saturation (many queued moves, a slow camera, or a small
configured capacity) a typed STOP or `stop_all_motion()` can fail immediately
with `RuntimeQueueFull`. Applications must keep their own headroom (cap
outstanding ordinary operations below `capacity` minus the stops they may
need) or cancel queued work before stopping. In blocking mode, even with the
reserve, a stop from a second thread can still fail with `TransportBusy` while
another thread holds the owner turn (see #780).

## Dependencies

- Prerequisite for #779 (owner-level halt needs guaranteed admission for up to
  three stops per target).
- #776: implement on the existing seam; does not require the coordinator
  refactor.
- #780: closes the remaining blocking-contention gap only if a worker is
  adopted.
- Coordinate the new error variant with #783's matrix update.

## Alternatives considered

- Reserve for all `Urgent` requests session-wide (not per target): one noisy
  target could consume it.
- Coalesced halt-intent slot per target: tighter bound, but needs an
  observer-list bound and changes receipt semantics; better in 2.x with #779.
- Separate control channel only: does not address the engine's shared entry
  cap, as the issue notes.
- Document the limitation only: leaves stop availability dependent on
  application discipline.

## Resolved questions

1. `admission_capacity` is the ordinary bound; total = capacity + reserve.
2. Reserve sized per target by supported stop paths (at most three).
3. `#[non_exhaustive]` on `MetricsSnapshot`: yes.
