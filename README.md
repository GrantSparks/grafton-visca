# grafton-visca

[![Crates.io](https://img.shields.io/crates/v/grafton-visca.svg)](https://crates.io/crates/grafton-visca)
[![Documentation](https://docs.rs/grafton-visca/badge.svg)](https://docs.rs/grafton-visca)
[![License](https://img.shields.io/crates/l/grafton-visca.svg)](LICENSE)
[![CI](https://github.com/GrantSparks/grafton-visca/actions/workflows/ci.yml/badge.svg)](https://github.com/GrantSparks/grafton-visca/actions)

A production-ready, pure Rust implementation of the VISCA protocol for controlling PTZ cameras over IP networks. Designed for professional broadcast, streaming, and conference room applications.

## Features

- **Type-safe camera profiles** - Compile-time validation of camera capabilities
- **Clean API separation** - Choose blocking OR async at compile time
- **Native blocking API** - Zero async dependencies when using blocking mode
- **Runtime-agnostic async** - Works with any async runtime or custom executor
- **Protocol-compliant** - Full VISCA protocol implementation with proper ACK/completion handling
- **Comprehensive command coverage** - 100+ VISCA commands implemented
- **Intelligent timeout management** - Automatic command categorization and deadline handling
- **Production-tested** - Used in professional broadcast and streaming environments
- **Zero-cost abstractions** - Type safety without runtime overhead

## Quick Start

### Blocking API

```rust
use grafton_visca::{
    Camera,
    camera::{BlockingMode, profiles::PtzOpticsG2},
    transport::blocking::tcp::Tcp,
};

fn main() -> grafton_visca::Result<()> {
    // Connect to camera
    let transport = Tcp::connect("192.168.0.110:5678")?;
    let camera = Camera::<BlockingMode, PtzOpticsG2, _, _>::new(transport);
    
    // Control the camera
    use grafton_visca::camera::methods::{
        zoom::ZoomControlBlocking,
        pan_tilt::PanTiltControlBlocking,
    };
    
    camera.pan_tilt_home()?;
    camera.zoom_tele_std()?;
    
    Ok(())
}
```

### Async API (with Tokio)

```rust
use grafton_visca::{
    CameraBuilder,
    camera::profiles::PtzOpticsG2,
    transport::Transport,
};

#[tokio::main]
async fn main() -> grafton_visca::Result<()> {
    // Connect with uniform Transport API - runtime auto-selected
    let transport = Transport::tcp()
        .address("192.168.0.110:5678")
        .connect()
        .await?;
    let camera = CameraBuilder::tokio()?
        .build_async::<PtzOpticsG2, _>(transport)
        .await?;
    
    // Control the camera
    use grafton_visca::camera::methods::{
        zoom::ZoomControl,
        pan_tilt::PanTiltControl,
    };
    
    camera.pan_tilt_home().await?;
    camera.zoom_tele_std().await?;
    
    Ok(())
}
```

### Runtime-Agnostic Async

```rust
use grafton_visca::{
    CameraBuilder,
    camera::profiles::GenericVisca,
    transport::AsyncTransport,
    runtime::executor::Executor,
};

// Works with ANY async runtime - provide your executor and transport
async fn control_camera<E, T>(executor: E, transport: T) -> grafton_visca::Result<()> 
where
    E: Executor,
    T: AsyncTransport,
{
    let camera = CameraBuilder::with_executor(executor)
        .build_async::<GenericVisca, _>(transport)
        .await?;
    
    // Use methods from the appropriate trait
    use grafton_visca::camera::methods::power::PowerControl;
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
    camera::{BlockingMode, profiles::*},
    capabilities::{NDFilter, NDFilterMode, Profile},
    transport::BlockingTransport,
};

fn configure_nd_filter<P, T>(camera: &Camera<BlockingMode, P, T, ()>) -> grafton_visca::Result<()>
where
    P: Profile + NDFilter,  // Only cameras with ND filter support
    T: BlockingTransport,
{
    use grafton_visca::camera::methods::nd_filter::{NDFilterControlBlocking, CommandNDFilterMode};
    camera.set_nd_filter_mode(CommandNDFilterMode::Variable)?;
    Ok(())
}

// This compiles for Sony FR7
use grafton_visca::transport::blocking::tcp::Tcp;
let transport = Tcp::connect("192.168.0.110:52381")?;
let sony = Camera::<BlockingMode, SonyFR7, _, _>::new(transport);
configure_nd_filter(&sony)?;  // ✅ Works

// This won't compile for PTZOptics G2
let transport = Tcp::connect("192.168.0.111:5678")?;
let ptz = Camera::<BlockingMode, PtzOpticsG2, _, _>::new(transport);
// configure_nd_filter(&ptz)?;  // ❌ Compile error - no ND filter
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
use grafton_visca::camera::methods::zoom::ZoomControlBlocking;
use grafton_visca::units::Normalized;
use std::thread::sleep;

loop {
    match camera.zoom_to(Normalized::new(0.5)) {
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

The library is built on a layered, modular architecture:

```
┌──────────────────────────────────────┐
│         Camera API Layer             │  High-level methods & type-safe profiles
├──────────────────────────────────────┤
│         Runtime Layer                │  Protocol compliance & execution
├──────────────────────────────────────┤
│         Protocol Layer               │  VISCA encoding/decoding
├──────────────────────────────────────┤
│         Transport Layer              │  Network communication (TCP/UDP)
└──────────────────────────────────────┘
```

### Key Components

- **Command System** - Type-safe command encoding with compile-time validation
- **Response Parser** - Robust parsing with detailed error information
- **Socket Manager** - Automatic socket allocation for ACK/Completion sequences
- **Priority Scheduler** - Intelligent command prioritization and retry handling
- **Timeout Manager** - Category-based timeout configuration

## Testing

The library includes comprehensive testing infrastructure:

- **Unit tests** - Every command and response parser tested
- **Integration tests** - Full protocol flow validation
- **Protocol compliance** - VISCA specification adherence tests
- **Deterministic testing** - Reproducible async execution with test-utils
- **Mock transports** - Testing without physical cameras
- **Property-based tests** - Fuzzing command encoding/decoding

Run tests:
```bash
# All tests with all features
cargo test --all-features

# Test specific feature combinations
cargo test --no-default-features
cargo test --no-default-features --features async
cargo test --no-default-features --features rt-tokio
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

Licensed under Apache-2.0. See [LICENSE](LICENSE) for details.

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