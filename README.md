# grafton‑visca

[![Crates.io](https://img.shields.io/crates/v/grafton-visca.svg)](https://crates.io/crates/grafton-visca)
[![Documentation](https://docs.rs/grafton-visca/badge.svg)](https://docs.rs/grafton-visca)
[![License](https://img.shields.io/crates/l/grafton-visca.svg)](LICENSE)
[![CI](https://github.com/GrantSparks/grafton-visca/actions/workflows/ci.yml/badge.svg)](https://github.com/GrantSparks/grafton-visca/actions/workflows/ci.yml)

A pure Rust library for controlling PTZ cameras via the VISCA protocol. Supports blocking and async APIs with pluggable runtime adapters (Tokio, async-std, smol), TCP/UDP/serial transports, and type-safe camera profiles.

---

## Pre-Release Notice

> **This crate is in pre-release (< 1.0.0).** Breaking changes may occur until version 1.0.0.

**Tested:** PTZOptics cameras G2 and G3 series over TCP and UDP.

**Experimental:** Other VISCA cameras, Sony encapsulation profiles, serial transport

We can only support what we can test. If you have access to different hardware, please [report issues](https://github.com/GrantSparks/grafton-visca/issues) or [submit pull requests](https://github.com/GrantSparks/grafton-visca/pulls).

---

## Features

- **Unified blocking/async API** — Single `Camera` type works in both modes; async futures are `Send`-safe
- **Multi-runtime support** — Pluggable adapters for Tokio, async-std, and smol (can coexist)
- **Type-safe profiles** — Compile-time protocol selection (raw VISCA vs Sony encapsulation) with capability-based APIs
- **Flexible transports** — TCP, UDP, and serial (RS-232/422) with configurable timeouts and retries
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
    let mut cam = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110")?;

    cam.power_on()?;
    cam.pan_tilt_home()?;

    let pos = cam.pan_tilt_position()?;
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

    let pos = cam.inquiry().pan_tilt_position().await?;
    let (pan, tilt) = pos.as_degrees();
    println!("Position: {:.1}°, {:.1}°", pan.0, tilt.0);

    cam.close().await
}
```

For async-std or smol, enable the corresponding feature and use its runtime adapter.

---

## Installation

```toml
[dependencies]
grafton-visca = "0.12"
```

### Common configurations

```toml
# Async with Tokio
grafton-visca = { version = "0.12", features = ["runtime-tokio"] }
tokio = { version = "1", features = ["full"] }

# With serialization
grafton-visca = { version = "0.12", features = ["serde"] }

# Serial transport (blocking)
grafton-visca = { version = "0.12", features = ["transport-serial"] }
```

### Feature flags

| Feature                | Enables                                    |
| ---------------------- | ------------------------------------------ |
| `runtime-tokio`        | Tokio async runtime adapter                |
| `runtime-async-std`    | async-std runtime adapter                  |
| `runtime-smol`         | smol runtime adapter                       |
| `transport-serial`     | Blocking serial (RS-232/422)               |
| `transport-serial-tokio` | Async serial (Tokio)                     |
| `serde`                | Serialize/Deserialize for all types        |
| `schemars`             | JSON Schema generation                     |
| `ts-rs`                | TypeScript type generation                 |
| `dyn-api`              | Object-safe camera traits                  |

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
| Builder API | `cargo run --example builder_api` |
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
