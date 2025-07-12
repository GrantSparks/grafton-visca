# grafton-visca

[![Crates.io](https://img.shields.io/crates/v/grafton-visca.svg)](https://crates.io/crates/grafton-visca)
[![Documentation](https://docs.rs/grafton-visca/badge.svg)](https://docs.rs/grafton-visca)
[![License](https://img.shields.io/crates/l/grafton-visca.svg)](LICENSE)

A Rust implementation of the VISCA over IP protocol for controlling PTZ (Pan-Tilt-Zoom) cameras.

**⚠️ Development Status**: This library is in active development and not yet production-ready. API and features may change.

Control your PTZ cameras with convenient APIs:
- 🎯 **Unified Client** - One client works in both sync and async contexts
- 🎬 **Natural Units** - Use degrees, percentages, and magnification instead of raw VISCA values
- 🔄 **Connection Management** - Automatic reconnection and connection pooling support
- 🚦 **Error Handling** - Typed errors to help handle different failure scenarios
- 🏗️ **PTZ Builder** - Compose complex camera movements with method chaining

Supports PTZOptics G2 cameras and other VISCA-compliant devices.

Make sure to check out our blog article introducing this library: [Controlling PTZ Cameras with Rust](https://blog.grafton.ai/using-the-grafton-visca-rust-crate-to-control-ptz-cameras-7545f3b4a5e4)

## Features

- ✅ **VISCA Command Coverage** - Many PTZOptics G2 commands implemented
- ✅ **Protocol Handling** - ACK/Completion state machine implementation
- ✅ **Async/Await Support** - Runtime-agnostic async with optional Tokio integration
- ✅ **Thread Safety** - Safe concurrent access from multiple tasks
- ⚠️ **Testing** - Test infrastructure in place, coverage being expanded
- ✅ **Documentation** - Core APIs documented
- ✅ **Error Handling** - Typed errors for different failure modes
- ✅ **Clean Design** - Focus on usability and maintainability

## What's New in v0.4.0

This release transforms grafton-visca from a low-level protocol implementation into a high-level camera control solution. Here's what's new:

### 🎯 Unified Client Design
No more choosing between sync and async - the new unified `Client` works in any context:

```rust
let client = Client::new("192.168.1.100:52381")?;
client.zoom_in()?;        // Works in sync code
client.zoom_in().await?;  // Works in async code
```

### 🎬 Natural Unit Camera Control
Control cameras using natural units instead of cryptic VISCA values:

```rust
// PTZ Builder for complex shots
client.ptz()
    .pan_tilt_to(-45.0, 15.0)     // Degrees!
    .zoom_to_magnification(10.0)   // 10x zoom!
    .wait()                        // Execute sequentially
    .execute()?;

// High-level operations
client.zoom_to_magnification(5.0)?;              // 5x zoom
client.pan_to_degrees(45.0)?;                    // 45 degrees right
client.set_pan_tilt_percentage(0.5, -0.25)?;     // Center-right, slightly down
```

### 🔄 Built-in Connection Pooling
Built-in connection pooling is now standard for managing multiple cameras:

```rust
// Use CameraPool for managing multiple cameras
use grafton_visca::{
    camera_pool::{CameraPool, CameraInfo, PoolConfig},
    camera::profiles::PTZOpticsG2,
    transport::AsyncTcpTransport,
};

let config = PoolConfig {
    health_check_interval: Duration::from_secs(60),
    auto_remove_unhealthy: true,
    max_idle_time: Some(Duration::from_secs(300)),
    max_cameras: Some(10),
};

let pool = CameraPool::<PTZOpticsG2>::new(config);

// Add cameras to the pool
let info = CameraInfo::new("camera1")
    .with_name("Studio Camera 1")
    .with_location("Main Stage");

let transport = Box::new(AsyncTcpTransport::new("192.168.1.100:5678").await?);
pool.add_camera_async(info, transport).await?;

// Use cameras from the pool
pool.with_camera_async("camera1", |camera| async move {
    camera.home().await
}).await?;
```

### 🚦 Enhanced Error Handling
Detailed errors tell you exactly what went wrong and how to fix it:

```rust
match result {
    Err(e) if e.is_retryable() => {
        // Network error - wait and retry
        sleep(e.suggested_retry_delay());
    }
    Err(Error::CameraMoving) => {
        // Camera is busy - wait for it
    }
    Err(Error::OutOfRange { param, min, max }) => {
        // Clear error message with valid range
    }
}
```

## Features

### Supported Commands

#### Camera Control
- ✅ **Power** - On/Standby control
- ✅ **Pan/Tilt** 
  - Directional movement (Up, Down, Left, Right, UpLeft, UpRight, DownLeft, DownRight)
  - Stop command
  - Home position
  - Reset
  - Absolute positioning
  - Relative positioning
  - Limit set/clear
- ✅ **Zoom**
  - Stop
  - Zoom In/Out (standard and variable speed)
  - Direct position control
- ✅ **Focus**
  - Stop
  - Far/Near (standard and variable speed)
  - Direct position
  - Auto/Manual mode
  - One Push Trigger
  - Infinity
  - Focus Zone selection (Top, Center, Bottom)
  - Auto Focus Sensitivity (High, Normal, Low)
  - Focus Near Limit setting
- ✅ **Preset Positions**
  - Reset, Set, Recall (up to 90 presets)

#### Exposure Control
- ✅ **Exposure Mode** - Auto, Manual, Shutter Priority, Iris Priority, Bright
- ✅ **Exposure Compensation** - On/Off, Reset, Up/Down, Direct value (-7 to +7)
- ✅ **Dynamic Range Control** - Direct level (0-8)
- ✅ **Backlight** - On/Off
- ✅ **Iris** - Reset, Up/Down, Direct (Close to F1.8)
- ✅ **Shutter** - Reset, Up/Down, Direct (1/30 to 1/10000)
- ✅ **Bright** - Reset, Up/Down, Direct (0-17)
- ✅ **Gain** - Reset, Up/Down, Direct (0-7)
- ✅ **Gain Limit** - Direct (0-15)
- ✅ **Anti-Flicker** - Off, 50Hz, 60Hz

#### Color & Image Control
- ✅ **White Balance** - Auto, Indoor, Outdoor, OnePush, Manual, Color Temperature
- ✅ **One-Push White Balance Trigger**
- ✅ **Red/Blue Gain Tuning** - Direct (-10 to +10)
- ✅ **Red/Blue Gain Direct** - Reset, Up/Down, Direct (0x00-0xFF)
- ✅ **Color Temperature** - Reset, Up/Down, Direct (2500K-8000K)
- ✅ **Saturation** - Direct (60% to 200%)
- ✅ **Hue** - Direct (0-14)
- ✅ **Luminance** - Direct (0-14)
- ✅ **Contrast** - Direct (0-14)
- ✅ **Sharpness** - Mode (Auto/Manual), Reset, Up/Down, Direct (0-11)
- ✅ **2D Noise Reduction** - Off, Level 1-5
- ✅ **3D Noise Reduction** - Off, Level 1-8
- ✅ **Black & White Mode** - On/Off
- ✅ **Image Flip** - Vertical On/Off, Combined (Off/Horizontal/Vertical/Both)

#### Inquiry Commands
All set commands have corresponding inquiry commands to read current values:
- ✅ Pan/Tilt Position
- ✅ Zoom Position
- ✅ Focus Position, Zone, AF Sensitivity, Near Limit
- ✅ Exposure Mode
- ✅ White Balance Mode
- ✅ All exposure settings (Compensation, Iris, Shutter, Bright, Gain, Dynamic Range, etc.)
- ✅ All color settings (Saturation, Hue, Red/Blue Gain, Color Temperature)
- ✅ Luminance, Contrast, Sharpness (including mode)
- ✅ Noise Reduction (2D/3D) status
- ✅ Backlight status
- ✅ Black & White mode status
- ✅ Image Flip status

### Transport Support
- ✅ UDP Transport
- ✅ TCP Transport
- ✅ Async Support (with `async` feature)
  - Non-blocking I/O using Tokio
  - Concurrent command execution (respecting VISCA's 2-socket limit)
  - Background response handling
  - Thread-safe client (can be cloned and shared across tasks)

### Camera Constants & Utilities
- ✅ **Camera-specific Constants** - Position limits, speed ranges, preset counts
- ✅ **Position Conversions** - Convert between VISCA units, degrees, and normalized values
- ✅ **Parameter Validation** - Validate positions, speeds, and IDs before sending
- ✅ **Model-Specific Constants** - Support for different camera models with appropriate limits

## Installation

Add the following to `Cargo.toml` under `[dependencies]`:

```toml
# Default: no features (blocking-only support)
grafton-visca = "0.4"

# For runtime-agnostic async (bring your own runtime)
grafton-visca = { version = "0.4", features = ["async"] }

# For async with built-in tokio support
grafton-visca = { version = "0.4", features = ["tokio"] }
```

Connection pooling is now built-in for managing multiple cameras!

## Usage Examples

### Basic Camera Control

#### Blocking Mode (Default)
```rust
use grafton_visca::{Camera, transport::blocking::create, camera::profiles::PTZOpticsG2};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create blocking transport - no async runtime needed!
    let transport = create::tcp("192.168.1.100:5678")?;
    let mut camera = Camera::<PTZOpticsG2>::new(transport);
    
    // Power on
    camera.power_on()?;
    
    // Move to home position
    camera.home()?;
    
    // Zoom control
    camera.zoom_in()?;
    camera.set_zoom(0x4000)?;  // Direct zoom position
    
    Ok(())
}
```

The library provides compile-time safety to prevent accidentally using async transports with blocking cameras:

```rust
// This won't compile - async transport can't be used with CameraBlocking
// let async_transport = AsyncTcpTransport::new("192.168.1.100:5678").await?;
// let camera = CameraBlocking::<PTZOpticsG2>::new(async_transport); // ❌ Compile error!

// Use blocking transports with CameraBlocking
let blocking_transport = TcpTransport::new("192.168.1.100:5678")?;
let camera = CameraBlocking::<PTZOpticsG2>::new(blocking_transport); // ✅ Works!
```

#### Async Mode (Optional)
```rust
use grafton_visca::{Camera, transport::create, camera::profiles::PTZOpticsG2};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create async transport
    let transport = create::tcp("192.168.1.100:5678").await?;
    let camera = Camera::<PTZOpticsG2>::new(transport);
    
    // Power on
    camera.power_on().await?;
    
    // Move to home position
    camera.home().await?;
    
    // Zoom control
    camera.zoom_in().await?;
    camera.set_zoom(0x4000).await?;
    
    Ok(())
}
```

### Camera Model Configuration

For best results, specify your camera model when creating a client:

```rust
use grafton_visca::{Client, constants::CameraModel};

// Configure camera model for automatic command validation
let client = Client::builder()
    .camera_model(CameraModel::PTZOpticsG2)
    .connect_udp("192.168.1.100:5678")?;

// Commands are now validated before sending
match client.send(&ZoomCommand::Direct(0x7AC0)) {  // 30X zoom position
    Err(Error::ModelValidation { model, command, reason }) => {
        // This would error for PTZOpticsG2 which only supports 20X zoom
        println!("Command {} not valid for {:?}: {}", command, model, reason);
    }
    Ok(_) => {
        // Command executed successfully
    }
}
```

Without model specification, all commands are sent directly to the camera,
which may respond with `CommandNotExecutable` errors for unsupported features.

#### Commands with Model-Specific Validation

When a camera model is configured, the following commands validate their parameters:

**Position & Movement:**
- **Zoom Direct Position** - Validates against camera's optical zoom range
  - PTZOpticsG2: 0x0000-0x7000 (20X)
  - PTZOptics30X: 0x0000-0x7AC0 (30X)
- **Pan/Tilt Absolute Position** - Validates pan and tilt ranges
  - Pan: -2448 to +2448 VISCA units
  - Tilt: -432 to +1296 VISCA units
- **Focus Direct Position** - Validates focus range
  - Range: 0x1000-0xF000 (infinity to near)
- **Focus Near Limit** - Uses same range as Focus Direct
  - Range: 0x1000-0xF000
- **Preset Numbers** - Validates preset ID range
  - PTZOpticsG2: 0-89 (90 presets)
  - Other models: 0-100

**Exposure & Gain:**
- **Shutter Speed Direct** - Validates against 17 specific shutter values
  - PTZOpticsG2: 0x01 (1/30) through 0x11 (1/10000)
- **Iris Level Direct** - Validates against 13 specific iris values
  - PTZOpticsG2: 0x00 (Close) through 0x0C (F1.8)
- **Gain Direct** - Validates against 8 specific gain values
  - PTZOpticsG2: 0x00 (0dB) through 0x07 (21dB)
- **Gain Limit** - Validates gain limit range
  - PTZOpticsG2: 0x0-0xF (16 values)
- **Brightness Direct** - Validates brightness level range
  - PTZOpticsG2: 0x00-0x11 (18 values)
- **Dynamic Range Direct** - Validates dynamic range level
  - PTZOpticsG2: 0-8 (9 values)
- **Exposure Compensation Direct** - Validates compensation range
  - PTZOpticsG2: -7 to +7

**Image & Color:**
- **Sharpness Direct** - Validates against 12 specific sharpness values
  - PTZOpticsG2: 0x00 through 0x0B
- **Luminance** - Validates luminance level range
  - PTZOpticsG2: 0x0-0xE (15 values)
- **Contrast** - Validates contrast level range
  - PTZOpticsG2: 0x0-0xE (15 values)
- **Red/Blue Tuning** - Validates tuning range
  - PTZOpticsG2: -10 to +10
- **Saturation** - Validates saturation level range
  - PTZOpticsG2: 0x0-0xE (15 values, 60%-200%)
- **Hue** - Validates hue level range
  - PTZOpticsG2: 0x0-0xE (15 values)

All other commands use built-in parameter validation that works across all camera models.

### Advanced Positioning

```rust
use grafton_visca::prelude::*;

// Move using degrees (so much clearer!)
camera.pan_to_degrees(45.0)?;
camera.tilt_to_degrees(-15.0)?;

// Or move to specific pan/tilt position
camera.pan_tilt_to_degrees(90.0, 30.0)?;

// Use percentages for normalized positioning
camera.set_pan_tilt_percentage(0.5, 0.0)?;  // Center horizontally, neutral tilt

// PTZ Builder for complex movements
camera.ptz()
    .pan_to_degrees(-45.0)
    .tilt_to_degrees(20.0)
    .zoom_to_magnification(10.0)
    .focus_auto()
    .wait()  // Execute in sequence
    .execute()?;

// Or execute movements in parallel
camera.ptz()
    .home()
    .zoom_to_magnification(1.0)
    .concurrent()  // Execute simultaneously
    .execute()?;
```

### Exposure Control

```rust
use grafton_visca::command::*;

// Set manual exposure mode
camera.send(&ExposureCommand { mode: ExposureMode::Manual })?;

// Adjust iris to F4.0
camera.send(&IrisCommand::Direct(0x06))?;

// Set shutter speed to 1/1000
camera.send(&ShutterCommand::Direct(0x0C))?;

// Enable exposure compensation with +3
camera.send(&ExposureCompensationCommand::On)?;
camera.send(&ExposureCompensationCommand::Direct(3))?;
```

### Color Adjustments

```rust
use grafton_visca::command::*;

// Perform one-push white balance
camera.send(&OnePushTriggerCommand)?;

// Adjust color saturation to 150%
camera.send(&SaturationCommand { level: 0x0A })?;

// Fine-tune red gain
camera.send(&RedTuningCommand { level: 5 })?;

// Set hue
camera.send(&HueCommand { level: 7 })?;
```

### Advanced Image Control

```rust
use grafton_visca::command::*;

// Set sharpness to manual mode and adjust
camera.send(&SharpnessCommand::Mode(SharpnessMode::Manual))?;
camera.send(&SharpnessCommand::Direct { value: 8 })?;

// Control noise reduction
camera.send(&NoiseReduction2DCommand::Level(3))?;
camera.send(&NoiseReduction3DCommand::Level(5))?;

// Switch to black & white mode
camera.send(&BlackWhiteCommand { on: true })?;

// Set color temperature to 5500K
camera.send(&ColorTemperatureCommand::Direct(0x20))?;

// Advanced focus control
camera.send(&FocusZoneCommand { zone: FocusZone::Center })?;
camera.send(&AFSensitivityCommand { sensitivity: AFSensitivity::High })?;
```

### Querying Camera Status

```rust
use grafton_visca::{Client, Response};
use grafton_visca::command::*;

// Query current zoom position
let response = camera.send(&ZoomPositionInquiry)?;
if let Response::InquiryResponse(InquiryResponse::ZoomPosition { position }) = response {
    println!("Current zoom position: 0x{:04X}", position);
}

// Query exposure mode
let response = camera.send(&ExposureModeInquiry)?;
if let Response::InquiryResponse(InquiryResponse::ExposureMode { mode }) = response {
    println!("Exposure mode: {:?}", mode);
}
```

### Advanced Features

```rust
use grafton_visca::prelude::*;
use std::time::Duration;

// Create a standard client
let client = Client::new("192.168.1.100:52381")?;

// Connection pooling for multiple cameras
let pool = ConnectionPool::builder()
    .add_camera("192.168.1.100:52381", "Camera 1")
    .add_camera("192.168.1.101:52381", "Camera 2")
    .add_camera("192.168.1.102:52381", "Camera 3")
    .with_health_check_interval(Duration::from_secs(30))
    .build()?;

// Get camera from pool and use it
let camera = pool.get("Camera 1").await?;
camera.pan_to_degrees(45.0)?;

// Smart error handling with retry logic
loop {
    match camera.zoom_to_magnification(10.0) {
        Ok(_) => break,
        Err(e) if e.is_retryable() => {
            log::warn!("Retryable error: {}, waiting...", e);
            std::thread::sleep(e.suggested_retry_delay());
            continue;
        }
        Err(e) => return Err(e.into()),
    }
}
```

### Async Usage

#### With Tokio (built-in support)

When using the `tokio` feature, you get ready-to-use async transports:

```rust
use grafton_visca::{Camera, ProfileId};
use grafton_visca::transport::tokio::Tcp;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Built-in TCP transport for tokio
    let transport = Tcp::connect("192.168.1.100:5678").await?;
    let camera = Camera::with_profile(ProfileId::PTZOpticsG2, transport);
    
    // All operations are async
    camera.power_on().await?;
    camera.pan_tilt_home().await?;
    camera.zoom_to_magnification(5.0).await?;
    
    Ok(())
}
```

#### Runtime-Agnostic Async (bring your own runtime)

With just the `async` feature, you can use any async runtime:

```rust
use grafton_visca::{Camera, ProfileId, transport::Transport};
use async_std::net::TcpStream;  // or smol, embassy, etc.

// Implement Transport for your runtime's types
struct MyTransport {
    stream: TcpStream,
}

impl Transport for MyTransport {
    // ... implementation details ...
}

// Use with your runtime's timeout facilities
use async_std::future::timeout;
use std::time::Duration;

#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stream = TcpStream::connect("192.168.1.100:5678").await?;
    let transport = MyTransport { stream };
    let camera = Camera::with_profile(ProfileId::PTZOpticsG2, transport);
    
    // Wrap operations with your runtime's timeout
    timeout(Duration::from_secs(5), camera.power_on()).await??;
    
    Ok(())
}
```

See `examples/runtime_agnostic_async.rs` for a complete example.

#### Unified Client (deprecated API)

The unified `Client` API from v0.4.0 is still available but deprecated:

```rust
use grafton_visca::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Same client, just use async methods!
    let camera = Client::new("192.168.1.100:52381")?;
    
    // All methods have async versions
    camera.power_on().await?;
    camera.home().await?;
    
    // Concurrent operations with PTZ builder
    camera.ptz()
        .pan_to_degrees(45.0)
        .zoom_to_magnification(5.0)
        .concurrent()  // Execute simultaneously
        .execute()
        .await?;
    
    // The client is Clone + Send + Sync, suitable for async
    let cam1 = camera.clone();
    let cam2 = camera.clone();
    
    // Use in multiple tasks
    let task1 = tokio::spawn(async move {
        cam1.get_zoom_position().await
    });
    
    let task2 = tokio::spawn(async move {
        cam2.get_pan_tilt_position().await
    });
    
    let (zoom, position) = tokio::try_join!(task1, task2)?;
    
    Ok(())
}
```

The client automatically manages VISCA's two-socket limitation, queuing commands as needed.

### Handling Unknown Response Types

The library now provides robust handling for unknown or unimplemented response types:

```rust
use grafton_visca::{Response, ResponseType};

// When querying camera features, unknown responses are captured
let response = camera.send(&ColorTemperatureInquiry)?;
match response {
    Response::InquiryResponse(data) => {
        // Handle known response type
    }
    Response::Unknown { response_type, data } => {
        // Unknown response type is preserved with raw data
        if let Some(rt) = response_type {
            println!("Unknown response for {:?}: {:?}", rt, data);
        }
    }
    _ => {}
}
```

This pattern ensures:
- Forward compatibility with new camera features
- Safe handling of vendor-specific extensions
- Debugging information for unimplemented features
- No silent data loss or misinterpretation

### Thread-Safe Camera Control

For concurrent camera control from multiple threads, use the `ChannelTransport`:

```rust
use grafton_visca::{
    Camera,
    camera::profiles::PTZOpticsG2,
    transport::{ChannelTransport, ChannelTransportBuilder, Priority, AsyncTcpTransport},
};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Create base transport
    let base_transport = AsyncTcpTransport::new("192.168.1.100:5678").await?;
    
    // Wrap with ChannelTransport for thread-safe access
    let transport = ChannelTransportBuilder::new(base_transport)
        .with_max_concurrent_commands(2)  // VISCA's 2-socket limit
        .with_queue_size(100)
        .with_operation_timeout(Duration::from_secs(5))
        .build()?;
    
    let camera = Camera::<PTZOpticsG2>::new(transport.clone());
    
    // Clone the camera for use in multiple tasks
    let cam1 = camera.clone();
    let cam2 = camera.clone();
    
    // Use from multiple async tasks concurrently
    let task1 = tokio::spawn(async move {
        cam1.home().await
    });
    
    let task2 = tokio::spawn(async move {
        cam2.zoom_to_magnification(5.0).await
    });
    
    // Commands are automatically queued and managed
    let (r1, r2) = tokio::try_join!(task1, task2)?;
    
    Ok(())
}
```

The `ChannelTransport` provides:
- Thread-safe access without explicit locking
- Automatic socket management for VISCA's 2-socket limitation
- Priority-based command queuing
- Configurable queue sizes and timeouts
- Works with any underlying transport (TCP or UDP)

## Migrating from v0.3.0

The v0.4.0 release simplifies the API while adding powerful new features:

```rust
// Old (v0.3.0)
let client = ViscaClient::new(...);      // Sync only
let client = AsyncViscaClient::new(...); // Async only

// New (v0.4.0) - One client for everything!
let client = Client::new("192.168.1.100:52381")?;

// Old: Manual VISCA units
camera.send(&PanTilt::AbsolutePosition { 
    pan: 0x1000, tilt: 0x0500, pan_speed: 0x10, tilt_speed: 0x10 
})?;

// New: Use degrees, percentages, or magnification
camera.pan_tilt_to_degrees(45.0, 15.0)?;
camera.set_pan_tilt_percentage(0.5, 0.0)?;
camera.zoom_to_magnification(5.0)?;
```

Key changes:
- Single `Client` replaces separate sync/async clients
- Connection pooling is now built-in
- High-level extension trait methods for common operations
- PTZ builder for complex camera movements
- Detailed error types with retry helpers
- Feature flags simplified to just `async` for async runtime support

See the [CHANGELOG](CHANGELOG.md) for complete migration details.

## Documentation

- [User Guide](docs/USER_GUIDE.md) - Comprehensive guide for using the library
- [API Documentation](https://docs.rs/grafton-visca) - Full API reference
- [Contributing Guide](CONTRIBUTING.md) - How to contribute to the project

## Contributing

Contributions are welcome! Please read our [Contributing Guide](CONTRIBUTING.md) for details on our code of conduct and the process for submitting pull requests.

## About

This is a project by the [Grafton Machine Shed](https://www.grafton.ai)

## License

This project is licensed under the Apache License, Version 2.0. See the [LICENSE](LICENSE) file for more details.
