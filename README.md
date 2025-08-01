# grafton‑visca  ·  VISCA for Rust

[![Crates.io](https://img.shields.io/crates/v/grafton-visca.svg)](https://crates.io/crates/grafton-visca)
[![Documentation](https://docs.rs/grafton-visca/badge.svg)](https://docs.rs/grafton-visca)
[![License](https://img.shields.io/crates/l/grafton-visca.svg)](LICENSE)

`grafton‑visca` aims to offer a **pure‑Rust, profile‑centric** implementation of the VISCA protocol.
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
    blocking::prelude::*,
    camera::profiles::PTZOpticsG2,
    CameraBuilder,
};

fn main() -> grafton_visca::Result<()> {
    env_logger::init();

    // Create camera using the builder pattern
    let cam = CameraBuilder::tcp("192.168.1.100:52381")
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
    r#async::prelude::*,
    camera::profiles::PTZOpticsG2,
    CameraBuilder,
};

#[tokio::main]
async fn main() -> grafton_visca::Result<()> {
    let cam = CameraBuilder::tokio_tcp("192.168.1.100:52381")
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
| **Blocking TCP/UDP**                               | Implemented                                              |
| **Tokio TCP/UDP**                                  | Implemented behind the `tokio` feature                   |
| **Socket manager (ACK/Completion, 2‑socket rule)** | Implemented & unit‑tested                                |
| **Extensive field testing**                        | **Still needed** – only PTZOptics units exercised so far |

If you run the crate against different hardware, please open an issue with:

* Camera make/model & firmware
* Transport used (TCP/UDP, blocking/Tokio)
* A concise reproduction (command + unexpected response/log)

---

## Installation

```toml
# Blocking‑only
grafton-visca = "0.5"

# Runtime‑agnostic async
grafton-visca = { version = "0.5", features = ["async"] }

# Tokio helpers
grafton-visca = { version = "0.5", features = ["tokio"] }
```

---

## Why a *profile‑centric* API?

Each profile implements capability traits (`HasZoom`, `HasNDFilter`, …).
If a capability is absent the corresponding extension trait is **not** in scope, so unsupported calls fail at compile time. Example:

```rust,ignore
let cam = CameraBuilder::tcp("192.168.1.100:52381")
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
