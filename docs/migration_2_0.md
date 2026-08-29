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
| 1.2.0's deprecation of the blocking `CameraSession` methods, which pointed at `BlockingClient` | `blocking::CameraSession<P>` again — see [The `BlockingClient` inversion](#the-blockingclient-inversion) below. |
| Legacy `Connect`/`CameraConfig` single-target shortcuts | Keep the convenience path, but use `SessionConfig` when configuration must be cloned, reused, or shared across targets. |
| Direct `Camera::new_*`, `open_*` compatibility constructors | `Connect`/`CameraConfig` for standard transports; `Session::open` for a caller-owned transport. |
| One implicit camera ID or raw camera-ID setters | `CameraId`, `SessionConfig::for_target`, `try_camera_id`, and explicit `camera_for`. |
| `CameraVariant` and root camera-number constants | `CameraId` plus a validated `ProfileSpec`/compile-time profile. |
| `RuntimeHandle` and private scheduler/runtime modules | `TokioRuntime`, `SmolRuntime`, or a coherent public `Executor`; never construct the owner directly. |

`SessionConfig` accepts only individual VISCA IDs 1–7. Broadcast, duplicate
registration, an empty registry, and unsupported profile/transport pairs are
errors before I/O. `camera()` is only for a sole target; multi-target code must
select `camera_for(target)`.

### The `BlockingClient` inversion

Read this if you acted on the 1.2.0 deprecation warnings.

1.2.0 deprecated the blocking-only `CameraSession` methods and told callers to
use `BlockingClient` instead: at the time `CameraSession::new` was gated behind
`mode-async`, so no blocking caller could construct one, and `BlockingClient`
was the single blocking handle.

2.0 reverses both halves of that. `BlockingClient` does not exist, and
`CameraSession` is the promoted name for the single-camera blocking handle:
`blocking::CameraSession<P>` owns its session and hands out the `P` camera view
with the profile bound at compile time. A caller who obeyed the 1.2.0 warning
migrated *away from* the name 2.0 kept.

Nothing about the deprecation is salvageable as a mechanical rename — the
constructors, the ownership model, and the noun surface all changed — so
migrate from wherever you are now:

* `BlockingClient` for one camera → `blocking::Connect::open_tcp_camera::<P>`
  or `open_udp_camera::<P>` (configured form:
  `blocking::CameraConfig::<P>::open_camera`), and, under `transport-serial`,
  `blocking::Connect::open_serial_camera::<P>` (configured form:
  `blocking::CameraConfig::<P>::open_serial_camera`). All return
  `blocking::CameraSession<P>`; `session.camera()` is the noun view and takes
  no turbofish.
* `BlockingClient` for several cameras → `blocking::Connect::open_tcp` /
  `open_udp`, which return `blocking::Session`, then
  `session.camera_for::<P>(target)` per target.
* A caller-owned transport → `blocking::Session::open(transport, config)`, or
  `blocking::CameraSession::open(transport, &CameraConfig::<P>::new())` for the
  single-camera bind.
* 1.x per-handle state on `BlockingClient` is not per-handle in 2.0. Camera ID
  is a registered target on `SessionConfig`, and timeout overrides are supplied
  to `CameraConfig::with_tuning`.

The 1.2.0 `CameraSession` **async** surface was never deprecated and maps to
the async `Session` / `CameraSession<P>` rows above.

## Static and dynamic nouns

| 1.x category | 2.0 destination |
| --- | --- |
| Root control-trait calls that duplicate noun views | `camera.power()`, `zoom()`, `system()`, `pan_tilt()`, `focus()`, `exposure()`, `white_balance()`, `image()`, `presets()`, `tally()`, `nd_filter()`, `motion_sync()`, `menu()`, and `advanced()`. |
| Root `is_moving`, `wait_until_idle`, and `stop_all_motion` aliases | `camera.motion().is_moving()` (no argument, samples `AffectedAxes::MOVEMENT`), `is_moving_axes(MotionQuery)` for an explicit axis set, `wait_until_idle(IdleWait)`, and `stop_all_motion()`. |
| Noun-specific idle/wait aliases | The separate `motion()` safety/observation view. |
| `AffectedAxes::ALL` as "everything that moves" | `AffectedAxes::MOVEMENT` — see [`AffectedAxes::ALL` changed meaning](#affectedaxesall-changed-meaning) below. `ALL` now also selects iris and ND filter. |
| `DynCameraControl` | `DynSessionCameraControl` plus `DynSessionCameraNouns`. |
| `DynPanTiltControl`, `DynZoomControl`, `DynFocusControl`, `DynPresetsControl`, and `DynMotionControl` | `DynPanTilt`, `DynZoom`, `DynFocus`, `DynPresets`, and `DynMotion`. The final dynamic surface also has `DynPower`, `DynSystem`, `DynExposure`, `DynWhiteBalance`, `DynImage`, `DynTally`, `DynNdFilter`, `DynMotionSync`, `DynMenu`, and `DynAdvanced`. |
| `IntoDynCamera` and wrapper-specific dynamic constructors | `DynSessionCamera::from_session` or `from_session_target`. |
| Metadata-only optional control fallback | Static `Has*` marker gates, or dynamic `supports_typed(...)` followed by the matching `Dyn*` noun. |
| Duplicate `NdFilterInquiry` accessor vocabulary | `nd_filter().position()` only. |
| Separate focus `lock()`/`unlock()` twins | One parameterized `focus().set_lock(FocusLock)`. |

Dynamic views remain async and object-safe. They erase profile/request types but
share the static session's owner, timeout, pacing, cancellation, and state
cache. There is no dynamic policy layer that can bypass static preparation.

### `AffectedAxes::ALL` changed meaning

This is the one motion change that ports cleanly and then fails on the device,
so port it deliberately.

In 1.x `AffectedAxes::ALL` was the three mechanical movement axes: pan/tilt,
zoom, and focus. In 2.0 `AffectedAxes` has five axes — pan/tilt, zoom, focus,
iris, and ND filter — and `ALL` means all five.

A motion observation only queries the axes it is given, and preparation
requires the profile to declare a position inquiry for every selected axis.
`ALL` therefore now demands iris **and** ND-filter position inquiries.
`SonyFR7` is the only built-in profile that declares both, so
`motion().is_moving_axes(MotionQuery::new(AffectedAxes::ALL))`,
`wait_until_idle(IdleWait::new(AffectedAxes::ALL, ..))`, and any operation
declaring `ALL` fail on eight of the nine built-in profiles with
`Error::FeatureNotSupported` before a frame is sent. The port compiles; it just
never runs.

Use `AffectedAxes::MOVEMENT` for "wait for everything that moves". It is
exactly 1.x's three axes and is what `MotionQuery::default()`,
`IdleWait::default()`, and the named `IdleWait` presets already select. Reserve
`ALL` for a profile you have checked declares iris and ND-filter position
inquiries.

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
| `runtime::Priority` and the `*_priority` camera methods | `SubmissionClass` and the `*_with_submission_class` / `set_submission_class` surface. `ControlClass::Urgent` is now intrinsic safety metadata, not caller QoS. See [Submission priority](#submission-priority). |

The built-in command classification remains one closed semantic ledger. A
custom request must declare its class explicitly; wire opcode or response shape
does not infer lifecycle semantics.

## Submission priority

1.x's `runtime::Priority` is gone. 2.0 separates caller-selected ordinary-work
QoS (`SubmissionClass`) from a request's intrinsic classification
(`ControlClass`). That separation prevents submission APIs from manufacturing
or demoting the urgent lane used by stops and protocol cancellation.

| 1.x `runtime::Priority` | 2.0 submission choice |
| --- | --- |
| `Priority::Low` | `SubmissionClass::Background` |
| `Priority::Normal` | `SubmissionClass::Normal` |
| `Priority::High` | `SubmissionClass::User` |
| `Priority::Critical` | No general submission override. Typed stops and owner-issued protocol cancellation are intrinsically `ControlClass::Urgent`; ordinary traffic may be raised only to `SubmissionClass::User`. |

The dispatch rule remains highest occupied class first and admission order
within a class; a class decides only which *queued* request is written next.
1.x had no per-request classification, so a handle's priority was the only
signal and everything a handle submitted sat in one lane. In 2.0 every request
already carries an intrinsic class — ordinary control is `Normal`, drives and
absolute moves are `User`, and the typed stops plus owner-issued protocol cancellation are `Urgent`
— so an emergency stop preempts queued work with no API call at all. Public QoS
can move ordinary traffic among the lower three lanes but cannot cross that
safety boundary.

| 1.x call | 2.0 call |
| --- | --- |
| `camera.set_command_priority(Priority::Low)` | `camera.set_submission_class(Some(SubmissionClass::Background))` |
| `camera.command_priority()` | `camera.submission_class()` — returns `Option<SubmissionClass>`, where `None` means "use each request's intrinsic class" |
| `camera.execute_with_priority(cmd, Priority::High)` | `camera.execute_with_submission_class(&cmd, SubmissionClass::User)` |
| `camera.execute_with_priority(stop, Priority::Critical)` | Submit the typed stop normally; it is intrinsically `ControlClass::Urgent`. |
| — (no 1.x equivalent) | `camera.inquire_with_submission_class(&inquiry, class)` and `camera.submit_with_submission_class::<K, _>(&operation, class)` |
| `BlockingClient` priority methods | The same names on `blocking::Camera` and `blocking::CameraSession` |
| — (no 1.x equivalent) | `DynSessionCamera::execute_with_submission_class`, `inquire_with_submission_class`, `submit_targeted_with_submission_class`, `submit_applied_with_submission_class`, and `set_submission_class` |

Three behavioural differences are worth reading before porting:

- **No public QoS override can weaken an urgent request.** A handle set to
  `Background`, and even a per-submission
  `SubmissionClass::Background`, still submits `PanTiltStop`, `ZoomStop`,
  `FocusStop`, and owner-issued protocol cancellation as `ControlClass::Urgent`. The public QoS
  type has no `Urgent` variant, so ordinary work cannot impersonate safety
  traffic either.
- **Inquiries are covered.** 1.x kept inquiries at a fixed polling priority; in
  2.0 they share the same four lanes, so a handle demoted to `Background` moves
  its telemetry reads out of the way as well as its commands. Owner-internal
  traffic — settlement polling behind `settled()`, and the observation inquiries
  behind `motion()` — keeps its own built-in class.
- **The default is per handle, not per session.** Cloning an async `Camera`
  copies the current value and then diverges, and two views taken from one
  `Session` are independent. This matches 1.x.

`runtime::testing::Priority` has no 2.0 equivalent, because 2.0's tests do not
need one: `SubmissionClass` is public in every build configuration, so a test
names the QoS through the same API an application uses, and the crate's own
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

### A refused `cancel` hands the handle back

Cancelling a command that has already been written needs profile support for the
standard VISCA socket-cancel command. `PtzOpticsG2` is the one built-in profile
without it, and there the owner refuses with `Error::NotSupported` and
deliberately leaves the original request scheduled and able to complete — the
same per-phase contract 1.x had, where a still-queued command cancels locally on
every profile.

Because the original is still live, `cancel` consumes the handle only when it
succeeds. A refusal returns `CancelRejected<H>`, which carries the handle back:

```rust,ignore
let operation = match operation.cancel().await {
    Ok(cancellation) => return Ok(cancellation),
    Err(rejected) => rejected
        .into_operation()
        .ok_or(Error::RuntimeShutdown)?,
};
// Still observable, still retryable — and the axis is stopped the usual way.
camera.zoom().stop().await?.applied().await?;
operation.detach();
```

`?` still works in a function returning `Error`: the `From<CancelRejected<H>>`
conversion keeps the reason and detaches the handle, which is exactly what the
consuming shape did before.

1.x's async and dyn handles borrowed for `cancel` (`InFlight::cancel(&self)` at
`src/camera/inflight.rs:344`, documented at `:337` as "This method borrows the
handle, so its exact response may still be awaited"), so a `NotSupported` there
also left the caller holding the handle. 2.0 keeps that recovery while keeping
its linear consuming terminal methods. 1.x's *blocking* handle consumed on this
path (`src/camera/inflight.rs:615`) and `src/runtime/blocking_runner.rs:413`
discarded the retained result with it; 2.0 does not reproduce that asymmetry —
both facades behave like the 1.x async one.

### Scoped stop-on-exit guard

To bound movement by a scope rather than by an explicit call on every path,
write a guard whose own `Drop` submits the typed STOP. This is caller-owned
code — the library deliberately offers no such type, so the policy, the axes,
and the failure handling stay yours.

Every Rust snippet on this page is compiled by the crate's own test suite, so
the `#[cfg(feature = "...")]` attributes below are load-bearing: they name the
Cargo feature a snippet needs.

```rust
use grafton_visca::blocking::Camera;
use grafton_visca::camera::profiles::PtzOpticsG2;
use grafton_visca::command::PanTiltDirection;
use grafton_visca::types::{PanSpeed, TiltSpeed};
use grafton_visca::Error;

struct StopPanTiltOnExit<'a, 'session> {
    camera: &'a Camera<'session, PtzOpticsG2>,
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

fn bounded_drive(
    camera: &Camera<'_, PtzOpticsG2>,
    direction: PanTiltDirection,
    pan: PanSpeed,
    tilt: TiltSpeed,
    do_fallible_work: impl FnOnce() -> Result<(), Error>,
) -> Result<(), Error> {
    // Hold the guard for the region that must stay bounded.
    let _stop_on_exit = StopPanTiltOnExit { camera };
    let drive = camera.pan_tilt().move_direction(direction, pan, tilt)?;
    do_fallible_work()?; // an early `?` here still stops pan/tilt
    drive.applied()
}
```

That guard runs on every way out of the scope, including a panic unwinding
through it.

`Drop` cannot await, so the async form is a wrapper rather than a guard type:
run the fallible region, stop, then propagate the body's result.

```rust
#[cfg(feature = "async")]
mod async_form {
    use grafton_visca::camera::profiles::PtzOpticsG2;
    use grafton_visca::command::PanTiltDirection;
    use grafton_visca::types::{PanSpeed, TiltSpeed};
    use grafton_visca::{Camera, Error};

    pub async fn bounded_drive(camera: &Camera<PtzOpticsG2>) -> Result<(), Error> {
        let result = drive_up(camera).await;
        if let Ok(stop) = camera.pan_tilt().stop().await {
            let _ = stop.applied().await;
        }
        result
    }

    async fn drive_up(camera: &Camera<PtzOpticsG2>) -> Result<(), Error> {
        camera
            .pan_tilt()
            .move_direction(PanTiltDirection::Up, PanSpeed::new(6)?, TiltSpeed::new(6)?)
            .await?
            .applied()
            .await
    }
}
```

**The two forms do not cover the same exits.** The wrapper covers exactly the
two ways `drive_up` can *return*: `Ok` and `Err`. It does not cover a panic
inside the body, and it does not cover the caller dropping the `bounded_drive`
future before it completes — a `select!` loser, a `tokio::time::timeout` that
expires, an aborted task. In both of those cases the wrapper's stop is simply
never reached and the camera keeps moving. The synchronous guard does cover
both, because `Drop` runs while unwinding and runs when the value goes out of
scope for any reason. If the async path must survive cancellation, hold a
guard that submits the STOP through a channel or a detached task from its own
`Drop`, or bound the movement at the camera instead of at the future.

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
| 1.x `Camera::set_timeout_config` / `timeout_config` | `Session::set_tuning` / `tuning` (and the same pair on `CameraSession`), taking an `OperationalTuning`. Standard `CameraConfig` construction uses `with_tuning`. See [Reconfiguring timeouts at runtime](#reconfiguring-timeouts-at-runtime). |

Serialization features (`serde`, `schemars`, `ts-rs`) remain opt-in data-shape
features. They do not reopen private modules or create a second semantic
registry. `test-utils` is for deterministic tests, not production construction.

### Reconfiguring timeouts at runtime

1.2.0's `Camera::set_timeout_config` took a `TimeoutConfig`; 2.0's
`Session::set_tuning` takes an `OperationalTuning`. The historical category
fields map directly to the corresponding tuning methods:

| 1.2.0 `TimeoutConfig` field | 2.0 `OperationalTuning` builder |
| --- | --- |
| `ack_timeout` | `ack_timeout` |
| `quick_timeout` | `quick_timeout` (an individual command-category override) |
| `movement_timeout` | `movement_timeout` |
| `preset_timeout` | `preset_timeout` |
| `long_timeout` | `long_running_timeout` |
| `network_timeout` | `network_timeout` |
| `default_timeout` | No direct counterpart: every 2.0 request selects a `TimeoutClass`; set the corresponding category deadline. |
| *(no 1.x field)* | `settlement_timeout` for the physical-settling budget; `inquiry_timeout` remains the profile inquiry-response deadline |
| `RetryConfig::max_retries`, `base_retry_delay`, `max_retry_duration` | `retry_limit` and `retry_timing` |

`CommandTimeouts` is the profile's exact command policy. Its default table
preserves the 1.x values (Quick 5 seconds, Movement 30 seconds, Preset 60
seconds, LongRunning 300 seconds, and Network 5 seconds); each built-in
profile records its exact category values explicitly. An operational category
override must be non-zero and cannot undercut that category's profile value.
Inquiries declare `TimeoutClass::Inquiry` and use `inquiry_timeout`; command
Quick and Network values are never inquiry deadlines.
Retry counts remain category-based, but v2 deliberately makes replay depend on
correlation evidence. Backoff begins at 50 ms by default, uses deterministic
equal jitter, and is capped at the larger of 500 ms and the profile busy
timeout. One admission-to-terminal retry budget remains active through every
later noncancelled phase and is the largest of ten seconds, twice the request's
governing deadline, and the profile busy timeout. `retry_timing` may make those
bounds more conservative; it cannot make an ambiguous raw replay safe.

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
| A sent raw command's outcome cannot be correlated (ACK/completion/cancellation ambiguity, receive fault while awaiting ACK, or active retry-budget expiry in `Sending`/`AwaitingAck`/`Executing`) | `UnsequencedCommandUnconfirmed` | `true` |
| The owner's transport or channel is gone | `NoTransport`, `TransportChannelClosed`, … | `true` |
| The application shut the session down | `RuntimeShutdown` | `false` |

`UnsequencedCommandUnconfirmed` is the raw-ambiguity mapping in that table:
discard the old owner, open a replacement session from the retained
`SessionConfig`, treat its cache as `Unknown`, and re-query/reconcile device
state before any deliberate resubmission. Never blindly replay the uncertain
command; it may already have acted on the camera.

`true` is positive proof that the session is finished; `false` only means the
error alone does not prove it. Ordinary per-request failures — timeouts, busy
states, protocol and parameter errors — are `false`, and so is a raw `Io`
failure, because a datagram write failure is isolated to its own transmission
and a stream failure reaches the caller as `StreamPoisoned`.

1.x's `Error::to_public_error()` is gone and has no replacement. It folded
`NoSocket` into `NoTransport`, which in 2.0 would move the camera's transient
`0x05` capacity answer into the set `requires_new_session()` calls session
death — a reconnect where a retry was correct. Use `Error::kind()` for the
coarse category and `requires_new_session()` for the reconnect decision.

```rust
use grafton_visca::blocking::{Camera, Session};
use grafton_visca::camera::profiles::PtzOpticsG2;
use grafton_visca::transport::{BlockingTransport, HasTransportConfig};
use grafton_visca::{Error, PlainCommand, SessionConfig};

fn execute_or_reopen<C, T>(
    camera: &Camera<'_, PtzOpticsG2>,
    command: &C,
    config: &SessionConfig,
    new_transport: impl FnOnce() -> Result<T, Error>,
) -> Result<(), Error>
where
    C: PlainCommand + ?Sized,
    T: BlockingTransport + HasTransportConfig + 'static,
{
    match camera.execute(command) {
        Ok(()) => {}
        Err(error) if error.requires_new_session() => {
            // Drop the old session and views, then rebuild from the retained
            // configuration. Re-query state before applying anything new.
            let session = Session::open(new_transport()?, config.clone())?;
            // ...
            session.close()?;
        }
        Err(error) => return Err(error),
    }
    Ok(())
}
```

For the complete construction, transport, target, tuning, noun, and lifecycle
examples, see [`usage_2_0.md`](usage_2_0.md). For bounded state and diagnostic
behavior, see [`observability_and_recovery.md`](observability_and_recovery.md).
