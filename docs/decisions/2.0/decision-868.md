# Decision record: One typed recovery verdict (#868)

Status: Accepted — 2026-11-15

Decision number: D28. This supersedes D20 item 5 in
[decision-783.md](decision-783.md), including its decision to keep the temporary
condition boolean. It resolves #841's deferred replay-safety API option. D20's
other decisions remain: `ObservationTimeout`, `FailureContext`, stages and
certainties describe what happened.

## Decision

`Error::recovery() -> Recovery` is the one recovery recipe. Its exhaustive match
assigns every public error a verdict; `Recovery` is non-exhaustive for callers.

- `Resubmit { after }` means the request had no effect and the condition is
  temporary. The delay is 2 seconds for timeouts, 200 milliseconds for capacity,
  and 50 milliseconds for isolated transport operations.
- `WaitAgain` means the original operation remains live. An ACK and a failed
  cancellation attempt cannot authorize another submission.
- `Reconcile` means the request may have affected the camera. This covers all
  live-session `Unconfirmed` contexts, including Sony and raw command failures.
- `ReopenSession` means the session is unusable. Session death takes precedence
  over an observation wrapper's certainty.
- `Abandon` means no automatic retry: invalid/permanent conditions, deliberate
  cancellation/shutdown/supersession, exhausted retries and latched lanes.

`is_retryable()` and `suggested_retry_delay()` are removed, with no replacement
boolean. `kind()`, `failure_context()` and `requires_new_session()` remain facts;
applications use the verdict rather than combining those facts into a second
replay policy. Human context wrappers preserve the underlying verdict. An
operation-scoped `TransportError` retains its original typed cause and certainty
without claiming session death (#869). `TransmissionNotSent` proves only that
the current attempt sent no bytes; its context preserves an earlier possible
effect. A permanent first-attempt validation cause is `Abandon`, while a prior
`Unconfirmed` attempt remains `Reconcile`.

A new raw inquiry shape can be refused with `InquiryShapeCapacityExhausted`
before admission when its target's declaration reservations are full. This is
`PreAdmission` / `NotAccepted`, keeps the session usable and returns `Abandon`;
the capacity category alone does not authorize a timed resubmission.

The engine wraps every failed inquiry terminal outcome once in
`InquiryFailed { source: Arc<Error> }`. The factory is idempotent. Its context is
fixed `Terminal` / `FailedConclusively`; kind and session validity still report
the original cause. Even a custom transport's command-scoped or still-live
error cannot turn a failed inquiry into `Reconcile` or `WaitAgain`. Caller
observation timeouts, preparation refusals and standalone decoder failures stay
outside this terminal wrapper.

A failed inquiry changes no camera state. Context-carrying inquiry errors use
`FailedConclusively`; command write timeouts retain `Unconfirmed`. Cancellation
observation failures instead describe the still-live original operation with the
fixed `CancellationAttempt` / `StillLive` context, retaining their source.

Deliberate `SettlementSuperseded` ends an obsolete settlement observation. It
adds no uncertainty about the original command's application, so its failure
context is `None` and its verdict is `Abandon`. `MotionSuperseded` retains the
actual `NotAccepted` or `Unconfirmed` context: no automatic replay after a halt,
and reconciliation when an earlier attempt may have taken effect.

Caller deadlines are exactly `ObservationTimeout`, admission
`Timeout { PreAdmission, NotAccepted }`, and query
`Timeout { Observation, NotAccepted }`. Settlement and supersession failures
cannot be rewritten into query timeouts merely because their stage is
`Observation` or `PreAdmission`.

## Evidence and checks

One test table covers every public `Error` variant, asserting its kind, context
and recovery verdict. It checks both the rustdoc matrix and the marked table in
`docs/observability_and_recovery.md`. Timeout tests span every stage and
certainty. Source-chain and observation-wrapper tests preserve the distinction
between a failed attempt and an unknown original effect.

Heartbeat policy compares `MetricsSnapshot::received_frames` for every nonfatal
failure, including a latched inquiry lane after a lost raw-stream reply. An
answered ACK-bearing command can reopen that lane; no background heartbeat or
unbounded retry policy is added to the library.
