# grafton‑visca

[![Crates.io](https://img.shields.io/crates/v/grafton-visca.svg)](https://crates.io/crates/grafton-visca)
[![Documentation](https://docs.rs/grafton-visca/badge.svg)](https://docs.rs/grafton-visca)
[![License](https://img.shields.io/crates/l/grafton-visca.svg)](LICENSE)
[![CI](https://github.com/GrantSparks/grafton-visca/actions/workflows/ci.yml/badge.svg)](https://github.com/GrantSparks/grafton-visca/actions/workflows/ci.yml)

A pure Rust library for controlling PTZ cameras via the VISCA protocol. Supports blocking and async APIs with built-in runtime adapters (Tokio and smol), TCP/UDP/serial transports, and type-safe camera profiles.

---

## 1.0 Support Matrix

The 1.0 contract is the camera-first API, the documented transport/runtime
configuration types, the root-level raw command extension surface, and the
optional feature surfaces listed below. Hardware validation is narrower than
software support: the library contract is tested automatically, while
device-specific firmware quirks are handled as reproducible bugs.

The rows below are the support promise. CI also includes compatibility checks
for feature unions that can appear in downstream dependency graphs; those checks
keep combinations buildable without expanding the public contract beyond the
documented rows they combine.

### APIs and runtimes

| Area | Supported 1.0 contract | Automated validation |
| ---- | ---------------------- | -------------------- |
| Blocking API | Baseline build when `mode-async` is not enabled | `cargo test --no-default-features` |
| Runtime-agnostic async | `mode-async` with caller-provided executor/runtime | `cargo test --no-default-features --features mode-async` |
| Tokio async | `runtime-tokio`, `TokioRuntime`, Tokio TCP/UDP adapters | `cargo test --no-default-features --features runtime-tokio` |
| smol async | `runtime-smol`, `SmolRuntime`, smol TCP/UDP adapters | `cargo test --no-default-features --features runtime-smol` |
| Runtime coexistence | `runtime-tokio` and `runtime-smol` may be enabled together; camera construction still chooses one runtime explicitly | `cargo check --no-default-features --features runtime-tokio,runtime-smol` |
| Dynamic API | `dyn-api` object-safe camera traits for async cameras, with runtime capabilities, command-completion timeouts, and cancellable in-flight handles | `cargo test --no-default-features --features runtime-tokio,dyn-api,test-utils --test dyn_api_integration_test`, `cargo test --no-default-features --features runtime-smol,dyn-api,test-utils --test dyn_api_smol_integration_test` |
| Raw command extension | Custom command and inquiry implementations through `grafton_visca::command::{ViscaCommand, CommandKind, InquiryKind, ResponseParser}` plus root command value re-exports | API contract tests |

### Transports

| Transport | Supported 1.0 contract | Automated validation |
| --------- | ---------------------- | -------------------- |
| TCP | Blocking, Tokio, and smol camera connections through `Connect`, `CameraConfig`, and transport config types | Runtime matrix above |
| UDP | Blocking, Tokio, and smol camera connections through `Connect`, `CameraConfig`, and transport config types | Runtime matrix above |
| Blocking serial | RS-232/422 configuration and blocking serial transport APIs behind `transport-serial` | `cargo test --no-default-features --features transport-serial` |
| Tokio serial | Async serial transport APIs behind `transport-serial-tokio` | `cargo test --no-default-features --features runtime-tokio,transport-serial-tokio` |
| Custom transports | Public transport traits and handles documented under `grafton_visca::transport` | API contract tests |

### Profiles

| Profile family | Protocol | 1.0 support |
| -------------- | -------- | ----------- |
| `GenericVisca` | Raw VISCA | Supported baseline profile with conservative capabilities |
| `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X` | Raw VISCA | Supported; G2/G3 TCP and UDP behavior is hardware-validated |
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
| Motion Sync controls and inquiries | Custom/evidenced profiles that explicitly implement `HasMotionSync`; no built-in profile is marked from the current specs |

### Profile-Gated Sub-Capabilities

Broad control families are also decomposed when model support differs. For
example, PTZOptics profiles still expose baseline zoom, exposure, and focus
controls, but do not expose typed VISCA digital zoom, one-push focus, or snap
focus methods.

| Typed control surface | Built-in profiles |
| --------------------- | ----------------- |
| Direct absolute zoom positioning | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7`, `SonyBRCH900`, `SonyEVIH100`, `SonyBRC300`, `NearusBRC300` |
| VISCA digital zoom toggle and optical-plus-digital positioning | `SonyFR7`, `SonyBRCH900` |
| Iris control, iris-priority mode, and iris inquiry | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7`, `SonyBRCH900`, `SonyEVIH100`, `SonyBRC300`, `NearusBRC300`, `GenericVisca` |
| Standard one-push focus | No built-in profile currently marks this typed capability |
| PTZOptics snap focus | No built-in profile currently marks this typed capability |
| Focus lock | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X` |
| Push auto focus | `SonyFR7` |
| Focus zone | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7` |
| Auto focus sensitivity | `SonyFR7` |
| Focus near-limit inquiry | `SonyFR7`, `SonyBRCH900`, `SonyEVIH100`, `SonyBRC300`, `NearusBRC300`, `GenericVisca` |
| Backlight compensation | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7`, `SonyBRCH900`, `SonyEVIH100`, `SonyBRC300`, `NearusBRC300` |
| Wide dynamic range | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7`, `SonyBRCH900` |
| Exposure compensation | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7` |
| Exposure brightness control and inquiry | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7`, `SonyBRCH900` |
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
| Picture effects | `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7`, `SonyBRCH900` |

Contributors adding or changing profile capabilities should follow the
[Camera Profile Support Guide](docs/camera_profile_support.md) and the
[VISCA Protocol Reference](docs/visca_reference.md), which explain how protocol
evidence, runtime metadata, typed support markers, and the profile-first marker
matrix fit together.

### Optional features

| Feature | 1.0 support |
| ------- | ----------- |
| `serde` | Stable serialization/deserialization for public value and configuration types |
| `schemars` | Stable JSON Schema generation for serde-backed public types |
| `ts-rs` | Stable TypeScript type generation for supported exported types |
| `dyn-api` | Stable object-safe async camera traits with command timeout and cancellation support |
| `test-utils` | Stable deterministic test transports, executors, and Tokio camera simulator under `grafton_visca::testing`; `runtime-tokio` alone does not expose test helpers |

### Compatibility-only checks

| Feature union | Scope | Automated validation |
| ------------- | ----- | -------------------- |
| `runtime-tokio,transport-serial` | Dependency-graph compatibility only. The supported serial APIs are the blocking `transport-serial` row and the Tokio `transport-serial-tokio` row above. | `cargo test --no-default-features --features runtime-tokio,transport-serial` |
| `runtime-smol,dyn-api` | Build/API compatibility for dyn-api with smol without test helpers. Runtime behavior is covered by the smol dyn-api integration row above. | `cargo test --no-default-features --features runtime-smol,dyn-api` |

---

## Features

- **Camera-first blocking/async API** — `Connect` for simple construction, `CameraConfig` for configured standard transports, and shared noun accessors across blocking and async
- **Multi-runtime support** — Pluggable adapters for Tokio and smol
- **Type-safe profiles** — Compile-time protocol selection (raw VISCA vs Sony encapsulation) with capability-based APIs
- **Flexible transports** — TCP, UDP, and serial (RS-232/422) with configurable timeouts, retries, and TCP keepalive
- **Ergonomic API** — One-line connection helpers, intuitive unit types (`Degrees`, `Percentage`), built-in inquiry conversions
- **Optional serialization** — Serde and JSON Schema support for all types
- **Zero-allocation hot path** — Stack-allocated command buffers for standard VISCA commands

---

## Quick Start

The quickstart snippets are read-only. They connect to a camera, query state,
and close the session. Movement and configuration changes are shown in focused
examples that opt in to hardware changes explicitly.

### Blocking

```rust
use grafton_visca::camera::Connect;
use grafton_visca::profiles::PtzOpticsG2;

fn main() -> Result<(), grafton_visca::Error> {
    let cam = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110")?;

    let power_is_on = cam.power().state()?;
    let zoom = cam.zoom().position()?;
    println!("Power: {}", if power_is_on { "on" } else { "off" });
    println!("Zoom position: 0x{:04X}", zoom.value());

    cam.close()
}
```

### Async (Tokio)

```rust
use grafton_visca::camera::Connect;
use grafton_visca::profiles::PtzOpticsG2;
use grafton_visca::runtime::TokioRuntime;

#[tokio::main]
async fn main() -> Result<(), grafton_visca::Error> {
    let runtime = TokioRuntime::from_current()?;
    let cam = Connect::open_tcp_async::<PtzOpticsG2, _>("192.168.0.110", runtime).await?;

    let power_is_on = cam.power().state().await?;
    let zoom = cam.zoom().position().await?;
    println!("Power: {}", if power_is_on { "on" } else { "off" });
    println!("Zoom position: 0x{:04X}", zoom.value());

    cam.close().await
}
```

For smol, enable `runtime-smol` and use `SmolRuntime`.

---

## Installation

```toml
[dependencies]
grafton-visca = "1"
```

### Common configurations

```toml
# Runtime-agnostic async (bring your own executor)
grafton-visca = { version = "1", features = ["mode-async"] }

# Async with Tokio
grafton-visca = { version = "1", features = ["runtime-tokio"] }
tokio = { version = "1", features = ["full"] }

# With serialization
grafton-visca = { version = "1", features = ["serde"] }

# Serial transport (blocking)
grafton-visca = { version = "1", features = ["transport-serial"] }

# Serial transport (Tokio)
grafton-visca = { version = "1", features = ["runtime-tokio", "transport-serial-tokio"] }
```

### Configuring Standard Transport Behavior

```rust
use grafton_visca::camera::CameraConfig;
use grafton_visca::profiles::PtzOpticsG2;
use grafton_visca::transport::TransportConfig;
use std::time::Duration;

let config = CameraConfig::<PtzOpticsG2>::new()
    .tcp()
    .address("192.168.0.110")
    .transport_config(TransportConfig {
        tcp_keepalive: Some(grafton_visca::transport::TcpKeepaliveConfig::new(Duration::from_secs(30))),
        ..TransportConfig::default()
    });
```

### Feature flags

| Feature                | Enables                                    |
| ---------------------- | ------------------------------------------ |
| `runtime-tokio`        | Tokio async runtime adapter                |
| `runtime-smol`         | smol runtime adapter                       |
| `transport-serial`     | Blocking serial (RS-232/422)               |
| `transport-serial-tokio` | Async serial (Tokio)                     |
| `serde`                | Serialize/Deserialize for all types        |
| `schemars`             | JSON Schema generation                     |
| `ts-rs`                | TypeScript type generation                 |
| `dyn-api`              | Object-safe async camera traits            |

---

## Camera Profiles

Profiles define protocol format and default ports:

| Profile              | Protocol           | TCP Port | UDP Port |
| -------------------- | ------------------ | -------: | -------: |
| `GenericVisca`       | Raw VISCA          |     5678 |     1259 |
| `PtzOpticsG2/G3/30X` | Raw VISCA          |     5678 |     1259 |
| `SonyEVIH100`       | Raw VISCA          |     5678 |     1259 |
| `SonyBRC300`         | Raw VISCA          |     5678 |     1259 |
| `NearusBRC300`       | Raw VISCA          |     5678 |     1259 |
| `SonyBRCH900`        | Sony encapsulation |      n/a |    52381 |
| `SonyFR7`            | Sony encapsulation |      n/a |    52381 |

Port can be omitted in connection strings; the profile default is used.

---

## Documentation

- **[API Reference](https://docs.rs/grafton-visca)** — Complete type and method documentation
- **[Examples](examples/)** — Maintained examples for common scenarios
- **[Example Policy](docs/examples.md)** — 1.0 examples contract and maintenance rules
- **[VISCA Protocol Reference](docs/visca_reference.md)** — Consolidated PTZOptics, Axis, and protocol evidence used for built-in profile decisions
- **[Camera Profile Support Guide](docs/camera_profile_support.md)** — Workflow and testing rules for adding camera capabilities
- **[CHANGELOG](CHANGELOG.md)** — Version history and migration guides

### Examples

| Example | Command |
| ------- | ------- |
| Blocking quickstart | `cargo run --example quickstart -- 192.168.0.110` |
| Blocking quickstart with movement | `cargo run --example quickstart -- 192.168.0.110 --move` |
| Async quickstart (Tokio) | `cargo run --example quickstart_async --features runtime-tokio -- 192.168.0.110` |
| Inquiry quickstart | `cargo run --example inquiry_quickstart` |
| Configured transport policy | `cargo run --example transport_builder_demo -- 192.168.0.110` |
| Caller-owned transport | `cargo run --example builder_api -- 192.168.0.110:1259` |
| Preset recall | `cargo run --example preset_demo -- 192.168.0.110 recall 1` |
| Error handling | `cargo run --example error_handling --features runtime-tokio` |
| Serial (async) | `cargo run --example serial_async_demo --features runtime-tokio,transport-serial-tokio` |

---

## Compatibility

- **MSRV:** Rust 1.88
- **Platforms:** Linux, macOS, Windows

---

## Contributing

Contributions welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

---

## License

Dual-licensed under [Apache 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
