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
    Camera,
    mode::Blocking,
    camera::profiles::PtzOpticsG2,
    // No trait imports needed with accessor pattern!
};

fn main() -> grafton_visca::Result<()> {
    // Connect to camera using unified API
    let mut camera = Camera::<Blocking, PtzOpticsG2>::open_tcp("192.168.0.110:5678")?;

    // Control the camera using discoverable accessor pattern
    camera.pan_tilt().home()?;
    camera.zoom().tele()?;

    // Explicit cleanup (optional - will auto-close on drop)
    camera.close()?;

    Ok(())
}
```

### Async API (with Tokio)

```rust
use grafton_visca::{
    Camera,
    camera::profiles::PtzOpticsG2,
    runtime_adapters::tokio::TokioRuntime,
    // No trait imports needed with accessor pattern!
};

#[tokio::main]
async fn main() -> grafton_visca::Result<()> {
    // Connect using camera-first API with runtime
    let runtime = TokioRuntime::new();
    let camera = Camera::<PtzOpticsG2, _, _>::connect_tcp(
        "192.168.0.110:5678",
        runtime
    ).await?;

    // Same discoverable accessor pattern, just add .await
    camera.pan_tilt().home().await?;
    camera.zoom().tele().await?;

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
    // No trait imports needed with accessor pattern!
};

#[tokio::main]
async fn main() -> grafton_visca::Result<()> {
    // Connect using unified API
    let runtime = TokioRuntime::new();
    let camera = Camera::<GenericVisca, _, _>::connect_tcp(
        "192.168.0.110:5678",
        runtime
    ).await?;

    // Discoverable accessor pattern works consistently across runtimes
    camera.power().on().await?;

    // Explicit cleanup (optional - will auto-close on drop)
    camera.shutdown().await?;

    Ok(())
}
```

## Usage Patterns

The library provides three clear usage patterns for different needs:

### 1. Simple (Blessed Path)
Most users should start here - simple and straightforward:

```rust
use grafton_visca::{Camera, mode::Blocking, camera::profiles::PtzOpticsG2};

// Blocking mode
let camera = Camera::<Blocking, PtzOpticsG2>::open_tcp("192.168.0.110:5678")?;

// Async mode (with Tokio)
let runtime = TokioRuntime::new();
let camera = Camera::<PtzOpticsG2, _, _>::connect_tcp("192.168.0.110:5678", runtime).await?;
```

### 2. Auto-Detection
When you don't know the camera's protocol configuration:

```rust
use grafton_visca::{Camera, mode::Blocking, camera::profiles::PtzOpticsG2};

// Automatically detects transport and protocol
let camera = Camera::<Blocking, PtzOpticsG2>::open_auto("192.168.0.110")?;

// Or use builder for more control
let camera = CameraBuilder::connect_auto("192.168.0.110")
    .profile::<PtzOpticsG2>()
    .open()?;
```

### 3. Advanced (BYO Transport)
For power users who need full control over transport configuration:

```rust
use grafton_visca::{
    CameraBuilder,
    camera::profiles::PtzOpticsG2,
    transport::Transport,
};
use std::time::Duration;

// Configure transport with custom settings
let transport = Transport::tcp()
    .address("192.168.0.110:5678")
    .connect_timeout(Duration::from_secs(10))
    .tcp_nodelay(true)
    .max_retries(5)
    .build_blocking()?;

// Build camera with custom transport
let camera = CameraBuilder::from_transport(transport)
    .profile::<PtzOpticsG2>()
    .protocol_style(ProtocolStyle::RawVisca)  // Override if needed
    .open()?;
```

## Installation

```toml
# Blocking API only (no async dependencies)
[dependencies]
grafton-visca = "0.7"

# Async support (runtime-agnostic)
[dependencies]
grafton-visca = { version = "0.7", features = ["mode-async"] }

# With Tokio runtime support (recommended)
[dependencies]
grafton-visca = { version = "0.7", features = ["runtime-tokio"] }
tokio = { version = "1", features = ["full"] }

# Multi-runtime support (NEW: runtimes can coexist!)
[dependencies]
grafton-visca = { version = "0.7", features = ["runtime-tokio", "runtime-async-std"] }
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
    NdFilterMode,     // Enum type for filter modes
};

fn configure_nd_filter<P, T>(camera: &Camera<P, T>) -> grafton_visca::Result<()>
where
    P: Profile + NDFilter,  // Only cameras with ND filter support
    T: grafton_visca::transport::BlockingTransport,
{
    // Discoverable accessor pattern - no trait imports needed
    camera.nd_filter().set_mode(NdFilterMode::Variable)?;
    Ok(())
}

// This compiles for Sony FR7
let mut sony = CameraBuilder::tcp("192.168.0.110:52381")
    .profile::<SonyFR7>()
    .open()?;  // open() explicitly connects
configure_nd_filter(&mut sony)?;  // ✅ Works

// This won't compile for PTZOptics G2
let mut ptz = CameraBuilder::tcp("192.168.0.111:5678")
    .profile::<PtzOpticsG2>()
    .open()?;  // open() explicitly connects
// configure_nd_filter(&mut ptz)?;  // ❌ Compile error - no ND filter
```

## API Design: Accessor Pattern

The library provides a clean, discoverable API through accessor methods for inquiries and state queries:

```rust
use grafton_visca::{Camera, mode::Blocking, camera::profiles::PtzOpticsG2};

let camera = Camera::<Blocking, PtzOpticsG2>::open_tcp("192.168.0.110:5678")?;

// Accessor-based inquiries - clean and discoverable
let power_state = camera.power().state()?;
let zoom_position = camera.zoom().position()?;
let pan_tilt_pos = camera.pan_tilt().position()?;

// System information
let version = camera.system().version()?;

// Advanced features (when available)
let nd_filter = camera.nd_filter().position()?;  // Sony cameras only
let menu_state = camera.menu().state()?;
```

This design provides:
- **Discoverability**: IDE autocomplete shows available accessors
- **Consistency**: All inquiries follow the same pattern
- **Type Safety**: Only available methods for your camera profile
- **No trait imports**: Accessors are always available on Camera

## Examples

The library includes comprehensive examples organized by complexity:

### Basic Examples
These examples demonstrate the recommended high-level accessor API:

| Example | Description | Features Required |
|---------|-------------|-------------------|
| [`quickstart`](examples/quickstart.rs) | Basic blocking usage with accessor API | None |
| [`quickstart_async`](examples/quickstart_async.rs) | Async control with any runtime (tokio/async-std/smol) | `runtime-tokio` or `runtime-async-std` or `runtime-smol` |
| [`preset_demo`](examples/preset_demo.rs) | Preset management using accessors | None |
| [`typed_inquiry_demo`](examples/typed_inquiry_demo.rs) | Type-safe inquiries with accessor API | None |
| [`inquiry_quickstart`](examples/inquiry_quickstart.rs) | Query camera state | `runtime-tokio` |
| [`concurrent_control`](examples/concurrent_control.rs) | Multi-camera control with accessors | `runtime-tokio` |
| [`error_handling`](examples/error_handling.rs) | Error recovery patterns | None or `runtime-tokio` |

### Advanced Examples
These examples demonstrate low-level features, transport configuration, and advanced patterns:

| Example | Description | Features Required |
|---------|-------------|-------------------|
| [`transports`](examples-advanced/transports.rs) | TCP vs UDP transport details | None |
| [`transport_builder_demo`](examples-advanced/transport_builder_demo.rs) | Advanced transport configuration | None |
| [`runtime_demo_lowlevel`](examples-advanced/runtime_demo_lowlevel.rs) | Low-level runtime interactions | `mode-async` |
| [`type_safe_commands`](examples-advanced/type_safe_commands.rs) | Direct command module usage | None |
| [`builder_api`](examples/builder_api.rs) | Builder configuration patterns | Multiple runtime features |
| [`runtime_agnostic`](examples/runtime_agnostic.rs) | Implementing custom runtime support | `mode-async` |
| [`runtime_demo`](examples/runtime_demo.rs) | Runtime adapter patterns | None |
| [`protocol_auto_detection`](examples/protocol_auto_detection.rs) | Auto-detect camera protocol | None |
| [`sony_encapsulation`](examples/sony_encapsulation.rs) | Sony protocol mode | None |
| [`serial_async_demo`](examples/serial_async_demo.rs) | Serial port control | `transport-serial`, `runtime-tokio` |

### Choosing Your Runtime
For async examples, you can choose between three runtimes:

```bash
# With tokio (recommended for most applications)
cargo run --example quickstart_async --features runtime-tokio

# With async-std
cargo run --example quickstart_async --features runtime-async-std

# With smol (lightweight runtime)
cargo run --example quickstart_async --features runtime-smol

# Blocking mode (no async runtime needed)
cargo run --example quickstart --no-default-features
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
    .open_async::<PtzOpticsG2, _>(transport)  // open_async() explicitly connects
    .await?;
```

## Error Handling

The library provides detailed error information with retry guidance:

```rust
use grafton_visca::{ZoomControl, units::Normalized};  // Unified trait
use std::thread::sleep;

loop {
    match camera.zoom().absolute(Normalized::new(0.5)) {
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
cargo test --no-default-features --features mode-async
cargo test --no-default-features --features runtime-tokio

# Multi-runtime testing
cargo test --features runtime-tokio,runtime-async-std
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
