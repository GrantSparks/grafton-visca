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
| Caller-selected lifecycle IDs, retry class, target, or settlement metadata | Owner-derived preparation metadata. Callers select a request, a timeout, and a scheduling class; they do not select protocol identity or queue positions. |
| `runtime::Priority` and the `*_priority` camera methods | `ControlClass` and the `*_with_class` / `set_command_class` surface. See [Submission priority](#submission-priority). |

The built-in command classification remains one closed semantic ledger. A
custom request must declare its class explicitly; wire opcode or response shape
does not infer lifecycle semantics.

## Submission priority

1.x's `runtime::Priority` is gone; `ControlClass` is the 2.0 spelling of the
same four scheduling lanes, and it is a public root export. The mapping is
one-to-one:

| 1.x `runtime::Priority` | 2.0 `ControlClass` |
| --- | --- |
| `Priority::Low` | `ControlClass::Background` |
| `Priority::Normal` | `ControlClass::Normal` |
| `Priority::High` | `ControlClass::User` |
| `Priority::Critical` | `ControlClass::Urgent` |

The levels are ordered identically and the dispatch rule is unchanged: highest
occupied class first, admission order within a class, and the class decides only
which *queued* request is written next. What changed is the vocabulary and the
default. 1.x had no per-request classification, so a handle's priority was the
only signal and everything a handle submitted sat in one lane. In 2.0 every
request already carries a class — ordinary control is `Normal`, drives and
absolute moves are `User`, and the typed stops and `CommandCancel` are `Urgent`
— so an emergency stop preempts queued work with no API call at all, which is
the case most 1.x `Priority::Critical` code existed to serve.

| 1.x call | 2.0 call |
| --- | --- |
| `camera.set_command_priority(Priority::Low)` | `camera.set_command_class(Some(ControlClass::Background))` |
| `camera.command_priority()` | `camera.command_class()` — returns `Option<ControlClass>`, where `None` means "each request's own class" |
| `camera.execute_with_priority(cmd, Priority::Critical)` | `camera.execute_with_class(&cmd, ControlClass::Urgent)` |
| — (no 1.x equivalent) | `camera.inquire_with_class(&inquiry, class)` and `camera.submit_with_class::<K, _>(&operation, class)` |
| `BlockingClient` priority methods | The same names on `blocking::Camera` and `blocking::CameraSession` |
| — (no 1.x equivalent) | `DynSessionCamera::execute_with_class`, `inquire_with_class`, `submit_targeted_with_class`, `submit_applied_with_class`, and `set_command_class` |

Three behavioural differences are worth reading before porting:

- **The handle default never demotes an urgent request.** A handle set to
  `Background` still submits `PanTiltStop`, `ZoomStop`, `FocusStop`, and
  `CommandCancel` as `Urgent`. 1.x had no such rule because it had no
  per-request class. Only an explicit per-submission class
  (`submit_with_class(&ZoomStop, ControlClass::Background)`) can demote a stop,
  and it does so for that one submission.
- **Inquiries are covered.** 1.x kept inquiries at a fixed polling priority; in
  2.0 they share the same four lanes, so a handle demoted to `Background` moves
  its telemetry reads out of the way as well as its commands. Owner-internal
  traffic — settlement polling behind `settled()`, and the observation inquiries
  behind `motion()` — keeps its own built-in class.
- **The default is per handle, not per session.** Cloning an async `Camera`
  copies the current value and then diverges, and two views taken from one
  `Session` are independent. This matches 1.x.

`runtime::testing::Priority` has no 2.0 equivalent, because 2.0's tests do not
need one: `ControlClass` is public in every build configuration, so a test names
the class through the same API an application uses, and the crate's own
lane-ordering tests (`tests/issue_630_submission_class_*.rs`) assert the
resulting dispatch order at the transport boundary rather than reaching into the
scheduler. For deterministic scheduling in downstream tests, use the
`test-utils` transports and executors.

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

Serialization features (`serde`, `schemars`, `ts-rs`) remain opt-in data-shape
features. They do not reopen private modules or create a second semantic
registry. `test-utils` is for deterministic tests, not production construction.

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
