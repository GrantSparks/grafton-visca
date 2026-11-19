# 2.0 usage and construction

The 2.0 API exposes symmetric `blocking` and `r#async` facades. Both use
`Camera<P>` for compile-time profiles and `Camera<AnyProfile>` for runtime
profiles, with the same owner-backed session model.

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
| Runtime-selected profiles | Use `AnyProfile` with the same facade features above |
| Serialization/schema/typescript | `serde`, `schemars`, and `ts-rs` as needed |
| Testkit steps and helpers | `test-utils` |
| Scripted blocking transport | `test-utils,blocking` |
| Manual owner clock and participation guard (`ManualClock`, `ClockParticipant`) | `test-utils,blocking` or `test-utils,async` |
| Manual-clock async executor (`ManualClockExecutor`, `LocalExecutor`) | `test-utils,async` |
| Async scripted transport and deterministic clock/executor | `test-utils,async` |
| VISCA camera simulator | `test-utils,runtime-tokio` |

Runtime features imply the canonical `async` facade. `runtime-tokio` and
`runtime-smol` may be enabled together; each session still receives one
explicit runtime. The default `blocking` feature is an independent native
synchronous implementation: a blocking-only application does not enable or
link Tokio, smol, or an async executor. CI checks that dependency boundary for
both blocking network and blocking serial builds.

## One standard connection

Use `CameraSession::<P>::connect_tcp` for a standard connection. The type parameter is the
compile-time profile, so transport support and optional control gates are
checked before opening the connection.

### Blocking

```rust
use grafton_visca::blocking::CameraSession;
use grafton_visca::profiles::PtzOpticsG2;

fn standard_connection() -> Result<(), grafton_visca::Error> {
    let session: CameraSession<PtzOpticsG2> =
        CameraSession::<PtzOpticsG2>::connect_tcp("192.168.0.110")?;
    let camera = session.camera();
    camera.power().on()?;
    camera.zoom().stop()?.applied()?;
    session.close()
}
```

Use `CameraSession::<P>::connect_udp` for UDP and, with `transport-serial`,
`CameraSession::<P>::connect_serial(port, baud_rate)` for one serial target.
All three return `CameraSession<P>`. A reusable `CameraConfig<P>` selects
transport options and policy; `CameraSession::connect(&config)` opens it.
For several registered serial targets, use `Session::connect(&session_config)`
and select each camera with `session.camera::<P>(target)`.

### Async

```rust
#[cfg(feature = "runtime-tokio")]
async fn standard_connection() -> Result<(), grafton_visca::Error> {
    use grafton_visca::profiles::PtzOpticsG2;
    use grafton_visca::runtime::TokioRuntime;
    use grafton_visca::r#async::CameraSession;

    let runtime = TokioRuntime::from_current()?;
    let session: CameraSession<PtzOpticsG2> =
        CameraSession::<PtzOpticsG2>::connect_tcp("192.168.0.110", runtime).await?;
    let camera = session.camera();
    camera.power().on().await?;
    camera.zoom().stop().await?.applied().await?;
    session.close().await
}
```

Use `runtime-smol` with `SmolRuntime`, or use `connect_udp` for UDP.
The async constructors take an explicit runtime as their last argument.
TCP and UDP addresses require bracketed IPv6 literals and canonical dotted-quad
IPv4 literals without leading zeros. Legacy forms such as `192.168.1.010`,
`127.1` and `0x7f000001` return `InvalidAddress` before name resolution or
socket creation. Hostnames such as `cam-01.studio` and `10.cams.example` remain
valid.
Tokio serial uses `transport-serial-tokio`; `connect_serial(port, baud_rate,
runtime)` returns the same single-camera `CameraSession<P>` shape.

## Reusable configuration and target selection

`config::CameraConfig<P>` is reusable single-camera configuration. Its
`with_*` methods select the address, checked `IndividualCameraId`, operational tuning,
and transport buffer/keepalive settings. Open it with
`blocking::CameraSession::connect(&config)` or
`r#async::CameraSession::connect(&config, runtime).await`.
`CameraSession::open(transport, &config[, executor])` attaches a caller-owned
transport with the same profile binding. `session_config()` lowers the data
into `config::SessionConfig` without opening a socket or serial device.

`SessionConfig` is the reusable multi-target boundary:

```rust
use grafton_visca::config::SessionConfig;
use grafton_visca::profiles::ProfileId;

let g2 = ProfileId::PtzOpticsG2.spec()?;
let generic = ProfileId::GenericVisca.spec()?;
let config = SessionConfig::new(g2)
    .with_target(grafton_visca::types::IndividualCameraId::new(2)?, generic)?;

// Apply one session-wide tuning value before opening. Tuning may only be
// more conservative than the profile floor, which is 150 ms here; opening the
// session checks it against every registered profile.
let config = config.with_tuning(
    grafton_visca::config::OperationalTuning::new()
        .with_inquiry_spacing(std::time::Duration::from_millis(250)),
);
# Ok::<(), grafton_visca::Error>(())
```

When defining a custom runtime profile, construct its timing facts as one
validated value and select it with `Camera<AnyProfile>`. This complete blocking-only
program uses the built-in TCP transport but never implements a compile-time
profile marker:

```rust,no_run
#[cfg(not(feature = "blocking"))]
fn main() {}

#[cfg(feature = "blocking")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::time::Duration;
    use grafton_visca::{
        blocking::CameraSession,
        capabilities::Capabilities,
        capabilities::{PowerCaps, PowerStatusQuery},
        command::power,
        profiles::AnyProfile,
        profiles::ProfileEnvelope,
        profiles::ProfileSpec,
        profiles::ProfileTiming,
        profiles::TransportCompatibility,
        profiles::CommandTimeouts,
        config::{CameraConfig, TransportOptions},
    };

    let mut capabilities = Capabilities::runtime_baseline("Custom camera", 1)?;
    capabilities.power = Some(PowerCaps {
        status_query: PowerStatusQuery::Standard,
        standby: false,
    });

    let timing = ProfileTiming::builder()
        .ack_timeout(Duration::from_millis(100))
        .command_timeouts(CommandTimeouts::default())
        .inquiry_timeout(Duration::from_secs(1))
        .cancellation_timeout(Duration::from_secs(1))
        .ambiguity_timeout(Duration::from_secs(1))
        .busy_timeout(Duration::ZERO)
        .raw_inquiry_reply_skew(Duration::ZERO)
        .inquiry_spacing(Duration::ZERO)
        .command_spacing(Duration::ZERO)
        .build()?;
    let profile = ProfileSpec::builder(capabilities)
        .transports(TransportCompatibility::new().with_tcp(5678))
        .envelope(ProfileEnvelope::RawVisca)
        .timing(timing)
        .maximum_command_sockets(grafton_visca::types::CommandSocketCount::ONE)
        .supports_operation_complete(true)
        .supports_command_cancel(false)
        .position_inquiries(None)
        .build()?;

    let config = CameraConfig::new(profile, TransportOptions::tcp("192.168.0.110:5678"));
    let session = CameraSession::connect(&config)?;
    session.camera().execute(&power::On)?;
    session.close()?;
    Ok(())
}
```

Every timing and protocol-safety fact is explicit.
`CommandTimeouts::default()` opts into the standard 1.x command-category
deadlines without making the rest of the profile implicit. Builder failures
name their Rust field identifiers (for example, `position_inquiries` and
`capabilities.inquiries`) so a failed invariant points back to the value
to fix. The raw inquiry reply skew may be zero, but cannot exceed the profile's
minimum inquiry spacing; it is retained only when a raw inquiry ends without a
matched reply.

The equivalent builder form is `SessionConfig::for_target(...).with_target(...)`.
`SessionConfig::from_compile_time::<P>()` creates a reusable camera-1 config
from a static profile. `with_target` accepts only IDs 1
through 7, reject broadcast and duplicate IDs, and cap the registry at seven
targets. Registration is immutable after the session starts; runtime-mutable
tuning is not — `Session::set_tuning` replaces it at runtime, and
`Session::tuning` reads back the live value. Every request prepared after the
update uses the new deadlines, retry budget and pacing; a request already in
flight keeps the deadlines it was admitted with.
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

`Session::camera::<P>(target)` always names an explicit registered target and
checks its complete `ProfileSpec` against `P`. `Session::camera::<AnyProfile>(target)`
selects a runtime profile. `Camera<P>::into_any()` changes only the profile
parameter; `into_typed::<Q>()` checks complete profile equality before returning
a typed view. These conversions retain the same owner, cache and transport.

## Standard transport and profile matrix

The selected profile supplies the default network ports.

<!-- BEGIN GENERATED PROFILE TRANSPORTS -->
| Profile | TCP default | UDP default | Serial | Envelope |
| --- | --- | --- | --- | --- |
| `PtzOpticsG2` | 5678 | 1259 | yes | `RawVisca` |
| `PtzOpticsG3` | 5678 | 1259 | yes | `RawVisca` |
| `PtzOptics30x` | 5678 | 1259 | yes | `RawVisca` |
| `SonyFr7` | n/a | 52381 | no | `SonyEncapsulated` |
| `SonyBrcH900` | n/a | 52381 | no | `SonyEncapsulated` |
| `SonyEviH100` | n/a | n/a | yes | `RawVisca` |
| `SonyBrc300` | n/a | n/a | yes | `RawVisca` |
| `NearusBrc300` | n/a | n/a | yes | `RawVisca` |
| `GenericVisca` | 5678 | 1259 | yes | `RawVisca` |
<!-- END GENERATED PROFILE TRANSPORTS -->

An explicit `host:port` overrides a network default. Bracket IPv6 when an
explicit port is present, for example `[2001:db8::1]:5678`. A profile/transport
pair is validated before endpoint resolution or connector I/O. Sony
encapsulation is not silently treated as raw VISCA.

Standard TCP, UDP, and Sony multi-target configurations are rejected before a
socket is created. Heterogeneous profiles are admitted only for a compatible
raw serial/custom transport; the custom transport must preserve framing and
response attribution. A caller-owned transport that reports a standard kind is
validated against the profile transport registry. A caller-owned adapter can declare
`TransportIdentity::Custom { semantics, addressing }` to use the custom
transport path. The session still validates target registration, envelope,
addressing, pacing, framing, buffer limits, and owner capacity.

A command declared `RawReplyShape::CompletionOnly` expects a terminal without
an ACK. With a Sony envelope its sequence identifies the answer, allowing
concurrent work within normal limits. On raw UDP, TCP, or serial it starts
only when the target has no live command or inquiry. Ordinary work queues
while its answer is outstanding. Raw UDP retains a bounded quarantine after
the terminal. On raw TCP or serial, the ledger retains an unanswered write
after its own deadline. When the ambiguity window ends, queued work fails
unwritten with `InquiryCorrelationLost` or `CommandCorrelationLost` until the
answer arrives. Urgent STOPs may cross the raw exclusion (#795).

`RawReplyShape::NoReply` returns on local write success. The raw transports
apply idle-target admission and a bounded quarantine for a possible late
response. On raw TCP or serial, the ledger can retain possible rejection
evidence past the quarantine. On a Sony envelope, other work can use the
camera concurrently within its normal limits. See the
[transport correlation rules](architecture_2_0.md#raw-response-shapes-and-correlation-ownership).

## Operational tuning

Use `OperationalTuning` with `SessionConfig::with_tuning` for a reusable
session-wide policy. Available overrides cover command and inquiry spacing,
command-socket capacity, acknowledgement/settlement/inquiry deadlines, each
command category (`quick_timeout`, `movement_timeout`,
`preset_timeout`, `long_running_timeout`, and `network_timeout`), retry count,
and retry backoff/budget.

Configure read and write timeouts and receive limits separately from
operational tuning. The standard connection configuration holds these
`IoBounds` alongside its TCP, UDP, or serial settings.

Tuning can make the owner more conservative but cannot weaken a profile's
minimum pacing, raise its socket limit, undercut a profile command category,
or use zero timeouts. When targets have different profile minima, the strictest
applicable pacing and capacity policy wins. A settlement timeout is separate
from protocol completion: a targeted operation may be acknowledged before the
profile-selected settlement evidence is established. `Settlement` distinguishes
exact profile completion from stable samples with a window and tolerance;
stable samples do not prove arrival at the endpoint or independently guarantee
that physical motion has ended. Inquiries always use `inquiry_timeout`.

## Caller-owned transports and executors

To create an async transport separately, use the chosen `Runtime`'s
`connect_tcp` or `connect_udp` hook with a parsed `NetworkEndpoint` and
`TcpConnect` or `UdpConnect` settings, plus checked `IoBounds`. The
`connect_serial` hook takes `SerialConfig` and the I/O bounds.

Use `Session::open` when the application already owns a transport or needs a
custom executor. The async path is:

```rust
#[cfg(feature = "async")]
async fn open_caller_owned<E, T>(
    custom_async_transport: T,
    executor: E,
) -> Result<(), grafton_visca::Error>
where
    E: grafton_visca::runtime::Executor,
    T: grafton_visca::transport::AsyncTransport
        + grafton_visca::transport::HasTransportConfig
        + 'static,
{
    use grafton_visca::profiles::PtzOpticsG2;

    let session = grafton_visca::r#async::Session::open(
        custom_async_transport,
        &grafton_visca::config::SessionConfig::from_compile_time::<PtzOpticsG2>()?,
        executor,
    )
    .await?;
    session.close().await
}
```

The blocking counterpart is `grafton_visca::blocking::Session::open(transport,
&config)`. `CameraSession::open(transport, &config)` — with the
executor as a third argument on the async side — is the single-camera form of
the same boundary, taking the profile from `P` instead of a runtime
`SessionConfig`. Implementors must
declare `AsyncTransport` or `BlockingTransport`, `HasTransportConfig`, and the
correct stream/datagram send semantics. A blocking implementation must bound
both `send_with_timeout` and `recv_into_with_timeout` by the supplied positive
duration; the owner cannot preempt arbitrary synchronous code. The blocking
worker reads in slices of at most 10 ms, so a read that overruns its duration
delays every STOP, cancellation, and `close` by the overrun. The `io_bounds()` method returns an
`IoBounds` value with checked read and write timeouts and buffer limits. A standard-kind caller-owned
transport is checked against the profile transport registry. A transport
declaring `TransportIdentity::Custom { semantics, addressing }` uses the custom
path, with validation of target registration, envelope, addressing, pacing,
framing, buffer limits, and owner capacity. A runtime-neutral async
executor must provide detached spawning, sleep, reusable timers, and clock behavior
from one runtime. Async construction gives each completed step precedence at
its deadline. TCP and UDP share one connect budget across resolution and
socket setup, and IP literals bypass name resolution. On Tokio, dropping or
timing out a connect cancels a queued hostname lookup. An executing system
lookup can finish in the background, and its result is discarded. On smol,
blocking-pool resolution can outlive the connect future and its result is
discarded. With either runtime, the abandoned lookup cannot proceed to socket
setup. A blocking connect waits for an available resolver helper within its
budget. If that budget expires during a lookup, the caller gets a timeout
and the helper can finish in the background.

`raw::InquiryRequest::new` uses the inquiry lifecycle's fixed
timeout and accepts no timeout argument; `from_fn`, `with_context`, and explicit
`request::InquiryPolicy` values follow the same rule. Inquiry constructors take
`request::InquiryRetryClass::Never` or `InquiryRetryClass::Inquiry`; the latter
preserves bounded lost-reply recovery. Commands and operations instead take
`profiles::CommandTimeoutClass` and `request::CommandRetryClass`, which cannot
express inquiry timing or retry semantics. Custom `Request::policy()` methods
return the same policy types selected by their sealed lifecycle class
(#854, #970, #1016).

### Sharing a blocking session

`blocking::Session` and its `Camera<P>` views are owned,
`Clone + Send + Sync` handles on one owner worker thread. Clone session or
camera views into other threads; operation handles are `Send + Sync` and may
move between threads, while waits borrow them. The worker serializes every
call, so a thread waiting on one operation never blocks another thread's submit, cancel,
or STOP.

```rust,no_run
use grafton_visca::{
    blocking,
    profiles::PtzOpticsG2,
};

fn share(session: &blocking::Session) -> grafton_visca::Result<()> {
    let camera = session.camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)?;
    std::thread::scope(|scope| {
        let worker = scope.spawn(move || camera.power().on());
        worker.join().expect("camera worker")
    })
}
```

## Compile-time, runtime-selected, and generic operations

Static cameras expose the same 14 noun views in blocking and async forms:
`power`, `zoom`, `system`, `pan_tilt`, `focus`, `exposure`, `white_balance`,
`image`, `presets`, `tally`, `nd_filter`, `motion_sync`, `menu`, and
`streaming`. Profile-gated methods are available only when the profile's
`Has*` marker permits them. `motion()` separately owns
`stop_all`, `is_moving`, and `wait_until_idle`, with
the same names, arities and contracts on the blocking and async cameras, the
runtime-selected cameras.
`is_moving(MotionQuery)` samples the query's axes; `MotionQuery::default()`
selects `AffectedAxes::MOVEMENT`.

`stop_all()` returns `HaltReport` under one end-to-end deadline. At owner
acceptance it fences older declared queued motion and future retries, then
admits each supported STOP independently while respecting protocol gates.
Subsequent motion on that target and axis waits until the pending STOP can
no longer write. Motion on unrelated axes and targets remains eligible.
Inspect each axis's `HaltOutcome`, or use `into_result()` to explicitly retain
only the first failure. Applied STOPs provide protocol evidence. Physical rest
requires separate observation. A `CommandBufferFull` rejection can trigger a retry
within the halt deadline. A `CommandNotExecutable` rejection ends its STOP
promptly. A PTZOptics G2 in auto-focus mode refuses the focus STOP, so `focus`
is `Failed(CommandNotExecutable)` and `into_result()` returns that error
while auto-focus owns the lens. Superseded
motion fails with `MotionSuperseded`, whose `failure_context()` is
`NotAccepted` when it never reached the camera.

At first dispatch, the FR7 halt selects one zoom STOP for the most recent
continuous zoom drive in this session that may have taken effect on the target.
It uses the standard STOP by default. Uncertain writes can leave either drive
family possible. The halt report records protocol application of the selected
STOP. Check movement separately to verify that the lens stopped. Sony assigns
each STOP to its drive family in the guide for the optional lens controller
and lists both encodings in the FR7 VISCA command list.
[Sony lens-controller guide, §§4.6.4–4.6.6](https://pro.sony/s3/2023/11/15112624/Developer-Guide-for-3rd-Party-Lens-Controller_V1.00.pdf#page=19) ·
[FR7 VISCA command list, p. 16](https://pro.sony/s3/2022/09/03065933/VISCA_Command_List_v4.pdf#page=16)

Targeted `settled*` waits return `Settlement` evidence. An unfinished
position proof returns `SettlementSuperseded` if overlapping motion is
admitted later, even if that motion is cancelled before writing. It also
returns that error if an older, overlapping request may have reached the
camera after the operation's own write. This can happen when a retry writes
after a newer request. Established cached evidence and exact profile
completion remain valid.

`applied_with_timeout` and `cancel_with_timeout` use the owner's delivery
time. A result delivered exactly at the deadline counts even if the caller
resumes later. A terminal delivered after the deadline remains on the handle.
The expired wait returns `ObservationTimeout`, and a fresh wait can observe
the result.

A motion observation reads one position snapshot, waits until at least
`MotionQuery::window()` (default 100 ms, set with `with_window`) has elapsed on
the owner clock, and reads a second. `true` means a selected axis moved by more
than its tolerance across that window. `false` means no movement was detected
over the window: an axis creeping slowly enough to stay within tolerance, or
returning to its start within the window, is not detected. A longer window
detects slower movement but makes the call take longer. A zero window is
rejected with `Error::InvalidParameter` before any inquiry, and a window that
cannot elapse within the observation deadline returns `Error::Timeout`, never
`false`. Motion observation, idle waits, and targeted settlement validate lens
samples against the registered profile's readback range or position table.
An impossible reply fails the observation before it can count toward evidence
of stability.

`MotionQuery` uses `axes()`, `tolerance()` and `window()` getters. Use its
`with_*` methods to change the tolerance or window. A zero window is also
refused during deserialization. `MovementTolerance` uses unsigned magnitudes
with per-axis getters and `with_*` methods.

`IdleWait` uses private fields with `axes()`, `timeout()`, `tolerance()` and
`interval()` getters and the corresponding `with_*` methods. Its sampling
interval must be positive: a zero interval returns `Error::InvalidParameter`
before any inquiry and is refused when deserializing a policy. `settled()`,
`wait_until_idle` and `is_moving` accept a position reply delivered by the
owner exactly at the observation deadline. A caller that resumes later still
uses that timely evidence; replies delivered after the deadline time out.

Async noun futures borrow the temporary accessor. A direct call such as
`camera.motion().stop_all().await` is fine; bind each accessor before a
macro retains several futures:

```rust,ignore
let sony_motion = sony.motion();
let raw_motion = raw.motion();
let (sony_result, raw_result) = tokio::join!(
    sony_motion.stop_all(),
    raw_motion.stop_all(),
);
sony_result?.into_result()?;
raw_result?.into_result()?;
```

Runtime-selected cameras use the same concrete `Camera<AnyProfile>` and
`Operation<K>` types as compile-time profiles. Both facades validate domain
presence, optional permission, inventory and profile-selected encoding before
I/O. Discover optional permissions with `camera.capabilities().supports(surface)`.

```rust
#[cfg(feature = "async")]
async fn runtime_views(
    camera: &grafton_visca::r#async::Camera<grafton_visca::profiles::AnyProfile>,
    query: grafton_visca::types::MotionQuery,
) -> Result<(), grafton_visca::Error> {
    camera.zoom().stop().await?.applied().await?;
    camera.motion().is_moving(query).await?;
    Ok(())
}
```

Use the generic `execute` for a plain request, `inquire` for a typed inquiry,
and `submit` for a typed operation. Custom requests declare their class with
`PlainClass`, `InquiryClass`, or `OperationClass<K>`. An applied-only handle has no settled
state. Every wait borrows its handle and caches what it observes, so a timed-out
wait can be retried and `settled` can follow `applied`; `cancel` is one
idempotent intent that reports its owner outcome, and `detach` is the explicit
fire-and-forget choice. Dropping a handle is exactly `detach` and never stops
hardware, so an error path leaves movement running until something else ends
it; see the scoped stop-on-exit guard in
[`migration_2_0.md`](migration_2_0.md#drop-never-stops-hardware).

On the async facade, dropping an `execute`, `inquire`, `submit`, or
`motion().stop_all()` future before its owner admission prevents the request
from being written, including work queued behind another write or a
raw-correlation release gate. An abandoned halt leaves other motion eligible. The owner drops
the abandoned boundary and restores its admission capacity. Admission and the
reply happen in one synchronous owner turn: dropping after admission detaches
observation, even if the caller has not yet received the handle. It does not
cancel admitted hardware work.

## Optional ecosystem surfaces

`serde` provides serialization for supported public values and configuration;
`schemars` adds JSON Schema; `ts-rs` generates TypeScript declarations. These
features describe data interchange and do not change protocol semantics.
Persisted runtime profiles include every current fact key; an absent optional
fact is explicit `null`, and a missing key requires rebuilding and persisting
the current `ProfileSpec` (#855). Unknown keys are refused at every level,
including operational tuning, and the JSON Schema requires the same complete
set of nullable fact keys. Standalone `ProfileTiming` deserialization runs
the builder's validation (#875).

`value` and `value_enum` are the supported downstream value attributes;
`ViscaInquiry` remains the inquiry derive. `test-utils` alone exposes `testkit::{Step, helpers}` for
tests. Add `blocking` for `ScriptedBlockingTransport`; add `async` for
`ScriptedTransport`, `LocalExecutor` and `ManualClockExecutor`; either facade exposes
`ManualClock`, `ClockParticipant`, and `ClockJoinHandle`. Add
`runtime-tokio` for `ViscaCameraSimulator`. It is not required by production
applications.

See [`architecture_2_0.md`](architecture_2_0.md) for ownership/order
invariants and [`observability_and_recovery.md`](observability_and_recovery.md)
for metrics, diagnostics, state, and recovery.
