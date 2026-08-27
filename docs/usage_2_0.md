# 2.0 usage and construction

The 2.0 API has two static calling conventions and one erased async
convention. All three end at the same owner-backed session model.

## Feature selection

| Need | Cargo features |
| --- | --- |
| Blocking only | `blocking` (the default feature) |
| Runtime-neutral async | `async` and a caller-owned `Executor` |
| Tokio async | `runtime-tokio` |
| smol async | `runtime-smol` |
| Blocking serial | `transport-serial` |
| Tokio serial | `runtime-tokio,transport-serial-tokio` |
| Dynamic async views | `dyn-api` plus `runtime-tokio` or `runtime-smol` |
| Serialization/schema/typescript | `serde`, `schemars`, and `ts-rs` as needed |
| Deterministic test transports | `test-utils` |

Runtime features imply the canonical `async` facade. `runtime-tokio`
and `runtime-smol` may be enabled together; each session still receives one
explicit runtime.

## One standard connection

Use `Connect` for a short, standard connection. The type parameter is the
compile-time profile, so transport support and optional control gates are
checked before opening the connection.

### Blocking

```rust,ignore
use grafton_visca::blocking::{CameraSession, Connect};
use grafton_visca::camera::profiles::PtzOpticsG2;

let session: CameraSession<PtzOpticsG2> =
    Connect::open_tcp_camera::<PtzOpticsG2>("192.168.0.110")?;
let camera = session.camera();
camera.power().on()?;
camera.zoom().stop()?.applied()?;
session.close()?;
# Ok::<(), grafton_visca::Error>(())
```

Use `Connect::open_udp_camera` for UDP. With `transport-serial`,
`Connect::open_serial::<P>(port, baud_rate)` opens the serial path and returns
a `Session`; select its view with `session.camera::<P>()`.

### Async

```rust,ignore
use grafton_visca::{CameraSession, Connect};
use grafton_visca::camera::profiles::PtzOpticsG2;
use grafton_visca::runtime::TokioRuntime;

let runtime = TokioRuntime::from_current()?;
let session: CameraSession<PtzOpticsG2> =
    Connect::open_tcp_camera::<PtzOpticsG2, _>("192.168.0.110", runtime).await?;
let camera = session.camera();
camera.power().on().await?;
camera.zoom().stop().await?.applied().await?;
session.close().await?;
# Ok::<(), grafton_visca::Error>(())
```

Use `runtime-smol` with `SmolRuntime`, or use `Connect::open_udp_camera` for
UDP. `Connect` and `CameraConfig` perform the same preflight; the `_camera`
constructors return the single-camera `CameraSession<P>`, and `open_tcp` /
`open_udp` return the multi-target owner-backed `Session`.

## Reusable configuration and target selection

`CameraConfig<P>` is convenient profile-typed standard-transport data. It can
set an address, `CameraId`, timeout policy, retry policy, transport options,
and buffer/keepalive settings before calling `open`, `open_async`, or the
serial-specific open method. `open_camera` and `open_camera_async` are the
single-camera forms of `open` and `open_async`, and `CameraSession::open` takes
a caller-owned transport with the same bind. `session_config()` lowers that pure
configuration into the shared mode-independent `SessionConfig` without DNS,
socket, serial, executor, or protocol work.

`SessionConfig` is the reusable multi-target boundary:

```rust,ignore
use grafton_visca::{ProfileSpec, SessionConfig};
use grafton_visca::camera::profiles::{GenericVisca, PtzOpticsG2};

let g2 = ProfileSpec::from_compile_time::<PtzOpticsG2>()?;
let generic = ProfileSpec::from_compile_time::<GenericVisca>()?;
let mut config = SessionConfig::new(g2);
config.register_target(grafton_visca::CameraId::new(2)?, generic)?;

// Apply one session-wide tuning value before opening.
let config = config.with_tuning(
    grafton_visca::OperationalTuning::new()
        .inquiry_spacing(std::time::Duration::from_millis(50)),
)?;
# Ok::<(), grafton_visca::Error>(())
```

The equivalent builder form is `SessionConfig::for_target(...).with_target(...)`.
`SessionConfig::from_compile_time::<P>()` creates a reusable camera-1 config
from a static profile. `register_target` and `with_target` accept only IDs 1
through 7, reject broadcast and duplicate IDs, and cap the registry at seven
targets. Registration and tuning are immutable after the session starts.

For one target, prefer the single-camera constructors: they name the profile
once and return a `CameraSession<P>` whose `camera()` is bound to that same `P`
at compile time, so there is no second profile naming for a mismatch to fail
on and no projection error to handle.

`session.camera::<P>()` remains the concise view selector for a `Session` that
was opened with the multi-target constructors. For two or more targets it
intentionally returns an error; use `session.camera_for::<P>(target)`.
`camera` and `camera_for` check the complete registered `ProfileSpec` at run
time, not only a profile name, because a `Session` carries no compile-time
profile. The dynamic equivalent is
`DynSessionCamera::from_session_target(&session, target)`.

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
response attribution. A custom transport is not a way to bypass profile,
addressing, pacing, or bounded-owner validation.

## Operational tuning

Use `OperationalTuning` with `SessionConfig::with_tuning` for a reusable
session-wide policy. Available overrides cover command and inquiry spacing,
command-socket capacity, acknowledgement/completion/settlement/inquiry
deadlines, retry count, and retry backoff/budget.

Tuning can make the owner more conservative but cannot weaken a profile's
minimum pacing, raise its socket limit, or use zero timeouts. When targets have
different profile minima, the strictest applicable pacing and capacity policy
wins. A settlement timeout is separate from protocol completion: a targeted
operation may be acknowledged before the camera physically settles.

## Caller-owned transports and executors

Use `Session::open` when the application already owns a transport or needs a
custom executor. The async path is:

```rust,ignore
let session = grafton_visca::Session::open(
    custom_async_transport,
    grafton_visca::SessionConfig::from_compile_time::<PtzOpticsG2>()?,
    executor,
)
.await?;
# Ok::<(), grafton_visca::Error>(())
```

The blocking counterpart is `grafton_visca::blocking::Session::open(transport,
config)`. `CameraSession::open(transport, &CameraConfig::<P>::new())` — with the
executor as a third argument on the async side — is the single-camera form of
the same boundary, taking the profile from `P` instead of a runtime
`SessionConfig`. Implementors must
declare `AsyncTransport` or `BlockingTransport`, `HasTransportConfig`, and the
correct stream/datagram send semantics. A runtime-neutral async executor must
provide coherent spawn, sleep, timeout, and clock behavior from one runtime.

## Static, dynamic, and generic operations

Static cameras expose the same 14 noun views in blocking and async forms:
`power`, `zoom`, `system`, `pan_tilt`, `focus`, `exposure`, `white_balance`,
`image`, `presets`, `tally`, `nd_filter`, `motion_sync`, `menu`, and
`advanced`. Profile-gated methods are available only when the profile's
`Has*` marker permits them. `motion()` separately owns
`stop_all_motion`, `is_moving`, and `wait_until_idle`.

Dynamic async code uses `DynSessionCamera` and its object-safe
`DynSessionCameraControl` plus `DynPower`, `DynZoom`, `DynSystem`,
`DynPanTilt`, `DynFocus`, `DynExposure`, `DynWhiteBalance`, `DynImage`,
`DynPresets`, `DynTally`, `DynNdFilter`, `DynMotionSync`, `DynMenu`,
`DynAdvanced`, and `DynMotion` traits:

```rust,ignore
let camera = grafton_visca::dynapi::DynSessionCamera::from_session(&session)?;
camera.zoom().stop().await?.applied().await?;
camera.motion().is_moving(query).await?;
# Ok::<(), grafton_visca::Error>(())
```

Use the generic `execute` for a plain request, `inquire` for a typed inquiry,
and `submit` for a typed operation. Dynamic custom requests use the explicit
targeted/applied-only request traits. An applied-only handle has no settled
state; cancellation reports its owner outcome, and `detach` is the explicit
fire-and-forget choice. Dropping a movement handle that was never resolved
enqueues the typed STOP for the axes it affects, so an error path cannot leave
hardware moving; `detach` opts out of that.

## Optional ecosystem surfaces

`serde` provides serialization for supported public values and configuration;
`schemars` adds JSON Schema; `ts-rs` generates TypeScript declarations. These
features describe data interchange and do not change protocol semantics.
`ViscaInquiry`, `ViscaEnum`, and `ViscaValue` are the supported downstream
derive entry points. `test-utils` exposes deterministic transports, clocks,
and executors for tests; it is not required by production applications.

See [`architecture_2_0.md`](architecture_2_0.md) for ownership/order
invariants and [`observability_and_recovery.md`](observability_and_recovery.md)
for metrics, diagnostics, state, and recovery.
