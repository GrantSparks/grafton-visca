# grafton-visca API Guide

This guide provides comprehensive documentation for using the grafton-visca library's API effectively.

## Table of Contents

- [Core Concepts](#core-concepts)
- [Camera Construction](#camera-construction)
- [Camera Profiles](#camera-profiles)
- [Command Execution](#command-execution)
- [Error Handling](#error-handling)
- [Timeout Management](#timeout-management)
- [Async vs Blocking](#async-vs-blocking)
- [Testing](#testing)

## Core Concepts

### Architecture Overview

The library is built on a layered architecture:

1. **Transport Layer** - Handles network communication (TCP/UDP)
2. **Protocol Layer** - VISCA encoding/decoding with type safety
3. **Runtime Layer** - Command execution, ACK/Completion handling, retries
4. **Camera API Layer** - High-level methods with profile-based type safety

### Type Safety

The library uses Rust's type system to ensure only supported commands are available for each camera model:

```rust
use grafton_visca::camera::{Camera, BlockingMode};
use grafton_visca::capabilities::{NDFilter, Profile};
use grafton_visca::transport::BlockingTransport;
use grafton_visca::Result;

// This function only accepts cameras with ND filter support
fn configure_nd_filter<P, T>(camera: &Camera<BlockingMode, P, T, ()>) -> Result<()>
where
    P: Profile + NDFilter,  // Compile-time requirement
    T: BlockingTransport,
{
    use grafton_visca::camera::controls::nd_filter::{NDFilterControlBlocking, CommandNDFilterMode};
    camera.set_nd_filter_mode(CommandNDFilterMode::Variable)?;
    Ok(())
}
```

## Camera Construction

### CameraBuilder Pattern

The `CameraBuilder` provides a fluent API for creating camera instances:

```rust
use grafton_visca::{Camera, CameraBuilder, camera::BlockingMode};
use grafton_visca::transport::blocking::tcp::Tcp;
use grafton_visca::camera::profiles::{PtzOpticsG2, SonyFR7, GenericVisca};
use grafton_visca::camera_id::CameraId;

// Blocking TCP connection - simpler API for blocking mode
let transport = Tcp::connect("192.168.0.110:5678")?;
let camera = Camera::<BlockingMode, PtzOpticsG2, _, _>::new(transport);

// Async with Tokio
use grafton_visca::transport::tokio::tcp::Tcp as TokioTcp;

let transport = TokioTcp::connect("192.168.0.110:52381").await?;
let camera = CameraBuilder::tokio()?
    .camera_id(CameraId::new(1))  // Optional: Set camera ID (default: 1)
    .build_async::<SonyFR7, _>(transport)
    .await?;

// Runtime-agnostic async
use grafton_visca::runtime::executor::Executor;

let executor = YourExecutor::new();
let transport = create_your_transport().await?;
let camera = CameraBuilder::with_executor(executor)
    .build_async::<GenericVisca, _>(transport)
    .await?;
```

### Transport Options

#### TCP (Recommended)
- Reliable, connection-oriented
- Automatic reconnection support
- Better for production use

```rust
use grafton_visca::{Camera, camera::BlockingMode};
use grafton_visca::transport::blocking::tcp::Tcp;
use grafton_visca::camera::profiles::PtzOpticsG2;

// Blocking
let transport = Tcp::connect("192.168.0.110:5678")?;
let camera = Camera::<BlockingMode, PtzOpticsG2, _, _>::new(transport);

// Async with Tokio
use grafton_visca::{CameraBuilder, transport::tokio::tcp::Tcp as TokioTcp};

let transport = TokioTcp::connect("192.168.0.110:5678").await?;
let camera = CameraBuilder::tokio()?
    .build_async::<PtzOpticsG2, _>(transport)
    .await?;
```

#### UDP
- Lower latency
- Connectionless
- May lose packets

```rust
use grafton_visca::{Camera, camera::BlockingMode};
use grafton_visca::transport::blocking::udp::Udp;
use grafton_visca::camera::profiles::PtzOpticsG2;

// Blocking
let transport = Udp::connect("192.168.0.110:1259")?;
let camera = Camera::<BlockingMode, PtzOpticsG2, _, _>::new(transport);

// Async with Tokio
use grafton_visca::{CameraBuilder, transport::tokio::udp::Udp as TokioUdp};

let transport = TokioUdp::connect("192.168.0.110:1259").await?;
let camera = CameraBuilder::tokio()?
    .build_async::<PtzOpticsG2, _>(transport)
    .await?;
```

## Camera Profiles

Profiles define camera capabilities at compile time:

### Available Profiles

| Profile | Manufacturer | Key Features |
|---------|-------------|--------------|
| `PtzOpticsG2` | PTZOptics | Basic PTZ, 20x zoom |
| `PtzOpticsG3` | PTZOptics | G2 + enhanced features |
| `PtzOptics30X` | PTZOptics | 30x optical zoom |
| `SonyFR7` | Sony | ND filter, advanced imaging |
| `SonyBRCH900` | Sony | Professional features |
| `SonyBRC300` | Sony | Standard PTZ |
| `GenericVisca` | Any | Basic VISCA commands |

### Profile Traits

Profiles implement capability marker traits that enable specific methods:

```rust
use grafton_visca::capabilities::{Zoom, NDFilter, Profile};
use grafton_visca::camera::{Camera, BlockingMode};
use grafton_visca::transport::BlockingTransport;
use grafton_visca::Result;

// These are capability traits that enable compile-time feature detection
// Most cameras have zoom, but only some have ND filters

// Usage - methods are available based on trait bounds
use grafton_visca::camera::controls::zoom::ZoomControlBlocking;

fn zoom_demo<P, T>(camera: &Camera<BlockingMode, P, T, ()>) -> Result<()>
where
    P: Profile + Zoom,  // Compile-time requirement
    T: BlockingTransport,
{
    camera.zoom_tele_std()?;  // Available because P: Zoom
    Ok(())
}
```

## Command Execution

### High-Level Methods

The camera provides high-level methods organized by feature:

```rust
use grafton_visca::camera::controls::{
    power::PowerControlBlocking,
    pan_tilt::PanTiltControlBlocking,
    zoom::ZoomControlBlocking,
    presets::PresetsControlBlocking,
    white_balance::WhiteBalanceControlBlocking,
    exposure::ExposureControlBlocking,
};
use grafton_visca::types::{PanSpeed, TiltSpeed};
use grafton_visca::units::{Degrees, Normalized};
use grafton_visca::command::white_balance::WhiteBalanceMode;
use grafton_visca::command::exposure::ExposureMode;
use grafton_visca::command::preset::PresetNumber;

// Power control
camera.power_on()?;
camera.power_standby()?;

// Movement
camera.pan_tilt_home()?;
camera.pan_tilt_absolute(
    Degrees::new(45.0),
    Degrees::new(-15.0),
    PanSpeed::new(10)?,
    TiltSpeed::new(10)?,
)?;
camera.zoom_to(Normalized::new(0.5))?;

// Presets
camera.preset_set(PresetNumber::new(1)?)?;
camera.preset_recall(PresetNumber::new(1)?)?;

// Imaging
camera.set_white_balance_mode(WhiteBalanceMode::Auto)?;
camera.set_exposure_mode(ExposureMode::Manual)?;
```

### Low-Level Commands

For direct VISCA command control:

```rust
use grafton_visca::command::zoom::{Zoom, ZoomSpeed};

// Create command
let cmd = Zoom::TeleVariable(ZoomSpeed::new(5)?);

// Send command (blocking)
camera.send_command(&cmd)?;

// Or use the high-level methods which wrap these commands
use grafton_visca::camera::controls::zoom::ZoomControlBlocking;
camera.zoom_tele_variable(ZoomSpeed::new(5)?)?;
```

### Inquiry Commands

Query camera state:

```rust
use grafton_visca::camera::controls::inquiry::InquiryControlBlocking;

// Get current position (returns raw values)
let (pan, tilt) = camera.pan_tilt().position()?;
println!("Pan: {}, Tilt: {}", pan, tilt);

// Get zoom level
let zoom = camera.zoom().position()?;
println!("Zoom position: {}", zoom);

// Get power state
let power = camera.power().state()?;
println!("Power: {:?}", power);
```

## Error Handling

### Error Categories

The library provides detailed error information:

```rust
use grafton_visca::Error;
use grafton_visca::camera::controls::zoom::ZoomControlBlocking;

match camera.zoom_tele_std() {
    Ok(_) => println!("Success"),
    Err(e) => {
        if e.is_retryable() {
            // Network issue, camera busy, etc. - retryable
            if let Some(delay) = e.suggested_retry_delay() {
                std::thread::sleep(delay);
                // retry...
            }
        } else {
            // Non-retryable error
            println!("Fatal error: {}", e);
        }
    }
}
```

### Retry Logic

The library provides retry guidance:

```rust
use grafton_visca::{Result, Error};

fn execute_with_retry<F, T>(mut f: F) -> Result<T>
where
    F: FnMut() -> Result<T>,
{
    let mut retries = 3;
    loop {
        match f() {
            Ok(val) => return Ok(val),
            Err(e) if e.is_retryable() && retries > 0 => {
                retries -= 1;
                if let Some(delay) = e.suggested_retry_delay() {
                    std::thread::sleep(delay);
                }
            }
            Err(e) => return Err(e),
        }
    }
}

// Usage
use grafton_visca::camera::controls::zoom::ZoomControlBlocking;
let result = execute_with_retry(|| camera.zoom_tele_std())?;
```

## Timeout Management

### Timeout Categories

Commands are automatically categorized for appropriate timeouts:

```rust
use grafton_visca::timeout::{TimeoutConfig, TimeoutCategory};
use std::time::Duration;

let config = TimeoutConfig::builder()
    .quick(Duration::from_secs(1))        // Power, stop commands
    .movement(Duration::from_secs(10))    // Pan/tilt/zoom movements
    .preset(Duration::from_secs(15))      // Preset recall
    .long_running(Duration::from_secs(30)) // Firmware updates
    .network(Duration::from_secs(5))      // Network operations
    .ack(Duration::from_millis(75))       // ACK timeout
    .build();

// Configure timeout on builder (async only)
let camera = CameraBuilder::tokio()?
    .timeout_config(config)
    .build_async::<PtzOpticsG2, _>(transport)
    .await?;
```


## Async vs Blocking

### Blocking API

Zero dependencies, no runtime required:

```rust
use grafton_visca::{Camera, camera::BlockingMode, Result};
use grafton_visca::transport::blocking::tcp::Tcp;
use grafton_visca::camera::profiles::PtzOpticsG2;
use grafton_visca::camera::controls::{
    zoom::ZoomControlBlocking,
    pan_tilt::PanTiltControlBlocking,
};

fn control_camera() -> Result<()> {
    let transport = Tcp::connect("192.168.0.110:5678")?;
    let camera = Camera::<BlockingMode, PtzOpticsG2, _, _>::new(transport);

    camera.pan_tilt_home()?;
    camera.zoom_tele_std()?;
    Ok(())
}
```

### Async API

Runtime-agnostic, works with any executor:

```rust
use grafton_visca::{CameraBuilder, Result};
use grafton_visca::transport::AsyncTransport;
use grafton_visca::runtime::executor::Executor;
use grafton_visca::camera::profiles::PtzOpticsG2;
use grafton_visca::camera::controls::{
    power::PowerControl,
    zoom::ZoomControl,
};

async fn control_camera<E, T>(executor: E, transport: T) -> Result<()>
where
    E: Executor,
    T: AsyncTransport,
{
    let camera = CameraBuilder::with_executor(executor)
        .build_async::<PtzOpticsG2, _>(transport)
        .await?;

    camera.power_on().await?;
    camera.zoom_tele_std().await?;
    Ok(())
}
```

### Tokio Integration

Convenience methods for Tokio users:

```rust
use grafton_visca::{CameraBuilder, transport::tokio::tcp::Tcp, Result};
use grafton_visca::camera::profiles::PtzOpticsG2;
use grafton_visca::camera::controls::{
    power::PowerControl,
    zoom::ZoomControl,
};

#[tokio::main]
async fn main() -> Result<()> {
    let transport = Tcp::connect("192.168.0.110:5678").await?;
    let camera = CameraBuilder::tokio()?
        .build_async::<PtzOpticsG2, _>(transport)
        .await?;

    // Concurrent operations
    let (power, zoom) = tokio::join!(
        camera.power_on(),
        camera.zoom_tele_std()
    );

    Ok(())
}
```

## Testing

### Scripted Transport

Test without physical cameras using scripted transport:

```rust
#[cfg(feature = "test-utils")]
mod tests {
    use grafton_visca::{Camera, camera::BlockingMode};
    use grafton_visca::testing::testkit::{ScriptedBlockingTransport, ScriptEntry};
    use grafton_visca::camera::profiles::GenericVisca;
    use grafton_visca::camera::controls::zoom::ZoomControlBlocking;

    #[test]
    fn test_zoom_command() {
        let script = vec![
            ScriptEntry::exchange(
                &[0x81, 0x01, 0x04, 0x07, 0x02, 0xFF],  // Zoom in command
                &[0x90, 0x41, 0xFF, 0x90, 0x51, 0xFF],  // ACK + Completion
            ),
        ];

        let transport = ScriptedBlockingTransport::new(script);
        let camera = Camera::<BlockingMode, GenericVisca, _, _>::new(transport);
        camera.zoom_tele_std().unwrap();
    }
}
```

### Deterministic Testing

For reproducible async tests:

```rust
#[cfg(feature = "test-utils")]
use grafton_visca::testing::testkit::{ScriptedTransport, ScriptEntry};
use grafton_visca::testing::testkit::DeterministicExecutor;
use grafton_visca::{CameraBuilder, camera::profiles::GenericVisca};
use grafton_visca::camera::controls::power::PowerControl;

#[test]
fn test_protocol_compliance() {
    let executor = DeterministicExecutor::new();

    let script = vec![
        ScriptEntry::exchange(
            &[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF],
            &[0x90, 0x41, 0xFF],  // ACK
        ),
        ScriptEntry::delay(100),
        ScriptEntry::send(&[0x90, 0x51, 0xFF]),  // Completion
    ];

    let transport = ScriptedTransport::new(script, executor.clone());

    executor.block_on(async {
        let camera = CameraBuilder::with_executor(executor.clone())
            .build_async::<GenericVisca, _>(transport)
            .await
            .unwrap();
        camera.power_on().await.unwrap();
    });
}
```

## Best Practices

1. **Use profiles** for type-safe camera control
2. **Handle errors** with proper retry logic
3. **Configure timeouts** based on network conditions
4. **Use async** for concurrent operations
5. **Test with mocks** before deploying to hardware
6. **Enable logging** for debugging (`RUST_LOG=debug`)
7. **Prefer TCP** for reliability in production

## Common Patterns

### Sequential Operations

```rust
use grafton_visca::camera::controls::{
    power::PowerControlBlocking,
    presets::PresetsControlBlocking,
    pan_tilt::PanTiltControlBlocking,
};
use std::time::Duration;

// Blocking
camera.power_on()?;
camera.preset_recall(PresetNumber::new(1)?)?;
camera.await_pan_tilt_idle(Duration::from_secs(10))?;

// Async
use grafton_visca::camera::controls::{
    power::PowerControl,
    presets::PresetsControl,
    pan_tilt::PanTiltControl,
};

camera.power_on().await?;
camera.preset_recall(PresetNumber::new(1)?).await?;
camera.await_pan_tilt_idle(Duration::from_secs(10)).await?;
```

### Concurrent Operations (Async)

```rust
use grafton_visca::camera::controls::{
    pan_tilt::PanTiltControl,
    zoom::ZoomControl,
};

// Execute multiple commands concurrently
let (pan_result, zoom_result) = tokio::join!(
    camera.pan_tilt_left(PanSpeed::new(5)?),
    camera.zoom_tele_std()
);
```

### State Monitoring

```rust
use grafton_visca::camera::controls::{
    pan_tilt::PanTiltControlBlocking,
    zoom::ZoomControlBlocking,
};
use std::time::Duration;

// Wait for movements to complete
camera.await_pan_tilt_idle(Duration::from_secs(10))?;
camera.await_zoom_idle(Duration::from_secs(5))?;
```

## Further Reading

- [VISCA Protocol Reference](visca_unified_reference.md)
- [Camera Compatibility Guide](../README.md#camera-profiles)
- [Examples](../examples/README.md)
- [API Documentation](https://docs.rs/grafton-visca)
