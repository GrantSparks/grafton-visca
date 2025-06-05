# grafton-visca

[![Crates.io](https://img.shields.io/crates/v/grafton-visca.svg)](https://crates.io/crates/grafton-visca)
[![Documentation](https://docs.rs/grafton-visca/badge.svg)](https://docs.rs/grafton-visca)
[![License](https://img.shields.io/crates/l/grafton-visca.svg)](LICENSE)

A production-ready Rust implementation of the VISCA over IP protocol for controlling PTZ (Pan-Tilt-Zoom) cameras.

This library provides comprehensive support for PTZOptics G2 VISCA over IP commands and is compatible with other VISCA-compliant cameras. It features a robust state machine for reliable command execution, full async/await support, and proper handling of the VISCA two-socket limitation.

Make sure to check out our blog article introducing this library: [Controlling PTZ Cameras with Rust](https://blog.grafton.ai/using-the-grafton-visca-rust-crate-to-control-ptz-cameras-7545f3b4a5e4)

## Production Ready Features

- ✅ **Complete VISCA Command Coverage** - All PTZOptics G2 commands implemented
- ✅ **Robust Protocol Handling** - Proper ACK/Completion state machine
- ✅ **Async/Await Support** - Modern async API with Tokio
- ✅ **Thread Safety** - Safe concurrent access from multiple tasks
- ✅ **Comprehensive Testing** - >90% test coverage with unit and integration tests
- ✅ **Full Documentation** - All public APIs documented with examples
- ✅ **Error Handling** - Detailed error types for all failure modes
- ✅ **Performance** - <5ms overhead per command

## Recent Improvements

### v0.4.0 (Unified Client)
- **Unified Client Architecture:** Single `ViscaClient` handles both blocking and async operations
- **Simplified API:** One client type with `send()` for blocking and `send_async()` for async
- **Smart Runtime Detection:** Blocking façade automatically uses existing tokio runtime when available
- **Feature Simplification:** Just `blocking-client` (default) and `async-client` features
- **Standard Features:** Connection pooling and reconnection now included by default
- **Enhanced Error Handling:** Retry helpers and error classification for robust operation
- **AI-Agent Friendly:** Designed for clear, self-describing API patterns

### v0.3.0 (Production Release)
- **Async/Await Support:** Added full async support with `AsyncViscaClient` for non-blocking camera control
- **Concurrent Command Execution:** Send up to 2 commands simultaneously with automatic socket management
- **Background Response Handling:** Responses are processed in a background task for optimal performance
- **Thread-Safe Design:** The async client can be cloned and shared safely across tasks
- **Backward Compatibility:** Sync API remains unchanged; async is opt-in via the `async` feature flag

### v0.2.2 (Sprint 2)
- **Correct ACK/Completion Handling:** The library now properly distinguishes between ACK (acknowledgment) and Completion responses from the camera, implementing a robust state machine that tracks command execution through its full lifecycle.
- **Socket Management:** Proper handling of VISCA's two-socket limitation, preventing command buffer full errors through internal state tracking.
- **Error Response Classification:** All VISCA error responses (Syntax Error, Command Buffer Full, Command Not Executable, etc.) are now properly parsed and returned as specific error types.

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
  - Tele/Wide (standard and variable speed)
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
- ✅ **Model Detection** - Detect camera model and use model-specific constants

## Installation

Add the following to `Cargo.toml` under `[dependencies]`:

```toml
# Default includes blocking client with all features
grafton-visca = "0.4"

# For async client support
grafton-visca = { version = "0.4", features = ["async-client"] }

# For both blocking and async clients
grafton-visca = { version = "0.4", features = ["blocking-client", "async-client"] }
```

The library includes connection pooling and automatic reconnection capabilities as standard features.

## Usage Examples

### Basic Camera Control

```rust
use grafton_visca::ViscaClient;
use grafton_visca::command::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to camera
    let camera = ViscaClient::connect_udp("192.168.1.100:5678")?;
    
    // Power on the camera
    camera.send(&PowerCommand { power: Power::On })?;
    
    // Move to home position
    camera.send(&PanTiltCommand::Home)?;
    
    // Zoom in
    camera.send(&ZoomCommand::TeleStandard)?;
    
    Ok(())
}
```

### Advanced Positioning

```rust
use grafton_visca::command::*;

// Absolute positioning
let abs_pos = PanTiltCommand::AbsolutePosition {
    pan: 1000,    // Pan position
    tilt: 500,    // Tilt position
    pan_speed: 0x10,
    tilt_speed: 0x10,
};
camera.send(&abs_pos)?;

// Relative positioning
let rel_pos = PanTiltCommand::RelativePosition {
    pan: -100,    // Move left by 100 units
    tilt: 50,     // Move up by 50 units
    pan_speed: 0x08,
    tilt_speed: 0x08,
};
camera.send(&rel_pos)?;
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
use grafton_visca::{ViscaClient, ViscaResponse};
use grafton_visca::command::*;

// Query current zoom position
let response = camera.send(&InquiryCommand::ZoomPosition)?;
if let ViscaResponse::InquiryResponse(ViscaInquiryResponse::ZoomPosition { position }) = response {
    println!("Current zoom position: 0x{:04X}", position);
}

// Query exposure mode
let response = camera.send(&InquiryCommand::ExposureMode)?;
if let ViscaResponse::InquiryResponse(ViscaInquiryResponse::ExposureMode { mode }) = response {
    println!("Exposure mode: {:?}", mode);
}
```

### Using Camera Constants and Position Conversion

```rust
use grafton_visca::constants::{CameraModel, CameraConstants, PositionConversion, DegreePosition};
use grafton_visca::constants;

// Use camera-specific constants
let model = CameraModel::PTZOpticsG2;
println!("Pan range: {:?} VISCA units", model.pan_range());
println!("Pan degrees: {} degrees", model.pan_degrees());

// Convert between units
let degrees = DegreePosition { pan: 45.0, tilt: 15.0 };
let visca_pos = degrees.to_visca(model);
println!("45° pan = {} VISCA units", visca_pos.pan);

// Validate parameters before sending
match constants::validate_pan_position(2000, model) {
    Ok(_) => println!("Position is valid"),
    Err(e) => println!("Invalid: {}", e),
}

// Move to position specified in degrees
let target = DegreePosition { pan: -90.0, tilt: 30.0 };
let visca = target.to_visca(model);
camera.send_command(&PanTiltCommand::AbsolutePosition {
    pan: visca.pan,
    tilt: visca.tilt,
    pan_speed: constants::speed::PAN_SPEED_DEFAULT,
    tilt_speed: constants::speed::TILT_SPEED_DEFAULT,
})?;
```

### Async Usage (with `async-client` feature)

The library supports asynchronous operation for non-blocking camera control:

```rust
use grafton_visca::{ViscaClient, ViscaResponse};
use grafton_visca::command::*;
use grafton_visca::command::pan_tilt::{PanTiltDirection, PanSpeed, TiltSpeed};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to camera asynchronously
    let camera = ViscaClient::connect_udp_async("192.168.1.100:5678").await?;
    
    // Send multiple commands concurrently
    let pan_tilt = camera.send_async(&PanTiltCommand::Move {
        direction: PanTiltDirection::UpRight,
        pan_speed: PanSpeed::new(0x10)?,
        tilt_speed: TiltSpeed::new(0x10)?,
    });
    let zoom = camera.send_async(&ZoomCommand::TeleStandard);
    
    // Both commands execute concurrently (respecting the 2-socket limit)
    let (pan_result, zoom_result) = tokio::join!(pan_tilt, zoom);
    
    println!("Pan/Tilt: {:?}, Zoom: {:?}", pan_result?, zoom_result?);
    
    Ok(())
}
```

#### Concurrent Commands with Socket Limiting

The async client automatically manages the VISCA two-socket limitation:

```rust
// Send three commands - the third will wait for a socket to become available
let cmd1 = camera.send_async(&PresetCommand { action: PresetAction::Set, preset_number: 1 });
let cmd2 = camera.send_async(&FocusCommand::NearStandard);
let cmd3 = camera.send_async(&ZoomCommand::WideStandard);

// The first two commands will execute immediately,
// the third will wait until one of them completes
let results = tokio::join!(cmd1, cmd2, cmd3);
```

#### Clone and Share Across Tasks

The async client is thread-safe and can be cloned:

```rust
let camera_clone = camera.clone();

// Use in multiple tasks
let task1 = tokio::spawn(async move {
    camera_clone.send_async(&PowerCommand { power: Power::On }).await
});

let camera_clone2 = camera.clone();
let task2 = tokio::spawn(async move {
    camera_clone2.send_async(&InquiryCommand::ZoomPosition).await
});

let (res1, res2) = tokio::join!(task1, task2);
```

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
