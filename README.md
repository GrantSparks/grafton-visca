# grafton‑visca

[![Crates.io](https://img.shields.io/crates/v/grafton-visca.svg)](https://crates.io/crates/grafton-visca)
[![Documentation](https://docs.rs/grafton-visca/badge.svg)](https://docs.rs/grafton-visca)
[![License](https://img.shields.io/crates/l/grafton-visca.svg)](LICENSE)
[![CI](https://github.com/GrantSparks/grafton-visca/actions/workflows/ci.yml/badge.svg)](https://github.com/GrantSparks/grafton-visca/actions/workflows/ci.yml)

A pure Rust library for controlling PTZ cameras via the VISCA protocol. Supports blocking and async APIs with built-in runtime adapters (Tokio and smol), TCP/UDP/serial transports, and type-safe camera profiles.

---

## Support Status

**Tested:** PTZOptics cameras G2 and G3 series over TCP and UDP.

**Best effort:** Other VISCA cameras, Sony encapsulation profiles, serial transport

We can only support what we can test. If you have access to different hardware, please [report issues](https://github.com/GrantSparks/grafton-visca/issues) or [submit pull requests](https://github.com/GrantSparks/grafton-visca/pulls).

---

## Features

- **Camera-first blocking/async API** — Shared noun accessors across blocking and async; async futures are `Send`-safe
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

### Configuring transport behavior

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
