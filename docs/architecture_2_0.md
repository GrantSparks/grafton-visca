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
* `shutdown` is an idempotent request to the one owner. `close` requests the
  same shutdown while consuming the session; neither API is a second protocol
  authority or an implicit command resubmission mechanism.

There are no public callbacks, user-supplied lifecycle IDs, unbounded queues,
or per-camera background workers. Dropping a camera view does not stop a
session or another view. An operation handle must be explicitly cancelled or
detached according to its documented lifecycle.

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

Standard TCP, UDP, and Sony-encapsulated multi-target configurations are
rejected before socket creation. Heterogeneous profile envelopes are accepted
only where the transport can safely carry them: compatible raw profiles over a
custom raw serial transport. A custom transport must still declare its
stream/datagram semantics and transport configuration; custom does not bypass
profile or framing validation.

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

This ordering is what permits a detached observer or a dropped subscription to
miss an event without losing an already-applied state update.

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
`motion().is_moving(...)`, and `motion().wait_until_idle(...)` are the only
camera-level motion safety/observation entry points. A dropped handle is not an
automatic STOP; emergency stopping is an explicit STOP or motion operation.

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
operational examples, see [`usage_2_0.md`](usage_2_0.md).
