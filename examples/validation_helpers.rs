//! Example demonstrating validation helpers and type-safe value creation
//!
//! This example shows how to use the library's type-safe value types
//! and validation helpers.

use grafton_visca::{
    command::{
        encode_visca::EncodeVisca,
        gain::Gain as GainCommand,
        pan_tilt::{PanTilt, PanTiltDirection},
    },
    types::{GainLevel, PanSpeed, TiltSpeed},
    Error,
};

fn main() {
    if let Err(e) = run_examples() {
        eprintln!("Error: {}", e);
    }
}

fn run_examples() -> Result<(), Error> {
    println!("=== Validation Helper Examples ===\n");

    // Example 1: Type-safe value creation
    println!("1. Type-safe value creation:");

    // Create a gain level (0-15 for most cameras)
    let gain_level = GainLevel::new(5)?;
    println!("  Created GainLevel: {}", gain_level.value());

    // Use in a command
    let gain_cmd = GainCommand::SetValue(gain_level);
    println!("  Created command: {:?}", gain_cmd);
    println!("  Command bytes: {:?}", gain_cmd.try_into_vec()?);

    println!();

    // Example 2: Multi-parameter validation with validate_all! macro
    println!("2. Multi-parameter validation with validate_all! macro:");

    let pan_speed_raw = 12;
    let tilt_speed_raw = 8;

    // Old way (verbose):
    let _pan_speed_old =
        PanSpeed::try_from(pan_speed_raw).map_err(|_| Error::InvalidParameter {
            parameter: "pan_speed",
            value: pan_speed_raw.to_string(),
            reason: format!("Invalid pan speed: {}", pan_speed_raw),
        })?;
    let _tilt_speed_old =
        TiltSpeed::try_from(tilt_speed_raw).map_err(|_| Error::InvalidParameter {
            parameter: "tilt_speed",
            value: tilt_speed_raw.to_string(),
            reason: format!("Invalid tilt speed: {}", tilt_speed_raw),
        })?;

    // New way with validate_all!:
    // validate_all! macro is not available, use individual validation
    let pan_speed = PanSpeed::try_from(pan_speed_raw).map_err(|_| Error::InvalidParameter {
        parameter: "pan_speed",
        value: pan_speed_raw.to_string(),
        reason: format!("Invalid pan speed: {}", pan_speed_raw),
    })?;
    let tilt_speed = TiltSpeed::try_from(tilt_speed_raw).map_err(|_| Error::InvalidParameter {
        parameter: "tilt_speed",
        value: tilt_speed_raw.to_string(),
        reason: format!("Invalid tilt speed: {}", tilt_speed_raw),
    })?;

    println!(
        "  Created PanSpeed({}) and TiltSpeed({})",
        pan_speed.value(),
        tilt_speed.value()
    );

    // Use in a command
    let move_cmd = PanTilt::Move {
        direction: PanTiltDirection::Up,
        pan_speed,
        tilt_speed,
    };

    println!("  Created command: {:?}", move_cmd);
    println!("  Command bytes: {:?}", move_cmd.try_into_vec()?);

    println!();

    // Example 3: Error handling with validate_all!
    println!("3. Error handling with validate_all! macro:");

    let invalid_pan = 30; // Max is 24 (0x18)
    let invalid_tilt = 25; // Max is 20 (0x14)

    println!(
        "  Trying to create PanSpeed({}) and TiltSpeed({})...",
        invalid_pan, invalid_tilt
    );
    // Try to create both speeds and handle the error
    let result = (|| {
        let _pan = PanSpeed::try_from(invalid_pan).map_err(|_| Error::InvalidParameter {
            parameter: "pan_speed",
            value: invalid_pan.to_string(),
            reason: format!("Invalid pan speed: {}", invalid_pan),
        })?;
        let _tilt = TiltSpeed::try_from(invalid_tilt).map_err(|_| Error::InvalidParameter {
            parameter: "tilt_speed",
            value: invalid_tilt.to_string(),
            reason: format!("Invalid tilt speed: {}", invalid_tilt),
        })?;
        Ok::<_, Error>(())
    })();

    match result {
        Ok(_) => println!("  Unexpected success"),
        Err(e) => println!("  ✓ Got expected error: {}", e),
    }

    println!();

    // Example 4: Direct type construction with validation
    println!("4. Direct type construction with validation:");

    // Valid values
    match GainLevel::new(10) {
        Ok(gain) => println!("  ✓ Created valid GainLevel({})", gain.value()),
        Err(e) => println!("  Error: {}", e),
    }

    // Invalid values
    match GainLevel::new(20) {
        Ok(_) => println!("  Unexpected success"),
        Err(e) => println!("  ✓ Got expected error for GainLevel(20): {}", e),
    }

    println!();

    // Example 5: Using constants
    println!("5. Using type constants:");
    println!("  PanSpeed::MIN = {}", PanSpeed::MIN.value());
    println!("  PanSpeed::MAX = {}", PanSpeed::MAX.value());
    println!("  TiltSpeed::MIN = {}", TiltSpeed::MIN.value());
    println!("  TiltSpeed::MAX = {}", TiltSpeed::MAX.value());

    Ok(())
}
