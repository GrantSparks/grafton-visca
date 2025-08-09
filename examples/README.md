# grafton-visca Examples

This directory contains comprehensive examples demonstrating how to use the grafton-visca library for controlling VISCA-compatible PTZ cameras.

## Getting Started

If you're new to the library, start with these examples in order:

1. **[quickstart.rs](quickstart.rs)** - Comprehensive blocking example with all camera features
2. **[quickstart_async.rs](quickstart_async.rs)** - Comprehensive async example with concurrent operations
3. **[error_handling.rs](error_handling.rs)** - Learn proper error handling patterns

## Examples by Category

### Basic Usage
- **[quickstart.rs](quickstart.rs)** - Comprehensive blocking example covering movement, presets, and imaging
- **[quickstart_async.rs](quickstart_async.rs)** - Async version with concurrent operations and state management

### Camera Control
- **[camera_inquiry.rs](camera_inquiry.rs)** - Query and read camera state, positions, and settings
- **[preset_demo.rs](preset_demo.rs)** - Working with preset positions
- **[type_safe_commands.rs](type_safe_commands.rs)** - Demonstrates compile-time type safety with profiles

### Connection & Transport
- **[transports.rs](transports.rs)** - Compare TCP vs UDP transports, configuration options
- **[builder_api.rs](builder_api.rs)** - Explore all CameraBuilder patterns and options

### Advanced Patterns
- **[concurrent_control.rs](concurrent_control.rs)** - Thread-safe operations from multiple threads
- **[error_handling.rs](error_handling.rs)** - Comprehensive error handling and recovery strategies

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
cargo run --example error_handling
```

Run async examples (requires tokio feature):
```bash
cargo run --example quickstart_async --features tokio
cargo run --example camera_inquiry --features tokio
cargo run --example concurrent_control --features tokio
```

### With Logging

Enable debug logging to see VISCA commands and responses:
```bash
RUST_LOG=debug cargo run --example camera_control
RUST_LOG=grafton_visca=debug cargo run --example quickstart
```

## Camera Profiles

The examples use different camera profiles to demonstrate type safety:

- `PTZOpticsG2` - PTZOptics Generation 2 cameras (most examples)
- `PTZOpticsG3` - PTZOptics Generation 3 cameras
- `SonyFR7` - Sony FR7 cameras with advanced features
- `GenericVisca` - Basic VISCA profile for unknown cameras

## Common Patterns

### Connection Setup
```rust
// Blocking TCP (port defaults to profile-specific: 5678 for PTZOptics)
let cam = CameraBuilder::tcp("192.168.0.110")
    .profile::<PTZOpticsG2>()
    .build()?;

// Async TCP with Tokio
let cam = CameraBuilder::tokio_tcp("192.168.0.110")
    .profile::<PTZOpticsG2>()
    .build()
    .await?;
```

### Error Handling
```rust
match cam.zoom_in() {
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
let (pan_result, zoom_result) = tokio::join!(
    cam.pan_tilt_right(speed),
    cam.zoom_in()
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
- Consider UDP for lower latency
- TCP provides better reliability
- Adjust timeouts based on network conditions

## Contributing

Found an issue or have an improvement? Please:
1. Test with your specific camera hardware
2. Include camera model and firmware version
3. Provide debug logs if reporting issues
4. Submit PRs with new examples for unique use cases

## License

These examples are part of the grafton-visca project and are licensed under Apache-2.0.