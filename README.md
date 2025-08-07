# grafton‑visca  ·  VISCA for Rust

[![Crates.io](https://img.shields.io/crates/v/grafton-visca.svg)](https://crates.io/crates/grafton-visca)
[![Documentation](https://docs.rs/grafton-visca/badge.svg)](https://docs.rs/grafton-visca)
[![License](https://img.shields.io/crates/l/grafton-visca.svg)](LICENSE)

`grafton‑visca` aims to offer a **pure‑Rust, profile‑centric** implementation of the VISCA protocol with both blocking and async APIs.

## Key Features

- **Dual API Design** - Native blocking and async implementations, not wrappers
- **Runtime Agnostic** - Async support works with ANY runtime (tokio, async-std, smol, etc.)
- **Zero Overhead** - Blocking API has no async dependencies when async features are disabled
- **Type-Safe Profiles** - Camera capabilities validated at compile time

All command encoders/decoders compile and have unit / property tests, but **the crate still needs extensive validation on physical camera fleets**. Please treat each release as *experimental* until we publish real‑world compatibility reports.

---

## Supported camera profiles & test coverage

| Profile (module)                | Implementation in crate  | Real‑camera testing                           |
| ------------------------------- | ------------------------ | --------------------------------------------- |
| PTZOptics G2 (`PTZOpticsG2`)    | Core traits implemented  | **Partial** – basic PT, zoom & power verified |
| PTZOptics G3 (`PTZOpticsG3`)    | Implemented              | **Partial** – same manual smoke tests as G2   |
| PTZOptics 30X (`PTZOptics30X`)  | Implemented              | **Partial** – optical zoom range only         |
| Sony FR7 (`SonyFR7`)            | Implemented              | **Untested on hardware**                      |
| Sony BRC‑H900 (`SonyBRCH900`)   | Implemented              | Untested                                      |
| Sony BRC‑300 (`SonyBRC300`)     | Implemented              | Untested                                      |
| Sony EVI‑H100 (`SonyEVIH100`)   | Implemented              | Untested                                      |
| Nearus BRC‑300 (`NearusBRC300`) | Implemented              | Untested                                      |
| Generic VISCA (`GenericVisca`)  | Baseline profile         | n/a (placeholder)                             |

> **Legend**   Partial = limited manual checks on a single unit
> Untested = no physical camera yet
> We welcome PRs or issue reports for any model, especially non‑PTZOptics devices.

---

## Quick start (blocking API)

```rust
use grafton_visca::{
    prelude::blocking::*,
    camera::profiles::PTZOpticsG2,
    CameraBuilder,
};

fn main() -> grafton_visca::Result<()> {
    env_logger::init();

    // Create camera using the builder pattern
    let cam = CameraBuilder::tcp("192.168.0.110:52381")
        .profile::<PTZOpticsG2>()
        .build()?;

    cam.power_on()?;            // turns the camera on
    cam.pan_tilt_home()?;       // move to home
    println!("Hello from VISCA camera!");
    Ok(())
}
```

### Async / Tokio

```rust
use grafton_visca::{
    prelude::r#async::*,
    camera::profiles::PTZOpticsG2,
    CameraBuilder,
};

#[tokio::main]
async fn main() -> grafton_visca::Result<()> {
    let cam = CameraBuilder::tokio_tcp("192.168.0.110:52381")
        .profile::<PTZOpticsG2>()
        .build()
        .await?;
    cam.power_on().await?;
    cam.zoom_in().await?;
    cam.zoom_stop().await?;
    Ok(())
}
```

---

## Project status

| Area                                               | Where we are                                             |
| -------------------------------------------------- | -------------------------------------------------------- |
| **Core encoding / decoding**                       | Complete and test‑covered                                |
| **`Camera<P, T>` generic API**                     | Stable                                                   |
| **Native blocking API**                            | Fully implemented (TCP/UDP)                              |
| **Runtime-agnostic async API**                     | Fully implemented with `async` feature                   |
| **Tokio integration**                              | Convenience helpers with `tokio` feature                 |
| **Socket manager (ACK/Completion, 2‑socket rule)** | Implemented & unit‑tested                                |
| **Extensive field testing**                        | **Still needed** – only PTZOptics units exercised so far |

If you run the crate against different hardware, please open an issue with:

* Camera make/model & firmware
* Transport used (TCP/UDP, blocking/Tokio)
* A concise reproduction (command + unexpected response/log)

---

## Installation

The library provides native implementations for both blocking and async APIs:

```toml
# Blocking API (no features needed - always available)
grafton-visca = "0.6"

# Runtime‑agnostic async (works with ANY async runtime)
grafton-visca = { version = "0.6", features = ["async"] }

# Tokio convenience helpers (includes async)
grafton-visca = { version = "0.6", features = ["tokio"] }
```

### Feature Flags

- **No features** - Blocking API only, zero async dependencies
- **`async`** - Enables async traits and methods, runtime-agnostic (bring your own runtime)
- **`tokio`** - Adds Tokio-specific transport implementations and timeout support (implies `async`)

The async implementation is truly runtime-agnostic - you can use it with tokio, async-std, smol, or any other async runtime by implementing the `Transport` trait for your runtime's networking types.

---

## Examples

We provide a comprehensive set of examples demonstrating different aspects of the library:

| Example | Description | Features Demonstrated |
| ------- | ----------- | --------------------- |
| [`quickstart`](examples/quickstart.rs) | Comprehensive blocking example | All camera movements, presets, imaging |
| [`quickstart_async`](examples/quickstart_async.rs) | Comprehensive async example | Concurrent operations, async patterns |
| [`transports`](examples/transports.rs) | TCP vs UDP comparison | Transport configuration, performance |
| [`builder_api`](examples/builder_api.rs) | Builder pattern usage | All builder options and configurations |
| [`camera_inquiry`](examples/camera_inquiry.rs) | Query camera state | Reading positions, settings, status |
| [`concurrent_control`](examples/concurrent_control.rs) | Thread-safe operations | Multiple threads controlling camera |
| [`error_handling`](examples/error_handling.rs) | Error recovery patterns | Retryable errors, timeouts, recovery |
| [`preset_demo`](examples/preset_demo.rs) | Preset management | Saving, recalling, and managing presets |
| [`type_safe_commands`](examples/type_safe_commands.rs) | Type safety demonstration | Profile-specific features and compile-time checks |

Run any example with:
```bash
# Blocking examples
cargo run --example quickstart

# Async examples (require tokio feature)
cargo run --example quickstart_async --features tokio
```

---

## Why a *profile‑centric* API?

Each profile implements capability traits (`HasZoom`, `HasNDFilter`, …).
If a capability is absent the corresponding extension trait is **not** in scope, so unsupported calls fail at compile time. Example:

```rust,ignore
let cam = CameraBuilder::tcp("192.168.0.110:52381")
    .profile::<SonyFR7>()
    .build()?;
// cam.set_nd_filter_mode(NDFilterMode::Variable)?;  // ✅ FR7 supports this
let b = CameraBuilder::tcp("192.168.1.101:52381")
    .profile::<PTZOpticsG2>()
    .build()?;
// b.set_nd_filter_mode(NDFilterMode::Variable)?;     // ❌ compile‑error
```

---

## Error handling

`Error` indicates whether an issue is retryable:

```rust
loop {
    match cam.zoom_absolute(0.6.into()) {
        Ok(_) => break,
        Err(e) if e.is_retryable() => std::thread::sleep(e.suggested_retry_delay().unwrap()),
        Err(e) => return Err(e.into()),
    }
}
```

---

## Contributing

*Try the crate on real hardware and tell us what breaks!*
We are particularly interested in edge‑cases (socket saturation, mixed command loads, Sony encapsulated mode quirks, etc.).

See `CONTRIBUTING.md` for coding guidelines.

---

© 2025 Grafton Machine Shed – Licensed under Apache‑2.0
