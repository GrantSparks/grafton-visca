# Migrating from 1.x to 2.0

2.0 is a clean break in naming and ownership. The mappings below describe the
supported destination for applications migrating from pre-2.0 vocabulary; the
historical names are not part of the 2.0 contract.

## Construction and feature selection

| 1.x category | 2.0 destination |
| --- | --- |
| `mode-async` | `async`; add `runtime-tokio` or `runtime-smol` when using a built-in runtime. |
| Generic mode/transport/executor `Camera<Mode, P, T, E>` and `AsyncCamera` | `blocking::Session` or async `Session`, then `Camera<P>` for a target view. |
| `BlockingClient` and direct generic camera constructors | `blocking::Connect`, `blocking::CameraConfig`, or `blocking::Session::open`. |
| Legacy `Connect`/`CameraConfig` single-target shortcuts | Keep the convenience path, but use `SessionConfig` when configuration must be cloned, reused, or shared across targets. |
| Direct `Camera::new_*`, `open_*` compatibility constructors | `Connect`/`CameraConfig` for standard transports; `Session::open` for a caller-owned transport. |
| One implicit camera ID or raw camera-ID setters | `CameraId`, `SessionConfig::for_target`, `try_camera_id`, and explicit `camera_for`. |
| `CameraVariant` and root camera-number constants | `CameraId` plus a validated `ProfileSpec`/compile-time profile. |
| `RuntimeHandle` and private scheduler/runtime modules | `TokioRuntime`, `SmolRuntime`, or a coherent public `Executor`; never construct the owner directly. |

`SessionConfig` accepts only individual VISCA IDs 1–7. Broadcast, duplicate
registration, an empty registry, and unsupported profile/transport pairs are
errors before I/O. `camera()` is only for a sole target; multi-target code must
select `camera_for(target)`.

## Static and dynamic nouns

| 1.x category | 2.0 destination |
| --- | --- |
| Root control-trait calls that duplicate noun views | `camera.power()`, `zoom()`, `system()`, `pan_tilt()`, `focus()`, `exposure()`, `white_balance()`, `image()`, `presets()`, `tally()`, `nd_filter()`, `motion_sync()`, `menu()`, and `advanced()`. |
| Root `is_moving`, `wait_until_idle`, and `stop_all_motion` aliases | `camera.motion().is_moving(...)`, `wait_until_idle(...)`, and `stop_all_motion()`. |
| Noun-specific idle/wait aliases | The separate `motion()` safety/observation view. |
| `DynCameraControl` | `DynSessionCameraControl` plus `DynSessionCameraNouns`. |
| `DynPanTiltControl`, `DynZoomControl`, `DynFocusControl`, `DynPresetsControl`, and `DynMotionControl` | `DynPanTilt`, `DynZoom`, `DynFocus`, `DynPresets`, and `DynMotion`. The final dynamic surface also has `DynPower`, `DynSystem`, `DynExposure`, `DynWhiteBalance`, `DynImage`, `DynTally`, `DynNdFilter`, `DynMotionSync`, `DynMenu`, and `DynAdvanced`. |
| `IntoDynCamera` and wrapper-specific dynamic constructors | `DynSessionCamera::from_session` or `from_session_target`. |
| Metadata-only optional control fallback | Static `Has*` marker gates, or dynamic `supports_typed(...)` followed by the matching `Dyn*` noun. |
| Duplicate `NdFilterInquiry` accessor vocabulary | `nd_filter().position()` only. |
| Separate focus `lock()`/`unlock()` twins | One parameterized `focus().set_lock(FocusLock)`. |

Dynamic views remain async and object-safe. They erase profile/request types but
share the static session's owner, timeout, pacing, cancellation, and state
cache. There is no dynamic policy layer that can bypass static preparation.

## Requests, inquiries, and operations

| 1.x category | 2.0 destination |
| --- | --- |
| Concrete async `_op` methods (`pan_tilt_*_op`, `set_*_op`, `preset_recall_op`, and similar) | Construct the typed request and call `submit`; use the returned targeted/applied-only handle. |
| `start_*`, `*_and_wait`, `_result`, and `*_op` twins | One noun method for ordinary completion, or one `submit` call for lifecycle control. No aliases or result twins. |
| `await_completion` | `applied`; use `settled` only on a targeted operation. |
| `InFlightDyn`/legacy dynamic operation wrappers | `DynTargetedOperation`, `DynAppliedOperation`, and `DynCancellation`. Applied-only handles have no settled operation. |
| Dropping an operation handle | Unchanged from 1.x: drop is `detach` and never stops hardware. See [Drop never stops hardware](#drop-never-stops-hardware) for the scoped stop-on-exit pattern. |
| Raw `command::RawInquiryPayload`/untyped response assumptions | `raw::Plain`, `raw::Inquiry`, `raw::Targeted`, or `raw::AppliedOnly`, with an explicit response parser/spec. |
| `ViscaCommand` response-associated-type extensions | The typed `Request`/`Inquiry`/`OperationCommand` contract and `ResponseParser` for custom decoding. |
| Plain requests submitted as operations or operations without affected axes | Match the request class exactly: `execute` for plain, `inquire` for inquiry, and `submit` for a typed operation with non-empty affected axes. |
| Caller-selected lifecycle IDs, priority, retry class, target, or settlement metadata | Owner-derived preparation metadata. Callers select a request and timeout, not protocol identity or scheduler policy. |

The built-in command classification remains one closed semantic ledger. A
custom request must declare its class explicitly; wire opcode or response shape
does not infer lifecycle semantics.

## Drop never stops hardware

Dropping an operation handle is exactly `detach`. It relinquishes the observer
and nothing else: the owner keeps the protocol lifecycle, never reads a dropped
handle as cancellation, and no STOP reaches the camera. An early `?`, a panic
unwinding past a live handle, or a forgotten binding therefore leaves physical
movement running until something else ends it.

**This is not a 2.0 change.** 1.x behaved identically — `src/camera/inflight.rs`
on the 1.x line documents "drop == detach ... Dropping never stops the command",
pinned there by `tests/issue_539_blocking_handle_test.rs`. Nothing to migrate;
it is documented here because the consequence is physical and easy to assume
otherwise.

To end motion, submit a stop: `camera.pan_tilt().stop()`, `camera.zoom().stop()`,
`camera.focus().stop()`, or `camera.motion().stop_all_motion()`. `cancel` records
protocol cancellation and does not by itself prove motion ended.

### Scoped stop-on-exit guard

To bound movement by a scope rather than by an explicit call on every path,
write a guard whose own `Drop` submits the typed STOP. This is caller-owned
code — the library deliberately offers no such type, so the policy, the axes,
and the failure handling stay yours:

```rust,ignore
struct StopPanTiltOnExit<'a, 'session> {
    camera: &'a grafton_visca::blocking::Camera<'session, PtzOpticsG2>,
}

impl Drop for StopPanTiltOnExit<'_, '_> {
    fn drop(&mut self) {
        // `Drop` cannot report a failure and may run while unwinding, so the
        // stop is best effort — as in any scope guard.
        if let Ok(stop) = self.camera.pan_tilt().stop() {
            let _ = stop.applied();
        }
    }
}

// Hold the guard for the region that must stay bounded.
{
    let _stop_on_exit = StopPanTiltOnExit { camera: &camera };
    let drive = camera.pan_tilt().move_direction(direction, pan, tilt)?;
    do_fallible_work()?; // an early `?` here still stops pan/tilt
    drive.applied()?;
}
```

`Drop` cannot await, so the async form is a wrapper rather than a guard type:
run the fallible region, stop unconditionally, then propagate its result.

```rust,ignore
async fn bounded_drive(camera: &grafton_visca::Camera<PtzOpticsG2>) -> Result<(), Error> {
    let result = drive_up(camera).await;
    if let Ok(stop) = camera.pan_tilt().stop().await {
        let _ = stop.applied().await;
    }
    result
}
```

Both forms are demonstrated end to end in `examples/operation_handles.rs` and
`examples/operation_handles_async.rs`.

## Values, optional features, and internal modules

| 1.x category | 2.0 destination |
| --- | --- |
| Raw normalized floats, root normalized helpers, `ZoomPosition` float conversions | Checked public values such as `UnitInterval::new/try_from`, `ZoomPosition`, and profile-aware noun methods. |
| Model-aware constructors that embed a profile in a value | Plain checked values plus the profile-gated camera noun; profile validation belongs at preparation. |
| Generic optional accessors or unsupported PTZOptics/Sony controls | Compile-time `Has*` gates; dynamic callers inspect capability support. Unsupported controls are not exposed through metadata fallback. |
| Direct PTZOptics ND filter, Motion Sync, variable-speed, Sony color-temperature, or legacy quality controls | The matching supported noun only when its profile marker permits it; otherwise use a raw extension deliberately. |
| Public `camera::*`, `command::*`, `protocol::*`, response, cache, runtime, or transport implementation modules | Supported root/module exports and owner methods. Implementation submodules are not extension points. |
| `diagnostics::Diagnostics`/probe-style compatibility API | `Session::metrics`, async `subscribe_diagnostics`, and blocking `drain_diagnostics`. |
| Legacy mutable `cache::StateCache` | Owner-backed read-only root `StateCache`; use `target()` and `value(StateKey)`. |
| `Camera::set_timeout_config` / `timeout_config` | `Session::set_tuning` / `tuning` (and the same pair on `CameraSession`), taking an `OperationalTuning` instead of a `TimeoutConfig`. See [Reconfiguring timeouts at runtime](#reconfiguring-timeouts-at-runtime) — the scope is narrower than 1.2.0's. |

Serialization features (`serde`, `schemars`, `ts-rs`) remain opt-in data-shape
features. They do not reopen private modules or create a second semantic
registry. `test-utils` is for deterministic tests, not production construction.

### Reconfiguring timeouts at runtime

1.2.0's `Camera::set_timeout_config` took a `TimeoutConfig`; 2.0's
`Session::set_tuning` takes an `OperationalTuning`, which is the same knob
lowered onto the owner's own vocabulary. The mapping is direct:

| 1.2.0 `TimeoutConfig` field | 2.0 `OperationalTuning` builder |
| --- | --- |
| `ack_timeout` | `ack_timeout` |
| `movement_timeout`, `preset_timeout`, `long_timeout`, `default_timeout` | `completion_timeout` (the owner has one completion budget; use the largest of the 1.x values) and `settlement_timeout` for the physical-settling budget |
| `quick_timeout`, `network_timeout` | `inquiry_timeout` |
| `RetryConfig::max_retries`, `base_retry_delay`, `max_retry_duration` | `retry_limit` and `retry_timing` |

Two differences matter in practice.

**The update is a whole replacement, not a merge.** Any field left unset returns
to the profile default rather than keeping a value an earlier call installed.
Build the complete `OperationalTuning` each time.

**In-flight work is not re-timed.** 1.2.0 recomputed deadlines on every
housekeeping pass, so widening `ack_timeout` also rescued a command that was
already waiting for its acknowledgement. 2.0 stamps a request's deadlines once,
at preparation, and the engine derives its absolute phase deadlines from that
stamp, so `set_tuning` governs **every request prepared after it** and leaves a
request already admitted on the deadlines it was admitted with. The owner's
session-wide pacing floor and per-target socket capacity *are* applied at once,
so work still queued behind pacing is released under the new values.

If a command that is already running must move onto a widened deadline, cancel
it and resubmit:

```rust,ignore
session.set_tuning(OperationalTuning::new().ack_timeout(Duration::from_secs(2)))?;
// `operation` was admitted before the update and keeps its old deadline.
let _ = operation.cancel()?.outcome(Duration::from_secs(1));
let operation = camera.submit::<AppliedOnly, _>(&command)?;
```

Runtime updates are validated on exactly the grounds `SessionConfig::with_tuning`
validates on, so tuning that would have been refused at construction is refused
here too and leaves the live configuration untouched.

## Recovery changes

Do not reconnect by reusing a poisoned owner, old view, subscriber, or operation
handle. Keep `SessionConfig`, open a fresh session, select fresh views, and
expect every cache entry to start `Unknown`. Re-query camera state explicitly;
the owner never automatically resubmits commands. Old handles must report the
old owner's terminal/closed outcome and cannot be rebound to the new owner.

Classify the terminal condition with `Error::requires_new_session()` rather than
matching `ErrorKind::IoClosed` or individual variants. Transport close, explicit
shutdown, and poison are deliberately distinct errors that share one kind, so
the kind alone cannot tell a field disconnect apart from a shutdown this
application requested:

| Terminal condition | Error | `requires_new_session()` |
| --- | --- | --- |
| The peer closed the connection | `ConnectionClosed` | `true` |
| The stream position became unknowable | `StreamPoisoned` | `true` |
| The owner's transport or channel is gone | `NoTransport`, `TransportChannelClosed`, … | `true` |
| The application shut the session down | `RuntimeShutdown` | `false` |

`true` is positive proof that the session is finished; `false` only means the
error alone does not prove it. Ordinary per-request failures — timeouts, busy
states, protocol and parameter errors — are `false`, and so is a raw `Io`
failure, because a datagram write failure is isolated to its own transmission
and a stream failure reaches the caller as `StreamPoisoned`.

```rust,ignore
match camera.execute(&command) {
    Ok(()) => {}
    Err(error) if error.requires_new_session() => {
        // Drop the old session and views, then rebuild from the retained
        // configuration. Re-query state before applying anything new.
        let session = Session::open(new_transport()?, config.clone())?;
        // ...
    }
    Err(error) => return Err(error),
}
```

For the complete construction, transport, target, tuning, noun, and lifecycle
examples, see [`usage_2_0.md`](usage_2_0.md). For bounded state and diagnostic
behavior, see [`observability_and_recovery.md`](observability_and_recovery.md).
