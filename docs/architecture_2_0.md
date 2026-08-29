# 2.0 architecture

This page describes the public 2.0 vocabulary and the ownership rules behind
it. The implementation has one protocol authority per session. A typed,
blocking, async, or dynamic camera value is a view onto that authority; it is
not another connection.

## Public vocabulary

| Concept | 2.0 meaning |
| --- | --- |
| `SessionConfig` | Mode-independent, reusable profile/target registry and operational tuning. |
| `blocking::Session` | Caller-driven owner for a blocking transport. |
| `Session` | Owner handle for an async transport and selected executor/runtime. |
| `Camera<P>` | A statically checked view for one registered target and compile-time profile `P`. |
| `DynSessionCamera` | A runtime-profile view for one registered target. It exposes `DynSessionCameraControl`, the 14 dynamic noun traits, and `DynMotion`. |
| `StateCache` | A read-only, target-local projection of exact applied write-only state. |
| `MetricsSnapshot` and `DiagnosticEvent` | Bounded scalar metrics and sanitized owner observations. |

The final noun entry points are `power`, `zoom`, `system`, `pan_tilt`, `focus`,
`exposure`, `white_balance`, `image`, `presets`, `tally`, `nd_filter`,
`motion_sync`, `menu`, and `advanced`. Motion safety and observation are a
separate `motion()` view. Dynamic code uses the corresponding `Dyn*` noun
traits. Names such as `DynCameraControl`, `DynPanTiltControl`, and
`IntoDynCamera` belong to the historical 1.x vocabulary and are not 2.0 API
names.

## Ownership and lifetime

Construction moves one transport into one serialized owner. The owner is the
only component allowed to perform protocol scheduling, pacing, correlation,
retry, cancellation, transport I/O, state-cache mutation, and diagnostic
delivery.

* A blocking session drives that owner on the caller's thread.
* An async session spawns one detached owner task on the caller-selected
  executor. Tokio and smol adapters may both be compiled, but a session picks
  one runtime at construction.
* Typed camera views clone or borrow the owner handle and retain only their
  target and validated profile facts.
* Dynamic views erase profile/request types, not ownership, transport, runtime,
  timeout, or state-cache policy.
* `shutdown` is an idempotent, non-joining signal accepted by the one owner.
  It returns once the signal is accepted and does not claim that actor or
  transport teardown has finished.
* `close` consumes the session, requests the same shutdown, and waits for the
  sole detached owner to finish its boundary drain and drop its
  driver/transport. It is the deterministic transport-teardown barrier for
  reopening the same endpoint, not an executor-specific task join. When
  shutdown is accepted, `close` returns success only for the explicit
  `RuntimeShutdown` terminal cause; a transport close or stream poison that
  wins the normative source ordering is returned exactly. An immediate error
  sending `close`'s shutdown signal is preserved. Neither API is a second
  protocol authority or an implicit command resubmission mechanism.

There are no public callbacks, user-supplied lifecycle IDs, unbounded queues,
or per-camera background workers. Dropping a session or camera view does not
implicitly stop the owner or another view; use `shutdown` for the signal or
consuming `close` for the release barrier. An operation handle must be
explicitly cancelled or detached according to its documented lifecycle.

### Timeout and retry ownership

Timeout policy has one ownership boundary. A validated profile owns its exact
`CommandTimeouts` category table and its inquiry, acknowledgement,
cancellation, ambiguity, busy, and pacing facts. `OperationalTuning` contains
only validated per-category or per-fact overrides; it is not a second profile
or transport policy. Pure request preparation selects one response deadline —
the inquiry deadline for an inquiry, otherwise the request's exact command
category — and lowers it with the acknowledgement, cancellation, ambiguity,
and retry policy into an inert request context. The owner/engine consumes that
context without reselecting or inventing a deadline. `TransportConfig` carries
socket/serial behavior only and does not own protocol retry or completion
policy.

Private request, transmission, and correlation identifiers advance
monotonically within a session and are never reused. If the 64-bit identity
space is exhausted, admission or transmission fails closed with
`RuntimeIdentityExhausted`; an old owner input must never alias new work.

The two execution modes share protocol state-machine semantics, not an async
implementation hidden behind a blocking wrapper. `blocking` drives
`BlockingTransport` directly and has no Tokio, smol, futures executor, or
pollster dependency in its downstream graph. `async` drives `AsyncTransport`
through the caller-selected executor. CI checks the native blocking dependency
boundary for the network, serial, and `test-utils` feature sets; the async
testkit's executor rides on `async`, so enabling `test-utils` on a blocking-only
build links no executor.

## Target registry and preflight

`SessionConfig` has exactly seven individual target slots, VISCA IDs 1 through
7. Broadcast is never a session target. Registration is immutable once the
owner starts, and duplicate IDs, an eighth target, an empty registry, and
profile/configuration conflicts are rejected before transport I/O.

`camera()` is valid only when exactly one target is registered. A multi-target
session must use `camera_for(target)`, which also verifies that the requested
compile-time profile exactly matches the registered `ProfileSpec`. Dynamic
views use the same sole-target rule and an explicit target selection for
multi-target sessions.

Multi-target registration is rejected before socket creation on any
IP-addressed transport, which is every standard TCP and UDP configuration and
every Sony-encapsulated one. Heterogeneous *envelopes* are never accepted:
admission requires every registered profile to report the same wire envelope
as the first, and any mismatch is an error. What a multi-target session does
accept is heterogeneous *profiles* that share one envelope — in practice
several raw-VISCA profiles — carried by a serial-addressed transport. The
built-in serial transports qualify: both `transport-serial` and
`transport-serial-tokio` declare `AddressingMode::Serial`, so a caller-owned
transport is not required. A caller-owned transport that reports a standard
kind is validated against the profile transport registry. A truly custom
transport that reports no standard kind (`None`) may bypass only the standard
profile/transport pair matrix as an intentional BYO escape hatch. It still
undergoes target-registry, envelope, addressing/topology, pacing, framing,
buffer, and bounded-owner validation, and must provide its stream/datagram
semantics and transport configuration.

The static profile marker gates remain compile-time permissions. A runtime
`ProfileSpec` is validated in full before owner admission, and dynamic callers
use `capabilities()`/`supports_typed(...)` before selecting optional controls.
Metadata is discovery information; it is not a fallback that exposes an
unsupported typed operation.

## Engine transition and ordering

The construction and request path has a fixed order:

1. Build or deserialize `SessionConfig` and validate target IDs, registry
   cardinality, profiles, transport envelope, addressing, tuning, and bounded
   owner policy.
2. Create the one owner and its fixed target-local state registry. No socket,
   serial device, or protocol frame is opened before step 1 succeeds.
3. Start the owner (async) or retain the caller-driven owner (blocking).
4. Project `Camera<P>` or `DynSessionCamera` views without creating a second
   owner, runtime, transport, or cache.
5. Prepare each request against the selected target/profile and admit it to the
   owner. Preparation failures do not update state.
6. Schedule writes, replies, retries, pacing, cancellation, and completion in
   the owner. The strictest applicable profile pacing and bounded retry policy
   always wins.
7. For a request with an exact `AppliedStateEffect`, update that target's cache
   before best-effort observer fanout. A failed write, timeout, pre-ACK failure,
   or merely prepared command cannot create a cache value.
8. Record terminal outcome, release admission capacity, and retain only the
   bounded diagnostic/history state promised by the public API.

Correlation before ACK is envelope-specific. A raw-VISCA target has at most
one unacknowledged command candidate across `Sending`, `AwaitingAck`, and
`AwaitingLateAck`. Once its ACK assigns a socket, the next command may be
written while the first executes, so a two-socket camera retains its useful
concurrency without asking FIFO order to identify an ACK. Raw ACK and error
routing never uses command FIFO or temporal recency. Sony-encapsulated requests
may pipeline before ACK because their envelope sequence provides an exact
correlation key. An unsequenced ACK is never attributed by a guess.
When a raw ACK names a free socket, that socket is exact evidence. When the
named socket is instead held by another request — the classic cause is a lost
completion frame that made the camera reuse the socket — the ACK falls back to
the target's other free socket (issues #620/#682). Its candidate request was
already uniquely identified before the assignment, so the fallback cannot
mis-attribute the ACK; it only keeps the command from wedging on `AwaitingAck`.
Only when no socket is free does the ACK remain inert with `SocketConflict`. A
socketless ACK likewise selects the first free registered socket.

For a raw socketless error, the evidence rule is equally strict: route the
unique unacknowledged command only when no inquiry owner is live; otherwise
route the legitimate per-target inquiry FIFO only when no unacknowledged
command exists. A command-plus-inquiry collision is ignored. An explicit
socket routes only the exact owner of that target/socket, and a socketless
error never falls back to an `Executing` command.

Retry follows the same evidence boundary. Sony timeout recovery resends the
same logical request with the same sequence. A conclusive camera rejection
such as buffer-full or no-socket proves that the command did not start and may
be retried under policy. A raw command that was successfully written but then
loses its ACK, completion, or cancellation resolution, or encounters a receive
fault while awaiting ACK, is different: replay could perform a relative move
or preset twice, while a naive continuation could let a late reply bind to later
work. By default the owner therefore fails only that one command with
`Error::UnsequencedCommandUnconfirmed` — a per-request outcome the session
survives — and quarantines the correlation still at stake (its owned socket, or
its place as the sole unacknowledged command) until the ambiguity deadline, so a
late reply is ignored rather than bound to a later command (issue #671). The
session and every unrelated request keep running; the caller reconciles that one
command's camera effect rather than replacing the session. An active
retry-budget expiry in `Sending`, `AwaitingAck`, or `Executing` follows the same
per-request rule, while a retry still in a safe ready/backoff state finishes with
its retained last error when the total budget expires. The whole-session poison
is retained only behind the opt-in `strict_unconfirmed_poison` tuning (default
off), which restores the pre-fix behavior and surfaces it as
`Error::StreamPoisoned` so those callers still establish a fresh session. The
budget applies to every later noncancelled retry phase; a cancellation or
per-request quarantine is separate and is never shortened by budget expiry.

Fixed-format ACK, completion, error, and network-change frames are classified
only at their exact lengths. A known fixed prefix with trailing bytes is
malformed, not an `Unknown` response. For fixed ACK, completion, and error
forms, the socket nibble is also strict: `0` decodes as the valid socketless
compatibility form, `1` and `2` decode as S1/S2, and `3..=15` is malformed.
The variable data-reply form is reserved for socket 0 with more than three
bytes; the canonical three-byte `z0 50 FF` socket-0 completion remains valid.

That strict classifier verdict is not a session verdict. A frame the framer
already delimited at an `FF` boundary but that the classifier rejects is a
*malformed frame to discard* on a byte stream — recorded as
`Ignored(MalformedFrame)` and counted in
`OwnerMetrics::ignored_malformed_frames` — exactly as a datagram already
discards it and as 1.x logged-and-continued over the same padded ACKs, vendor
socket nibbles, RS-485 echoes, address-set replies, and truncated frames. The
session stays `Running` and the frame's real reply still settles the outstanding
command. Only the framer *losing its position* — cumulative buffer overflow, or
a boundary-free read growing past `max_buffer_size` — is a genuine framing
failure that still poisons a stream. By the same decoupling, a single stream
read that decodes more than `frames_per_receive` frames stops at the limit and
leaves the remaining complete frames buffered for the next receive (the framer
retains bytes across reads, bounded by `max_buffer_size`), rather than failing
`ResponseTooLarge` over a large-but-valid burst. A single-target IP session,
which carries no routable source, accepts any `0x9y..=0xFy` reply source and
attributes it to its sole target, so a camera configured with a non-default
chain address still settles its command (#590/#598); the strict `0x90` check is
kept only when more than one target is registered.

Blocking operation submission has one additional ownership boundary: a
returned operation handle always names a request whose initial transport write
already succeeded. One obstacle is drained rather than rejected. On a raw
profile the engine keeps at most one command in its unacknowledged window (the
single-candidate gate), so a first write submitted while a caller still holds an
un-awaited raw operation handle would otherwise lose the dispatch race even
though a command socket is free the instant the prior command's ACK lands. When
that pre-ACK gate is the *sole* obstacle and socket capacity would be available
once it clears, the blocking owner pumps the peer's ACK — bounded by the
submitting request's own ACK budget — so the first write wins and an emergency
`stop_all_motion`/`Urgent` stop still reaches a moving camera (#673). Genuine
socket-capacity contention (every command socket already occupied) and losing
the global dispatch race are *not* drained: the newly admitted request is
terminalized as `Error::TransportBusy` immediately and no handle escapes, since
pumping an ACK there would not free a socket. Ordinary blocking commands,
inquiries, and owner-internal requests retain bounded queueing, as does the
async operation API — whose always-running actor already pumps the ACK, so it
never exhibited the raw first-write stall.

This ordering is what permits a detached observer or a dropped subscription to
miss an event without losing an already-applied state update.

## Engine phases and the transition table

The engine keeps exactly one authoritative entry per admitted request. Each
entry carries a protocol `Phase` and a cancellation substate (`CancelState`);
both are `pub(crate)` in `src/runtime/engine/types.rs`, so they never reach
rustdoc and are documented here instead. The tables below are derived from the
transition functions in `src/runtime/engine/mod.rs` and describe the settled
post-#671 behavior, not an aspirational one.

### Protocol phases

| `Phase` | Meaning |
| --- | --- |
| `Ready` | Admitted and queued; nothing written yet. Holds a lazy-deletion `QueueTicket`. |
| `Sending` | The request's transport write is in flight; carries its `TransmissionId`. |
| `AwaitingAck` | A command was written and awaits its ACK (deadline = sent + ack). |
| `Executing` | The ACK assigned a socket; awaits completion (deadline = ack + completion). |
| `AwaitingReply` | An inquiry was written and awaits its reply (deadline = sent + inquiry). |
| `Backoff` | A retryable rejection or timeout scheduled a retry; re-enters `Ready` at `ready_at`. |
| `AwaitingCancellationResolution` | A socket cancellation was sent, or a lost completion is quarantined while the socket is still owned; awaits the original completion or the protocol-cancel terminal. |
| `AwaitingLateAck` | A sent raw command lost its ACK correlation and is quarantined until the ambiguity deadline so a late reply cannot misbind (issue #671). |

### Cancellation substates

| `CancelState` | Meaning |
| --- | --- |
| `None` | No cancel intent. It is also the marker for a #671 unconfirmed-command quarantine. |
| `Requested` | Cancel intent recorded before a socket was assigned; suppresses every later retry. |
| `Sending` | A cancel frame's write is in flight. |
| `AwaitingTerminal` | A cancel frame was written; awaits the original completion (`Completed`) or the protocol-cancel terminal (`Cancelled`). |
| `ObservationFailed` | A datagram cancel write failed; the token was resolved with that error while the original request stays live. |

### Transition table

| Input / state | Transition or result |
| --- | --- |
| Admit while `Running`, within capacity, target registered, policy compatible | Allocate a non-colliding `RequestId`, create one `Ready` entry with `CancelState::None`, enqueue it, and emit `Admitted`. No write happens here. |
| Admit over capacity | Reject with `Error::RuntimeQueueFull`; no id, entry, observer, or queue slot is created. |
| Admit with the id/generation space exhausted | Reject with `Error::RuntimeIdentityExhausted`. |
| Admit to a non-`Running` session | Reject with the session's terminal error (or `Error::RuntimeShutdown`). |
| Select a `Ready` request | Transition to `Sending`, allocate one `TransmissionId`, emit exactly one request `Transmit` (Sony carries its retained sequence; raw carries none). |
| Successful command send | Record any Sony sequence and transition to `AwaitingAck`. |
| Successful inquiry send | Record any Sony sequence, transition to `AwaitingReply`, and take a per-target FIFO position for a raw inquiry. |
| Failed command send, datagram transport | Terminally fail that one request with the exact transport error; every other entry keeps running. |
| Failed command send, stream transport | Poison the session (`Error::StreamPoisoned`) and resolve every active entry. |
| ACK in `AwaitingAck`/`AwaitingLateAck` with a free socket | Assign the socket and transition to `Executing`; if cancel intent is already recorded on a supported target, emit one socket cancellation. |
| ACK naming a busy socket | Fall back to the target's other free socket when it has more than one (issues #620/#682); when none is free the ACK stays inert as `Ignored(SocketConflict)`. |
| ACK while still `Sending` | Latch it once as a deferred ACK, applied when the send result lands. |
| Completion in `Executing` | `finish` with `RuntimeOutcome::Applied`; a retained cancellation observer maps this to `Completed`. |
| Inquiry reply in `AwaitingReply` | `finish` with the attributed payload. |
| Retryable conclusive rejection (buffer-full `0x03`/`0x05`, movement `0x41`), no cancel intent | Increment the bounded attempt and enter `Backoff`. |
| Retryable rejection with cancel intent | Suppress retry and `finish` with `Cancelled`, because no executing attempt exists. |
| `0x04` command-cancelled terminal | `finish` with `Cancelled`. |
| ACK timeout, Sony envelope, retryable | Retry within policy on ACK-capped backoff, replaying the exact sequence. |
| ACK/completion loss, a receive fault while `AwaitingAck`, or retry-budget expiry in an active raw phase | Default: fail only that command with `Error::UnsequencedCommandUnconfirmed` (the session survives) and quarantine its still-owned correlation — `AwaitingLateAck` when only its unacknowledged slot is at stake, `AwaitingCancellationResolution` when it still owns a socket (issue #671). |
| Quarantine / ambiguity deadline expiry | `finish` with `Error::UnsequencedCommandUnconfirmed` (raw) or `Error::CancellationUnconfirmed` (Sony or an open cancellation), releasing the reserved socket or slot only then. |
| Any of the two rows above under the `strict_unconfirmed_poison` opt-in | Poison the session and report `Error::StreamPoisoned`, restoring the pre-#671 behavior. |
| Retry becomes eligible (`Backoff` → ready) | Return to `Ready` and dispatch through the ordinary capacity and pacing gates. |
| Cancel in `Ready` or `Backoff` | Remove without I/O; emit `CancellationRecorded`, then `finish` with `Cancelled`. |
| Cancel in `Sending` (supported target) | Record `Requested` intent; the send result drives the next state. |
| Cancel in `AwaitingAck` (supported target) | Record `Requested` intent, suppress retries, and wait for socket assignment or the ambiguity deadline. |
| Cancel in `Executing` (supported target) | Record intent, emit one socket cancellation, and retain the original completion correlation. |
| Cancel after transmission on a target without socket cancellation | The cancellation observation fails with `Error::NotSupported`; record no intent, send no frame, and leave the original request active. |
| Cancel of an inquiry | The cancellation observation fails with `Error::InquiryNotCancelable`. |
| Duplicate cancel of the same request | `Ignored(DuplicateCancellation)`. |
| Successful cancel send | Transition to `AwaitingCancellationResolution` with `AwaitingTerminal`; wait for the original completion (`Completed`) or the protocol-cancel terminal (`Cancelled`). |
| Failed datagram cancel send | Resolve the token with the error (`ObservationFailed`), retain the original request and routing, and never retry that cancel for the same socket assignment. |
| Failed stream cancel send | Poison the session and resolve every entry and observer. |
| Observer detach or timeout | No engine input and no protocol transition. |
| Close / shutdown / poison | Resolve every active entry once, in admission order, with the distinct terminal error; clear every index and transmission; reject or drain boundary admissions deliberately. |

Non-retryable camera and transport errors stay exact even when cancel intent
exists. `Cancelled` is never used merely because a caller stopped waiting or the
engine lost certainty. When one input emits both a cancellation acknowledgement
and an immediate terminal result, `CancellationRecorded` precedes `Terminal`.

The four terminal conditions are distinct errors, and `Error::requires_new_session()`
separates a dead session from a recoverable one:

| Terminal condition | Error | `requires_new_session()` |
| --- | --- | --- |
| The peer closed the connection | `ConnectionClosed` | `true` |
| The stream position became unknowable, or the strict opt-in poisoned an unconfirmable raw command | `StreamPoisoned` | `true` |
| A sent raw command's outcome cannot be correlated (default per-request mode) | `UnsequencedCommandUnconfirmed` | `false` |
| The application shut the session down | `RuntimeShutdown` | `false` |

## Operational invariants

The engine and its serialized owner uphold the following invariants in every
supported configuration. They are the properties the deterministic engine, the
bounded owner boundaries, and the `#![forbid(unsafe_code)]` crate attribute
exist to guarantee.

- Runtime mutation is serialized per transport.
- Every admitted request has exactly one authoritative entry until safe terminal removal.
- Every request and cancellation transmission has one active identity and one result.
- Every request reaches at most one terminal engine transition.
- Every retained operation or cancellation observer resolves at most once.
- Every target/socket pair has at most one active owner.
- Every sequence owner is an active, target-compatible, and phase-compatible entry.
- Stale queue, retry, deadline, correlation, admission, or transmission tickets cannot send or resolve work.
- A cancel transmission is emitted at most once for one socket assignment.
- Observer removal never mutates protocol state or releases protocol capacity early.
- Malformed, duplicate, stale, reordered, and unsolicited frames cannot panic or mutate unrelated state.
- No user decoder, callback, subscriber, transport I/O, or sleep runs while mutable engine state is borrowed.
- The owner copies or moves each effect out of the engine and ends the mutable engine/observer borrow before transport I/O, borrowing the engine again only to apply the identified result.
- No slow observer or subscriber can block protocol progress.
- Request-identity wraparound cannot alias active or quarantined work.
- All boundary, engine, observer, diagnostics, framing, and subscription memory has a documented bound.
- Stream poison, close, and shutdown cannot leave a retained waiter blocked indefinitely.
- A fresh session contains no state from a poisoned session.
- The runtime code implementing this architecture contains no `unsafe`.
- A returned blocking operation handle names a request whose initial transport write succeeded.
- Target-local state changes occur only from exact `AppliedStateEffect` delivery and never depend on an observer.
- Built-in encoding and per-transmission framing allocate nothing; lifecycle allocations remain fixed and admission-bounded.

## Scheduling and async source arbitration

Each request owns an intrinsic `ControlClass`. Typed stops and protocol
cancellation are `Urgent`; that is safety metadata rather than caller QoS.
Camera handles and individual calls may choose a `SubmissionClass` for ordinary
traffic (`Background`, `Normal`, or `User`). The public QoS type has no urgent
variant, and both override forms preserve an intrinsically urgent request.
Within each effective class the owner retains admission order; class selection
only chooses the next eligible queued write and never interrupts in-flight I/O.
Urgency bypasses ordinary admission backlog, not the target's physical pacing
floor. Owner-issued socket cancellation is selected before ordinary ready work
when eligible, but is transmitted only after the shared command-spacing
deadline and itself advances that deadline.

The async actor expresses simultaneous readiness as deterministic,
progress-sensitive phases rather than a randomized race or a history
threshold:

1. Receive is first while it produces a valid non-empty frame batch. Buffered
   ACKs, completions, and replies therefore update protocol state before a
   simultaneously ready control observer sees that state.
2. A receive that makes no protocol progress — idle/no-data, a partial frame,
   a transient fault, an empty UDP datagram discarded under the current overall
   deadline, or a receive whose only frames were discarded as malformed (a whole
   bad datagram, or delimited-but-unclassifiable frames on a stream) — makes the
   next selection poll boundary sources first in the fixed order shutdown,
   cancellation, admission, control, then timer. Discarding an empty datagram
   never starts a fresh deadline; the async UDP adapter also yields cooperatively
   before polling again.
3. A boundary win, or a later valid frame batch when no boundary was ready,
   returns the actor to receive-first.
4. A **fairness ceiling** bounds the *succeeding* arm as well. After a run of
   consecutive receive-first wins (tied to the receive batch limit,
   `frames_per_receive`), one boundary-first turn is forced regardless of
   progress, then the run restarts. This is the case #625's acceptance criterion
   names directly — the boundary channels are always eventually polled — and it
   is what keeps a *babbling* peer (one that returns a valid frame on every
   poll) from winning the left-biased selection forever and starving shutdown,
   cancellation, admission, control, and the timer. When no boundary is queued
   the receive still wins the forced turn, so a genuinely busy transport is
   never stalled; only guaranteed to yield the front periodically. A burst large
   enough to reach the ceiling is adversarial rather than a real camera's reply
   stream, so the settle-first ordering of (1) still holds for real traffic.

This retains the protocol's strict source order for meaningful input while
preventing both an always-idle/always-failing transport **and** an
always-succeeding (babbling) one from starving shutdown and control. Phase
choice depends on the result of the current receive; the only count involved is
the bounded fairness ceiling of (4), never an open-ended receive history.
Transmission effects produced by either phase are driven immediately before the
next selection.

The actor also bounds each transport operation itself, because the
runtime-agnostic async transports hold no timer of their own. Every read is
raced against the session's `read_timeout`; if it elapses the read is treated as
an idle no-data receive (it consumed nothing), which makes the advertised knob
live on the async surface rather than inert. Every write is raced against
`write_timeout`; if it elapses the write is abandoned as a failure — a stream
write that can no longer be confirmed poisons the session, a datagram write
fails only its own request — so a stalled peer can never park the actor and
block `close()`. Finally, a run of immediately-returning no-data reads is paced
by the same escalating, next-wake-clamped pause the transient-fault path uses
(recording no fault and spending no retry budget), so a transport that reports
"no data" without blocking cannot hot-spin the actor.

## Request and motion semantics

Plain commands complete when the owner has protocol-applied them. Typed
inquiries return their decoded response. Targeted operations have a meaningful
physical end state and expose applied plus settled completion. Applied-only
operations represent actuation without a meaningful physical target and expose
applied completion only. Dynamic handles preserve this distinction:
`DynTargetedOperation` has `applied`, `settled`, `cancel`, and `detach`, while
`DynAppliedOperation` has no settled operation.

Use the noun view for ordinary controls and `submit` when a caller needs an
explicit operation lifecycle. `motion().stop_all_motion()`,
`motion().is_moving()`, `motion().is_moving_axes(...)`, and
`motion().wait_until_idle(...)` are the only camera-level motion
safety/observation entry points. A dropped handle is not an automatic STOP;
emergency stopping is an explicit STOP or motion operation.

Broadcast address assignment and interface clear are transport-lifecycle
controls, not camera requests; the serial handshake owns them before the target
registry starts. Socket cancellation is likewise owner-only because only the
owner knows which live operation owns a camera-assigned socket. None of those
three wire primitives is exposed as a generic `request::builtin` command.

## Preserved implementation boundaries

The authoritative built-in semantic ledger remains
`src/command/semantics.rs`; facade inventories are projections, not a second
wire or semantic registry. Profile marker implementations and runtime
discovery facts come from the profile registry. The owner is shared by static,
dynamic, and state/diagnostic projections. These boundaries keep blocking and
async behavior equivalent while allowing their calling conventions to remain
different.

For the preservation inventory and ecosystem list, see
[`architecture_inventory.md`](architecture_inventory.md). For construction and
operational examples, see [`usage_2_0.md`](usage_2_0.md). The critical 1.x/v2
trade-off, size, and reuse analysis is recorded in
[`issue_542_design_review.md`](issue_542_design_review.md).
