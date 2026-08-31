# grafton‑visca

[![Crates.io](https://img.shields.io/crates/v/grafton-visca.svg)](https://crates.io/crates/grafton-visca)
[![Documentation](https://docs.rs/grafton-visca/badge.svg)](https://docs.rs/grafton-visca)
[![License](https://img.shields.io/crates/l/grafton-visca.svg)](#license)
[![CI](https://github.com/GrantSparks/grafton-visca/actions/workflows/ci.yml/badge.svg)](https://github.com/GrantSparks/grafton-visca/actions/workflows/ci.yml)

A pure Rust library for controlling PTZ cameras via the VISCA protocol. Supports blocking and async APIs with built-in runtime adapters (Tokio and smol), TCP/UDP/serial transports, and type-safe camera profiles.

> **2.0.0-rc.1** — The prerelease owner-backed API is available for review. It
> keeps one protocol owner per session and exposes typed static, blocking, async,
> and dynamic views over that owner.

---

## 2.0.0-rc.1 Support Matrix

The 2.0 contract is owner-backed construction, mode-native sessions, typed
profile views, and one request path for each semantic class. Hardware validation
is narrower than software support: the library contract is checked in CI, while
device-specific firmware behavior is tracked in the
[hardware release checklist](docs/hardware_release_checklist.md).

The release candidate is intentionally a clean break from the pre-2.0 API. Use
`SessionConfig` for reusable target/profile registration, `Connect` or
`CameraConfig` for standard transports, and `Session::open` for caller-owned
transports.

### APIs and runtimes

| Area | Supported 2.0 contract | Automated validation |
| ---- | ---------------------- | -------------------- |
| Blocking API | `blocking::Session`, `blocking::Camera<P>`, and blocking noun views; native synchronous I/O with no async runtime/executor dependency | Blocking API contracts plus dependency-graph gate |
| Runtime-neutral async | `Session`, `Camera<P>`, and a caller-provided `Executor` | Async API contract tests |
| Tokio async | `runtime-tokio`, `TokioRuntime`, and Tokio TCP/UDP adapters | Tokio construction and noun suites |
| smol async | `runtime-smol`, `SmolRuntime`, and smol TCP/UDP adapters | smol construction and noun suites |
| Runtime coexistence | Tokio and smol may be enabled together; each session receives one explicit runtime | Coexistence API contract |
| Dynamic API | `dyn-api` owner-backed `DynSessionCamera` with runtime profile checks and the same operation lifecycle | Dynamic Tokio/smol suites |
| Request extension | Typed `Request`, `PlainCommand`, `Inquiry`, and `OperationCommand` contracts; `ResponseParser` handles custom inquiry decoding | Request/API contract tests |

### Transports

| Transport | Supported 2.0 contract | Automated validation |
| --------- | ---------------------- | -------------------- |
| TCP | Blocking, Tokio, and smol owner sessions through `Connect`/`CameraConfig` | Runtime matrix above |
| UDP | Blocking, Tokio, and smol owner sessions through `Connect`/`CameraConfig` | Runtime matrix above |
| Blocking serial | RS-232/422 through `transport-serial` and `blocking::Session` | Serial feature matrix |
| Tokio serial | Tokio serial through `transport-serial-tokio` and `Session` | Tokio serial feature matrix |
| Custom transports | Caller-owned `BlockingTransport`/`AsyncTransport` with `Session::open` | Custom transport contract tests |

### Profiles

| Profile family | Protocol | 2.0 support |
| -------------- | -------- | ----------- |
| `GenericVisca` | Raw VISCA | Supported baseline profile with conservative capabilities |
| `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X` | Raw VISCA | Supported by the software/profile registry; physical G2/G3 TCP and UDP validation remains pending — see the [hardware release checklist](docs/hardware_release_checklist.md) |
| `SonyEVIH100`, `SonyBRC300`, `NearusBRC300` | Raw VISCA | Supported through profile capability gates and protocol tests |
| `SonyBRCH900`, `SonyFR7` | Sony encapsulation | Supported through Sony encapsulation, profile capability gates, and protocol tests |

### Profile-Gated Vendor Controls

Typed vendor-specific controls are exposed only for profiles whose documented
model capabilities support them. Runtime capability metadata remains available for
discovery on every profile, and raw command escape hatches remain available for
custom integrations.

| Typed control surface | Profiles |
| --------------------- | -------- |
| ND filter controls and inquiries | `SonyFR7` |
| Variable speed mode controls | `SonyFR7` |
| Tally controls and inquiries | `SonyFR7`, `SonyBRCH900` |
| Direct menu controls | `SonyFR7` |
| PTZOptics anti-flicker control and inquiry | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X` |
| PTZOptics settings-save command | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X` |
| PTZOptics preset-recall speed control | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X` |
| Sony spotlight controls | `SonyFR7`, `SonyBRCH900` |
| Sony automatic slow-shutter controls | `SonyEVIH100`, `SonyBRC300` |
| PTZOptics multicast-streaming controls | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X` |
| PTZOptics NDI-quality control | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X` |
| Motion Sync controls and inquiries | Custom/evidenced profiles that explicitly implement `HasMotionSync`; no built-in profile is marked from the current specs |

### Profile-Gated Sub-Capabilities

Broad control families are also decomposed when model support differs. For
example, PTZOptics profiles still expose baseline zoom, exposure, and focus
controls, but do not expose typed VISCA digital zoom, one-push focus, or snap
focus methods.

Dynamic callers get the same marker-derived permission model through
`Capabilities::typed_support` and `Capabilities::supports_typed(...)`. Metadata
fields such as `has_digital_zoom` and `supports_direct_zoom` remain runtime
discovery facts; use typed support checks before calling optional dyn typed
operations.

| Typed control surface | Built-in profiles |
| --------------------- | ----------------- |
| Direct absolute zoom positioning | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7`, `SonyBRCH900`, `SonyEVIH100`, `SonyBRC300`, `NearusBRC300` |
| VISCA digital zoom toggle and optical-plus-digital positioning | `SonyFR7`, `SonyBRCH900` |
| Iris control, iris-priority mode, and iris inquiry | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7` |
| Standard one-push focus | No built-in profile currently marks this typed capability |
| PTZOptics snap focus | No built-in profile currently marks this typed capability |
| Focus lock | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X` |
| Push auto focus | `SonyFR7` |
| Focus zone control | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X` |
| Focus zone inquiry | `PtzOpticsG2`, `PtzOptics30X` |
| Auto focus sensitivity | `SonyFR7` |
| Focus near-limit inquiry | `SonyFR7`, `SonyBRCH900`, `SonyEVIH100`, `SonyBRC300`, `NearusBRC300`, `GenericVisca` |
| Backlight compensation | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7`, `SonyBRCH900`, `SonyEVIH100`, `SonyBRC300`, `NearusBRC300` |
| Wide dynamic range | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7`, `SonyBRCH900` |
| Exposure compensation | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7` |
| Exposure brightness control and inquiry | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X` |
| One-push white balance | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7`, `SonyBRCH900`, `SonyEVIH100`, `GenericVisca` |
| Auto-tracking white balance | `SonyFR7` |
| Auto white-balance sensitivity | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X` |
| Color temperature controls and inquiry | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyBRCH900`, `SonyEVIH100` |
| RGB gain controls and inquiries | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7` |
| RGB tuning controls and inquiries | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7`, `SonyBRCH900`, `SonyEVIH100` |
| Flip and mirror controls | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7`, `SonyBRCH900`, `SonyEVIH100` |
| Combined image flip mode | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X` |
| Contrast control and inquiry | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7`, `SonyBRCH900` |
| Sharpness control and inquiry | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7`, `SonyBRCH900` |
| Saturation control and inquiry | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7`, `SonyBRCH900`, `NearusBRC300` |
| Hue control and inquiry | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7` |
| Luminance control and inquiry | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X` |
| Gamma control and inquiry | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7`, `SonyBRCH900`, `SonyEVIH100` |
| Aggregate noise-reduction inquiry | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7`, `SonyBRCH900`, `SonyEVIH100` |
| 2D/3D noise reduction | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7`, `SonyBRCH900` |
| Picture effects | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X` |
| USB audio control and inquiry | `PtzOpticsG2`, `PtzOptics30X` |

The registry keeps model-specific iris ranges in runtime metadata where they
are useful for discovery, but that metadata alone does not grant the typed
`IrisControl` surface or a targeted settlement inquiry. Those APIs are enabled
only for the PTZOptics profiles covered by the shared VISCA reference and the
Sony FR7 entry with an exact registry-backed inquiry. Other Sony, EVI, Nearus,
and generic entries remain available through the raw command escape hatch until
model-specific evidence is added.

Contributors adding or changing profile capabilities should follow the
[Camera Profile Support Guide](docs/camera_profile_support.md) and the
[VISCA Protocol Reference](docs/visca_reference.md), which explain how protocol
evidence, runtime metadata, typed support markers, and the profile-first marker
matrix fit together.

### Optional features

| Feature | 2.0 support |
| ------- | ----------- |
| `blocking` | Canonical native blocking facade (enabled by default); it does not enable or link Tokio or another async runtime/executor |
| `async` | Canonical owner-backed async facade; may be enabled with `blocking` |
| `serde` | Stable serialization/deserialization for public value and configuration types |
| `schemars` | Stable JSON Schema generation for serde-backed public types |
| `ts-rs` | Stable TypeScript type generation for supported exported types |
| `dyn-api` | Owner-backed object-safe async views with applied/settled operation lifecycle, cancellation, and detach |
| `test-utils` | Exposes the stable `grafton_visca::testing` module. It enables no facade of its own, so what it exposes depends on the union: `testkit::{Step, helpers}` always, `ScriptedBlockingTransport` with `blocking`, `ScriptedTransport` and the deterministic executor with `async`, and `ViscaCameraSimulator` only with `runtime-tokio`. Neither `runtime-tokio` nor `blocking` exposes any of it on its own. |

### Feature-union checks

Each union below is a CI matrix leg, named by its job title in the feature
matrix of `.github/workflows/ci.yml`.

| Feature union | Scope | Automated validation |
| ------------- | ----- | -------------------- |
| `runtime-tokio,transport-serial` | Blocking serial plus Tokio async dependency coexistence; use `transport-serial-tokio` for Tokio serial. | `Tokio + blocking serial` matrix leg |
| `runtime-smol,dyn-api` | Dynamic API with smol and no test helpers. | `smol + dyn-api` matrix leg |
| `test-utils,blocking,runtime-tokio` | The shipped test toolkit driven through a real facade. `test-utils` alone enables neither `blocking` nor a runtime, so the scripted transports, deterministic executor, and camera simulator only *execute* under a union like this one. | `test-utils + blocking + Tokio` matrix leg |

---

## Features

- **Camera-first blocking/async API** — `Connect` for simple construction, `CameraConfig` for configured standard transports, and shared noun accessors across blocking and async
- **Multi-runtime support** — Pluggable adapters for Tokio and smol
- **Type-safe profiles** — Compile-time protocol selection (raw VISCA vs Sony encapsulation) with capability-based APIs
- **Flexible transports** — TCP, UDP, and serial (RS-232/422) with configurable timeouts, retries, and TCP keepalive
- **Ergonomic API** — One-line connection helpers, checked unit types (`Degrees` for typed pan/tilt input, `Percentage` and `UnitInterval` for checked value conversions), built-in inquiry conversions
- **Optional serialization** — Serde and JSON Schema support for public value and configuration types
- **Zero-allocation hot path** — Stack-allocated command buffers for standard VISCA commands

---

## Quick Start

The quickstart snippets are read-only. They connect to a camera, query state,
and close the session. Movement and configuration changes are shown in focused
examples that opt in to hardware changes explicitly.

Standard TCP and UDP sessions support one camera. `Connect::open_tcp_camera::<P>`
/ `open_udp_camera::<P>` return an owner-backed `CameraSession<P>`: the profile
is named once and bound at compile time, and `session.camera()` hands out the
typed `Camera` view that carries the noun accessors. `Connect::open_tcp` /
`open_udp` also open one camera, returning a `Session` whose view is selected
with `session.camera::<P>()`. Multi-target sessions require a raw-VISCA
serial-addressed or compatible custom transport, and select each view with
`session.camera_for::<P>(target)`.

Every Rust snippet in this README is compiled by the crate's own test suite, so
the `#[cfg(feature = "...")]` attributes below are load-bearing: they name the
Cargo feature a snippet needs.

### Blocking

```rust
use grafton_visca::blocking::Connect;
use grafton_visca::camera::profiles::PtzOpticsG2;

fn quick_start() -> Result<(), grafton_visca::Error> {
    let session = Connect::open_tcp_camera::<PtzOpticsG2>("192.168.0.110")?;
    let camera = session.camera();

    let power_is_on = camera.power().state()?;
    let zoom = camera.zoom().position()?;
    println!("Power: {}", if power_is_on { "on" } else { "off" });
    println!("Zoom position: 0x{:04X}", zoom.value());

    session.close()
}
```

### Async (Tokio)

Enable the `runtime-tokio` feature and call this from inside a Tokio runtime.

```rust
#[cfg(feature = "runtime-tokio")]
async fn quick_start() -> Result<(), grafton_visca::Error> {
    use grafton_visca::camera::profiles::PtzOpticsG2;
    use grafton_visca::runtime::TokioRuntime;
    use grafton_visca::Connect;

    let runtime = TokioRuntime::from_current()?;
    let session = Connect::open_tcp_camera::<PtzOpticsG2, _>("192.168.0.110", runtime).await?;
    let camera = session.camera();

    let power_is_on = camera.power().state().await?;
    let zoom = camera.zoom().position().await?;
    println!("Power: {}", if power_is_on { "on" } else { "off" });
    println!("Zoom position: 0x{:04X}", zoom.value());

    session.close().await
}
```

For smol, enable `runtime-smol` and use `SmolRuntime`.

---

## Bounded Movement Operations

Ordinary noun methods remain the simplest command-completion surface. When a
caller needs a per-command deadline, cancellation, or a physical settle signal,
submit a built-in movement command and drive its operation handle explicitly.

`submit` manages lifecycle; it does not add profile capability or range
validation beyond the command's own checks. Prefer typed noun controls for
profile-validated ergonomic input. Built-in commands provide exact completion
metadata. Custom operation requests must declare their targeted or applied-only
class explicitly.

```rust
use grafton_visca::blocking::Connect;
use grafton_visca::camera::profiles::PtzOpticsG2;
use grafton_visca::request::builtin::{PanTiltHome, ZoomStop};

fn move_home() -> Result<(), grafton_visca::Error> {
    let session = Connect::open_tcp_camera::<PtzOpticsG2>("192.168.0.110")?;
    let camera = session.camera();

    // Blocking submit performs initial synchronous dispatch before returning.
    camera.submit(&PanTiltHome)?.settled()?;

    // Applied-only commands have no meaningful settled state.
    camera.submit(&ZoomStop)?.applied()?;

    session.close()
}
```

The typed noun accessors return the same handles, so
`camera.pan_tilt().home()?.settled()?` and `camera.zoom().stop()?.applied()?`
are the profile-validated equivalents of the two `submit` calls above.

The async form has the same vocabulary and awaits submission and terminal
operations:

```rust
#[cfg(feature = "runtime-tokio")]
async fn move_home() -> Result<(), grafton_visca::Error> {
    use grafton_visca::camera::profiles::PtzOpticsG2;
    use grafton_visca::request::builtin::{PanTiltHome, ZoomStop};
    use grafton_visca::runtime::TokioRuntime;
    use grafton_visca::Connect;

    let runtime = TokioRuntime::from_current()?;
    let session = Connect::open_tcp_camera::<PtzOpticsG2, _>("192.168.0.110", runtime).await?;
    let camera = session.camera();

    let handle = camera.submit(&PanTiltHome).await?;
    handle.settled().await?;

    camera.submit(&ZoomStop).await?.applied().await?;

    session.close().await
}
```

- `applied` means the exact command was accepted and protocol-completed.
- `settled` additionally means targeted physical motion ended. Profiles
  with an operation-complete signal use it; other profiles poll only the affected
  axes under the same total deadline.
- `cancel` requests owner-owned, ID/socket-safe cancellation. Queued work is
  removable locally; sent work requires profile support and otherwise returns
  `Error::NotSupported`. Success does not prove physical motion stopped, so use
  a bounded STOP for continuous movement.
- Dropping a handle never stops hardware. Drop is `detach`: the submitted
  command remains owner-owned, may still be dispatched and complete, and
  physical movement continues, so an early `?` or a panic leaves the camera
  driving until something ends it. This matches 1.x and is not a 2.0 change.
  To bound movement by a scope, write a guard whose `Drop` submits the typed
  STOP — see the pattern in
  [`docs/migration_2_0.md`](docs/migration_2_0.md#drop-never-stops-hardware).
- `detach` is explicit fire-and-forget, the spelled-out form of what drop
  already does. `#[must_use]` warns only when a returned handle is ignored
  directly.

See the maintained [blocking](examples/operation_handles.rs) and
[Tokio](examples/operation_handles_async.rs) examples for complete programs that
close the session on every path and bound movement with a stop-on-exit guard.

### Checked normalized input

`UnitInterval` is the checked `0.0..=1.0` value the zoom noun accepts directly.
`set_normalized` always maps across the profile's documented optical range;
`set_normalized_in_domain` selects the domain explicitly and refuses
`OpticalPlusDigital` on a profile with no documented digital maximum.

```rust
use grafton_visca::blocking::Connect;
use grafton_visca::camera::profiles::{PtzOpticsG2, SonyFR7};
use grafton_visca::units::UnitInterval;
use grafton_visca::ZoomDomain;

fn half_zoom() -> Result<(), grafton_visca::Error> {
    let session = Connect::open_tcp::<PtzOpticsG2>("192.168.0.110")?;
    let camera = session.camera::<PtzOpticsG2>()?;

    camera
        .zoom()
        .set_normalized(UnitInterval::new(0.5)?)?
        .settled()?;

    session.close()
}

// `set_normalized_in_domain` requires `HasDirectZoom`; only
// `OpticalPlusDigital` additionally requires `HasDigitalZoomRange` at runtime.
fn full_range_zoom() -> Result<(), grafton_visca::Error> {
    let session = Connect::open_udp::<SonyFR7>("192.168.0.110")?;
    let camera = session.camera::<SonyFR7>()?;

    camera
        .zoom()
        .set_normalized_in_domain(UnitInterval::ONE, ZoomDomain::OpticalPlusDigital)?
        .settled()?;

    session.close()
}
```

---

## Public API Boundaries

- Use `Connect`, `CameraConfig`, and `SessionConfig` for construction. Standard
  TCP and UDP sessions have one target: `Connect::open_tcp_camera::<P>` /
  `open_udp_camera::<P>` (or `CameraConfig::<P>::open_camera` /
  `open_camera_async`) return a `CameraSession<P>` whose `camera()` view is
  bound to `P` at compile time, while `Connect::open_tcp` / `open_udp` return a
  one-target `Session` accessed with `Session::camera::<P>()`. Multi-target
  `Session::camera_for::<P>(target)` is for raw-VISCA serial-addressed or
  compatible custom transports.
- Use the 14 inherent noun accessors on `Camera<P>`; profile-gated methods are
  checked by `Has*` marker bounds and runtime `ProfileSpec` validation.
- Use `UnitInterval::new(value)?` or `UnitInterval::try_from(value)?` for the
  checked `0.0..=1.0` values taken by `camera.zoom().set_normalized(..)` and
  `set_normalized_in_domain(..)`, and by the inquiry conversion helpers such as
  `ZoomPositionExt::normalize_with_max` and `zoom_from_normalized`. Other typed
  control input uses `Degrees`, `SpeedLevel`, and the profile-checked `types`
  values.
- Use `CameraId` with `camera_id(...)`, or `try_camera_id(u8)` when converting
  a raw VISCA camera number from configuration.

---

## Installation

```toml
[dependencies]
grafton-visca = "=2.0.0-rc.1"
```

### Common configurations

```toml
# Runtime-agnostic async (bring your own executor)
grafton-visca = { version = "=2.0.0-rc.1", features = ["async"] }

# Async with Tokio
grafton-visca = { version = "=2.0.0-rc.1", features = ["runtime-tokio"] }
tokio = { version = "1", features = ["full"] }

# With serialization
grafton-visca = { version = "=2.0.0-rc.1", features = ["serde"] }

# Serial transport (blocking)
grafton-visca = { version = "=2.0.0-rc.1", features = ["transport-serial"] }

# Serial transport (Tokio)
grafton-visca = { version = "=2.0.0-rc.1", features = ["runtime-tokio", "transport-serial-tokio"] }
```

### Configuring Standard Transport Behavior

```rust
use grafton_visca::camera::CameraConfig;
use grafton_visca::profiles::PtzOpticsG2;
use grafton_visca::transport::TransportConfig;
use grafton_visca::CameraId;
use std::time::Duration;

let config = CameraConfig::<PtzOpticsG2>::tcp("192.168.0.110")
    .camera_id(CameraId::CAMERA_1)
    .transport_config(TransportConfig {
        tcp_keepalive: Some(grafton_visca::transport::TcpKeepaliveConfig::new(
            Duration::from_secs(30),
        )),
        ..TransportConfig::default()
    });
```

TCP keepalive is a socket-level liveness mechanism. It can detect broken peers
and may keep idle network-path state active, but it does not send VISCA commands
or guarantee that camera firmware will retain an idle application session. When
an operation fails, ask `Error::requires_new_session()`: it reports `true` for
terminal connection failures such as `ConnectionClosed` and `StreamPoisoned`; it
reports `false` for a `RuntimeShutdown` this application requested and for the raw
ambiguity error `UnsequencedCommandUnconfirmed`, which since #671 fails only that
one command while the session keeps running (opt into
`OperationalTuning::strict_unconfirmed_poison` for the old whole-session poison,
surfaced as `StreamPoisoned`). On `true`, discard that session
and establish a replacement from the retained configuration. Its state cache
starts `Unknown`, so re-query and reconcile camera state before any deliberate
resubmission. Never blindly replay an operation whose completion is uncertain.

### Feature flags

| Feature                | Enables                                    |
| ---------------------- | ------------------------------------------ |
| `blocking`             | Canonical owner-backed blocking facade (default) |
| `async`                | Canonical owner-backed async facade        |
| `runtime-tokio`        | Tokio async runtime adapter                |
| `runtime-smol`         | smol runtime adapter                       |
| `transport-serial`     | Blocking serial (RS-232/422)               |
| `transport-serial-tokio` | Async serial (Tokio)                     |
| `serde`                | Serialize/deserialize public value and configuration types |
| `schemars`             | JSON Schema generation                     |
| `ts-rs`                | TypeScript type generation                 |
| `dyn-api`              | Object-safe async camera traits            |

---

## Camera Profiles

Profiles define protocol format, supported standard transports, and default ports:

| Profile              | Protocol           | TCP Port | UDP Port |
| -------------------- | ------------------ | -------: | -------: |
| `GenericVisca`       | Raw VISCA          |     5678 |     1259 |
| `PtzOpticsG2/G3/30X` | Raw VISCA          |     5678 |     1259 |
| `SonyEVIH100`       | Raw VISCA          |     5678 |     1259 |
| `SonyBRC300`         | Raw VISCA          |     5678 |     1259 |
| `NearusBRC300`       | Raw VISCA          |     5678 |     1259 |
| `SonyBRCH900`        | Sony encapsulation |      n/a |    52381 |
| `SonyFR7`            | Sony encapsulation |      n/a |    52381 |

Port can be omitted for supported network transports; the profile default for
that transport is used. Unsupported profile/transport pairs are rejected by the
typed constructors at compile time and by deserialized/runtime configurations
before any socket or serial device is opened.

---

## Documentation

- **[API Reference](https://docs.rs/grafton-visca)** — Complete type and method documentation
- **[Examples](examples/)** — Maintained examples for common scenarios
- **[2.0 Example Policy](docs/examples.md)** — Example contract and maintenance rules
- **[2.0 Usage and Construction](docs/usage_2_0.md)** — Sessions, profiles, transports, and noun views
- **[2.0 Architecture](docs/architecture_2_0.md)** — Ownership, target registration, and lifecycle invariants
- **[Preservation Inventory](docs/architecture_inventory.md)** — Closed profile, transport, noun, and extension inventory
- **[Request Semantics](docs/request_semantics.md)** — Plain, inquiry, targeted, and applied-only request classes
- **[Allocation and Validation](docs/allocation_and_validation.md)** — Release gates for bounded work and preflight validation
- **[Observability and Recovery](docs/observability_and_recovery.md)** — Metrics, diagnostics, cache, and recovery behavior
- **[Hardware Release Checklist](docs/hardware_release_checklist.md)** — Assigned physical validation rows and evidence status
- **[2.0 Migration Guide](docs/migration_2_0.md)** — Destination APIs for pre-2.0 applications
- **[VISCA Protocol Reference](docs/visca_reference.md)** — Consolidated PTZOptics, Axis, and protocol evidence used for built-in profile decisions
- **[Camera Profile Support Guide](docs/camera_profile_support.md)** — Workflow and testing rules for adding camera capabilities
- **[CHANGELOG](CHANGELOG.md)** — Version history and migration guides

### Examples

| Example | Command |
| ------- | ------- |
| Blocking quickstart | `cargo run --example quickstart -- 192.168.0.110` |
| Blocking quickstart with movement | `cargo run --example quickstart -- 192.168.0.110 --move` |
| Async quickstart (Tokio) | `cargo run --example quickstart_async --features runtime-tokio -- 192.168.0.110` |
| Blocking operation handles | `cargo run --example operation_handles -- 192.168.0.110` |
| Async operation handles (Tokio) | `cargo run --example operation_handles_async --features runtime-tokio -- 192.168.0.110` |
| Inquiry quickstart | `cargo run --example inquiry_quickstart -- 192.168.0.110` |
| Configured transport policy | `cargo run --example transport_builder_demo -- 192.168.0.110` |
| Caller-owned transport | `cargo run --example builder_api -- 192.168.0.110:1259` |
| Preset recall | `cargo run --example preset_demo -- 192.168.0.110 recall 1` |
| Error handling | `cargo run --example error_handling --features runtime-tokio -- 192.168.0.110` |
| Serial (async) | `cargo run --example serial_async_demo --features runtime-tokio,transport-serial-tokio -- /dev/ttyUSB0 1` |

---

## Platform and MSRV

- **MSRV:** Rust 1.88
- **Platforms:** Linux, macOS, Windows

---

## Contributing

Contributions welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

---

## License

Dual-licensed under [Apache 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
