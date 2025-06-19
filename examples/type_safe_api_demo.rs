//! Example demonstrating the type-safe Camera API in grafton-visca.
//!
//! This example shows how the strongly-typed Camera API with profiles prevents
//! runtime errors and provides compile-time guarantees.

use grafton_visca::{
    camera::{profiles::PTZOpticsG2, Camera, CameraProfile},
    transport::blocking::create,
};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Connect to camera using the new Camera API
    let transport = create::udp("192.168.1.100:5678")?;
    let mut camera = Camera::<PTZOpticsG2>::new(transport);

    println!("=== Type-Safe Camera API Demo ===\n");

    // 1. Camera profile provides compile-time type safety
    println!("1. Camera profile information:");
    let caps = camera.capabilities();
    println!("   Model: {}", caps.model_name);
    println!("   Pan range: {:?} degrees", caps.pan_range_degrees);
    println!("   Tilt range: {:?} degrees", caps.tilt_range_degrees);
    println!("   Max pan speed: {}", caps.max_pan_speed);
    println!("   Max tilt speed: {}", caps.max_tilt_speed);
    println!("   ✓ All capabilities are type-safe and model-specific");

    // 2. Position control with type-safe units
    println!("\n2. Type-safe position control:");
    use grafton_visca::camera::units::{Degrees, Normalized, ViscaUnits};

    // Move using degrees
    camera.set_position(Degrees(45.0), Degrees(15.0))?;
    println!("   ✓ Moved to 45° pan, 15° tilt");
    thread::sleep(Duration::from_secs(2));

    // Move using VISCA units
    camera.set_position_units(ViscaUnits(1000), ViscaUnits(500))?;
    println!("   ✓ Moved using VISCA units");
    thread::sleep(Duration::from_secs(2));

    // Move using normalized coordinates
    camera.set_position_normalized(Normalized(0.5), Normalized(-0.25))?;
    println!("   ✓ Moved using normalized coordinates");
    thread::sleep(Duration::from_secs(2));

    // 3. Profile-specific preset types
    println!("\n3. Profile-specific preset operations:");
    use grafton_visca::camera::profiles::G2PresetId;

    // PTZOpticsG2 has specific preset constraints (0-89)
    let preset = G2PresetId::new(5)?;
    camera.set_preset(preset)?;
    println!("   ✓ Saved position to preset");

    thread::sleep(Duration::from_secs(1));
    camera.home()?;
    thread::sleep(Duration::from_secs(2));

    camera.recall_preset(preset)?;
    println!("   ✓ Recalled preset");

    // 4. Profile-specific gain values
    println!("\n4. Profile-specific gain control:");
    use grafton_visca::camera::profiles::G2Gain;

    // PTZOpticsG2 has specific gain values
    camera.set_gain(G2Gain::Gain12dB)?;
    println!("   ✓ Set gain to 12dB (profile-specific value)");

    camera.set_gain_limit(6)?; // 18dB = value 6
    println!("   ✓ Set gain limit to 18dB");

    // 5. All commands are validated at compile time
    println!("\n5. Compile-time validation:");
    println!("   ✓ All speeds are validated (0-24 for pan, 0-20 for tilt)");
    println!("   ✓ Position ranges are enforced by the profile");
    println!("   ✓ Invalid preset IDs are caught at creation time");
    println!("   ✓ Gain values are restricted to valid camera options");

    // 6. Profile-aware conversions
    println!("\n6. Profile-aware unit conversions:");
    let pan_degrees = camera.profile().pan_units_to_degrees(1000);
    let tilt_degrees = camera.profile().tilt_units_to_degrees(500);
    println!(
        "   VISCA units (1000, 500) = ({:.1}°, {:.1}°)",
        pan_degrees, tilt_degrees
    );

    let pan_units = camera.profile().pan_degrees_to_units(45.0);
    let tilt_units = camera.profile().tilt_degrees_to_units(15.0);
    println!(
        "   Degrees (45°, 15°) = VISCA units ({}, {})",
        pan_units, tilt_units
    );

    println!("\n=== Demo Complete ===");
    println!("\nBenefits of the type-safe Camera API:");
    println!("• Camera-specific constraints enforced at compile time");
    println!("• Profile-aware conversions and validations");
    println!("• Type-safe position units prevent mixing degrees/units/normalized");
    println!("• Model-specific types (presets, gain) ensure compatibility");
    println!("• Self-documenting API with clear parameter constraints");

    Ok(())
}
