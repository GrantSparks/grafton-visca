# Observability, state, and recovery

The public observability surface is deliberately smaller than the private
owner trace. It is stable, bounded, and safe to consume from an application
without granting control over the protocol engine.

## Metrics

`MetricsSnapshot` is a copy of scalar owner state. Its stable fields are:

| Field | Meaning |
| --- | --- |
| `admitted` | Requests admitted into authoritative engine state. |
| `admission_rejected` | Requests rejected before admission, including abandoned async submissions (`Cancelled` admission diagnostics); those never produce dropped-observer events. |
| `control_reserve_admitted` | Urgent typed STOPs admitted into their camera's control reserve rather than an ordinary slot. |
| `control_reserve_rejected` | Urgent typed STOPs rejected because their camera's control reserve and ordinary admission were both full (also counted in `admission_rejected`). |
| `writes` | Transport writes attempted. |
| `write_failures` | Writes that returned an error. |
| `terminal` | Requests reaching a terminal outcome. |
| `cancellations` | Cancellation requests observed by the owner. |
| `cache_updates` | Exact applied-state effects committed to a target cache. |
| `ack_timeouts` | Acknowledgement deadlines that expired on a sent command. |
| `completion_timeouts` | Completion deadlines that expired while an acknowledged command or completion-only raw command awaited completion. |
| `inquiry_timeouts` | Reply deadlines that expired on a sent inquiry. |
| `busy_errors` | Error frames saying the camera cannot accept the request now. |
| `protocol_errors` | Every other error frame, excluding the cancellation reply. |
| `retries_scheduled` | Requests re-queued for another attempt, for any reason. |
| `received_frames` | Valid decoded VISCA response frames received from the camera; compare snapshots around an application heartbeat for positive liveness evidence. |
| `ignored_unmatched_sequenced_replies` | Sequenced replies matching no request. |
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

## Diagnostics

Both facades expose `subscribe_diagnostics(capacity)`. An async subscription
uses `r#async::Diagnostics`, and a blocking subscription uses
`blocking::Diagnostics`. Both provide `try_recv`, `recv`, and
`recv_with_timeout`; the async receives are awaited. A subscription capacity must be `1..=128`, and
one owner retains at most four live subscriptions. A fifth subscription fails
with nonretryable `DiagnosticSubscriberLimit { capacity }`; drop an existing
subscription to make room. Request-admission capacity does not change this
subscriber limit. Subscriptions receive events recorded after they are created;
there is no retained diagnostic history or replay.

`DiagnosticEvent` is a bounded, `#[non_exhaustive]` enum containing only
sanitized categories: admission, frame category, protocol phase transition,
write success, retry, cancellation, applied-state, terminal outcome, session
state, and ignored-input observations. Its helper enums are also bounded and
non-exhaustive. Events contain no wire bytes, arbitrary strings, timestamps,
socket/transmission authority, or public numeric lifecycle IDs.

`DiagnosticId` is opaque. It can be copied, compared, hashed, and used to
correlate events from a subscription, but it cannot be constructed, inspected,
or converted into a number by application code. Public diagnostics are not a
replacement for private debug tracing; applications that need raw frames for
support should enable an explicitly private, access-controlled trace path.

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

`KnownState` carries each key's native value: booleans, enums, checked speeds, `ImageFlipMode`, or a typed
`PanTiltLimitUpdate`. Tally flashing stores `TallyMode::Flash`; it does not
invalidate the key. `cache.motion_sync().speed()` stores `MotionSyncSpeed`. A cleared limit is a known corner-local
update whose `position()` is `None`; it remains `StateEntry::Set`. The owner
registry has seven target slots and one inline slot per `StateKey`, without
allocation, eviction, or a separate capacity limit. A view contains one target
index and a shared registry reference; cloning it shares the table.

An exact `AppliedStateEffect` establishes a known value. A write that may have
taken effect but ends without confirmation makes its key `Unknown`, including
a session-ending failure after the write left. A command never written, or
conclusively rejected before ACK with no possibly applied earlier attempt,
leaves the previous value intact. Preparing a command and an ACK alone cannot
establish a value. The owner commits effects before notifying observers or
resolving the terminal outcome, independently of observer attachment. A fresh
session starts every target at `Unknown`; target views remain isolated.

## What is retried, and for how long

Retry requires evidence that replay is safe. Every retry class except
`RetryClass::Never` may retry a camera reporting a full command buffer (`0x03`)
or no free socket (`0x05`), because that response conclusively rejected the
attempt. `0x41` (`CommandNotExecutable`) is the one answer whose retryability
depends on the class: it is transient for movement and preset work, where the
camera is reporting a state that passes, and terminal everywhere else, where
it is the camera's verdict on the command.

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

Raw inquiry replies are unkeyed too. The production Raw adapter admits one live
inquiry per target. A matched reply releases that lane immediately, without a
hold. An uncertain datagram release (terminal error/timeout or retry requeue)
retains only the profile's `raw_inquiry_reply_skew`, bounded by minimum inquiry
spacing. ACK-bearing commands, including STOPs, remain independently eligible.
The hold filters stale unkeyed inquiry data and socketless errors while exact
attributable command evidence remains eligible. On a raw stream a written,
unanswered inquiry is never resent: its first reply deadline fails it, and the
ledger keeps owing the reply. That debt can latch the camera's inquiry lane;
it never blocks STOP delivery or ends the session (#712/#795). At an exact
response or skew deadline, complete input wins before due work; retained partial
bytes follow the [byte-stream ambiguity rule](architecture_2_0.md#engine-transition-and-ordering).

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
`RetryClass::Never` overrides the table with zero retries — one attempt — for
every timeout category.

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
| `InquiryNotCancellable` | `InvalidParameter` | — | `Abandon` |
| `InvalidAddress` | `InvalidParameter` | — | `Abandon` |
| `UnsupportedTransport` | `Unsupported` | — | `Abandon` |
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
limit — becomes `Error::StreamPoisoned`. On a datagram transport one undecodable
datagram is one bad datagram: nothing else was consumed and the next datagram
frames independently, so it is discarded while the session keeps running.

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

Silence does **not** produce that verdict. A default built-in inquiry retries
within its bounded policy and ordinarily reports `Error::Timeout` (stage
`Terminal`, certainty `FailedConclusively`) at the ten-second total
retry-budget floor (roughly 10.05 seconds when the first backoff is included).
On a raw-VISCA stream (TCP) the inquiry is instead written once and fails at
its first reply deadline: a stream cannot lose the request, so a resend would
only add late replies. Its reply is then owed. Until it arrives, that camera's
next inquiries wait; once the profile's ambiguity window has passed they fail
at once, unwritten, with `Error::InquiryCorrelationLost { camera }`
(`kind() == ErrorKind::NotExecutable`, `failure_context()` = `Terminal` /
`NotAccepted`, `recovery() == Recovery::Abandon`). A silent camera therefore stops
answering *inquiries* on that session. The session is still live, so
`requires_new_session()` is `false`. Meanwhile ACK-bearing commands and stops
to that camera are still written. The engine resolves every frame on a raw
stream in write order (the correlation ledger in `docs/architecture_2_0.md`),
and a camera answers in the order it reads: the owed reply — data or a
socketless rejection — reopens the lane whenever it arrives, and so does the
camera's answer to any command written after that inquiry, which proves the
reply will never come. A camera that never answers one inquiry therefore keeps
accepting commands, and its next command answer reopens its inquiries; a
later command's own answer still reaches that command. To recover inquiries
to a camera that answers nothing at all, close and reopen the session. A raw
`CompletionOnly` command is exclusive on its camera until its completion or
rejection arrives: inquiries and ordinary commands to that camera queue
behind it (and fail with the errors above once its window ends unanswered),
while STOPs are always sent. Everything that needs an inquiry to that
camera fails with `InquiryCorrelationLost` meanwhile: `settled()`
observed-stable settlement polling (as `SettlementObservationFailed` with that
source), `is_moving`, and `wait_until_idle`. The same applies to commands: a
command that ended `UnsequencedCommandUnconfirmed` before its ACK still owes
that answer, and once its ambiguity window has passed ordinary commands to the
camera fail unwritten with `CommandCorrelationLost { camera }` until it
arrives or the camera answers a later inquiry or STOP (a `CompletionOnly`
completion is settled only by itself); STOPs are always sent (#795).

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
  (blocking, Tokio, and smol) via `TransportConfig::tcp_keepalive`, whose
  default `TcpKeepaliveConfig` starts probing after ten seconds idle and repeats
  every ten seconds. The crate does not override the OS probe count; on Linux's
  usual count of nine, a black-holed peer is therefore detected after roughly
  100 seconds, not after the first ten-second idle period. Tune it with the
  blocking builder's `tcp_keepalive(...)` /
  `disable_tcp_keepalive()`, or by setting `TransportConfig::tcp_keepalive`
  before handing the config to a runtime connector or `CameraConfig`. Keepalive
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

   ```rust,ignore
   // Application-owned idle heartbeat; the library starts no timer of its own.
   let before = session.metrics()?.received_frames;
   let result = session.camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)?.power().status();
   if result.as_ref().is_err_and(|error| error.recovery() == Recovery::ReopenSession) {
       rebuild(&config)?;
   } else if session.metrics()?.received_frames > before {
       record_positive_liveness();
   } else {
       // Includes a latched inquiry lane after a prior unanswered raw inquiry.
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
| Admitted request lifecycle entries (pending or active, including quarantine) | `SessionConfig::admission_capacity()` (64 by default) for ordinary work, plus each camera's control reserve (one slot per supported typed STOP, at most three) | yes — `SessionConfig::with_admission_capacity()`; the reserve is set by each profile |
| Owner transport-read scratch / largest copied read | see below | yes — `BufferConfig::recv_buffer_size` |
| Single framed response | see below | yes — `BufferConfig::recv_buffer_size` |
| Stream input retained between reads | 8192 by default (the framer also holds one more read) | yes — `BufferConfig::max_buffer_size` |

Admission capacity is fixed when the session opens and bounds each admitted
boundary plus its pending/active owner lifecycle entry until safe terminal
removal, including ambiguity quarantine. It defaults to 64; callers may choose
any non-zero value below `usize::MAX` with
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

The three transport-buffer rows are set from `TransportConfig::buffer_config`
every time a production session is built, so the first two have no single
number. `OwnerLimits::default()` retains internal 4096/8192 test defaults, but
the production adapter replaces both values before allocating owner buffers.
Every built-in entry point — `CameraConfig`, the blocking `Transport` /
`NetTransportBuilder`, the runtime connectors and the direct transport
constructors — starts from the same per-transport defaults:

| Defaults | `recv_buffer_size` | Used by |
| --- | ---: | --- |
| `TransportConfig::for_tcp()` | 256 | every built-in TCP transport |
| `TransportConfig::for_udp()` | 1024 | every built-in UDP transport |
| `TransportConfig::for_serial()` | 256 | every built-in serial transport |
| `TransportConfig::default()` | 128 | the transport-neutral base for a caller-owned transport |

Change one field by starting from the matching constructor, for example
`let mut config = TransportConfig::for_tcp(); config.buffer_config.max_buffer_size = 16_384;`,
then pass it to `CameraConfig::<P>::transport_config(config)`; a supplied
configuration is used as given. The blocking `NetTransportBuilder` also has
`recv_buffer_size` / `max_buffer_size` methods, and a caller-owned transport
reports its own through `HasTransportConfig::transport_config()`.

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
