# grafton-visca

[![Crates.io](https://img.shields.io/crates/v/grafton-visca.svg)](https://crates.io/crates/grafton-visca)
[![Documentation](https://docs.rs/grafton-visca/badge.svg)](https://docs.rs/grafton-visca)
[![License](https://img.shields.io/crates/l/grafton-visca.svg)](LICENSE)
[![CI](https://github.com/GrantSparks/grafton-visca/actions/workflows/ci.yml/badge.svg)](https://github.com/GrantSparks/grafton-visca/actions)

A production-ready, pure Rust implementation of the VISCA protocol for controlling PTZ cameras over IP networks. Designed for professional broadcast, streaming, and conference room applications.

## Features

- **Unified API Architecture** - Single consistent interface for both blocking and async modes
- **Type-safe camera profiles** - Compile-time validation of camera capabilities
- **Native blocking API** - Zero async dependencies when using blocking mode
- **Multi-runtime async support** - Works with Tokio, async-std, smol (supports coexistence)
- **Protocol-compliant** - Full VISCA protocol implementation with proper ACK/completion handling
- **Comprehensive command coverage** - 130+ VISCA commands across 17 unified traits
- **Intelligent timeout management** - Automatic command categorization and deadline handling
- **Production-tested** - Used in professional broadcast and streaming environments
- **Zero-cost abstractions** - Type safety without runtime overhead

## Quick Start

### Blocking API

```rust
use grafton_visca::{
    BlockingCamera,
    camera::profiles::PtzOpticsG2,
    // Import unified traits that work for both blocking and async
    ZoomControl,
    PanTiltControl,
};

fn main() -> grafton_visca::Result<()> {
    // Connect to camera using camera-first API
    let mut camera = BlockingCamera::<PtzOpticsG2, _>::connect_tcp("192.168.0.110:5678")?;

    // Control the camera with unified API
    camera.pan_tilt_home()?;
    camera.zoom_tele_std()?;

    Ok(())
}
```

### Async API (with Tokio)

```rust
use grafton_visca::{
    Camera,
    camera::profiles::PtzOpticsG2,
    runtime_adapters::tokio::TokioRuntime,
    // Same unified traits work for both blocking and async
    ZoomControl,
    PanTiltControl,
};

#[tokio::main]
async fn main() -> grafton_visca::Result<()> {
    // Connect using camera-first API with runtime
    let runtime = TokioRuntime::new();
    let camera = Camera::<PtzOpticsG2, _, _>::connect_tcp(
        "192.168.0.110:5678",
        runtime
    ).await?;

    // Same unified API, just add .await
    camera.pan_tilt_home().await?;
    camera.zoom_tele_std().await?;

    Ok(())
}
```

### Multi-Runtime Support

```rust
// Supports coexistence of multiple runtimes!
use grafton_visca::{
    Camera,
    camera::profiles::GenericVisca,
    runtime_adapters::tokio::TokioRuntime,
    PowerControl, // Unified trait works everywhere
};

#[tokio::main]
async fn main() -> grafton_visca::Result<()> {
    // Connect using camera-first API
    let runtime = TokioRuntime::new();
    let camera = Camera::<GenericVisca, _, _>::connect_tcp(
        "192.168.0.110:5678",
        runtime
    ).await?;

    let camera = CameraBuilder::tokio()?
        .build_async::<GenericVisca, _>(transport)
        .await?;

    // Unified API works consistently across runtimes
    camera.power_on().await?;
    Ok(())
}
```

## Installation

```toml
# Blocking API only (no async dependencies)
[dependencies]
grafton-visca = "0.7"

# Async support (runtime-agnostic)
[dependencies]
grafton-visca = { version = "0.7", features = ["async"] }

# With Tokio runtime support (recommended)
[dependencies]
grafton-visca = { version = "0.7", features = ["rt-tokio"] }
tokio = { version = "1", features = ["full"] }

# Multi-runtime support (NEW: runtimes can coexist!)
[dependencies]
grafton-visca = { version = "0.7", features = ["rt-tokio", "rt-async-std"] }
```

## Camera Profiles

The library includes profiles for various VISCA cameras:

| Profile | Type | Default Port | Features |
|---------|------|--------------|----------|
| `PtzOpticsG2` | PTZOptics G2 series | 5678 | Pan/Tilt, Zoom, Presets |
| `PtzOpticsG3` | PTZOptics G3 series | 5678 | G2 + Enhanced features |
| `PtzOptics30X` | PTZOptics 30X optical | 5678 | 30X optical zoom |
| `SonyFR7` | Sony FR7 | 52381 | ND Filter, Advanced imaging |
| `SonyBRCH900` | Sony BRC-H900 | 52381 | Professional features |
| `SonyBRC300` | Sony BRC-300 | 52381 | Standard PTZ features |
| `SonyEVIH100` | Sony EVI-H100 | 52381 | Conference camera |
| `NearusBRC300` | Nearus BRC-300 | 5678 | BRC-300 compatible |
| `GenericVisca` | Generic VISCA | 5678 | Basic VISCA commands |

### Type-Safe Command Access

Camera profiles use Rust's type system to ensure only supported commands are available:

```rust
use grafton_visca::{
    Camera,
    camera::profiles::*,
    capabilities::{NDFilter, Profile},
    CameraBuilder,
    NdFilterControl,  // Unified trait
    command::nd_filter::CommandNDFilterMode,
};

fn configure_nd_filter<P, T>(camera: &mut Camera<P, T>) -> grafton_visca::Result<()>
where
    P: Profile + NDFilter,  // Only cameras with ND filter support
    T: grafton_visca::transport::BlockingTransport,
{
    // Same unified trait works for blocking and async modes
    camera.set_nd_filter_mode(CommandNDFilterMode::Variable)?;
    Ok(())
}

// This compiles for Sony FR7
let mut sony = CameraBuilder::tcp("192.168.0.110:52381")
    .profile::<SonyFR7>()
    .build()?;
configure_nd_filter(&mut sony)?;  // ✅ Works

// This won't compile for PTZOptics G2
let mut ptz = CameraBuilder::tcp("192.168.0.111:5678")
    .profile::<PtzOpticsG2>()
    .build()?;
// configure_nd_filter(&mut ptz)?;  // ❌ Compile error - no ND filter
```

## Examples

The library includes comprehensive examples demonstrating various use cases:

| Example | Description | Features Required |
|---------|-------------|-------------------|
| [`quickstart`](examples/quickstart.rs) | Basic blocking usage | None |
| [`quickstart_async`](examples/quickstart_async.rs) | Async with Tokio | `rt-tokio` |
| [`runtime_agnostic`](examples/runtime_agnostic.rs) | Any async runtime | `async` |
| [`builder_api`](examples/builder_api.rs) | Builder configuration | `rt-tokio` |
| [`camera_inquiry`](examples/camera_inquiry.rs) | Query camera state | `rt-tokio` |
| [`concurrent_control`](examples/concurrent_control.rs) | Multi-threaded control | `rt-tokio` |
| [`error_handling`](examples/error_handling.rs) | Error recovery | `rt-tokio` |
| [`preset_demo`](examples/preset_demo.rs) | Preset management | None |
| [`transports`](examples/transports.rs) | TCP vs UDP | None |
| [`type_safe_commands`](examples/type_safe_commands.rs) | Profile type safety | None |
| [`sony_encapsulation`](examples/sony_encapsulation.rs) | Sony protocol mode | `rt-tokio` |
| [`inquiry_demo`](examples/inquiry_demo.rs) | Advanced queries | `rt-tokio` |

Run examples:
```bash
# Blocking examples
cargo run --example quickstart

# Async examples
cargo run --example quickstart_async --features rt-tokio
```

## Timeout Configuration

The library automatically categorizes commands and applies appropriate timeouts:

```rust
use grafton_visca::{
    CameraBuilder,
    camera::profiles::PtzOpticsG2,
    timeout::TimeoutConfig,
    transport::tokio::tcp::Tcp,
};
use std::time::Duration;

let transport = Tcp::connect("192.168.0.110:5678").await?;
let camera = CameraBuilder::tokio()?
    .timeout_config(TimeoutConfig::builder()
        .quick(Duration::from_secs(1))       // Power, stop commands
        .movement(Duration::from_secs(10))   // Pan/tilt/zoom movements
        .preset(Duration::from_secs(15))     // Preset recall/save
        .long_running(Duration::from_secs(30)) // Firmware updates
        .network(Duration::from_secs(5))     // Network operations
        .build())
    .build_async::<PtzOpticsG2, _>(transport)
    .await?;
```

## Error Handling

The library provides detailed error information with retry guidance:

```rust
use grafton_visca::{ZoomControl, units::Normalized};  // Unified trait
use std::thread::sleep;

loop {
    match camera.zoom_absolute(Normalized::new(0.5)) {
        Ok(_) => break,
        Err(e) if e.is_retryable() => {
            // Camera busy, network issue, etc.
            sleep(e.suggested_retry_delay().unwrap_or_default());
        }
        Err(e) => return Err(e), // Non-retryable error
    }
}
```

## Transport Layers

The library supports multiple transport implementations:

- **TCP** - Reliable, connection-oriented (recommended)
- **UDP** - Low latency, connectionless
- **Serial** - RS-232/RS-485 (with `serial` feature)
- **Custom** - Implement `BlockingTransport` or `AsyncTransport` traits for custom protocols

## Architecture

The library is built on a unified, layered architecture:

```
┌──────────────────────────────────────┐
│    Unified Camera API Layer          │  17 unified traits, 130+ methods
├──────────────────────────────────────┤
│    Runtime Coexistence Layer         │  Multi-runtime support (Tokio/async-std/smol)
├──────────────────────────────────────┤
│    Protocol Layer                    │  VISCA encoding/decoding (envelope framing)
├──────────────────────────────────────┤
│    Transport Layer                   │  Network communication (TCP/UDP/Serial)
└──────────────────────────────────────┘
```

### Key Components

- **Unified Trait System** - Single consistent API for both blocking and async modes
- **Feature-Gated Methods** - Compile-time mode selection with zero runtime overhead
- **Runtime Coexistence** - Multiple async runtimes can coexist with priority-based selection
- **Socket Manager** - Automatic socket allocation for ACK/Completion sequences
- **Priority Scheduler** - Intelligent command prioritization and retry handling
- **Timeout Manager** - Category-based timeout configuration

## Testing

The library includes comprehensive testing infrastructure with **930+ tests**:

- **Unit tests** - Every command and response parser tested (459 blocking + 471 async)
- **Integration tests** - Full protocol flow validation
- **Protocol compliance** - VISCA specification adherence tests
- **Deterministic testing** - Reproducible async execution with test-utils
- **Mock transports** - Testing without physical cameras
- **Multi-runtime testing** - Validates runtime coexistence and priority selection
- **Property-based tests** - Fuzzing command encoding/decoding

Run tests:
```bash
# All tests with all features
cargo test --all-features

# Test specific feature combinations
cargo test --no-default-features
cargo test --no-default-features --features async
cargo test --no-default-features --features rt-tokio

# Multi-runtime testing
cargo test --features rt-tokio,rt-async-std
```

## Performance

The library is optimized for production use:

- **Zero-copy parsing** - Minimal allocations in hot paths
- **Const evaluation** - Many operations computed at compile time
- **Efficient buffering** - Stack-allocated command buffers
- **Smart retries** - Exponential backoff with jitter

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

Areas where help is especially appreciated:
- Testing with physical cameras (especially Sony models)
- Additional camera profile implementations
- Protocol edge case documentation
- Performance profiling and optimizations
- Serial transport implementation

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

## Resources

- [API Documentation](https://docs.rs/grafton-visca)
- [API Guide](docs/API_GUIDE.md)
- [Error Handling Guide](docs/error-handling.md)
- [VISCA Reference](docs/visca_unified_reference.md)
- [PTZOptics G2 Command List](docs/PTZOptics-G2-VISCA-over-IP-Command-List.md)
- [Examples](examples/)
- [GitHub Issues](https://github.com/GrantSparks/grafton-visca/issues)

## Acknowledgments

This library is developed by [Grafton Machine Shed](https://www.grafton.ai) for professional PTZ camera control applications.

Special thanks to:
- PTZOptics for camera testing support
- The Rust async community for runtime design patterns
- Contributors and users providing feedback and bug reports
