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
| `completion_timeouts` | Completion deadlines that expired while an acknowledged command or completion-only raw command awaited completion. |
| `inquiry_timeouts` | Reply deadlines that expired on a sent inquiry. |
| `busy_errors` | Error frames saying the camera cannot accept the request now. |
| `protocol_errors` | Every other error frame, excluding the cancellation reply. |
| `retries_scheduled` | Requests re-queued for another attempt, for any reason. |
| `received_frames` | Valid decoded VISCA response frames received from the camera; compare snapshots around an application heartbeat for positive liveness evidence. |
| `ignored_unmatched_sequenced_replies` | Sequenced replies matching no request. |
| `ignored_malformed_frames` | Delimited frames, and consumed oversized/malformed datagrams rejected before framing, discarded because they did not classify as a valid VISCA response. |
| `dropped_diagnostics` | Events evicted from bounded diagnostic staging or the owner diagnostic ring. |
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
terminal error rather than silently succeeding (#690).

## StateCache

Each `Camera<P>`, `BlockingDynSessionCamera`, and `DynSessionCamera` returns a
cheap read-only `StateCache` view for exactly one target. `StateEntry` is
`#[non_exhaustive]`, so a match
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
an ACK/completion/cancellation result that cannot be correlated is never replayed
blindly. By default only that request fails and the evidence still at stake is
quarantined; callers reconcile its camera effect while the session continues.
The exact phases, quarantine scopes, transient-fault behavior, and
`strict_unconfirmed_poison` alternative are defined once in
[Raw unconfirmed outcomes and strict recovery](architecture_2_0.md#raw-unconfirmed-outcomes-and-strict-recovery).

Raw inquiry replies are unkeyed too. The production Raw adapter admits one live
inquiry per target, so a reply, terminal error/timeout, or retry release/requeue
holds that target's response correlation for the profile ambiguity timeout
before same-target response-bearing successor work can send. The hold does not
ignore every same-target frame: it filters stale unkeyed inquiry reply/data and
socketless-error evidence, while an already-live attributable ACK or completion
(including a uniquely attributable socketless completion) and a named exact
socket error remain eligible for the ordinary resolver. At the exact deadline,
complete input is still processed before the due pass releases a successor;
retained partial evidence follows the [byte-stream ambiguity-expiry
rule](architecture_2_0.md#engine-transition-and-ordering). This bounded
single-flight policy does not claim that a wider raw FIFO can identify duplicate
replies.

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

The default observer attached to a prepared typed inquiry lasts for the larger
of its reply deadline and its admission-to-terminal retry budget. That keeps a
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

## Transient transport faults and session death

Not every transport failure ends a session. A receive that fails without
proving the connection is gone — the classic case is a UDP `recv` reporting
ECONNREFUSED after an ICMP port-unreachable for an earlier datagram — may retry
a sequenced Sony command still waiting for its ACK under that command's bounded
policy. A raw command has no sequence to replay; its complete transient-fault
decision is the architecture guide's
[raw recovery rule](architecture_2_0.md#raw-unconfirmed-outcomes-and-strict-recovery).
When no such raw command is awaiting ACK, a read that proves the connection is
gone (`ConnectionClosed`, or an `Io` failure whose kind is `TimedOut`,
`ConnectionReset`, `ConnectionAborted`, `BrokenPipe`, `UnexpectedEof`, or
`NotConnected`) ends the session. A raw OS `TimedOut` is included because a
connected TCP socket commonly reports it when keepalive has exhausted; it is
not the spelling for an application-owned idle timer. The owner normalizes
that fatal receive closure to
`Error::ConnectionClosed` and retains the original transport error's text in
the closure reason; it does not expose the raw `Io` value as the session-death
verdict or call it stream poison. A failed read consumes nothing and so cannot
desynchronize framing.

A read that reports *no data* is a third case and not a fault at all.
`Error::Timeout`, and the raw `Io` spellings `WouldBlock` and `Interrupted`,
mean an idle read timeout expired with nothing to show for it.
Both owners treat that as "this read produced no frames": the session lives,
framing state is untouched, and no request's retry budget is spent. A transport
with an internal read timeout — the shape `BlockingTransport::recv_into_with_timeout`
documents, and the natural way to write a custom async transport — must return
`Error::Timeout` rather than forwarding `io::ErrorKind::TimedOut`. UDP adapters
additionally discard valid zero-length datagrams
inside the adapter and keep receiving; they never translate a datagram with no
payload into the `Ok(0)` value reserved for stream EOF. A timed blocking
receive retains one overall deadline while discarding such datagrams; an empty
datagram cannot reset or extend that deadline. The async UDP adapter yields
cooperatively after an empty datagram before polling again, so a stream of empty
packets cannot starve owner controls.

The async owner enforces `read_timeout` and `write_timeout` even when a custom
transport has no timer, and both owners pace immediately idle reads. The exact
stream/datagram failure split, pacing, and source-fairness rules live in
[Transport I/O deadlines and idle pacing](architecture_2_0.md#transport-io-deadlines-and-idle-pacing)
and the preceding arbitration section.

A fault that never stops repeating stops being called transient. Consecutive
transient faults, with no successful read between them, escalate their pause
from 10 ms to a 250 ms ceiling, and a read that has failed twelve times in a row
over at least a second ends the session as `ConnectionClosed` with the count and
underlying transport error retained in its reason, rather than retrying against
a dead adapter forever. One successful read, or a five-second gap between
faults, clears the run.

Fixed-format ACK, completion, error, and network-change frames are accepted
only at their exact lengths: three bytes for ACK, completion, or network change,
and four bytes for an error. A known fixed prefix with trailing bytes is
malformed, never `Unknown`. Variable data replies are reserved for socket 0
with more than three bytes; the canonical three-byte `z0 50 FF` completion
remains valid.

UDP has an additional boundary check before this decode: a datagram that does
not fit `recv_buffer_size` is discarded as `Error::ResponseTooLarge`. Its copied
prefix is never treated as a complete frame, even when that prefix would itself
be a valid ACK or completion.

Decoding is classified by transport. On a byte stream, a frame that the framer
has already delimited at its `FF` boundary but the strict decoder cannot
classify is discarded and recorded as `Ignored(MalformedFrame)`; it does not
poison the session. Only a genuine loss of the framing position — for example
an unrecoverable buffer overflow or a boundary-free read beyond the configured
limit — becomes `Error::StreamPoisoned`. On a datagram transport one undecodable
datagram is one bad datagram: nothing else was consumed and the next datagram
frames independently, so it is discarded while the session keeps running.

Writes are classified by transport too. A failed datagram write fails exactly
one request, with its own transport error, and the session continues; because
the session survives, an error value that claims a replacement session is
required is normalized to a plain per-request transport error, and the receive
side stays the authority on session death. A failed stream write poisons the
session: no transport trait in this crate reports how many bytes of a frame
reached the wire, so a partial write must be assumed and the byte-stream
position treated as unknowable. The exact transport cause is carried in the
`StreamPoisoned` reason.

## Silent peers and connection liveness

A long-lived TCP control link can fail in two materially different ways. The
camera or network may close it, or the socket may stay open while the camera
stops answering VISCA. A close surfaces here as `Error::ConnectionClosed`
(`requires_new_session() == true`) with the reason `peer closed connection`,
and — because a dead session retains its original terminal cause (#680) — every
later command, inquiry, cancellation, or control call on that session keeps
reporting the same peer-closure cause rather than a generic channel error. This
is the failure a long-running supervisor must expect during quiet periods.

Silence does **not** produce that verdict. A default built-in inquiry retries
within its bounded policy and ordinarily reports `Error::Timeout` at the
ten-second total retry-budget floor (roughly 10.05 seconds when the first
backoff is included). That error is retryable and
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

The library never sends VISCA on its own — there are no per-camera background
workers (see [`architecture_2_0.md`](architecture_2_0.md)) — so if a camera
enforces an application idle timeout there are two application-side levers:

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
   match session.camera::<PtzOpticsG2>()?.power().state() {
       Ok(_) if session.metrics()?.received_frames > before => {
           /* positive liveness; keep waiting for real work */
       }
       Err(error) if error.requires_new_session() => rebuild(&config)?,
       Err(Error::Timeout) if session.metrics()?.received_frames == before => {
           record_unanswered_heartbeat(); // rebuild when your threshold is met
       }
       Err(error) => return Err(error),
       Ok(_) => record_unanswered_heartbeat(),
   }
   ```

2. **Recover from it with a supervisor loop.** Whether or not a heartbeat is
   used, treat an idle close as a completed session and rebuild, following the
   fresh-session steps below. A supervisor keeps the reusable `SessionConfig`
   and a re-callable transport factory outside the session, classifies each
   failure with `requires_new_session()`, and on `true` opens a fresh session,
   re-queries state, and resumes. `examples/recovery.rs` demonstrates both the
   absent-frame silence decision and positive liveness after the rebuild.

Which cameras enforce an application idle timeout, what that interval is, and
whether a heartbeat prevents the close are firmware-specific and belong to
hardware validation rather than a library guarantee; the idle-disconnect row in
[`hardware_release_checklist.md`](hardware_release_checklist.md) captures that
evidence (including the FIN/RST direction that distinguishes a camera-initiated
close from an intervening network device).

## Fresh-session poison recovery

Treat a closed or poisoned owner as a completed session, not as a queue to
restart in place:

0. Classify the failure with `Error::requires_new_session()`. It returns `true`
   for transport-level session death (`ConnectionClosed` or `StreamPoisoned`)
   and `false` for deliberate `RuntimeShutdown` and the default per-request
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

The current owner policy is intentionally explicit. The first nine rows are
fixed; the remaining four are caller-tunable bounds:

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
| Admitted request lifecycle entries (pending or active, including quarantine) | `SessionConfig::admission_capacity()` (64 by default) | yes — `SessionConfig::with_admission_capacity()` |
| Owner transport-read scratch / largest copied read | see below | yes — `BufferConfig::recv_buffer_size` |
| Single framed response | see below | yes — `BufferConfig::recv_buffer_size` |
| Retained incomplete framing bytes | 8192 by default | yes — `BufferConfig::max_buffer_size` |

Admission capacity is fixed when the session opens and bounds each admitted
boundary plus its pending/active owner lifecycle entry until safe terminal
removal, including ambiguity quarantine. It defaults to 64; callers may choose
any non-zero value below `usize::MAX` with
`SessionConfig::with_admission_capacity()`.

The three transport-buffer rows are set from `TransportConfig::buffer_config`
every time a production session is built, so the first two have no single
number. `OwnerLimits::default()` retains internal 4096/8192 test defaults, but
the production adapter replaces both values before allocating owner buffers.
Reach the public configuration with
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

Every one of these keeps `max_buffer_size` at 8192, which is where the retained
incomplete-byte row's default comes from. A caller that raises
`recv_buffer_size` raises the owner's per-session transport-read scratch
allocation by exactly that amount; raising `max_buffer_size` raises the ceiling
on retained incomplete framing bytes. `recv_buffer_size` is independently the
framer's maximum accepted single-frame size, so
lowering it below a profile's largest reply turns that reply into
`Error::ResponseTooLarge`. For UDP it is also the maximum accepted datagram
size: an over-size datagram is rejected before framing rather than silently
truncated to this limit.

These are implementation policy limits surfaced here so integrations can
budget memory. They are not permission to add per-frame, per-retry,
per-subscriber, or per-handle unbounded allocation.

See [`architecture_2_0.md`](architecture_2_0.md) for owner ordering and
[`allocation_and_validation.md`](allocation_and_validation.md) for release
allocation gates.
