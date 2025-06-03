# grafton-visca

Rust based VISCA over IP implementation for controlling PTZ Cameras

This library provides comprehensive support for PTZOptics G2 VISCA over IP commands and should work with other VISCA-compatible cameras.

Make sure to check out our blog article introducing this library: [Controlling PTZ Cameras with Rust](https://blog.grafton.ai/using-the-grafton-visca-rust-crate-to-control-ptz-cameras-7545f3b4a5e4)

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

## Installation

Add the following to `Cargo.toml` under `[dependencies]`:

```toml
grafton-visca = "0.2"
```

## Usage Examples

### Basic Camera Control

```rust
use grafton_visca::{UdpTransport, ViscaTransport};
use grafton_visca::command::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to camera
    let mut camera = UdpTransport::new("192.168.1.100:5678")?;
    
    // Power on the camera
    camera.send_command(&PowerCommand { power: Power::On })?;
    
    // Move to home position
    camera.send_command(&PanTiltCommand::Home)?;
    
    // Zoom in
    camera.send_command(&ZoomCommand::TeleStandard)?;
    
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
camera.send_command(&abs_pos)?;

// Relative positioning
let rel_pos = PanTiltCommand::RelativePosition {
    pan: -100,    // Move left by 100 units
    tilt: 50,     // Move up by 50 units
    pan_speed: 0x08,
    tilt_speed: 0x08,
};
camera.send_command(&rel_pos)?;
```

### Exposure Control

```rust
use grafton_visca::command::*;

// Set manual exposure mode
camera.send_command(&ExposureCommand { mode: ExposureMode::Manual })?;

// Adjust iris to F4.0
camera.send_command(&IrisCommand::Direct(0x06))?;

// Set shutter speed to 1/1000
camera.send_command(&ShutterCommand::Direct(0x0C))?;

// Enable exposure compensation with +3
camera.send_command(&ExposureCompensationCommand::On)?;
camera.send_command(&ExposureCompensationCommand::Direct(3))?;
```

### Color Adjustments

```rust
use grafton_visca::command::*;

// Perform one-push white balance
camera.send_command(&OnePushTriggerCommand)?;

// Adjust color saturation to 150%
camera.send_command(&SaturationCommand { level: 0x0A })?;

// Fine-tune red gain
camera.send_command(&RedTuningCommand { level: 5 })?;

// Set hue
camera.send_command(&HueCommand { level: 7 })?;
```

### Advanced Image Control

```rust
use grafton_visca::command::*;

// Set sharpness to manual mode and adjust
camera.send_command(&SharpnessCommand::Mode(SharpnessMode::Manual))?;
camera.send_command(&SharpnessCommand::Direct { value: 8 })?;

// Control noise reduction
camera.send_command(&NoiseReduction2DCommand::Level(3))?;
camera.send_command(&NoiseReduction3DCommand::Level(5))?;

// Switch to black & white mode
camera.send_command(&BlackWhiteCommand { on: true })?;

// Set color temperature to 5500K
camera.send_command(&ColorTemperatureCommand::Direct(0x20))?;

// Advanced focus control
camera.send_command(&FocusZoneCommand { zone: FocusZone::Center })?;
camera.send_command(&AFSensitivityCommand { sensitivity: AFSensitivity::High })?;
```

### Querying Camera Status

```rust
use grafton_visca::{send_command_and_wait, ViscaResponse};
use grafton_visca::command::*;

// Query current zoom position
let response = send_command_and_wait(&mut camera, &InquiryCommand::ZoomPosition)?;
if let ViscaResponse::InquiryResponse(ViscaInquiryResponse::ZoomPosition { position }) = response {
    println!("Current zoom position: 0x{:04X}", position);
}

// Query exposure mode
let response = send_command_and_wait(&mut camera, &InquiryCommand::ExposureMode)?;
if let ViscaResponse::InquiryResponse(ViscaInquiryResponse::ExposureMode { mode }) = response {
    println!("Exposure mode: {:?}", mode);
}
```

## Contributing

Contributions are welcome! Please submit a pull request or open an issue to discuss what you would like to change.

## About

This is a project by the [Grafton Machine Shed](https://www.grafton.ai)

## License

This project is licensed under the Apache License, Version 2.0. See the [LICENSE](LICENSE) file for more details.
