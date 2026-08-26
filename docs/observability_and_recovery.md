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

## StateCache

Each `Camera<P>` and `DynSessionCamera` returns a cheap read-only `StateCache`
view for exactly one target:

```rust,ignore
let cache = camera.state_cache();
assert_eq!(cache.target(), camera.target());
match cache.value(grafton_visca::StateKey::ImageFreeze) {
    grafton_visca::StateEntry::Unknown => { /* no exact applied value yet */ }
    grafton_visca::StateEntry::Set(value) => println!("{} values", value.len()),
    grafton_visca::StateEntry::Clear(value) => println!("cleared: {:?}", value.as_slice()),
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

## Fresh-session poison recovery

Treat a closed or poisoned owner as a completed session, not as a queue to
restart in place:

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

The current owner policy is intentionally explicit:

| Resource | Bound |
| --- | ---: |
| Registered targets | 7 |
| State keys per target | 64 |
| Scalars per state value | 4 |
| Diagnostic history | 128 events |
| Diagnostic subscribers | 4 |
| Events per diagnostic subscriber | 128 |
| Applied-state subscribers | 16 |
| Events per applied subscriber | 64 |
| Frames per receive batch | 64 |
| Received payload bytes | 4096 |
| Reusable framing bytes | 8192 |

These are implementation policy limits surfaced here so integrations can
budget memory. They are not permission to add per-frame, per-retry,
per-subscriber, or per-handle unbounded allocation.

See [`architecture_2_0.md`](architecture_2_0.md) for owner ordering and
[`allocation_and_validation.md`](allocation_and_validation.md) for release
allocation gates.
