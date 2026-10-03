# Decision record: Native blocking owner worker with autonomous progress (#780)

Status: Accepted (expanded: native worker in 2.0, as the only blocking model) — 2026-10-02

Decision number: D24 (`docs/issue_542_design_review.md`). A worker would revise the
documented invariant "There are no public callbacks, user-supplied lifecycle
IDs, unbounded queues, or per-camera background workers"
(`docs/architecture_2_0.md:57-58`) and the caller-thread model stated at
`docs/architecture_2_0.md:36`; if adopted, record that as a new decision and a
`### Changed` entry, not as an edit to the old text.

## Decision

1. Every blocking session owns one native worker thread, which runs the
   shared executor-free coordinator (D25). Callers talk to it through bounded
   channels. The `blocking` feature stays executor-free (#723).
2. There is one blocking execution model; no opt-in or caller-driven mode.
3. No `BlockingTransport` trait change. The trait is already `Send`, and
   `recv_into_with_timeout` is already bounded, so the worker stays
   interruptible at read-timeout granularity. The transport documentation
   states that bound as the close latency.
4. Blocking submission success means the owner accepted the operation. A
   transport write failure is reported through the operation's outcome.
5. Concurrent use of a blocking session no longer returns `TransportBusy`.
   The implementation PR removes the variant if no other producer remains.
6. Detached operations, timers and correlation cleanup progress without a
   caller inside the session.
7. Close, drop and constructor failure are defined in the implementation PR.
   `close()` joins the worker. Dropping the last handle signals it without
   self-join.
8. The implementation PR records resource measurements (thread count, idle
   CPU, memory, control latency) as validation.

Revises the documented invariant "no per-camera background workers"
(`docs/architecture_2_0.md`) and the caller-thread model. Record both as a
`### Changed` entry naming this decision; do not edit the old text in place.

Implemented in #793. The worker reads the transport in slices of at most
10 ms (1 ms when another source is already ready), so control, STOP and close
latency is bounded by one slice rather than by the transport's read timeout;
this tightens item 3 without changing the trait. `drain_diagnostics` is
replaced by `subscribe_diagnostics`, and `TransportBusy` is removed. The
measurements (one thread per session, about 0.1% idle CPU and 94 KiB per
session, STOP p50 5.6 ms / p99 10.7 ms) are recorded in
`docs/architecture_2_0.md`.

## Context

The blocking session keeps the owner, wire driver, reader and decoder behind
one `Mutex` (`src/runtime/owner/blocking.rs:367-392`), and every owner turn
uses `try_lock`, returning `Error::TransportBusy` on contention
(`src/runtime/owner/blocking.rs:425-449`, line 438). Progress (timers, reads,
correlation cleanup) happens only while some caller is inside the owner.
Blocking operation submission includes an immediate first write and returns
`TransportBusy` if it cannot win first dispatch (`src/blocking.rs:72-76`),
whereas async submission means admission. The canonical failure matrix
documents `TransportBusy` for blocking re-entry (`src/error.rs:29`). The
`blocking` feature must remain executor-free
(`docs/issue_542_design_review.md:82-84`, `docs/architecture_2_0.md:87-94`,
#723); a native thread does not violate that.

## Original recommendation

The ratified decision above expands this recommendation. It is kept as
the record of what was proposed.

**Defer to 2.x; in 2.0, reserve the contract so a worker can be introduced
without a 3.0.**

What lands in 2.0 (documentation and classification only):

1. Document that blocking submission success means the owner accepted the
   operation; a transport write failure may be reported either from submission
   or through the operation's outcome. (Today it is always the former; the
   documentation simply stops promising it.)
2. Document `TransportBusy` on concurrent owner turns as a property of the
   current caller-thread driver, not a guarantee, and keep the matrix row.
3. Document the progress model: no autonomous progress while no caller is
   inside the owner; detached operations progress only when some caller drives
   the session.
4. Scope the "no per-camera background workers" sentence to async and to the
   2.0 blocking driver.

Deferred to 2.x: a native worker thread per blocking session over the existing
executor-free coordination, either opt-in at construction or as the new
default, chosen after measuring thread count, idle CPU, memory and control
latency.

## Rationale

A worker changes transport ownership, shutdown/close semantics (self-join,
last-handle drop, constructor failure), read interruptibility for custom
`BlockingTransport` implementations, and the lifecycle boundary at which
blocking submission fails. That is XL work, needs measurement the issue
itself requires before choosing a default, and would land after the stable
hardware pass would otherwise have been recorded (the pass is tied to the
final Rust sources, `docs/hardware_release_checklist.md` "Stable-release
sign-off"). The public blocking types already borrow an opaque host
(`Operation<'session, K>` holds `&dyn BlockingControlHost`,
`src/blocking.rs:52-63`), so the public shape does not need to change. What
would break users is only the documented contract, and that can be loosened in
2.0 at no implementation cost.

## Breaking-change impact

2.0 slice: documentation-only weakening of the submission guarantee; no
signature change. Code written to the 2.0 docs keeps working under a 2.x
worker. A 2.x worker that becomes the default changes resource use (one thread
per session) and removes `TransportBusy` on ordinary overlap; neither breaks
code written to the loosened 2.0 docs. If the 2.0 documentation is not
loosened, a later worker changes a documented guarantee (first-write failure
at submission) and should wait for 3.0 or ship opt-in only.

## Rough effort

2.0 slice: S (`src/blocking.rs` doc comments, `src/error.rs` matrix wording,
`docs/architecture_2_0.md`, `docs/usage_2_0.md`, migration note).
Worker: XL (`src/runtime/owner/blocking.rs` split into worker loop and
handles, bounded command channels, interruptible bounded reads in
`src/runtime/owner/blocking_transport.rs`, close/drop/startup failure
semantics, differential facade tests, native dependency-boundary CI, resource
measurements, transport-contract documentation for custom transports).

## User-visible limitation if rejected or deferred

A blocking `Session` shared between threads serializes owner turns with
fail-fast `TransportBusy`: while one thread waits on an operation, another
thread's submit, cancel or STOP can fail and must retry. Operations whose
observer is detached, and timers, make no progress unless some thread calls
into the session. Applications must either funnel all camera calls through
one application thread (their own worker), retry `TransportBusy` with backoff,
or use the async API with an executor.

## Dependencies

- #776: the worker should consume the same executor-free coordination seam
  rather than duplicate async arbitration.
- #778: the control reserve only helps blocking stops once owner-turn
  contention is gone.
- #779: blocking concurrent STOP dispatch depends on this.
- #777: borrowing waits are independent of the execution model.

## Alternatives considered

- Adopt the worker as default for 2.0: XL under freeze, no measurements yet,
  and a re-run of the hardware pass.
- Reject permanently and keep caller-thread pumping: valid, but the progress
  and contention limits then become a permanent API property and should be
  stated as such (this record's 2.0 documentation does that either way).
- Blocking wait on the mutex instead of `try_lock`: removes spurious
  `TransportBusy` but makes a STOP wait behind an arbitrary long owner turn
  and still provides no autonomous progress.
- Two permanent public modes: the issue warns about maintenance cost; if
  chosen, prefer opt-in worker in 2.x and decide the default in 3.0.

## Resolved questions

1. Deferral is superseded: the worker lands in 2.0.
2. The worker is the only blocking model.
3. No bounded-wait variant of the current mutex design.
