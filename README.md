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
| Blocking API | Default build with no enabled features, plus `mode-blocking` for cfg-based downstream detection | `cargo test --no-default-features`, `cargo test --no-default-features --features mode-blocking` |
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

### Blocking

```rust
use grafton_visca::camera::Connect;
use grafton_visca::profiles::PtzOpticsG2;

fn main() -> Result<(), grafton_visca::Error> {
    let cam = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110")?;

    cam.power().on()?;
    cam.pan_tilt().home()?;

    let pos = cam.pan_tilt().position()?;
    let (pan, tilt) = pos.as_degrees();
    println!("Position: {:.1}°, {:.1}°", pan.0, tilt.0);

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

    cam.power().on().await?;
    cam.pan_tilt().home().await?;

    let pos = cam.pan_tilt().position().await?;
    let (pan, tilt) = pos.as_degrees();
    println!("Position: {:.1}°, {:.1}°", pan.0, tilt.0);

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
| `SonyBRCH900`        | Sony encapsulation |    52381 |    52381 |
| `SonyFR7`            | Sony encapsulation |    52381 |    52381 |

Port can be omitted in connection strings; the profile default is used.

---

## Documentation

- **[API Reference](https://docs.rs/grafton-visca)** — Complete type and method documentation
- **[Examples](examples/)** — Working code for common scenarios
- **[CHANGELOG](CHANGELOG.md)** — Version history and migration guides

### Examples

| Example | Command |
| ------- | ------- |
| Blocking quickstart | `cargo run --example quickstart` |
| Async inquiry (Tokio) | `cargo run --example inquiry_quickstart --features runtime-tokio` |
| Advanced custom transport builder | `cargo run --example builder_api` |
| Transport builder | `cargo run --example transport_builder_demo` |
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
