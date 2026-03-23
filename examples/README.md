# grafton-visca Examples

This directory contains working examples for the current `grafton-visca` API surface.

Preferred usage:
- Use `Connect` or `CameraConfig` for high-level application code.
- Use `CameraBuilder::with_executor(...).from_transport(...)` only when you need custom async transport wiring.
- Treat raw transport and protocol examples as advanced integration/reference material.

## Important: Feature Flags

This library uses feature flags to control dependencies:
- **No features** (default): Blocking API only
- **`mode-async`**: Runtime-agnostic async API; provide your own executor/runtime integration
- **`runtime-tokio`**: Built-in Tokio runtime support (implies `mode-async`)
- **`runtime-smol`**: Built-in smol runtime support (implies `mode-async`)
- **`transport-serial`**: Blocking serial (RS-232/RS-422)
- **`transport-serial-tokio`**: Tokio serial transport (implies `runtime-tokio`)
- **`test-utils`**: Testing utilities (not for production)

## Getting Started

If you're new to the library, start with these examples in order:

1. **[quickstart.rs](quickstart.rs)** - Blocking example using high-level methods (preferred)
2. **[inquiry_quickstart.rs](inquiry_quickstart.rs)** - Blocking inquiry flow with the current accessor API
3. **[quickstart_async.rs](quickstart_async.rs)** - Async example using the current runtime-specific features
4. **[transport_builder_demo.rs](transport_builder_demo.rs)** - Transport configuration and builder terminology

## Examples by Category

### Basic Usage
- **[quickstart.rs](quickstart.rs)** - Blocking example covering movement, presets, and imaging (high-level)
- **[quickstart_async.rs](quickstart_async.rs)** - Async version for Tokio or smol
- **[inquiry_quickstart.rs](inquiry_quickstart.rs)** - High-level inquiry accessors and typed responses
- **[preset_demo.rs](preset_demo.rs)** - Working with preset positions
- **[type_safe_commands.rs](type_safe_commands.rs)** - Compile-time profile and capability safety

### Connection, Transport, and Configuration
- **[transports.rs](transports.rs)** - Compare TCP vs UDP transports and connection behavior
- **[transport_builder_demo.rs](transport_builder_demo.rs)** - Blocking transport builder API and async transport-config guidance
- **[builder_api.rs](builder_api.rs)** - Explore `CameraBuilder` flows for blocking and async cameras
- **[sony_encapsulation.rs](sony_encapsulation.rs)** - Sony encapsulated protocol with 8-byte header (advanced)
- **[serial_async_demo.rs](serial_async_demo.rs)** - Tokio serial transport setup

### Advanced Patterns
- **[runtime_agnostic.rs](runtime_agnostic.rs)** - Bring your own executor and async transport
- **[runtime_demo.rs](runtime_demo.rs)** - Runtime integration details and lower-level flows
- **[runtime_demo_lowlevel.rs](runtime_demo_lowlevel.rs)** - Lower-level runtime plumbing
- **[concurrent_control.rs](concurrent_control.rs)** - Concurrent async control patterns
- **[error_handling.rs](error_handling.rs)** - Comprehensive error handling and recovery strategies

### Validation and Reference
- **[typed_inquiry_demo.rs](typed_inquiry_demo.rs)** - Typed inquiry API walkthrough
- **[validate_inquiries.rs](validate_inquiries.rs)** - Inquiry validation/reference tool
- **[validate_ae_commands.rs](validate_ae_commands.rs)** - Auto-exposure command validation/reference tool

## Running Examples

### Prerequisites

1. Ensure you have a VISCA-compatible camera connected to your network
2. Update the IP address in the examples to match your camera (default: `192.168.0.110`)
3. Verify the port number; defaults vary by camera model:
   - PTZOptics cameras: TCP port `5678`, UDP port `1259`
   - Sony cameras: TCP port `52381`, UDP port `52381`
   - The selected profile will provide the default port when you omit it

### Basic Execution

Run blocking examples:
```bash
cargo run --example quickstart
cargo run --example inquiry_quickstart
cargo run --example preset_demo
cargo run --example transports
cargo run --example type_safe_commands
cargo run --example transport_builder_demo
```

Run async examples:
```bash
cargo run --example quickstart_async --features runtime-tokio
cargo run --example builder_api --features runtime-tokio
cargo run --example concurrent_control --features runtime-tokio
cargo run --example error_handling --features runtime-tokio
cargo run --example runtime_demo --features runtime-tokio
cargo run --example sony_encapsulation --features runtime-tokio
cargo run --example serial_async_demo --features runtime-tokio,transport-serial-tokio

# Runtime-agnostic async example
cargo run --example runtime_agnostic --features mode-async
```

### With Logging

Enable debug logging to see VISCA commands and responses:
```bash
RUST_LOG=debug cargo run --example quickstart
RUST_LOG=grafton_visca=debug cargo run --example quickstart_async --features runtime-tokio
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

## Current API Shape

```rust
use grafton_visca::camera::{CameraConfig, Connect};
use grafton_visca::profiles::PtzOpticsG2;
use grafton_visca::runtime::TokioRuntime;
use grafton_visca::transport::{TcpKeepaliveConfig, TransportConfig};

let _blocking = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110")?;

let runtime = TokioRuntime::from_current()?;
let _async_camera = CameraConfig::<PtzOpticsG2>::new()
    .address("192.168.0.110")
    .transport_config(TransportConfig {
        tcp_keepalive: Some(TcpKeepaliveConfig::default()),
        ..TransportConfig::default()
    })
    .open_async(runtime)
    .await?;
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
- Adjust timeouts, retry policy, and TCP keepalive based on network conditions
- The library uses zero-copy parsing and stack-allocated buffers
- Runtime-agnostic design means zero overhead when not using async

## Contributing

Found an issue or have an improvement? Please:
1. Test with your specific camera hardware
2. Include camera model and firmware version
3. Provide debug logs if reporting issues
4. Submit PRs with new examples for unique use cases

## License

These examples are part of the grafton-visca project and are licensed under MIT OR Apache-2.0 dual license.
