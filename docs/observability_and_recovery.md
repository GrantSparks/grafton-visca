# Observability, state, and recovery

The public observability surface is deliberately smaller than the private
owner trace. It is stable, bounded, and safe to consume from an application
without granting control over the protocol engine.

## Metrics

`MetricsSnapshot` is a copy of scalar owner state. Its stable fields are:

| Field | Meaning |
| --- | --- |
| `admitted` | Requests admitted into authoritative engine state. |
| `admission_rejected` | Requests rejected before admission, including abandoned async submissions and each prepared STOP in an abandoned halt (`Cancelled` admission diagnostics). Abandonment before acceptance produces no dropped-observer events. |
| `control_reserve_admitted` | Urgent typed STOPs admitted into their camera's control reserve rather than an ordinary slot. |
| `control_reserve_rejected` | Urgent typed STOPs rejected because their camera's control reserve and ordinary admission were both full (also counted in `admission_rejected`). |
| `writes` | Transport writes attempted. |
| `write_failures` | Writes that returned an error. |
| `terminal` | Requests reaching a terminal outcome. |
| `cancellations` | Cancellation intents the engine recorded. Repeated cancellation calls from one handle and refused cancellations are excluded. |
| `cache_updates` | Exact applied-state effects committed to a target cache. |
| `cache_invalidations` | Cache keys made unknown by unconfirmed writes. |
| `ack_timeouts` | Acknowledgement deadlines that expired on a sent command. A disputed ACK deadline extended for more evidence counts once if the extended deadline ends the request. |
| `completion_timeouts` | Completion deadlines that expired while an acknowledged command or completion-only raw command awaited completion. |
| `inquiry_timeouts` | Reply deadlines that expired on a sent inquiry. |
| `busy_errors` | Error frames saying the camera cannot accept the request now. |
| `protocol_errors` | Every other error frame, excluding the cancellation reply. |
| `retries_scheduled` | Requests re-queued for another attempt, for any reason. |
| `received_frames` | Valid decoded VISCA response frames received from the camera; compare snapshots around an application heartbeat for positive liveness evidence. |
| `ignored_unmatched_sequenced_replies` | Sequenced replies matching no request, or a non-unique full-width identity. Ambiguous lower-16 matches are excluded. |
| `ignored_reply_shape_mismatches` | Inquiry replies discarded because their payload did not match the expected reply shape. |
| `ignored_malformed_frames` | Delimited frames, and consumed oversized/malformed datagrams rejected before framing, discarded because they did not classify as a valid VISCA response. |
| `dropped_staged_diagnostics` | Events evicted from bounded pre-admission staging before any subscriber could receive them. |
| `dropped_diagnostic_events` | Events dropped because a subscriber queue was full. |
| `dropped_observer_events` | Operation-observer events (terminal outcomes and cancellation failures) dropped because their handle was already detached or dropped. |
| `dropped_boundary_work` | Acting boundary messages dropped before the terminal snapshot was published. Queued metrics observations are answered and do not count as dropped work. |
| `active` | Admitted, non-terminal requests currently retained. |
| `pending` | Queued admissions plus one retained admission, if present; retained cancellations are not pending requests. |
| `session` | `Running`, or the concluded owner’s terminal status: `Closed`, `Shutdown` or `Poisoned`. |

The counter fields saturate at `u64::MAX`; queue and active counts remain
bounded by owner policy. While the owner runs, both facades obtain `metrics()`
through a separate bounded control request. A concluded owner retains its
terminal snapshot beside its terminal cause, and either facade reads it directly
from the handle. Its `active` and `pending` counts are zero and all counters
survive, including work and diagnostics dropped before publication. The snapshot
is published once at normal conclusion and remains identical through teardown
and after `close()`. Control and cancellation requests issued after the terminal
verdict return that verdict without entering their lanes. Teardown still answers
messages that raced the handle check, without revising the published counters.
An owner that unwound before concluding instead returns `OwnerTerminated`,
because its unfinished engine state cannot provide a trustworthy snapshot.
Neither facade clones subscriber queues, wire buffers, or the state registry.

`busy_errors` counts the codes the scheduler itself treats as transient
camera-side backpressure — command buffer full (`0x03`), no socket (`0x05`),
and not executable in the current state (`0x41`). The cancellation reply
(`0x04`) is an answer rather than a fault and is counted in neither error
bucket. Both error counters count frames as the owner decodes them, so a
camera answering requests the engine can no longer correlate still shows up.
`retries_scheduled` counts wherever the engine emits a permitted retry — a busy
camera, an eligible expired deadline, or an eligible sequenced receive fault
all count the same. A raw receive fault while an unsequenced command awaits ACK
neither replays it nor (by default) ends the session — the command rides to its
own ACK deadline — so it does not increment this retry path.

Sony replies whose upper sixteen bits are zero use the lower-16 correlation
index, including replies to a new session's first 65,536 transmissions. A
non-unique lower-16 match emits `AmbiguousLower16Sequence` diagnostics and keeps
all candidates live. It does not increment `ignored_unmatched_sequenced_replies`,
which retains its existing `UnmatchedSequencedFrame` classification; a non-unique
full-width identity does increment it. This preserves the counter's behavior
while making the probes and fixtures exercise the production header path.

## Diagnostics

Both facades expose `subscribe_diagnostics(capacity)`. An async subscription
uses `r#async::Diagnostics`, and a blocking subscription uses
`blocking::Diagnostics`. Both provide `try_recv`, `recv`, and
`recv_with_timeout`. The async receives are awaited. A checked
`observability::DiagnosticCapacity` sets the queue size to 1 through 128
events. One owner retains up to four live subscriptions. A fifth subscription fails
with nonretryable `DiagnosticSubscriberLimit { capacity }`; drop an existing
subscription to make room. Request-admission capacity does not change this
subscriber limit. Subscriptions receive events recorded after they are created;
there is no retained diagnostic history or replay.

`DiagnosticEvent` is a bounded, `#[non_exhaustive]` enum containing only
sanitized categories: admission, frame category, protocol phase transition,
write success, retry, cancellation, applied state, state invalidation, terminal
outcome, session end, and ignored-input observations. Its helper enums are also bounded and
non-exhaustive. Events contain no wire bytes, arbitrary strings, timestamps,
or socket and transmission details.

Request events carry the same `OperationId` as handles and errors. Compare the
identities to correlate observations within a session. An ID grants no
authority over the request. Applications that need raw frames for support can
enable an explicitly private, access-controlled trace path.

Delivery is best effort. The owner uses non-blocking sends, so a slow
subscriber never delays command admission, pacing, transport I/O, retries, or
shutdown. A full subscriber queue increments `dropped_diagnostic_events` and
affects only that subscriber. Disconnected subscribers are reclaimed when the
owner records or creates subscriptions. This design has no callbacks and no
unbounded channel. After queued events drain, a disconnected subscription
returns the owner's published terminal cause: `ConnectionClosed`,
`StreamPoisoned`, `OwnerTerminated`, or deliberate `RuntimeShutdown`. A dropped
final diagnostic event does not replace that cause with `RuntimeShutdown`.

## Shutdown and close

An async session has one detached owner actor and one transport. All session
clones share that owner; neither operation starts a second protocol authority.
`shutdown()` is an idempotent, non-joining signal. It completes when the
owner's bounded shutdown boundary accepts the signal (or when another clone
has already accepted it), but it does not wait for the actor or transport to
finish tearing down. New admissions are rejected with
`Error::RuntimeShutdown` after shutdown has been requested.

`close()` consumes the session, requests that same shutdown, and then waits
for the sole actor to finish its boundary drain and drop its driver, including
the owned transport. It is the deterministic transport-teardown barrier, not
an executor-specific task join: reopening an endpoint after `close()` is safe
because completion is ordered after transport release. If the shutdown signal is accepted, an explicit
`RuntimeShutdown` terminal result is returned as success; a transport close, or
a stream / strict-opt-in session poison that wins the source ordering, is
returned unchanged. An immediate error while sending `close()`'s signal is
also preserved. Dropping a non-final view or calling `shutdown()` alone is not
a transport-release barrier. Dropping the final async `Session`/camera-view
owner handle may release the actor and transport, but provides no completion
barrier and does not issue a protocol STOP; use `close()` when release must be
observed before reopening an endpoint.

On either facade, `shutdown()`/`close()` called on a session the *engine* has
already terminated — a deadline expiry, the strict `strict_unconfirmed_poison`
opt-in, or a stream/framing self-poison, none of which is an owner-supplied
`Close`/`Poison`/`Shutdown` input — returns that terminal error rather than
masking it as a deliberate `RuntimeShutdown` (#680). This matters for a cleanup
path, a `Drop` guard, or a supervisor that keys its rebuild on the result of
`shutdown()`/`close()` itself: it still sees `requires_new_session() == true`
and rebuilds, instead of treating the dead session as one that ended on purpose.
`set_tuning` observes the same terminal boundary and returns the session's
terminal error rather than silently succeeding (#690). An owner panic is
`OwnerTerminated`, which is nonretryable and positively requires a new session.
It is distinct from live-session `InvalidState` misuse such as closing from the
worker thread (#851).

## StateCache

Each `Camera<P>`, including `Camera<AnyProfile>`, returns a cheap read-only
`StateCache` view for exactly one target. Typed per-noun getters return the
setter's own value type, for example `cache.focus().lock()`,
`cache.nd_filter().mode()`, and `cache.tally().mode()`. `None` means unknown.
The generic key/value view remains available. `StateEntry` is
`#[non_exhaustive]`, so a match
over it needs a wildcard arm:

```rust,ignore
let cache = camera.state_cache();
assert_eq!(cache.target(), camera.target());
if let Some(enabled) = cache.image().freeze() {
    println!("last applied image freeze: {enabled}");
}
match cache.value(grafton_visca::state_cache::StateKey::ImageFreeze) {
    grafton_visca::state_cache::StateEntry::Unknown => { /* no exact applied value is currently known */ }
    grafton_visca::state_cache::StateEntry::Set(grafton_visca::state_cache::KnownState::ImageFreeze(enabled)) => {
        println!("freeze enabled: {enabled}");
    }
    _ => {}
}
```

`KnownState` carries each key's native value, including booleans, enums, checked
speeds, and the typed `PanTiltLimitUpdate`. Tally flashing stores
`TallyMode::Flash`, and `cache.motion_sync().speed()` stores `MotionSyncSpeed`.
A limit set records the corner and the applied `PanTiltPosition` in degrees
decoded from the units sent to the camera. At a raw endpoint, both the cache
and the position inquiry use the nominal degree bound when the inverse falls
outside it and the bound encodes the same wire word. A cleared limit retains
its corner as `StateEntry::Set` with `position() == None`. The owner
registry has seven target slots and one inline slot per `StateKey`, without
allocation, eviction, or a separate capacity limit. A view contains one target
index and a shared registry reference; cloning it shares the table.

An exact applied-state effect establishes a known value and emits
`AppliedState`. A write that may have taken effect but ends without confirmation
makes its key `Unknown` and emits `StateInvalidated`, including a session-ending
failure after the write left. A command never written, or
conclusively rejected before ACK with no possibly applied earlier attempt,
leaves the previous value intact. Preparing a command and an ACK alone cannot
establish a value. The owner commits effects before notifying observers or
resolving the terminal outcome, independently of observer attachment. A fresh
session starts every target at `Unknown`; target views remain isolated.

On an FR7, a pan/tilt drive using speed 25–50 waits for an in-flight
speed-mode change on that target, including when the drive is ready to retry.
The write requires a confirmed `Fine50`. An applied `Standard24` or an
uncertain mode change removes that permission. A conclusively rejected
change preserves the prior confirmation.

## What is retried, and for how long

Retry requires evidence that replay is safe. Commands and operations select
`request::CommandRetryClass`; inquiries select `request::InquiryRetryClass`.
A zero retry count rules out replay after a protocol deadline or camera
rejection. With a positive count,
a full command buffer (`0x03`) or no free socket (`0x05`) conclusively rejects
an attempt and permits a retry. `0x41` (`CommandNotExecutable`) permits a
retry for movement and preset commands, for which the reported camera state
may pass.

For Sony-encapsulated traffic, a lost ACK may be retried within policy with
the same sequence number, preserving the logical request's identity. Once an
ACK proves acceptance, neither a completion timeout nor a later camera error
replays that command (#795). Raw VISCA has no sequence key. After a raw command was successfully sent,
an ACK/completion/cancellation result that cannot be correlated is never replayed
blindly. By default only that request fails and the evidence still at stake is
quarantined; callers reconcile its camera effect while the session continues.
The exact phases, quarantine scopes, transient-fault behavior, and
`strict_unconfirmed_poison` alternative are defined once in
[Raw unconfirmed outcomes and strict recovery](architecture_2_0.md#raw-unconfirmed-outcomes-and-strict-recovery).

Raw inquiry replies carry no identity. A target can have one staged or live
inquiry per overlapping payload declaration class, and disjoint classes can
run together. An uncertain datagram release retains the profile's
`raw_inquiry_reply_skew`, bounded by minimum inquiry spacing. On streams,
the [correlation ledger](architecture_2_0.md#the-raw-byte-stream-correlation-ledger)
tracks unanswered inquiries and applies the ambiguity window to overlapping
work. At an exact response or skew deadline, complete input wins before due
work. Retained partial bytes follow the
[byte-stream ambiguity rule](architecture_2_0.md#engine-transition-and-ordering).

`0x02` (`SyntaxError`) is retried on one narrow path: an inquiry issued through
this crate's own built-in typed inquiry surface. Cameras answer a built-in
inquiry's exact syntax inconsistently enough that one replay is worth having.
An inquiry submitted through the downstream/custom raw-request API carries the
application's own syntax, so `0x02` is that inquiry's terminal verdict and is
never replayed. This distinction is about request provenance, not the raw or
Sony transport envelope. No plain or operation command retries `0x02`.

How many *retries* a request gets is derived from its *timeout* category, not
its retry class. These are retries after the first attempt, so a request makes
at most one more attempt than its budget:

| Timeout category | Retry budget | Attempts at the default base |
| --- | --- | ---: |
| `Quick`, `Inquiry` | base + 2 | 6 |
| `Movement`, `Preset` | base | 4 |
| `Network` | base - 1, never below 1 | 3 |
| `LongRunning` | 1 | 2 |

`OperationalTuning::retry_limit` may lower that base, which defaults to 3;
`validate_tuning` rejects a larger value, so tuning never adds retries.
`CommandRetryClass::Never` and `InquiryRetryClass::Never` override the table
with zero retries — one attempt — for their respective lifecycles.

Two bounds stop a request retrying: the count above, and one wall-clock budget
counted from admission. That same budget remains active through every later
noncancelled attempt phase — backoff, ready, send, ACK, execution, and reply —
so an active attempt cannot silently extend the total. A cancellation or
per-request unconfirmed quarantine is separate, governed by its own ambiguity
deadline, and is never shortened by budget expiry. That wall-clock budget is
the largest of ten seconds, twice the request's response deadline
(completion for a command, reply for an inquiry), and the profile's busy timeout.
For an ACK-bearing command it is also at least one full ACK-plus-completion
attempt. This additional floor permits a longer valid ACK override without
ending a healthy executing command; untuned built-in budgets are unchanged
(#842). Completion-only commands and inquiries keep one governing response
window. Whichever bound is reached first produces the terminal
error, and a request that runs out of wall-clock time reports the error that
caused its last retry rather than an incidental later timeout. If that expiry
catches a successfully sent raw command in an ambiguous phase, the per-request
unsequenced rule above governs it: the one command fails
`UnsequencedCommandUnconfirmed` and quarantines its correlation (or, under the
strict opt-in, poisons the session) rather than finishing with the retained
last error.

Default observers cover the prepared request's whole governing attempt and
admission-to-terminal retry budget. For a typed inquiry that is the larger of
its reply deadline and retry budget. That keeps a
crate-authorized replay — including its bounded raw correlation hold —
observable by default. An explicit observation or settlement deadline remains
the caller's exact bound and is not widened.

Backoff doubles from the initial delay (50 ms by default) up to
`maximum_backoff` (500 ms by default, raised to the profile's busy timeout
where that is longer), with the exponent additionally capped at five for a lost
ACK: a camera that has not even accepted a frame should not inherit a ceiling
raised to accommodate slow completions. Each wait is then drawn from the
half-open equal-jitter band `[ceiling / 2, ceiling)`, so no attempt ever waits
longer than the undithered ceiling and two commands that time out on the same
instant do not retry on the same instant. That draw is a pure function of the
engine's seed, the request identity and the attempt number: it never reads the
clock or process entropy, so the same engine input sequence produces identical
scheduling.

## Timeouts: stage and certainty

A timeout records the deadline stage and what is known about the request.
`Error::recovery()` combines those facts with session validity and the condition
into one verdict. Only `Resubmit { after }` permits submitting a request again;
`WaitAgain` observes the existing operation, `Reconcile` inspects state,
`ReopenSession` replaces a dead session, and `Abandon` ends automatic recovery.
An ACK is `WaitAgain`, an uncertain effect is `Reconcile`, and deliberate
supersession never authorizes automatic replay. Session death takes precedence
when an observation failure retains a fatal source.

The recovery rules apply in this order: session death gives `ReopenSession`,
`StillLive` gives `WaitAgain`, `Unconfirmed` gives `Reconcile`, and a conclusive
failure gives `Abandon` or the kind's `Resubmit` delay. Three conditions name an
exception: `CommandPending` is live without a failure context;
`MaxRetriesExceeded` and `InquiryShapeCapacityExhausted` are permanent and give
`Abandon`. Terminal wrappers replace source liveness and uncertainty with their
own facts, but retain a permanent source's conclusive verdict. Context strings
preserve every source fact and verdict.

`SettlementSuperseded` deliberately ends a settlement observation. It adds no
new uncertainty about the original command's application, so its context is
`None` and its verdict is `Abandon`. `MotionSuperseded` retains the actual
`NotAccepted` or `Unconfirmed` fact; the latter requires reconciliation.
Cancellation-attempt failures always report `CancellationAttempt` / `StillLive`
while retaining their original source. A repeated cancellation observes that
cached answer; wait for the operation or stop the axis.

The table is checked against one value of every public `Error` variant, with
additional rows for context-dependent outcomes.

Serial startup distinguishes device I/O from the topology reported by Address
Set. `CameraNotAddressed { port, camera, addressed }` means the registered camera
is beyond the reported chain; `SerialChainTooLong { port, reported }` means the
chain exceeds seven cameras. Both are `InvalidParameter` / `Abandon`: correct
the chain or configuration before opening again. An absent serial device remains
`ConnectionFailed` with its original operating-system cause. A silent chain that
exhausts all Address Set attempts remains `MaxRetriesExceeded` (`Timeout` /
`Abandon`), because it provides no topology evidence.

<!-- BEGIN GENERATED RECOVERY VERDICTS -->
| Error / condition | Kind | Stage / certainty | Recovery |
| --- | --- | --- | --- |
| `ConnectionFailed` | `IoRefused` | — | `Abandon` |
| `CameraNotAddressed` | `InvalidParameter` | — | `Abandon` |
| `SerialChainTooLong` | `InvalidParameter` | — | `Abandon` |
| `ConnectionClosed` | `IoClosed` | — | `ReopenSession` |
| `CommandPending / ACK received` | `Busy` | — | `WaitAgain` |
| `InvalidResponse` | `Protocol` | — | `Abandon` |
| `FeatureNotSupported` | `Unsupported` | — | `Abandon` |
| `Io / other OS error` | `Other` | — | `Abandon` |
| `SyntaxError` | `InvalidParameter` | — | `Abandon` |
| `CommandBufferFull` | `BufferFull` | — | `Resubmit after 200ms` |
| `CommandCancelled` | `Cancelled` | — | `Abandon` |
| `NoSocket` | `BufferFull` | — | `Resubmit after 200ms` |
| `CommandNotExecutable` | `NotExecutable` | — | `Abandon` |
| `InvalidResponseFormat` | `Protocol` | — | `Abandon` |
| `InvalidResponseLength` | `Protocol` | — | `Abandon` |
| `UnexpectedResponseType` | `Protocol` | — | `Abandon` |
| `Unknown` | `Protocol` | — | `Abandon` |
| `InvalidRequest` | `InvalidParameter` | — | `Abandon` |
| `MessageLengthError` | `Protocol` | — | `Abandon` |
| `TransportError / no uncertain effect` | `Transport` | — | `Resubmit after 50ms` |
| `TransmissionNotSent / first attempt, invalid request` | `InvalidParameter` | Terminal / NotAccepted | `Abandon` |
| `TransmissionNotSent / earlier attempt uncertain` | `InvalidParameter` | Terminal / Unconfirmed | `Reconcile` |
| `InquiryFailed / timeout cause` | `Timeout` | Terminal / FailedConclusively | `Resubmit after 2s` |
| `InquiryFailed / command-scoped source` | `Unconfirmed` | Terminal / FailedConclusively | `Abandon` |
| `InquiryFailed / still-live source` | `Timeout` | Terminal / FailedConclusively | `Resubmit after 2s` |
| `InquiryFailed / session ended` | `IoClosed` | Terminal / FailedConclusively | `ReopenSession` |
| `InvalidParameter` | `InvalidParameter` | — | `Abandon` |
| `BufferTooSmall` | `InvalidParameter` | — | `Abandon` |
| `ParameterOutOfRange` | `InvalidParameter` | — | `Abandon` |
| `Timeout / terminal, not accepted` | `Timeout` | Terminal / NotAccepted | `Resubmit after 2s` |
| `MotionSuperseded / never sent` | `Cancelled` | Terminal / NotAccepted | `Abandon` |
| `SettlementSuperseded / deliberate observation end` | `Cancelled` | — | `Abandon` |
| `SettlementObservationFailed / live session` | `InvalidParameter` | Observation / Unconfirmed | `Reconcile` |
| `CancellationObservationFailed / operation still live` | `Timeout` | CancellationAttempt / StillLive | `WaitAgain` |
| `ObservationTimeout` | `Timeout` | Observation / StillLive | `WaitAgain` |
| `ControlReserveExhausted` | `BufferFull` | — | `Resubmit after 200ms` |
| `InquiryShapeCapacityExhausted` | `BufferFull` | PreAdmission / NotAccepted | `Abandon` |
| `MaxRetriesExceeded` | `Timeout` | — | `Abandon` |
| `CancellationNotSupported` | `Unsupported` | — | `Abandon` |
| `InvalidState` | `NotExecutable` | — | `Abandon` |
| `OwnerTerminated` | `IoClosed` | — | `ReopenSession` |
| `DiagnosticSubscriberLimit` | `Other` | — | `Abandon` |
| `RuntimeShutdown / deliberate` | `IoClosed` | — | `Abandon` |
| `CancellationUnconfirmed` | `Unconfirmed` | CancellationAttempt / Unconfirmed | `Reconcile` |
| `UnsequencedCommandUnconfirmed` | `Unconfirmed` | Terminal / Unconfirmed | `Reconcile` |
| `InquiryCorrelationLost / lane latched` | `NotExecutable` | Terminal / NotAccepted | `Abandon` |
| `CommandCorrelationLost / lane latched` | `NotExecutable` | Terminal / NotAccepted | `Abandon` |
| `CommandFailedAfterAck` | `Unconfirmed` | Terminal / Unconfirmed | `Reconcile` |
| `RequestUnconfirmed / later rejection after a lost ACK` | `Unconfirmed` | Terminal / Unconfirmed | `Reconcile` |
| `RuntimeIdentityExhausted` | `Other` | — | `Abandon` |
| `RuntimeQueueFull` | `BufferFull` | — | `Resubmit after 200ms` |
| `StreamPoisoned` | `IoClosed` | — | `ReopenSession` |
| `InvalidCameraId` | `InvalidParameter` | — | `Abandon` |
| `ResponseTooLarge` | `Protocol` | — | `Abandon` |
| `InvalidAddress` | `InvalidParameter` | — | `Abandon` |
| `UnsupportedTransport / built-in or custom profile` | `Unsupported` | — | `Abandon` |
| `MissingRuntime` | `Unsupported` | — | `Abandon` |
| `WithContext / pending command` | `Busy` | — | `WaitAgain` |
| `Timeout / inquiry reply` | `Timeout` | Terminal / FailedConclusively | `Resubmit after 2s` |
| `Timeout / command outcome unknown` | `Timeout` | Terminal / Unconfirmed | `Reconcile` |
| `Timeout / write outcome unknown` | `Timeout` | Session / Unconfirmed | `Reconcile` |
| `Timeout / cancel attempt deadline` | `Timeout` | CancellationAttempt / StillLive | `WaitAgain` |
| `TransportError / uncertain write` | `Transport` | Session / Unconfirmed | `Reconcile` |
| `MotionSuperseded / earlier attempt uncertain` | `Cancelled` | Terminal / Unconfirmed | `Reconcile` |
<!-- END GENERATED RECOVERY VERDICTS -->

```rust
use grafton_visca::{
    Error,
    error::Recovery,
};

fn next_step(error: &Error) -> &'static str {
    match error.recovery() {
        Recovery::Resubmit { .. } => "submit again after the specified delay",
        Recovery::WaitAgain => "observe the existing operation",
        Recovery::Reconcile => "inspect camera state before deciding",
        Recovery::ReopenSession => "open a new session",
        Recovery::Abandon => "do not retry automatically",
        _ => "inspect this future recovery verdict",
    }
}

assert_eq!(next_step(&Error::CommandPending), "observe the existing operation");
assert_eq!(next_step(&Error::io_timeout()), "inspect camera state before deciding");
```

The same recipe is compiled in the `Error` and `FailureContext` rustdoc.

## Transient transport faults and session death

Not every transport failure ends a session. A receive that fails without
proving the connection is gone — the classic case is a UDP `recv` reporting
ECONNREFUSED after an ICMP port-unreachable for an earlier datagram — may retry
a sequenced Sony command still waiting for its ACK under that command's bounded
policy. On Windows UDP, Winsock `WSAECONNRESET` (10054) reports that earlier
ICMP error and is normalized to transient `ConnectionRefused`, preserving its
original source; [Microsoft documents this UDP receive behavior](https://learn.microsoft.com/windows/win32/api/winsock/nf-winsock-recv).
A stream `ConnectionReset` still proves closure (#843). A raw command has no
sequence to replay; its complete transient-fault
decision is the architecture guide's
[raw recovery rule](architecture_2_0.md#raw-unconfirmed-outcomes-and-strict-recovery).
A read that proves the connection is gone ends the session. This includes
`ConnectionClosed`, runtime shutdown, and the raw I/O kinds in the
[shared fatal receive table](../src/transport/receive_errors.rs), generated
alongside `transport::FATAL_RECEIVE_IO_KINDS` for both transport traits. A raw OS `TimedOut` is included because a
connected TCP socket commonly reports it when keepalive has exhausted; it is
not the spelling for an application-owned idle timer. The owner normalizes
that fatal receive closure to
`Error::ConnectionClosed` and retains the original typed transport error in
its `source()` chain. The outer session-death verdict remains `ConnectionClosed`;
a framing or write failure uses `StreamPoisoned` with its own typed cause. A failed read consumes nothing and so cannot
desynchronize framing.

A read that reports *no data* is a third case and not a fault at all.
Any `Error::Timeout`, and the raw `Io` spellings `WouldBlock` and `Interrupted`,
mean an idle read timeout expired with nothing to show for it.
Both owners treat that as "this read produced no frames": the session lives,
framing state is untouched, and no request's retry budget is spent. A transport
with an internal read timeout — the shape `BlockingTransport::recv_into_with_timeout`
documents, and the natural way to write a custom async transport — must return
`Error::io_timeout()` rather than forwarding `io::ErrorKind::TimedOut`. UDP adapters
additionally discard valid zero-length datagrams
inside the adapter and keep receiving; they never translate a datagram with no
payload into the `Ok(0)` value reserved for stream EOF. A timed blocking
receive retains one overall deadline while discarding such datagrams; an empty
datagram cannot reset or extend that deadline. The async UDP adapter yields
cooperatively after an empty datagram before polling again, so a stream of empty
packets cannot starve owner controls.

The async owner enforces `read_timeout` and `write_timeout` even when a custom
transport has no timer. The blocking owner passes the configured bound to its
only read/write trait operations, which implementations must honor, and rejects
zero bounds before owner construction. Both owners pace immediately idle
reads. The exact stream/datagram failure split, pacing, and source-fairness rules live in
[Transport I/O deadlines and idle pacing](architecture_2_0.md#transport-io-deadlines-and-idle-pacing)
and the preceding arbitration section.

A fault that never stops repeating stops being called transient. Consecutive
transient faults, with no successful read between them, escalate their pause
from 10 ms to a 250 ms ceiling, and a read that has failed twelve times in a row
over at least a second ends the session as `ConnectionClosed` with the count and
underlying typed transport error retained through `WithContext` in its source
chain, rather than retrying against
a dead adapter forever. One successful read, or a five-second gap between
faults, clears the run.

Fixed-format ACK, completion, error, and network-change frames are accepted
only at their exact lengths: three bytes for ACK, completion, or network change,
and four bytes for an error. A known fixed prefix with trailing bytes is
malformed, never `Unknown`. Variable data replies are reserved for socket 0
with more than three bytes; the canonical three-byte `z0 50 FF` completion
remains valid.

Datagram transports have an additional boundary check before this decode: every
receive reports a `ReceiveOutcome`, and a datagram that is not complete (one the
OS reported truncated, or one a custom transport could not prove fitted
`recv_buffer_size`) is discarded as one malformed input on both facades. Its
copied prefix is never treated as a complete frame, even when that prefix would
itself be a valid ACK or completion.

Decoding is classified by transport. On a byte stream, a frame that the framer
has already delimited at its `FF` boundary but the strict decoder cannot
classify is discarded and recorded as `Ignored(MalformedFrame)`; it does not
poison the session. Only a genuine loss of the framing position — for example
an unrecoverable buffer overflow or a boundary-free read beyond the configured
limit — becomes `Error::StreamPoisoned`. Complete Sony replies preceding that
failure in a stream read reach the owner in wire order. A completion can
therefore confirm a command and update its cache before the session terminates.
The owner discards an undecodable datagram as a whole. Each subsequent
datagram frames independently, and the session keeps running.

Writes are classified by transport too. A failed datagram write fails exactly
one request and the session continues. Bytes may have reached the camera:
raw commands report `UnsequencedCommandUnconfirmed` and retain an inert
correlation hold, while Sony commands report `RequestUnconfirmed` with the
exact transport cause. Both require reconciliation; the strict raw policy
poisons instead. Raw inquiries fail conclusively and retain their reply-skew
hold. These rules are independent of the retry budget. A failed stream write poisons the
session: no transport trait in this crate reports how many bytes of a frame
reached the wire, so a partial write must be assumed and the byte-stream
position treated as unknowable. The exact transport cause is carried in the
`StreamPoisoned::source()` chain.

## Silent peers and connection liveness

A long-lived TCP control link can fail in two materially different ways. The
camera or network may close it, or the socket may stay open while the camera
stops answering VISCA. A close surfaces here as `Error::ConnectionClosed`
(`requires_new_session() == true`) with an optional typed peer-closure cause,
and — because a dead session retains its original terminal cause (#680) — every
later acting call — command, inquiry, cancellation, tuning change, diagnostic
subscription, shutdown or close — keeps reporting the same peer-closure cause
rather than a generic channel error. The read-only `metrics()` observation
returns the retained terminal snapshot with `session == Closed`. This
is the failure a long-running supervisor must expect during quiet periods.

On a datagram transport, a built-in inquiry is written at most
`1 + (retry base + 2)` times. The default allows six writes.
`OperationalTuning::with_retry_limit` can lower the base. Each attempt waits
for the profile's inquiry deadline, which is one second on every built-in
profile. Retries use backoffs from 50 ms to 500 ms, with a higher ceiling when
the profile's busy timeout exceeds 500 ms. Transport pacing also applies.
The inquiry ends at the earlier of the attempt limit and the retry budget,
`max(10 s, 2 × inquiry deadline, busy timeout)` before tuning. An untuned
built-in therefore fails after about six inquiry deadlines plus backoffs and
pacing. The facade returns `Error::InquiryFailed { source: Timeout { .. }, .. }`
with stage `Terminal` and certainty `FailedConclusively`.

On a raw VISCA stream, an inquiry is written once. After its reply deadline,
the reply stays owed. Inquiries with overlapping declarations wait through
the ambiguity window and then fail unwritten with
`Error::InquiryCorrelationLost { camera }` (`NotExecutable`, `Terminal` /
`NotAccepted`, `Recovery::Abandon`). Disjoint inquiries, including position
queries used by `is_moving`, `wait_until_idle`, and settlement polling, can
continue. Their answers can settle older debts by write order. An owed
`CompletionOnly` answer holds all inquiries to its camera. ACK-bearing
commands and STOPs remain eligible when the only debt is an inquiry reply.
See the [raw stream correlation ledger](architecture_2_0.md#the-raw-byte-stream-correlation-ledger)
for the rules for inquiry and command debts, including
`Error::CommandCorrelationLost`. To clear a debt on a camera that never answers
any work, close and reopen the session.

The inquiry timeout itself is retryable and
`requires_new_session() == false`: it proves only that this request received no
answer. A response-bearing command on a raw-VISCA envelope can instead end as
`UnsequencedCommandUnconfirmed` once its ACK/completion and ambiguity windows
expire. That result has the dedicated `ErrorKind::Unconfirmed` and returns
`requires_new_session() == false`, because the session remains usable by
default and only that command's outcome is unknown. Neither result authorizes
an infinite retry loop or a blind replay.

`MetricsSnapshot::received_frames` is the positive liveness signal. It counts
every valid decoded VISCA response, including an error or an unmatched
sequenced reply. Sample it before and after an application heartbeat. An
increase proves the peer produced a valid response frame; no increase is not
proof of transport death, so the application must define how many unanswered
heartbeats or how much wall time constitutes a silent peer. Once that policy is
met, deliberately close and replace the session even though the final timeout's
`requires_new_session()` value is false.

There are two independent layers of connection liveness, and it is worth being
precise about which one a failure belongs to:

- **OS-level TCP keepalive.** Enabled by default on every TCP transport
  (blocking, Tokio, and smol) via `TcpConnect::keepalive`, whose
  default `TcpKeepaliveConfig` starts probing after ten seconds idle and repeats
  every ten seconds. The crate does not override the OS probe count; on Linux's
  usual count of nine, a black-holed peer is therefore detected after roughly
  100 seconds from the start of the idle period. Set the idle duration with
  `TcpKeepaliveConfig::new(idle)` and the interval with `with_interval(Some(interval))`.
  Pass that value to `TcpConnect::default().with_keepalive(Some(keepalive))`.
  Use `with_keepalive(None)` to keep the operating system default. Supply the
  resulting settings to `CameraConfig::tcp_configured`. See the
  [configuration example in the migration guide](migration_2_0.md#extensible-public-payloads-788).
  Keepalive
  probes detect a peer that has gone away and keep NAT/firewall path state warm
  across an idle gap. They are **OS-level packets, not VISCA traffic**, so their
  presence does not tell the camera's firmware that the *application* session is
  still in use. When the OS finally reports `io::ErrorKind::TimedOut`, both
  owners now classify that first keepalive-exhaustion error as terminal and
  normalize it to `ConnectionClosed`; it is not swallowed as another idle read.
- **Application-level VISCA activity.** Some camera firmwares close an idle
  control session on their own timer, counting only VISCA requests as activity.
  Keepalive cannot prevent that close, because to the firmware the session has
  been silent. A regular, fixed idle interval before the drop (rather than a
  drop correlated with network events) is the signature of this application-side
  timeout.

The library never sends VISCA on its own: the owner, including a blocking
session's worker thread, only carries out work the application submitted (see
[`architecture_2_0.md`](architecture_2_0.md)). If a camera enforces an
application idle timeout there are two application-side levers:

1. **Detect or prevent it with an application heartbeat.** While the session would
   otherwise be idle, periodically issue a cheap inquiry (for example a power
   or version inquiry) at an interval comfortably below the camera's idle
   timeout. The inquiry resets the firmware's activity timer when the firmware
   is responsive. Its error alone does not guarantee an early
   `requires_new_session()` signal for a silent open socket; compare the frame
   counter and apply a bounded application policy:

   An unwritten heartbeat can also reflect a latched
   [raw inquiry overlap class](architecture_2_0.md#the-raw-byte-stream-correlation-ledger).

   ```rust,ignore
   // Application-owned idle heartbeat; the library starts no timer of its own.
   let before = session.metrics()?.received_frames;
   let result = session.camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)?.power().status();
   if result.as_ref().is_err_and(|error| error.recovery() == Recovery::ReopenSession) {
       rebuild(&config)?;
   } else if session.metrics()?.received_frames > before {
       record_positive_liveness();
   } else {
       record_unanswered_heartbeat(); // rebuild when your threshold is met
   }
   ```

2. **Recover from it with a supervisor loop.** Whether or not a heartbeat is
   used, treat an idle close as a completed session and rebuild, following the
   fresh-session steps below. A supervisor keeps the reusable `SessionConfig`
   and a re-callable transport factory outside the session, classifies each
   failure with `recovery()`, and on `ReopenSession` opens a fresh session,
   re-queries state, and resumes. `examples/recovery.rs` demonstrates both the
   absent-frame silence decision and positive liveness after the rebuild.

Which cameras enforce an application idle timeout, what that interval is, and
whether a heartbeat prevents the close are firmware-specific, not a library
guarantee. An optional private device check can distinguish a camera-initiated
close from an intervening network device by observing the FIN/RST direction.

## Fresh-session poison recovery

Treat a closed or poisoned owner as a completed session, not as a queue to
restart in place:

0. Classify the failure with `Error::requires_new_session()`. It returns `true`
   for transport-level session death (`ConnectionClosed` or `StreamPoisoned`)
   and owner death (`OwnerTerminated`), and `false` for deliberate `RuntimeShutdown` and the default per-request
   `UnsequencedCommandUnconfirmed` result. The latter now has
   `ErrorKind::Unconfirmed`; the terminal errors still share
   `ErrorKind::IoClosed`, so use the predicate for the reconnect decision.
   Fatal receive closure is normalized to `ConnectionClosed`
   with the original cause text; `StreamPoisoned` is reserved for an
   unknowable stream framing or write position. Note that
   `UnsequencedCommandUnconfirmed` returns `false` (issue #671): by default it is
   a per-request failure on a still-live session, so reconcile that one command's
   camera effect rather than rebuilding the session. The strict
   `strict_unconfirmed_poison` opt-in reports its session kill as `StreamPoisoned`
   instead, which this step already classifies as requiring a replacement.
1. Keep the validated, reusable `SessionConfig` outside the session.
2. Request `shutdown`/`close` and stop using views from the old owner.
3. Resolve or discard every old operation handle. Handles from the old owner
   must report their actual terminal/closed outcome; they are never silently
   rebound to a new owner.
4. Open a fresh session with the same configuration and select fresh target
   views. Their caches are `Unknown`, even if the old session had known values.
5. Re-query camera state explicitly through typed inquiries before making
   decisions that depend on device state.
6. Resubmit nothing automatically. If an application wants to repeat a command,
   it must make that choice after the fresh inquiries and normal validation.

The fresh owner rebuilds its bounded registry, pacing, retry, and transport
policy. It does not inherit stale lifecycle IDs, subscriber queues, or state
values from the poisoned owner.

## Memory bounds

The current owner policy is intentionally explicit. The first seven rows are
fixed; the remaining four are caller-tunable bounds:

| Resource | Bound | Caller-tunable |
| --- | ---: | --- |
| Registered targets | 7 | no |
| State entries per target | one slot per `StateKey` | no |
| Pre-admission diagnostic staging | 128 events | no |
| Queued control requests | 16 | no |
| Diagnostic subscribers | 4 | no |
| Events per diagnostic subscriber | 128 | no |
| Frames per receive batch | 64 | no |
| Admitted request lifecycle entries (pending or active) | `SessionConfig::admission_capacity()` (64 by default) for ordinary work, plus each camera's control reserve (one slot per supported typed STOP, at most three) | yes — `SessionConfig::with_admission_capacity()`; the reserve is set by each profile |
| Owner transport-read scratch / largest copied read | see below | yes — `BufferConfig::recv_buffer_size` |
| Single framed response | see below | yes — `BufferConfig::recv_buffer_size` |
| Stream input retained between reads | 8192 by default (the framer also holds one more read) | yes — `BufferConfig::max_buffer_size` |

Admission capacity is fixed when the session opens. The owner releases the
admission slot when the request reaches terminal. Ambiguity holds have a
separate bound of 20 per target. Capacity defaults to 64 and callers can
choose any nonzero value below `usize::MAX` with
`SessionConfig::with_admission_capacity()`.

The capacity bounds ordinary work. Each registered camera also reserves one
admission slot per typed STOP its profile supports — pan/tilt, zoom, and focus,
so at most three — and only an urgent typed STOP may use those slots. A STOP
takes its camera's reserve first and an ordinary slot only when the reserve is
held, so queued ordinary work cannot lock a STOP out, and one camera's stops
cannot use another camera's reserve. When a camera's reserve and the ordinary
budget are both full, the STOP fails with `Error::ControlReserveExhausted`
(D26, #778). An admitted STOP still waits for protocol pacing, socket
availability, and in-progress writes.

`OwnerLimits` fixes the 4096-byte receive scratch and the owner collection
bounds. The production adapter sizes transport buffers from
`IoBounds::buffer_config`. The retention limit defaults to 8192 bytes.
Standard camera constructors select their transport's buffer preset.
Caller-owned transports report bounds through `HasTransportConfig::io_bounds()`.

| Defaults | `recv_buffer_size` | Used by |
| --- | ---: | --- |
| `BufferConfig::for_tcp()` | 256 | built-in TCP transports |
| `BufferConfig::for_udp()` | 1024 | built-in UDP transports |
| `BufferConfig::for_serial()` | 256 | built-in serial transports |
| `BufferConfig::default()` | 128 | transport-neutral caller-owned defaults |

To raise the TCP retention bound, pass
`BufferConfig::for_tcp().with_max_buffer_size(16_384)` to
`IoBounds::default().with_buffer_config(...)`. Supply the checked bounds to
`CameraConfig::tcp_configured`.

Every default keeps `max_buffer_size` at 8192. The two limits mean:

- `recv_buffer_size` is the largest accepted reply frame — a raw VISCA frame
  including its terminator, a Sony header plus payload, or one UDP datagram —
  and the most bytes one transport read requests, so it is also the owner's
  per-session read scratch. Validation requires at least
  `BufferConfig::MIN_RECV_BUFFER_SIZE` (24 bytes: the largest VISCA reply, 16
  bytes, behind Sony's 8-byte header), so every valid reply fits one read. An
  over-size datagram is rejected before framing rather than silently
  truncated. Standard TCP transports read directly into the supplied slice:
  no additional user-space read-ahead buffer retains bytes outside these bounds.
- `max_buffer_size` bounds the stream input carried from one read to the next:
  an incomplete frame, or complete frames the owner has not delivered yet. The
  framer always has room for one more full read on top of it, so a healthy
  stream whose replies are split across reads never overflows, and a byte
  stream never retains more than `max_buffer_size + recv_buffer_size` bytes.
  Validation requires `recv_buffer_size <= max_buffer_size`.

These are implementation policy limits surfaced here so integrations can
budget memory. They are not permission to add per-frame, per-retry,
per-subscriber, or per-handle unbounded allocation.

See [`architecture_2_0.md`](architecture_2_0.md) for owner ordering and
[`allocation_and_validation.md`](allocation_and_validation.md) for release
allocation gates.
