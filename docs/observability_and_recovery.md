# Observability, state, and recovery

The public observability surface is deliberately smaller than the private
owner trace. It is stable, bounded, and safe to consume from an application
without granting control over the protocol engine.

## Metrics

`MetricsSnapshot` is a copy of scalar owner state. Its stable fields are:

| Field | Meaning |
| --- | --- |
| `admitted` | Requests admitted into authoritative engine state. |
| `admission_rejected` | Requests rejected before admission. |
| `writes` | Transport writes attempted. |
| `write_failures` | Writes that returned an error. |
| `terminal` | Requests reaching a terminal outcome. |
| `cancellations` | Cancellation requests observed by the owner. |
| `cache_updates` | Exact applied-state effects committed to a target cache. |
| `ack_timeouts` | Acknowledgement deadlines that expired on a sent command. |
| `completion_timeouts` | Completion deadlines that expired on an acknowledged command. |
| `inquiry_timeouts` | Reply deadlines that expired on a sent inquiry. |
| `busy_errors` | Error frames saying the camera cannot accept the request now. |
| `protocol_errors` | Every other error frame, excluding the cancellation reply. |
| `retries_scheduled` | Requests re-queued for another attempt, for any reason. |
| `ignored_unmatched_sequenced_replies` | Sequenced replies matching no request. |
| `dropped_diagnostics` | Events evicted from the owner diagnostic ring. |
| `dropped_diagnostic_events` | Events dropped because a subscriber queue was full. |
| `dropped_observer_events` | Completion-observer events dropped after receiver loss. |
| `dropped_applied_events` | Applied-state events dropped by full subscriber queues. |
| `dropped_boundary_work` | Boundary messages discarded during owner termination. |
| `active` | Admitted, non-terminal requests currently retained. |
| `pending` | Requests staged at the owner boundary but not admitted. |
| `session` | `Running`, `Closed`, `Shutdown`, or `Poisoned`. |

The counter fields saturate at `u64::MAX`; queue and active counts remain
bounded by owner policy. `metrics()` is a separate bounded control request and
does not clone the diagnostic ring, subscriber queues, wire buffers, or state
registry.

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

Async sessions expose `subscribe_diagnostics(capacity)` and
`DiagnosticSubscription::{try_recv,recv}`. Blocking sessions expose
`drain_diagnostics()`. A subscription capacity must be `1..=128`, and one owner
retains at most four live subscriptions. The owner ring retains at most 128
events. Blocking drains return at most that ring bound; another drain after an
empty drain is empty until a new event is recorded.

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
unbounded channel.

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
also preserved. Dropping a view or calling `shutdown()` alone is not a
transport-release barrier.

## StateCache

Each `Camera<P>` and `DynSessionCamera` returns a cheap read-only `StateCache`
view for exactly one target. `StateEntry` is `#[non_exhaustive]`, so a match
over it needs a wildcard arm:

```rust,ignore
let cache = camera.state_cache();
assert_eq!(cache.target(), camera.target());
match cache.value(grafton_visca::StateKey::ImageFreeze) {
    grafton_visca::StateEntry::Unknown => { /* no exact applied value yet */ }
    grafton_visca::StateEntry::Set(value) => println!("{} values", value.len()),
    grafton_visca::StateEntry::Clear(value) => println!("cleared: {:?}", value.as_slice()),
    _ => {}
}
```

`StateValue` stores at most four inline `i64` values. The owner registry has
seven target slots and at most 64 state keys per target. A view contains one
target index and a shared registry reference; cloning a view does not create a
second map, worker, or queue.

The cache changes only after an exact `AppliedStateEffect`. The owner commits
that effect before notifying applied-state or diagnostic observers, and the
commit does not depend on an observer remaining attached. Preparing a command,
an ACK alone, a failed write, a timeout, or a pre-ACK error does not change the
cache. A fresh session starts every target at `Unknown`; target 1 and target 2
views never read or mutate each other's entries.

## What is retried, and for how long

Retry requires evidence that replay is safe. Every retry class except
`RetryClass::Never` may retry a camera reporting a full command buffer (`0x03`)
or no free socket (`0x05`), because that response conclusively rejected the
attempt. `0x41` (`CommandNotExecutable`) is the one answer whose retryability
depends on the class: it is transient for movement and preset work, where the
camera is reporting a state that passes, and terminal everywhere else, where
it is the camera's verdict on the command.

For Sony-encapsulated traffic, a lost ACK or post-ACK completion timeout may be
retried with the same sequence number, preserving the logical request's
identity. Raw VISCA has no such key. After a raw command was successfully sent,
an ACK timeout, completion timeout, unresolved cancellation, receive fault
while awaiting ACK, or active retry-budget expiry while an attempt is in
`Sending`, `AwaitingAck`, or `Executing` leaves both its physical outcome and
any later reply ownership uncertain. The engine never replays it (that would
risk a duplicate relative move or preset). By default (issue #671) it fails only
that one command with `Error::UnsequencedCommandUnconfirmed` — a per-request
outcome the session survives — and holds the correlation still at stake (its
owned socket, or its place as the sole unacknowledged command) quarantined until
the ambiguity deadline, so a late ACK or completion is ignored rather than bound
to a later command. The session and every unrelated request keep running; the
caller reconciles that one command's camera effect rather than replacing the
session. Deployments that would rather hard-fail an entire session than risk a
subtle correlation error can opt into `OperationalTuning::strict_unconfirmed_poison`,
which restores the whole-session poison (surfaced as `Error::StreamPoisoned`,
which requires a replacement session). This rule deliberately covers
non-idempotent relative motion and presets rather than asking a retry class to
guess whether a particular payload is harmless.

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

`OperationalTuning::retry_limit` sets that base, which defaults to 3.
`RetryClass::Never` overrides the table with zero retries — one attempt — for
every timeout category.

Two bounds stop a request retrying: the count above, and one wall-clock budget
counted from admission. That same budget remains active through every later
noncancelled attempt phase — backoff, ready, send, ACK, execution, and reply —
so an active attempt cannot silently extend the total. A cancellation or
per-request unconfirmed quarantine is separate, governed by its own ambiguity
deadline, and is never shortened by budget expiry. That wall-clock budget is
the largest of ten seconds, twice the request's own governing deadline
(its completion deadline for a command, its reply deadline for an inquiry —
doubling it always leaves room for one further full-length attempt), and the
profile's busy timeout. Whichever bound is reached first produces the terminal
error, and a request that runs out of wall-clock time reports the error that
caused its last retry rather than an incidental later timeout. If that expiry
catches a successfully sent raw command in an ambiguous phase, the per-request
unsequenced rule above governs it: the one command fails
`UnsequencedCommandUnconfirmed` and quarantines its correlation (or, under the
strict opt-in, poisons the session) rather than finishing with the retained
last error.

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

## Transient transport faults and session death

Not every transport failure ends a session. A receive that fails without
proving the connection is gone — the classic case is a UDP `recv` reporting
ECONNREFUSED after an ICMP port-unreachable for an earlier datagram — may retry
a sequenced Sony command still waiting for its ACK under that command's bounded
policy. A raw command waiting for its ACK has no sequence to replay, but a
transient receive fault consumes nothing and cannot desynchronize raw framing,
so by default (issue #671) the owner does not terminalize it on the fault at
all: it is left to ride to its own ACK deadline, where — if no ACK arrives — it
fails per-request and quarantines its slot rather than poisoning the session.
The strict `strict_unconfirmed_poison` opt-in instead ends the whole session
(surfaced as `StreamPoisoned`) on such a fault. Neither path ever replays the
command.
When no such raw command is awaiting ACK, a read that proves the connection is
gone (`ConnectionClosed`, or an `Io` failure whose kind is `ConnectionReset`,
`ConnectionAborted`, `BrokenPipe`, `UnexpectedEof`, or `NotConnected`) ends the
session, and it ends it as a close: a failed read consumes nothing and so
cannot desynchronize framing.

A read that reports *no data* is a third case and not a fault at all.
`Error::Timeout`, and the raw `Io` spellings `TimedOut`, `WouldBlock` and
`Interrupted`, mean an idle read timeout expired with nothing to show for it.
Both owners treat that as "this read produced no frames": the session lives,
framing state is untouched, and no request's retry budget is spent. A transport
with an internal read timeout — the shape `BlockingTransport::recv_into_with_timeout`
documents, and the natural way to write a custom async transport — therefore
costs nothing. UDP adapters additionally discard valid zero-length datagrams
inside the adapter and keep receiving; they never translate a datagram with no
payload into the `Ok(0)` value reserved for stream EOF. A timed blocking
receive retains one overall deadline while discarding such datagrams; an empty
datagram cannot reset or extend that deadline. The async UDP adapter yields
cooperatively after an empty datagram before polling again, so a stream of empty
packets cannot starve owner controls.

A fault that never stops repeating stops being called transient. Consecutive
transient faults, with no successful read between them, escalate their pause
from 10 ms to a 250 ms ceiling, and a read that has failed twelve times in a row
over at least a second ends the session with the underlying transport error
rather than retrying against a dead adapter forever. One successful read, or a
five-second gap between faults, clears the run.

Fixed-format ACK, completion, error, and network-change frames are accepted
only at their exact lengths: three bytes for ACK, completion, or network change,
and four bytes for an error. A known fixed prefix with trailing bytes is
malformed, never `Unknown`. Variable data replies are reserved for socket 0
with more than three bytes; the canonical three-byte `z0 50 FF` completion
remains valid.

Decoding is classified by transport. On a byte stream a decode failure means the
stream position is unknowable, so the session is poisoned. On a datagram
transport one undecodable datagram is one bad datagram: nothing else was
consumed and the next datagram frames independently, so it is discarded and
recorded as `Ignored(MalformedFrame)` while the session keeps running.

Writes are classified by transport too. A failed datagram write fails exactly
one request, with its own transport error, and the session continues; because
the session survives, an error value that claims a replacement session is
required is normalized to a plain per-request transport error, and the receive
side stays the authority on session death. A failed stream write poisons the
session: no transport trait in this crate reports how many bytes of a frame
reached the wire, so a partial write must be assumed and the byte-stream
position treated as unknowable. The exact transport cause is carried in the
`StreamPoisoned` reason.

## Fresh-session poison recovery

Treat a closed or poisoned owner as a completed session, not as a queue to
restart in place:

0. Classify the failure with `Error::requires_new_session()`. It returns `true`
   for transport-level session death (`ConnectionClosed`, `StreamPoisoned`, and
   the transport/channel-unavailable errors) and `false` for the deliberate
   `RuntimeShutdown`, which all share `ErrorKind::IoClosed`. Do not match the
   kind or individual variants to make this decision. Note that
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

The current owner policy is intentionally explicit. The first nine rows are
fixed; the last two are the two buffer sizes a caller can tune, and their
values come from the transport's `BufferConfig`, not from an owner constant:

| Resource | Bound | Caller-tunable |
| --- | ---: | --- |
| Registered targets | 7 | no |
| State keys per target | 64 | no |
| Scalars per state value | 4 | no |
| Diagnostic history | 128 events | no |
| Diagnostic subscribers | 4 | no |
| Events per diagnostic subscriber | 128 | no |
| Applied-state subscribers | 16 | no |
| Events per applied subscriber | 64 | no |
| Frames per receive batch | 64 | no |
| Received payload bytes | see below | yes — `BufferConfig::recv_buffer_size` |
| Reusable framing bytes | 8192 | yes — `BufferConfig::max_buffer_size` |

The two tunable rows are set from `TransportConfig::buffer_config` every time
a session is built, so the receive row has no single number. Reach them with
`CameraConfig::<P>::transport_config(TransportConfig { buffer_config, .. })`
for a standard transport, with the blocking `NetTransportBuilder`'s
`recv_buffer_size` / `max_buffer_size` methods, or from a caller-owned
transport's `HasTransportConfig::transport_config()`. The per-transport
defaults are:

| Buffer profile | `recv_buffer_size` | Selected by |
| --- | ---: | --- |
| `BufferConfig::default()` | 128 | a caller-owned transport that does not override it |
| `BufferConfig::for_udp()` | 1024 | the built-in UDP transports |
| `BufferConfig::for_sony_ip()` | 512 | `NetTransportBuilder::sony_ip_buffers()` |
| `BufferConfig::for_raw_ip()` | 256 | the built-in TCP transports |
| `BufferConfig::for_serial()` | 256 | the built-in serial transports |

Every one of these keeps `max_buffer_size` at 8192, which is where the framing
row's number comes from. A caller that raises `recv_buffer_size` raises the
owner's per-session receive allocation by exactly that amount; raising
`max_buffer_size` raises the ceiling on retained incomplete framing bytes.
`recv_buffer_size` is also the framer's maximum accepted frame size, so
lowering it below a profile's largest reply turns that reply into
`Error::ResponseTooLarge`.

These are implementation policy limits surfaced here so integrations can
budget memory. They are not permission to add per-frame, per-retry,
per-subscriber, or per-handle unbounded allocation.

See [`architecture_2_0.md`](architecture_2_0.md) for owner ordering and
[`allocation_and_validation.md`](allocation_and_validation.md) for release
allocation gates.
