# 2.0 usage and construction

The 2.0 API has blocking and async static calling conventions plus
runtime-profile projections for both. Every view ends at the same owner-backed
session model.

Every Rust snippet on this page is compiled by the crate's own test suite, so
the `#[cfg(feature = "...")]` attributes below are load-bearing: they name the
Cargo feature a snippet needs.

## Feature selection

| Need | Cargo features |
| --- | --- |
| Blocking only | `blocking` (the default feature) |
| Runtime-neutral async | `async` and a caller-owned `Executor` |
| Tokio async | `runtime-tokio` |
| smol async | `runtime-smol` |
| Blocking serial | `transport-serial` |
| Tokio serial | `runtime-tokio,transport-serial-tokio` |
| Blocking runtime-profile views | `blocking,dyn-api` |
| Dynamic async views | `async,dyn-api` with a caller-owned `Executor`, or `runtime-tokio,dyn-api` / `runtime-smol,dyn-api` |
| Serialization/schema/typescript | `serde`, `schemars`, and `ts-rs` as needed |
| Testkit steps and helpers | `test-utils` |
| Scripted blocking transport | `test-utils,blocking` |
| Async scripted transport and deterministic clock/executor | `test-utils,async` |
| VISCA camera simulator | `test-utils,runtime-tokio` |

`dyn-api` does not imply `async`: its blocking projection has no futures,
executor, or async-runtime dependency. Runtime features imply the canonical
`async` facade. `runtime-tokio`
and `runtime-smol` may be enabled together; each session still receives one
explicit runtime. The default `blocking` feature is an independent native
synchronous implementation: a blocking-only application does not enable or
link Tokio, smol, or an async executor. CI checks that dependency boundary for
both blocking network and blocking serial builds.

## One standard connection

Use `Connect` for a short, standard connection. The type parameter is the
compile-time profile, so transport support and optional control gates are
checked before opening the connection.

### Blocking

```rust
use grafton_visca::blocking::{CameraSession, Connect};
use grafton_visca::camera::profiles::PtzOpticsG2;

fn standard_connection() -> Result<(), grafton_visca::Error> {
    let session: CameraSession<PtzOpticsG2> =
        Connect::open_tcp::<PtzOpticsG2>("192.168.0.110")?;
    let camera = session.camera();
    camera.power().on()?;
    camera.zoom().stop()?.applied()?;
    session.close()
}
```

Use `Connect::open_udp` for UDP. With `transport-serial`,
`Connect::open_serial::<P>(port, baud_rate)` returns the multi-target `Session`;
select a registered address with `session.camera_for::<P>(target)`.

Code generic over the network kind can pass `TransportOptions::tcp(...)` or
`TransportOptions::udp(...)` to `Connect::open::<P>`. That form requires only
`CompileTimeProfile`; it checks transport compatibility at runtime and returns
the same `CameraSession<P>`. Serial and custom options are deliberately
rejected there because they use the multi-target `Session` construction path.

### Async

```rust
#[cfg(feature = "runtime-tokio")]
async fn standard_connection() -> Result<(), grafton_visca::Error> {
    use grafton_visca::camera::profiles::PtzOpticsG2;
    use grafton_visca::runtime::TokioRuntime;
    use grafton_visca::{CameraSession, Connect};

    let runtime = TokioRuntime::from_current()?;
    let session: CameraSession<PtzOpticsG2> =
        Connect::open_tcp::<PtzOpticsG2, _>("192.168.0.110", runtime).await?;
    let camera = session.camera();
    camera.power().on().await?;
    camera.zoom().stop().await?.applied().await?;
    session.close().await
}
```

Use `runtime-smol` with `SmolRuntime`, or use `Connect::open_udp` for UDP.
`Connect` and `CameraConfig` perform the same preflight and return the
single-camera `CameraSession<P>` for TCP/UDP. Async serial is Tokio-only: with
`transport-serial-tokio`, `Connect::open_serial::<P, _>(port, baud_rate,
runtime)` returns the multi-target owner-backed `Session`.

## Reusable configuration and target selection

`CameraConfig<P>` is convenient profile-typed standard-transport data. It can
set an address, `CameraId`, timeout policy, retry policy, transport options,
and buffer/keepalive settings before calling `open`, `open_async`, or the
serial-specific open methods. `open` and `open_async` return a
`CameraSession<P>` for standard network transports; `open_serial` and
`open_serial_async` return a multi-target `Session`. `CameraSession::open`
takes a caller-owned transport with the same single-camera bind.
`session_config()` lowers that pure
configuration into the shared mode-independent `SessionConfig` without DNS,
socket, serial, executor, or protocol work.

`SessionConfig` is the reusable multi-target boundary:

```rust
use grafton_visca::{ProfileSpec, SessionConfig};
use grafton_visca::camera::profiles::{GenericVisca, PtzOpticsG2};

let g2 = ProfileSpec::from_compile_time::<PtzOpticsG2>()?;
let generic = ProfileSpec::from_compile_time::<GenericVisca>()?;
let mut config = SessionConfig::new(g2);
config.register_target(grafton_visca::CameraId::new(2)?, generic)?;

// Apply one session-wide tuning value before opening. Tuning may only be
// more conservative than the profile floor, which is 150 ms here.
let config = config.with_tuning(
    grafton_visca::OperationalTuning::new()
        .inquiry_spacing(std::time::Duration::from_millis(250)),
)?;
# Ok::<(), grafton_visca::Error>(())
```

When defining a custom runtime profile, construct its timing facts as one
validated value and select it with `camera_dyn`. This complete blocking-only
program uses the built-in TCP transport but never implements a compile-time
profile marker:

```rust,no_run
#[cfg(not(all(feature = "blocking", feature = "dyn-api")))]
fn main() {}

#[cfg(all(feature = "blocking", feature = "dyn-api"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::time::Duration;
    use grafton_visca::{
        blocking::Session,
        capabilities::Capabilities,
        command::PowerOn,
        profile::{
            PositionInquirySupport, ProfileEnvelope, ProfileSpec, ProfileTiming,
            TransportCompatibility,
        },
        transport::Transport,
        CommandTimeouts, SessionConfig,
    };

    let mut capabilities = Capabilities::runtime_baseline("Custom camera", 1)?;
    capabilities.has_power = true;
    capabilities.power_on_time = Duration::from_secs(1);

    let timing = ProfileTiming::builder()
        .ack_timeout(Duration::from_millis(100))
        .command_timeouts(CommandTimeouts::default())
        .inquiry_timeout(Duration::from_secs(1))
        .cancellation_timeout(Duration::from_secs(1))
        .ambiguity_timeout(Duration::from_secs(1))
        .busy_timeout(Duration::ZERO)
        .raw_inquiry_reply_skew(Duration::ZERO)
        .minimum_inquiry_spacing(Duration::ZERO)
        .minimum_command_spacing(Duration::ZERO)
        .build()?;
    let profile = ProfileSpec::builder(capabilities)
        .transports(TransportCompatibility::new(Some(5678), None, false))
        .envelope(ProfileEnvelope::RawVisca)
        .timing(timing)
        .maximum_command_sockets(1)
        .supports_operation_complete(true)
        .supports_command_cancel(false)
        .preset_recall_axes(None)
        .position_inquiries(PositionInquirySupport::new(false, false, false))
        .build()?;

    let transport = Transport::tcp()
        .address("192.168.0.110:5678")
        .build_blocking()?;
    let session = Session::open(transport, SessionConfig::new(profile))?;
    session.camera_dyn()?.execute(&PowerOn::new())?;
    session.close()?;
    Ok(())
}
```

Every timing and protocol-safety fact is explicit.
`CommandTimeouts::default()` opts into the standard 1.x command-category
deadlines without making the rest of the profile implicit. Builder failures
name their Rust field identifiers (for example, `position_inquiries` and
`capabilities.inquiry_support`) so a failed invariant points back to the value
to fix. The raw inquiry reply skew may be zero, but cannot exceed the profile's
minimum inquiry spacing; it is retained only when a raw inquiry ends without a
matched reply.

The equivalent builder form is `SessionConfig::for_target(...).with_target(...)`.
`SessionConfig::from_compile_time::<P>()` creates a reusable camera-1 config
from a static profile. `register_target` and `with_target` accept only IDs 1
through 7, reject broadcast and duplicate IDs, and cap the registry at seven
targets. Registration is immutable after the session starts; runtime-mutable
tuning is not — `Session::set_tuning` replaces it at runtime, and
`Session::tuning` reads back the live value. Every request prepared after the
update uses the new deadlines, retry budget and pacing; a request already in
flight keeps the deadlines it was admitted with. The
Construction-only recovery policy is separate from tuning. Configure
`strict_unconfirmed_poison` with
`SessionConfig::with_strict_unconfirmed_poison` (or the matching
`CameraConfig` builder) before opening; `set_tuning` then replaces every field
in `OperationalTuning` without pretending to mutate that immutable policy. See
[Operational tuning](#operational-tuning) and [Reconfiguring timeouts at
runtime](migration_2_0.md#reconfiguring-timeouts-at-runtime).

For one target, prefer the single-camera constructors: they name the profile
once and return a `CameraSession<P>` whose `camera()` is bound to that same `P`
at compile time, so there is no second profile naming for a mismatch to fail
on and no projection error to handle.

`session.camera::<P>()` remains the concise view selector for a `Session` that
was opened with the multi-target constructors. For two or more targets it
intentionally returns an error; use `session.camera_for::<P>(target)`.
`camera` and `camera_for` check the complete registered `ProfileSpec` at run
time, not only a profile name, because a `Session` carries no compile-time
profile. Runtime profiles use `session.camera_dyn()` for one target and
`session.camera_dyn_for(target)` for an explicit target. Those selectors return
`BlockingDynSessionCamera` on the blocking facade and `DynSessionCamera` on the
async facade.

## Standard transport and profile matrix

The default network ports are profile facts, not global constants:

| Profile family | Envelope | TCP | UDP | Serial |
| --- | --- | ---: | ---: | --- |
| `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X` | Raw VISCA | 5678 | 1259 | Supported |
| `SonyEVIH100`, `SonyBRC300`, `NearusBRC300`, `GenericVisca` | Raw VISCA | 5678 | 1259 | Supported |
| `SonyFR7`, `SonyBRCH900` | Sony encapsulated VISCA | — | 52381 | Not a standard path |

An explicit `host:port` overrides a network default. Bracket IPv6 when an
explicit port is present, for example `[2001:db8::1]:5678`. A profile/transport
pair is validated before endpoint resolution or connector I/O. Sony
encapsulation is not silently treated as raw VISCA.

Standard TCP, UDP, and Sony multi-target configurations are rejected before a
socket is created. Heterogeneous profiles are admitted only for a compatible
raw serial/custom transport; the custom transport must preserve framing and
response attribution. A caller-owned transport that reports a standard kind is
validated against the profile transport registry. A truly custom transport
that reports no standard kind (`None`) may bypass only the standard
profile/transport pair matrix as an intentional BYO escape hatch; it still
undergoes target-registry, envelope, addressing/topology, pacing, framing,
buffer, and bounded-owner validation.

## Operational tuning

Use `OperationalTuning` with `SessionConfig::with_tuning` for a reusable
session-wide policy. Available overrides cover command and inquiry spacing,
command-socket capacity, acknowledgement/settlement/inquiry deadlines, each
command category (`quick_timeout`, `movement_timeout`,
`preset_timeout`, `long_running_timeout`, and `network_timeout`), retry count,
and retry backoff/budget.

Tuning can make the owner more conservative but cannot weaken a profile's
minimum pacing, raise its socket limit, undercut a profile command category,
or use zero timeouts. When targets have different profile minima, the strictest
applicable pacing and capacity policy wins. A settlement timeout is separate
from protocol completion: a targeted operation may be acknowledged before the
profile-selected protocol settlement condition is met. This is not a
bench-verified assertion of physical rest; see the
[hardware release checklist](hardware_release_checklist.md). Inquiries always
use `inquiry_timeout`.

## Caller-owned transports and executors

Use `Session::open` when the application already owns a transport or needs a
custom executor. The async path is:

```rust
#[cfg(feature = "async")]
async fn open_caller_owned<E, T>(
    custom_async_transport: T,
    executor: E,
) -> Result<(), grafton_visca::Error>
where
    E: grafton_visca::Executor,
    T: grafton_visca::transport::AsyncTransport
        + grafton_visca::transport::HasTransportConfig
        + 'static,
{
    use grafton_visca::camera::profiles::PtzOpticsG2;

    let session = grafton_visca::Session::open(
        custom_async_transport,
        grafton_visca::SessionConfig::from_compile_time::<PtzOpticsG2>()?,
        executor,
    )
    .await?;
    session.close().await
}
```

The blocking counterpart is `grafton_visca::blocking::Session::open(transport,
config)`. `CameraSession::open(transport, &CameraConfig::<P>::new())` — with the
executor as a third argument on the async side — is the single-camera form of
the same boundary, taking the profile from `P` instead of a runtime
`SessionConfig`. Implementors must
declare `AsyncTransport` or `BlockingTransport`, `HasTransportConfig`, and the
correct stream/datagram send semantics. A blocking implementation must bound
both `send_with_timeout` and `recv_into_with_timeout` by the supplied positive
duration; the owner cannot preempt arbitrary synchronous code. Zero advertised
read/write timeouts are rejected at construction. A standard-kind caller-owned
transport is checked against the profile transport registry; one that reports
no standard kind (`None`) may bypass only that standard profile/transport pair matrix. The
custom path still undergoes target-registry, envelope, addressing/topology,
pacing, framing, buffer, and bounded-owner validation. A runtime-neutral async
executor must provide coherent spawn, sleep, timeout, and clock behavior from
one runtime.

### Sharing a blocking session

`blocking::Session` and its borrowed `Camera<'_, P>` views are `Send + Sync`.
The owner still admits only one fail-fast turn at a time, so overlapping calls
from multiple threads return `Error::TransportBusy`. `Session::metrics()` is a
read-only exception: it returns the last completed owner-turn snapshot while a
different thread is in a bounded transport wait. If callers should wait rather
than retry for a control operation, put the session in `Arc<Mutex<Session>>`,
lock it for one operation, and derive the camera view from the guard. The view
must remain inside the guard's scope:

```rust,no_run
use std::sync::{Arc, Mutex};
use grafton_visca::{blocking, profiles::PtzOpticsG2};

fn share(session: blocking::Session) -> grafton_visca::Result<()> {
    let session = Arc::new(Mutex::new(session));
    std::thread::scope(|scope| {
        let session = Arc::clone(&session);
        let worker = scope.spawn(move || -> grafton_visca::Result<()> {
            let guard = session.lock().expect("camera session lock");
            let camera = guard.camera::<PtzOpticsG2>()?;
            camera.power().on()
        });
        worker.join().expect("camera worker")
    })?;
    Ok(())
}
```

## Static, dynamic, and generic operations

Static cameras expose the same 14 noun views in blocking and async forms:
`power`, `zoom`, `system`, `pan_tilt`, `focus`, `exposure`, `white_balance`,
`image`, `presets`, `tally`, `nd_filter`, `motion_sync`, `menu`, and
`advanced`. Profile-gated methods are available only when the profile's
`Has*` marker permits them. `motion()` separately owns
`stop_all_motion`, `is_moving`, `is_moving_axes`, and `wait_until_idle`.
`is_moving()` takes no argument and samples `AffectedAxes::MOVEMENT`;
`is_moving_axes(MotionQuery)` is the axis-selecting form.

Async noun futures borrow the temporary accessor. A direct call such as
`camera.motion().stop_all_motion().await` is fine; bind each accessor before a
macro retains several futures:

```rust,ignore
let sony_motion = sony.motion();
let raw_motion = raw.motion();
let (sony_result, raw_result) = tokio::join!(
    sony_motion.stop_all_motion(),
    raw_motion.stop_all_motion(),
);
```

Blocking runtime-profile code uses `BlockingDynSessionCamera`. Its generic
`execute`, `inquire`, and `submit` methods return native synchronous results and
blocking operation handles; it also exposes the motion observers, state cache,
and submission-class controls without enabling `async`.

Dynamic async code uses `DynSessionCamera` and its object-safe
`DynSessionCameraControl` plus `DynPower`, `DynZoom`, `DynSystem`,
`DynPanTilt`, `DynFocus`, `DynExposure`, `DynWhiteBalance`, `DynImage`,
`DynPresets`, `DynTally`, `DynNdFilter`, `DynMotionSync`, `DynMenu`,
`DynAdvanced`, and `DynMotion` traits:

```rust
#[cfg(all(feature = "dyn-api", feature = "async"))]
async fn dynamic_views(
    session: &grafton_visca::Session,
    query: grafton_visca::camera::MotionQuery,
) -> Result<(), grafton_visca::Error> {
    use grafton_visca::dynapi::{DynMotion, DynZoom};

    let camera = grafton_visca::dynapi::DynSessionCamera::from_session(session)?;
    camera.zoom().stop().await?.applied().await?;
    camera.motion().is_moving_axes(query).await?;
    Ok(())
}
```

Use the generic `execute` for a plain request, `inquire` for a typed inquiry,
and `submit` for a typed operation. Dynamic custom requests use the explicit
targeted/applied-only request traits. An applied-only handle has no settled
state; cancellation reports its owner outcome, and `detach` is the explicit
fire-and-forget choice. Dropping a handle is exactly `detach` and never stops
hardware, so an error path leaves movement running until something else ends
it; see the scoped stop-on-exit guard in
[`migration_2_0.md`](migration_2_0.md#drop-never-stops-hardware).

## Optional ecosystem surfaces

`serde` provides serialization for supported public values and configuration;
`schemars` adds JSON Schema; `ts-rs` generates TypeScript declarations. These
features describe data interchange and do not change protocol semantics.
`ViscaInquiry`, `ViscaEnum`, and `ViscaValue` are the supported downstream
derive entry points. `test-utils` alone exposes `testkit::{Step, helpers}` for
tests. Add `blocking` for `ScriptedBlockingTransport`; add `async` for
`ScriptedTransport`, `DeterministicClock`, and `DeterministicExecutor`; and add
`runtime-tokio` for `ViscaCameraSimulator`. It is not required by production
applications.

See [`architecture_2_0.md`](architecture_2_0.md) for ownership/order
invariants and [`observability_and_recovery.md`](observability_and_recovery.md)
for metrics, diagnostics, state, and recovery.
