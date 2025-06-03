# grafton-visca User Guide

This guide provides detailed information on using the grafton-visca library to control VISCA-compatible PTZ cameras.

## Table of Contents

- [Getting Started](#getting-started)
- [Setting Up Your Camera](#setting-up-your-camera)
- [Connection Types](#connection-types)
- [Basic Camera Control](#basic-camera-control)
- [Advanced Features](#advanced-features)
- [Understanding Responses](#understanding-responses)
- [Error Handling](#error-handling)
- [Performance Tips](#performance-tips)
- [Troubleshooting](#troubleshooting)

## Getting Started

### Installation

Add grafton-visca to your `Cargo.toml`:

```toml
[dependencies]
# For synchronous usage only
grafton-visca = "0.3"

# For async support
grafton-visca = { version = "0.3", features = ["async"] }

# For both sync and async
grafton-visca = { version = "0.3", features = ["full"] }
```

### Your First Program

Here's a minimal example to get you started:

```rust
use grafton_visca::{UdpTransport, ViscaTransport, send_command_and_wait};
use grafton_visca::command::{PowerCommand, power::Power};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to camera at IP 192.168.1.100
    let mut transport = UdpTransport::new("192.168.1.100:1259")?;
    
    // Power on the camera
    let command = PowerCommand { power: Power::On };
    let response = send_command_and_wait(&mut transport, &command)?;
    
    println!("Camera powered on: {:?}", response);
    Ok(())
}
```

## Setting Up Your Camera

### Network Configuration

Before using grafton-visca, ensure your camera is properly configured for VISCA over IP:

1. **Enable VISCA over IP** in your camera's network settings
2. **Note the IP address** assigned to your camera
3. **Verify the port numbers**:
   - UDP: Usually port 1259 (PTZOptics default)
   - TCP: Usually port 5678 (alternative port)

### Testing Connectivity

You can test if your camera is reachable:

```bash
# Test UDP connectivity
nc -u -v 192.168.1.100 1259

# Test TCP connectivity
nc -v 192.168.1.100 5678
```

## Connection Types

### UDP Transport

UDP is the most common transport for VISCA over IP:

```rust
use grafton_visca::UdpTransport;

// Connect using UDP (PTZOptics default port)
let mut transport = UdpTransport::new("192.168.1.100:1259")?;
```

**Advantages:**
- Lower latency
- Standard for most PTZ cameras
- No connection state to manage

**Disadvantages:**
- No guaranteed delivery
- May lose packets on congested networks

### TCP Transport

TCP provides reliable communication:

```rust
use grafton_visca::TcpTransport;

// Connect using TCP
let mut transport = TcpTransport::new("192.168.1.100:5678")?;
```

**Advantages:**
- Guaranteed delivery
- Connection state ensures camera is reachable
- Better for unreliable networks

**Disadvantages:**
- Slightly higher latency
- Connection must be maintained

## Basic Camera Control

### Movement Commands

Control pan and tilt movement:

```rust
use grafton_visca::command::{PanTiltCommand, pan_tilt::{PanTiltDirection, PanSpeed, TiltSpeed}};

// Move to home position
transport.send_command(&PanTiltCommand::Home)?;

// Move in a specific direction
let move_cmd = PanTiltCommand::Move {
    direction: PanTiltDirection::UpRight,
    pan_speed: PanSpeed::new(0x10)?,  // Medium speed
    tilt_speed: TiltSpeed::new(0x10)?,
};
transport.send_command(&move_cmd)?;

// Stop movement
let stop_cmd = PanTiltCommand::Move {
    direction: PanTiltDirection::Stop,
    pan_speed: PanSpeed::new(0)?,
    tilt_speed: TiltSpeed::new(0)?,
};
transport.send_command(&stop_cmd)?;
```

### Zoom Control

Control camera zoom:

```rust
use grafton_visca::command::ZoomCommand;

// Zoom in at standard speed
transport.send_command(&ZoomCommand::TeleStandard)?;

// Zoom out at variable speed (0-7, where 7 is fastest)
transport.send_command(&ZoomCommand::WideVariable(5))?;

// Stop zooming
transport.send_command(&ZoomCommand::Stop)?;

// Set specific zoom position
transport.send_command(&ZoomCommand::Direct(0x4000))?;
```

### Focus Control

Manage camera focus:

```rust
use grafton_visca::command::FocusCommand;

// Set auto focus
transport.send_command(&FocusCommand::Auto)?;

// Manual focus adjustment
transport.send_command(&FocusCommand::Manual)?;
transport.send_command(&FocusCommand::NearStandard)?;

// One-push auto focus
transport.send_command(&FocusCommand::OnePushTrigger)?;
```

### Preset Positions

Save and recall camera positions:

```rust
use grafton_visca::command::{PresetCommand, preset::PresetAction};

// Save current position as preset 1
let save_preset = PresetCommand {
    action: PresetAction::Set,
    preset_number: 1,
};
transport.send_command(&save_preset)?;

// Recall preset 1
let recall_preset = PresetCommand {
    action: PresetAction::Recall,
    preset_number: 1,
};
transport.send_command(&recall_preset)?;

// Clear preset 1
let clear_preset = PresetCommand {
    action: PresetAction::Reset,
    preset_number: 1,
};
transport.send_command(&clear_preset)?;
```

## Advanced Features

### Absolute Positioning

Move the camera to exact coordinates:

```rust
use grafton_visca::command::PanTiltCommand;

// Move to specific pan/tilt position
let abs_pos = PanTiltCommand::AbsolutePosition {
    pan: 1000,      // Pan position
    tilt: 500,      // Tilt position
    pan_speed: 0x18,  // Maximum speed
    tilt_speed: 0x14, // Maximum speed
};
send_command_and_wait(&mut transport, &abs_pos)?;
```

### Relative Positioning

Move relative to current position:

```rust
// Move 100 units left and 50 units up from current position
let rel_pos = PanTiltCommand::RelativePosition {
    pan: -100,     // Negative = left
    tilt: 50,      // Positive = up
    pan_speed: 0x10,
    tilt_speed: 0x10,
};
send_command_and_wait(&mut transport, &rel_pos)?;
```

### Exposure Control

Fine-tune camera exposure:

```rust
use grafton_visca::command::*;

// Set exposure mode
transport.send_command(&ExposureCommand { 
    mode: ExposureMode::Manual 
})?;

// Adjust individual parameters
transport.send_command(&IrisCommand::Direct(0x08))?;     // F5.6
transport.send_command(&ShutterCommand::Direct(0x0A))?;  // 1/250
transport.send_command(&GainCommand::Direct(0x02))?;     // +6dB

// Enable exposure compensation
transport.send_command(&ExposureCompensationCommand::On)?;
transport.send_command(&ExposureCompensationCommand::Direct(2))?; // +2 EV
```

### Color Adjustment

Control white balance and color:

```rust
use grafton_visca::command::*;

// Set white balance mode
transport.send_command(&WhiteBalanceCommand { 
    mode: WhiteBalanceMode::Auto 
})?;

// Manual color temperature (2500K-8000K)
transport.send_command(&WhiteBalanceCommand { 
    mode: WhiteBalanceMode::Manual 
})?;
transport.send_command(&ColorTemperatureCommand::Direct(0x20))?; // ~5500K

// Adjust color parameters
transport.send_command(&SaturationCommand { level: 0x08 })?;  // 100%
transport.send_command(&HueCommand { level: 0x07 })?;         // Neutral
```

## Understanding Responses

### Response Types

The library returns different response types:

```rust
use grafton_visca::{ViscaResponse, ViscaInquiryResponse};

match send_command_and_wait(&mut transport, &command)? {
    ViscaResponse::Ack => {
        println!("Command acknowledged");
    }
    ViscaResponse::Completion => {
        println!("Command completed successfully");
    }
    ViscaResponse::InquiryResponse(data) => {
        match data {
            ViscaInquiryResponse::ZoomPosition { position } => {
                println!("Zoom position: 0x{:04X}", position);
            }
            ViscaInquiryResponse::PanTiltPosition { pan, tilt } => {
                println!("Pan: {}, Tilt: {}", pan, tilt);
            }
            _ => println!("Other inquiry response: {:?}", data),
        }
    }
    ViscaResponse::Error(err) => {
        println!("Camera error: {:?}", err);
    }
}
```

### Inquiry Commands

Query camera state:

```rust
use grafton_visca::command::InquiryCommand;

// Query zoom position
let response = send_command_and_wait(
    &mut transport, 
    &InquiryCommand::ZoomPosition
)?;

// Query pan/tilt position
let response = send_command_and_wait(
    &mut transport, 
    &InquiryCommand::PanTiltPosition
)?;

// Query exposure mode
let response = send_command_and_wait(
    &mut transport, 
    &InquiryCommand::ExposureMode
)?;
```

## Error Handling

### Common Errors

The library provides detailed error information:

```rust
use grafton_visca::ViscaError;

match send_command_and_wait(&mut transport, &command) {
    Ok(response) => println!("Success: {:?}", response),
    Err(ViscaError::CommandBufferFull) => {
        println!("Camera is busy, retry later");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Err(ViscaError::CommandNotExecutable) => {
        println!("Command cannot be executed in current state");
    }
    Err(ViscaError::SyntaxError) => {
        println!("Invalid command syntax");
    }
    Err(ViscaError::Io(e)) => {
        println!("Network error: {}", e);
    }
    Err(e) => {
        println!("Other error: {:?}", e);
    }
}
```

### Retry Logic

Implement retry for transient errors:

```rust
fn send_with_retry<T: ViscaTransport>(
    transport: &mut T,
    command: &dyn ViscaCommand,
    max_retries: u32,
) -> Result<ViscaResponse, ViscaError> {
    let mut retries = 0;
    
    loop {
        match send_command_and_wait(transport, command) {
            Ok(response) => return Ok(response),
            Err(ViscaError::CommandBufferFull) if retries < max_retries => {
                retries += 1;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(e),
        }
    }
}
```

## Performance Tips

### Command Timing

The VISCA protocol has limitations:
- Cameras can process only 2 commands simultaneously
- Mechanical movements take time to complete
- Some commands block others (e.g., zoom blocks focus)

### Best Practices

1. **Wait for Completion**: Use `send_command_and_wait` to ensure commands complete
2. **Avoid Command Flooding**: Space out rapid commands
3. **Check Camera State**: Query before commanding when necessary
4. **Handle Errors Gracefully**: Implement retry logic for busy cameras

### Async Performance

For better performance with multiple cameras or commands:

```rust
#[cfg(feature = "async")]
use grafton_visca::AsyncViscaClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let camera = AsyncViscaClient::connect_udp("192.168.1.100:1259").await?;
    
    // Commands execute concurrently (up to 2 at once)
    let pan_future = camera.send(&PanTiltCommand::Home);
    let zoom_future = camera.send(&ZoomCommand::Direct(0x4000));
    
    let (pan_result, zoom_result) = tokio::join!(pan_future, zoom_future);
    
    Ok(())
}
```

## Troubleshooting

### Camera Not Responding

1. **Check Network Connection**:
   - Ping the camera IP
   - Verify VISCA over IP is enabled
   - Check firewall settings

2. **Verify Port Numbers**:
   - UDP typically uses 1259
   - TCP typically uses 5678
   - Check camera documentation

3. **Test with Simple Commands**:
   - Try power on/off first
   - Use inquiry commands to test communication

### Commands Failing

1. **Command Buffer Full**:
   - Camera is processing other commands
   - Add delays between commands
   - Use async for better concurrency

2. **Command Not Executable**:
   - Camera may be powered off
   - Feature might not be available
   - Camera might be in wrong mode

3. **Invalid Parameters**:
   - Check speed limits (pan: 0-24, tilt: 0-20)
   - Verify position ranges
   - Ensure preset numbers are valid (0-89)

### Performance Issues

1. **Slow Response**:
   - Use UDP instead of TCP for lower latency
   - Check network congestion
   - Reduce command frequency

2. **Lost Commands**:
   - Switch to TCP for reliability
   - Implement acknowledgment checking
   - Add retry logic

## Example Applications

### Camera Tour

Create an automated camera tour:

```rust
use std::time::Duration;
use std::thread;

fn camera_tour(transport: &mut dyn ViscaTransport) -> Result<(), ViscaError> {
    let positions = vec![
        (1000, 500),   // Position 1
        (-1000, 500),  // Position 2
        (0, 0),        // Center
    ];
    
    for (pan, tilt) in positions {
        let cmd = PanTiltCommand::AbsolutePosition {
            pan,
            tilt,
            pan_speed: 0x10,
            tilt_speed: 0x10,
        };
        
        send_command_and_wait(transport, &cmd)?;
        thread::sleep(Duration::from_secs(5)); // Wait at position
    }
    
    Ok(())
}
```

### Auto-Tracking

Simple motion tracking example:

```rust
fn track_to_position(
    transport: &mut dyn ViscaTransport,
    target_pan: i16,
    target_tilt: i16,
) -> Result<(), ViscaError> {
    // Query current position
    let response = send_command_and_wait(
        transport,
        &InquiryCommand::PanTiltPosition
    )?;
    
    if let ViscaResponse::InquiryResponse(
        ViscaInquiryResponse::PanTiltPosition { pan, tilt }
    ) = response {
        // Calculate relative movement
        let pan_diff = target_pan - pan;
        let tilt_diff = target_tilt - tilt;
        
        // Move to target
        let cmd = PanTiltCommand::RelativePosition {
            pan: pan_diff,
            tilt: tilt_diff,
            pan_speed: 0x18,
            tilt_speed: 0x14,
        };
        
        send_command_and_wait(transport, &cmd)?;
    }
    
    Ok(())
}
```

## Further Resources

- [VISCA Protocol Documentation](https://www.sony.net/Products/CameraSystem/CA/BRC_X1000_BRC_H800/Technical_Document/C456100121.pdf)
- [PTZOptics Documentation](https://ptzoptics.com/wp-content/uploads/2014/09/PTZOptics_TCP_UDP_CGI_Control-1.pdf)
- [grafton-visca API Documentation](https://docs.rs/grafton-visca)