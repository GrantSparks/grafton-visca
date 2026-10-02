# Decision record: Reusable operation observation and separate cancellation intent (#777)

Status: Proposed — awaiting maintainer ratification

Decision number: assigned at ratification (next free entry after D18 in
`docs/issue_542_design_review.md`). If accepted, this record supersedes part of
the #552 exit criteria ("Wait, cancel, and detach consume the owning receipt";
"Drop and observer timeout detach"), so it needs its own CHANGELOG
`### Changed` entry naming #552 and must not be filed under #552.

## Context

Every terminal wait on an operation handle consumes the handle:
`applied(self)`, `applied_with_timeout(self, ..)`, `settled(self)` and
`settled_with_timeout(self, ..)` in `src/operation.rs:81-101` and
`src/operation.rs:165-186`; the blocking handle has the same shape in
`src/blocking.rs:98-108` and `src/blocking.rs:179-192`, and the dynamic handles
in `src/dynapi/owner_projection.rs:53-97` and `:129-160`. The doc comment at
`src/operation.rs:91-92` states that an expired explicit observer deadline
consumes and detaches the handle. The owner delivers one observation through a
`flume::bounded(1)` channel (`src/runtime/owner/mod.rs:767`) whose payload is
either a terminal outcome or `CancellationFailed` (`src/runtime/owner/mod.rs:727-730`),
so a cancellation-send failure consumes the only route to the original
operation's later terminal result. Recoverable cancel refusal
(`CancelRejected<Self>`, #612) is present and correct
(`src/operation.rs:125-146`). `CancellationOutcome` is an exhaustive two-variant
enum (`src/outcome.rs:16-21`). For history: 1.1.0 `InFlight::cancel` borrowed
the handle (`docs/migration_2_0.md:546-552`), so a borrowing shape is closer to
1.x than the current one.

## Recommendation

**Accept for 2.0 (observation shape); defer independent idempotent
cancellation intent and dual cancellation/original delivery to 2.x.**

What lands in 2.0:

1. `applied`, `applied_with_timeout`, `settled`, `settled_with_timeout` take
   `&mut self` on the async `Operation<K>`, blocking `Operation<'_, K>`, and
   `DynTargetedOperation`/`DynAppliedOperation`.
2. The handle caches the authoritative terminal observation once received;
   later waits return the cached result (`Applied` repeatedly; `settled` after
   a prior `applied` continues from the cached application).
3. An expired explicit observer deadline, or a dropped wait future, releases
   only that wait. It does not detach, cancel, renew or shorten engine budgets
   (#749 relationship preserved).
4. `cancel(self) -> Result<Cancellation, CancelRejected<Self>>` and
   `detach(self)` keep their current shape. Drop remains detach (#567).
5. Add `#[non_exhaustive]` to `CancellationOutcome` so 2.x can report
   additional honest outcomes without a 3.0.

Deferred to 2.x (additive): a borrowing `request_cancel(&mut self)` /
idempotent intent API, and retaining the original terminal result after a
cancellation-send failure (needs a second observation slot in the owner).

## Rationale

The consuming shape causes ordinary async code to lose operations: a
`tokio::select!` branch that loses drops the handle, and a short timeout used
for UI progress detaches it. These are frequent patterns, and the fix changes
method receivers, which cannot be done compatibly after 2.0.0. The 2.x
alternative (adding a parallel set of borrowing methods) would leave two
permanent spellings for every wait on three handle families. The
handle-local cache keeps storage bounded by handle lifetime and needs no
session-wide history. Keeping `cancel` consuming in 2.0 limits scope: refusal
is already recoverable, and the harder owner change (two observation slots)
is additive later.

## Breaking-change impact

Breaking, and must land before 2.0.0 final or wait for 3.0. Call sites that
bind the handle immutably (`let op = ...; op.applied().await`) need `let mut`.
Chained calls on a temporary (`camera.zoom().stop().await?.applied().await?`)
keep compiling. Code that relied on a timed-out wait detaching the observer
must call `detach()` explicitly. `CancellationOutcome` becoming
non-exhaustive breaks exhaustive matches (add a wildcard arm). API snapshots
in `api/2.0.0-rc.1/*.txt` change for all three surfaces.

## Rough effort

M. Touches `src/operation.rs`, `src/blocking.rs`, `src/dynapi/owner_projection.rs`,
the receipt types in `src/runtime/owner/{mod.rs,async_actor.rs,blocking.rs}`
(cache the observation; make the async settlement continuation restartable
after a dropped wait), `src/outcome.rs`. Tests: timeout-then-reobserve,
lost-`select!`-branch, applied-then-settled, equal-time completion/deadline,
for async (tokio and smol), blocking and dyn facades; compile-pass/fail
contracts (no `settled` on `AppliedOnly`, `Send` futures). Docs: migration
section and examples (`examples/operation_handles*.rs`), snapshot refresh.

## User-visible limitation if rejected or deferred

An application cannot time out a wait and then keep waiting on the same
operation, cannot put a wait in a `select!` without losing the handle when
another branch wins, and cannot observe `applied` and then `settled` on one
targeted operation. Workaround: never use explicit-timeout waits, spawn a task
that owns the handle and forwards its result over an application channel, or
wrap the wait in an application-owned future that is never dropped.

## Dependencies

- #783: an observation-deadline error is only meaningful if the handle
  survives it; decide together (recommended order: #777 then #783).
- #782: re-observation for settlement evidence builds on the cached handle.
- #779: a halt report that collects per-axis results benefits from
  borrowing waits.
- Independent of #775 and #781.

## Alternatives considered

- Keep consuming waits and add borrowing variants in 2.x: compatible, but two
  permanent spellings; rejected for API clarity.
- Clonable handles / shared receiver: a cloned flume receiver creates
  competing consumers, not reusable observation (as the issue notes).
- Full proposal in 2.0 including `&self` idempotent cancel and dual delivery:
  more owner work (second observation slot, idempotent intent recording) than
  is realistic before freeze; additive later.

## Open questions for the maintainer

1. Accept the receiver change (`self` -> `&mut self`) before 2.0 final?
2. Should `cancel` also move to `&mut self` now (larger owner change), or stay
   consuming with `CancelRejected` as proposed here?
3. After a terminal result is cached, should a later `cancel` return the
   cached outcome (`Completed`) or an `InvalidState` error?
4. Approve `#[non_exhaustive]` on `CancellationOutcome`?
