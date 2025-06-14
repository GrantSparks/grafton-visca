//! Example program

//! Demonstrates camera model validation functionality
//!
//! This example shows how to use camera model configuration to catch
//! invalid commands before they're sent to the camera.

#[cfg(not(feature = "blocking-client"))]
fn main() {
    eprintln!("This example requires the 'blocking-client' feature.");
    eprintln!("Run with: cargo run --example model_validation_demo --features blocking-client");
}

#[cfg(feature = "blocking-client")]
use grafton_visca::{
    command::{
        focus::FocusCommand,
        pan_tilt::{PanSpeed, PanTiltCommand, TiltSpeed},
        preset::{PresetAction, PresetCommand, PresetNumber},
        zoom::ZoomCommand,
    },
    constants::CameraModel,
    Client, Error,
};

#[cfg(feature = "blocking-client")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Create a client with camera model specified
    let client = Client::builder()
        .camera_model(CameraModel::PTZOpticsG2)
        .connect_udp("192.168.1.100:5678")?;

    println!("=== Camera Model Validation Demo ===");
    println!("Configured camera model: PTZOpticsG2");
    println!();

    // Example 1: Valid zoom position for G2 (20X optical)
    println!("1. Testing valid zoom position for G2:");
    let valid_zoom = ZoomCommand::Direct(0x7000); // 20X position
    match client.send(&valid_zoom) {
        Ok(_) => println!("   ✓ Zoom to 20X position sent successfully"),
        Err(e) => println!("   ✗ Error: {}", e),
    }
    println!();

    // Example 2: Invalid zoom position for G2 (beyond 20X)
    println!("2. Testing invalid zoom position for G2:");
    let invalid_zoom = ZoomCommand::Direct(0x7AC0); // 30X position
    match client.send(&invalid_zoom) {
        Ok(_) => println!("   ✗ Command sent (shouldn't happen)"),
        Err(Error::ModelValidation {
            model,
            command,
            reason,
        }) => {
            println!("   ✓ Validation prevented invalid command:");
            println!("     Model: {:?}", model);
            println!("     Command: {}", command);
            println!("     Reason: {}", reason);
        }
        Err(e) => println!("   ✗ Unexpected error: {}", e),
    }
    println!();

    // Example 3: Valid pan/tilt position
    println!("3. Testing valid pan/tilt position:");
    let valid_pt = PanTiltCommand::AbsolutePosition {
        pan: 1000,
        tilt: 500,
        pan_speed: PanSpeed::new(0x10).unwrap(),
        tilt_speed: TiltSpeed::new(0x10).unwrap(),
    };
    match client.send(&valid_pt) {
        Ok(_) => println!("   ✓ Pan/tilt position sent successfully"),
        Err(e) => println!("   ✗ Error: {}", e),
    }
    println!();

    // Example 4: Invalid pan position (out of range)
    println!("4. Testing invalid pan position:");
    let invalid_pt = PanTiltCommand::AbsolutePosition {
        pan: 3000, // Beyond G2's pan range of ±2448
        tilt: 0,
        pan_speed: PanSpeed::new(0x10).unwrap(),
        tilt_speed: TiltSpeed::new(0x10).unwrap(),
    };
    match client.send(&invalid_pt) {
        Ok(_) => println!("   ✗ Command sent (shouldn't happen)"),
        Err(Error::ModelValidation {
            model,
            command,
            reason,
        }) => {
            println!("   ✓ Validation prevented invalid command:");
            println!("     Model: {:?}", model);
            println!("     Command: {}", command);
            println!("     Reason: {}", reason);
        }
        Err(e) => println!("   ✗ Unexpected error: {}", e),
    }
    println!();

    // Example 5: Valid focus position
    println!("5. Testing valid focus position:");
    let valid_focus = FocusCommand::Direct(0x8000); // Mid-range focus
    match client.send(&valid_focus) {
        Ok(_) => println!("   ✓ Focus position sent successfully"),
        Err(e) => println!("   ✗ Error: {}", e),
    }
    println!();

    // Example 6: Invalid focus position (out of range)
    println!("6. Testing invalid focus position:");
    let invalid_focus = FocusCommand::Direct(0x0500); // Below minimum
    match client.send(&invalid_focus) {
        Ok(_) => println!("   ✗ Command sent (shouldn't happen)"),
        Err(Error::ModelValidation {
            model,
            command,
            reason,
        }) => {
            println!("   ✓ Validation prevented invalid command:");
            println!("     Model: {:?}", model);
            println!("     Command: {}", command);
            println!("     Reason: {}", reason);
        }
        Err(e) => println!("   ✗ Unexpected error: {}", e),
    }
    println!();

    // Example 7: Valid preset for G2 (supports 0-89)
    println!("7. Testing valid preset number for G2:");
    let valid_preset = PresetCommand {
        action: PresetAction::Recall,
        preset_number: PresetNumber::new(89).unwrap(),
    };
    match client.send(&valid_preset) {
        Ok(_) => println!("   ✓ Preset 89 recall sent successfully"),
        Err(e) => println!("   ✗ Error: {}", e),
    }
    println!();

    // Example 8: Invalid preset for G2 (beyond 89)
    println!("8. Testing invalid preset number for G2:");
    // Note: PresetNumber::new(90) would fail at construction time
    // For demo purposes, we'll show what happens with a valid PresetNumber
    // that's invalid for G2 specifically
    let preset_90 = PresetNumber::new(89).unwrap(); // Max for G2
    let preset_cmd = PresetCommand {
        action: PresetAction::Recall,
        preset_number: preset_90,
    };
    match client.send(&preset_cmd) {
        Ok(_) => println!("   ✓ Preset command sent (89 is valid for G2)"),
        Err(e) => println!("   ✗ Error: {}", e),
    }
    println!();

    println!("=== Demo Complete ===");
    println!();
    println!("Note: Without camera model configuration, all these commands");
    println!("would be sent to the camera, potentially causing errors.");

    Ok(())
}
