# grafton-visca Examples

This directory contains comprehensive examples demonstrating how to use the grafton-visca library for controlling VISCA-compatible PTZ cameras.

Preferred usage: Use the high-level Camera API wherever possible.
- Build cameras via `CameraBuilder` and a concrete profile.
- Call methods from `camera::controls` (power, pan_tilt, zoom, focus, exposure, presets, inquiry).
- Avoid sending raw VISCA bytes directly in applications; that is reserved for advanced demos.

## Important: Feature Flags

This library uses feature flags to control dependencies:
- **No features** (default): Blocking API only, zero async dependencies
- **`async`**: Runtime-agnostic async support (requires executor)
- **`rt-tokio`**: Tokio runtime integration (includes async)
- **`rt-async-std`**: async-std runtime integration (includes async)
- **`rt-smol`**: smol runtime integration (includes async)
- **`serial`**: Serial port support for RS-232/RS-422 (partial implementation)
- **`test-utils`**: Testing utilities (not for production)

## Getting Started

If you're new to the library, start with these examples in order:

1. **[quickstart.rs](quickstart.rs)** - Blocking example using high-level methods (preferred)
2. **[quickstart_async.rs](quickstart_async.rs)** - Async example using high-level methods (preferred)
3. **[error_handling.rs](error_handling.rs)** - Learn proper error handling patterns

## Examples by Category

### Basic Usage
- **[quickstart.rs](quickstart.rs)** - Blocking example covering movement, presets, and imaging (high-level)
- **[quickstart_async.rs](quickstart_async.rs)** - Async version with concurrent operations and state management (high-level)
- **[runtime_agnostic.rs](runtime_agnostic.rs)** - Works with any async runtime (smol, async-std, etc.)

### Camera Control
- **[camera_inquiry.rs](camera_inquiry.rs)** - Query and read camera state, positions, and settings
- **[preset_demo.rs](preset_demo.rs)** - Working with preset positions
- **[type_safe_commands.rs](type_safe_commands.rs)** - Demonstrates compile-time type safety with profiles

### Connection & Transport
- **[transports.rs](transports.rs)** - Compare TCP vs UDP transports, configuration options
- **[builder_api.rs](builder_api.rs)** - Explore all CameraBuilder patterns and options
- **[sony_encapsulation.rs](sony_encapsulation.rs)** - Sony encapsulated protocol with 8-byte header (advanced)

### Advanced Patterns
- **[concurrent_control.rs](concurrent_control.rs)** - Thread-safe operations from multiple threads
- **[error_handling.rs](error_handling.rs)** - Comprehensive error handling and recovery strategies
- **[runtime_demo.rs](runtime_demo.rs)** - Runtime internals and low-level flows (advanced)
- **[inquiry_demo.rs](inquiry_demo.rs)** - Camera state queries using high-level inquiry methods

## Running Examples

### Prerequisites

1. Ensure you have a VISCA-compatible camera connected to your network
2. Update the IP address in the examples to match your camera (default: `192.168.0.110`)
3. Verify the port number - defaults vary by camera model:
   - PTZOptics cameras: TCP port `5678`, UDP port `1259`
   - Sony cameras: TCP port `52381`, UDP port `52381`
   - The CameraBuilder will use appropriate defaults based on the profile you select

### Basic Execution

Run blocking examples:
```bash
cargo run --example quickstart
cargo run --example preset_demo
cargo run --example transports
cargo run --example type_safe_commands
```

Run async examples (requires rt-tokio feature):
```bash
cargo run --example quickstart_async --features rt-tokio
cargo run --example camera_inquiry --features rt-tokio
cargo run --example concurrent_control --features rt-tokio
cargo run --example error_handling --features rt-tokio
cargo run --example builder_api --features rt-tokio
cargo run --example sony_encapsulation --features rt-tokio
cargo run --example inquiry_demo --features rt-tokio
cargo run --example runtime_demo --features rt-tokio

# Runtime-agnostic async example
cargo run --example runtime_agnostic --features async
```

### With Logging

Enable debug logging to see VISCA commands and responses:
```bash
RUST_LOG=debug cargo run --example quickstart
RUST_LOG=grafton_visca=debug cargo run --example quickstart_async --features rt-tokio
```

## Camera Profiles

The examples use different camera profiles to demonstrate compile-time type safety:

- `PtzOpticsG2` - PTZOptics Generation 2 cameras (most examples)
- `PtzOpticsG3` - PTZOptics Generation 3 cameras with enhanced features
- `PtzOptics30X` - PTZOptics 30X optical zoom models
- `SonyFR7` - Sony FR7 cameras with ND filter and advanced imaging
- `SonyBRCH900` - Sony BRC-H900 professional cameras
- `SonyBRC300` - Sony BRC-300 standard PTZ cameras
- `GenericVisca` - Basic VISCA profile for unknown cameras

Profiles enable compile-time validation of camera capabilities. Commands not supported by a profile won't compile, preventing runtime errors.

## Common Patterns

### Connection Setup
```rust
use grafton_visca::{Camera, camera::BlockingMode};
use grafton_visca::transport::blocking::tcp::Tcp;
use grafton_visca::camera::profiles::PtzOpticsG2;

// Blocking TCP
let transport = Tcp::connect("192.168.0.110:5678")?;
let cam = Camera::<BlockingMode, PtzOpticsG2, _, _>::new(transport);

// Async TCP with Tokio runtime
use grafton_visca::{CameraBuilder, transport::tokio::tcp::Tcp as TokioTcp};

let transport = TokioTcp::connect("192.168.0.110:5678").await?;
let cam = CameraBuilder::tokio()?
    .build_async::<PtzOpticsG2, _>(transport)
    .await?;

// Runtime-agnostic async (bring your own executor)
use grafton_visca::runtime::executor::Executor;

let executor = YourExecutor::new();
let transport = your_async_transport().await?;
let cam = CameraBuilder::with_executor(executor)
    .build_async::<PtzOpticsG2, _>(transport)
    .await?;
```

### Error Handling
```rust
use grafton_visca::camera::controls::zoom::ZoomControlBlocking;

match cam.zoom_tele_std() {
    Ok(_) => println!("Success"),
    Err(e) if e.is_retryable() => {
        std::thread::sleep(e.suggested_retry_delay().unwrap());
        // retry...
    }
    Err(e) => eprintln!("Fatal error: {}", e),
}
```

### Concurrent Operations (Async)
```rust
use grafton_visca::camera::controls::{
    pan_tilt::PanTiltControl,
    zoom::ZoomControl,
};

let (pan_result, zoom_result) = tokio::join!(
    cam.pan_tilt_right(speed),
    cam.zoom_tele_std()
);
```

## Troubleshooting

### Connection Issues
- Verify camera IP address and port
- Check network connectivity: `ping <camera-ip>`
- Ensure camera supports VISCA over IP
- Try both TCP and UDP transports

### Command Failures
- Enable debug logging: `RUST_LOG=debug`
- Check camera is powered on
- Verify camera profile matches your hardware
- Some commands require specific camera modes

### Performance
- Use async for concurrent operations
- Consider UDP for lower latency (but less reliable)
- TCP provides better reliability and is recommended
- Adjust timeouts based on network conditions
- The library uses zero-copy parsing and stack-allocated buffers
- Runtime-agnostic design means zero overhead when not using async

## Contributing

Found an issue or have an improvement? Please:
1. Test with your specific camera hardware
2. Include camera model and firmware version
3. Provide debug logs if reporting issues
4. Submit PRs with new examples for unique use cases

## License

These examples are part of the grafton-visca project and are licensed under Apache-2.0.
