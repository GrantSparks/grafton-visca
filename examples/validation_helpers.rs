//! Example demonstrating the new validation helper macros

use grafton_visca::{
    command::{GainCommand, PanTiltCommand, PanTiltDirection},
    types::{Gain, PanSpeed, TiltSpeed},
    validate_all, Command, Error,
};

fn main() {
    if let Err(e) = run_examples() {
        eprintln!("Error: {}", e);
    }
}

fn run_examples() -> Result<(), Error> {
    println!("Validation Helper Examples\n");

    // Example 1: ViscaValue macro with model constraints
    println!("1. ViscaValue macro with model_constraints attribute:");

    // Gain now has model_constraints = "PTZOpticsG2" in its derive
    let gain = Gain::new(0x05)?;
    println!("  Created Gain: {}", gain);

    // The macro generates G2_VALID_VALUES constant for backwards compatibility
    println!("  G2 valid values: {:?}", Gain::G2_VALID_VALUES);
    println!("  Valid values include gain levels from 0dB (0x00) to 21dB (0x07)");

    // Use in a command
    let cmd = GainCommand::SetValue(gain);
    println!("  Created command: {:?}", cmd);
    println!("  Command bytes: {:?}", cmd.to_bytes()?);

    println!();

    // Example 2: Multi-parameter validation with validate_all! macro
    println!("2. Multi-parameter validation with validate_all! macro:");

    let pan_speed_raw = 12;
    let tilt_speed_raw = 8;

    // Old way (verbose):
    let _pan_speed_old = PanSpeed::new(pan_speed_raw)
        .map_err(|_| Error::InvalidParameter(format!("Invalid pan speed: {}", pan_speed_raw)))?;
    let _tilt_speed_old = TiltSpeed::new(tilt_speed_raw)
        .map_err(|_| Error::InvalidParameter(format!("Invalid tilt speed: {}", tilt_speed_raw)))?;

    // New way with validate_all!:
    let (pan_speed, tilt_speed) = validate_all! {
        pan_speed: PanSpeed::new(pan_speed_raw),
        tilt_speed: TiltSpeed::new(tilt_speed_raw),
    }?;

    println!(
        "  Created PanSpeed({}) and TiltSpeed({})",
        pan_speed.value(),
        tilt_speed.value()
    );

    // Use in a command
    let move_cmd = PanTiltCommand::Move {
        direction: PanTiltDirection::Up,
        pan_speed,
        tilt_speed,
    };

    println!("  Created command: {:?}", move_cmd);
    println!("  Command bytes: {:?}", move_cmd.to_bytes()?);

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
        pan_speed: PanSpeed::new(invalid_pan),
        tilt_speed: TiltSpeed::new(invalid_tilt),
    } {
        Ok(_) => println!("  Unexpected success"),
        Err(e) => println!("  ✓ Got expected error: {}", e),
    }

    println!();

    // Example 4: MIN/MAX constants generated for valid_values
    println!("4. MIN/MAX constants auto-generated from valid_values:");
    println!("  Gain::MIN = {:#02X}", Gain::MIN.value());
    println!("  Gain::MAX = {:#02X}", Gain::MAX.value());

    Ok(())
}
