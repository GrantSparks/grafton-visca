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
    validate_all, Error,
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
    let _pan_speed_old = PanSpeed::try_from(pan_speed_raw)
        .map_err(|_| Error::InvalidParameter(format!("Invalid pan speed: {}", pan_speed_raw)))?;
    let _tilt_speed_old = TiltSpeed::try_from(tilt_speed_raw)
        .map_err(|_| Error::InvalidParameter(format!("Invalid tilt speed: {}", tilt_speed_raw)))?;

    // New way with validate_all!:
    let (pan_speed, tilt_speed) = validate_all! {
        pan_speed: PanSpeed::try_from(pan_speed_raw),
        tilt_speed: TiltSpeed::try_from(tilt_speed_raw),
    }?;

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
    match validate_all! {
        pan_speed: PanSpeed::try_from(invalid_pan),
        tilt_speed: TiltSpeed::try_from(invalid_tilt),
    } {
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
