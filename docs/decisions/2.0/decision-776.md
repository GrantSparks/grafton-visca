# Decision record: Explicit owner boundary arbitration and invariant replay (#776)

Status: Proposed — awaiting maintainer ratification

Decision number: assigned at ratification. Builds on #723's unification audit
and D2/D18 (receive arbitration, correlation-expiry boundary) in
`docs/issue_542_design_review.md`; reverses none of them.

## Context

`src/runtime/owner/turn.rs:47-60` already holds the executor-free
`RawReleaseTurn` (latch, no-input fence, retained-prefix wait) shared by both
shells (#723). The async actor still combines the deferred raw boundary
(`Option<DeferredRawBoundary>` declared at `src/runtime/owner/async_actor.rs:1932`),
source phase, fairness counters and several release branches in its loop
(`src/runtime/owner/async_actor.rs:1930-2225`); the single-slot invariant is
guarded only by `debug_assert!` at `:2196` and `:2212`, which is the #775
defect. `src/runtime/owner/async_actor.rs` is 3,284 lines and
`src/runtime/owner/blocking.rs` 5,359 lines. Existing focused tests live in
`src/runtime/owner/async_actor/tests/` (on `main`: `release_boundary.rs`,
`fairness.rs`, `faults.rs`, `lifecycle.rs`, `receipts.rs`). No public API is
involved.

## Recommendation

**Defer to 2.x**, with one 2.0 slice delivered by the #775 fix rather than by
this proposal:

- In 2.0: #775's fix makes deferred-slot occupancy a selection invariant
  enforced in release builds (not only by `debug_assert!`), with its
  multi-boundary regression. Optionally, the #775 PR adds a short transition
  table for retained boundaries / release proof / shutdown to
  `docs/architecture_2_0.md` ("Scheduling and async source arbitration").
- In 2.x: introduce or extend the coordinator in `turn.rs` to own source
  eligibility, retained-boundary ordering and deadline turns, with generated
  bounded event sequences and differential facade tests, as the base for
  #778/#779/#780 implementation.

## Rationale

The refactor has no public surface, so deferring it costs no compatibility. It
does touch the most delicate part of the owner, and the stable 2.0 release
requires a hardware pass against final Rust sources; a broad arbitration
rewrite after rc.2 would increase release risk without changing what users
can do. The concrete defect does not need the refactor (the issue says so).
The refactor pays off when new owner actions arrive (control reserve, halt,
blocking worker), which are 2.x work in the recommended plan.

## Breaking-change impact

None. Internal only. No API snapshot change. Nothing to reserve in 2.0.

## Rough effort

L. `src/runtime/owner/turn.rs`, `src/runtime/owner/async_actor.rs`,
`src/runtime/owner/blocking.rs`; new property/generated tests over bounded
event sequences (receive/wake/admission/cancel/control/shutdown), conservation
assertions for boundary payloads and permits, differential production-facade
fixtures; transition-table documentation.

## User-visible limitation if rejected or deferred

None directly. The risk is maintainability: future changes to arbitration
must reconstruct source-eligibility combinations from control flow, and
defects like #775 are found by review rather than by generated tests. Users
are affected only through such defects.

## Dependencies

- After #775 (its regression is the first acceptance example).
- Before or alongside 2.x implementation of #778 (reserve accounting), #779
  (halt ordering/suppression), #780 (worker on the same seam), #782 (motion
  generations).

## Alternatives considered

- Do the refactor before 2.0: high risk, no user-visible gain, delays the
  hardware pass.
- Reject: acceptable only if each later owner feature brings its own
  invariant tests; the issue's own guidance is to reject an abstraction that
  only relocates nesting, which remains the acceptance test for the 2.x work.

## Open questions for the maintainer

1. Accept deferral, with #775 delivering the enforced single-slot invariant
   for 2.0?
2. Should the #775 PR also add the retained-boundary transition table to
   `docs/architecture_2_0.md`, or leave documentation to the 2.x refactor?
