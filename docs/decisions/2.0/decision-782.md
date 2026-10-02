# Decision record: Settlement evidence and supersession for targeted operations (#782)

Status: Proposed — awaiting maintainer ratification

Decision number: assigned at ratification. Any change to what `settled`
means is a revision of the #542 targeted-settlement contract and needs its own
`### Changed` CHANGELOG entry naming #542 plus a migration example; it must
not be recorded by editing the existing #542 entries.

## Context

Preparation selects either `SettlementPlan::CompletionIsSettled` (profile
declares operation-complete) or `SettlementPlan::Poll` over the affected axes
with `MovementTolerance::default()` and an interval of at least 25 ms
(`src/prepared.rs:396-429`). The async poll loop returns success on the first
below-tolerance pair of snapshots (`src/runtime/owner/async_actor.rs:893-943`).
The plan holds no requested endpoint and no motion generation, and the public
result is `Result<(), Error>`, discarding the continuation result
(`src/operation.rs:165-186`, `.map(drop)`). The architecture document already
states that `settled` observes "the profile-selected protocol settlement
condition, not a bench-verified assertion of physical rest"
(`docs/architecture_2_0.md:657-659`). It does not state that a later
conflicting move on the same axes can satisfy an earlier operation's
settlement.

## Recommendation

**Defer to 2.x; in 2.0, document the current meaning precisely and reserve
room for a report and a supersession outcome.**

What lands in 2.0:

1. Documentation of `settled`/`settled_with_timeout` on all facades and in
   `docs/architecture_2_0.md`: polling settlement reports that the selected
   axes were observed stable, not that the operation's requested endpoint was
   reached, and a later library-issued move on the same axes can be what is
   observed.
2. Documentation that a later 2.x release may report a distinct, documented
   error when a later conflicting library operation on the same target/axes
   supersedes the observation. (The error variant itself can be added in 2.x;
   `Error` is `#[non_exhaustive]`.)
3. If #777 is accepted, the borrowing handle shape leaves room for an additive
   `settlement_report(&mut self)` (name illustrative) next to `settled`.

Deferred to 2.x (additive): a small `#[non_exhaustive]` settlement report
(evidence class, axes, window/tolerance), owner-ordered motion generations per
target/axis with a chosen supersession point, and optional position-reached
evidence where coordinates permit.

## Rationale

The report is additive. Supersession changes the outcome of an existing
method for an edge case (overlapping library moves on the same axes), which
is defensible as a 2.x correctness improvement if 2.0 documents the
possibility now. Implementation needs prepared motion metadata in the owner
(shared with #779), a generation counter and a precise admission-versus-dispatch
point, plus a sampling policy that #781 is defining. Doing it before 2.0
would stack three behavioral changes on the release path and delay the
hardware pass. The current documentation is already honest about physical
rest; the gap is attribution, which the 2.0 text can state.

## Breaking-change impact

2.0 slice: documentation only. 2.x report: additive. 2.x supersession: a
behavior change in `settled()` for overlapping same-axis moves (`Ok(())`
becomes a documented error); compatible in practice only if 2.0 documents it,
otherwise treat as a deliberate revision with a CHANGELOG `### Changed` entry.
Does not require a 3.0 if the 2.0 documentation lands.

## Rough effort

2.0 slice: S (doc comments in `src/operation.rs`, `src/blocking.rs`,
`src/dynapi/owner_projection.rs`, `docs/architecture_2_0.md`,
`docs/usage_2_0.md`).
Full: L (motion metadata from `src/prepared.rs` through
`src/runtime/engine/types.rs`; generation tracking in the engine or owner
state; report type; async and blocking poll loops; dyn projection; tests for
overlap admitted-then-cancelled, rejected-before-admission, dispatched,
unrelated axis/target, wrong-position stabilization; snapshot refresh).

## User-visible limitation if rejected or deferred

`settled()` returning `Ok(())` does not tell the application whether
settlement came from a profile operation-complete signal or from position
polling, and under polling it cannot distinguish "A stopped at its target"
from "a later move B on the same axes stopped". Applications that need
endpoint confirmation must query position after `settled()` and compare it
with their requested target themselves, and must avoid issuing overlapping
same-axis moves while awaiting an earlier settlement, or track that ordering
themselves.

## Dependencies

- #781 (temporal sampling window) first: the evidence report should describe
  the sampling policy #781's PR establishes.
- #777: re-observation after `applied` and an additive report method assume
  borrowing waits.
- #779: share the owner-ordered motion generation and prepared axis metadata.
- #776: generation/supersession rules belong in the coordination seam.

## Alternatives considered

- Change `settled()` to return a report in 2.0: breaking, and the report's
  evidence fields depend on #781's not-yet-ratified sampling contract.
- Endpoint verification only (no supersession): does not cover relative moves
  or presets with unknown coordinates.
- Leave as is with no documentation: callers may infer endpoint arrival from
  `Ok(())`.

## Open questions for the maintainer

1. Accept deferral with the 2.0 documentation of current meaning and the
   reserved supersession outcome?
2. For 2.x: supersession point at admission of the later operation, or at its
   first write?
3. Should `CompletionIsSettled` profiles (`supports_operation_complete`) also
   be subject to supersession, or is the profile completion signal treated as
   exact?
