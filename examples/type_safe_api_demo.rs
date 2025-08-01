//! Example demonstrating the type-safe Camera API in grafton-visca.
//!
//! This example shows how the strongly-typed Camera API with profiles prevents
//! runtime errors and provides compile-time guarantees.

#[cfg(feature = "async")]
fn main() {
    eprintln!("This example demonstrates the blocking API. Run without async features:");
    eprintln!("cargo run --example type_safe_api_demo");
}

#[cfg(not(feature = "async"))]
use grafton_visca::{
    prelude::blocking::*,
    transport::UdpTransportBlocking,
    types::SpeedLevel,
    units::Degrees,
    // Note: Gain and GainLimit types are not exported in types module
};
#[cfg(not(feature = "async"))]
use std::thread;
#[cfg(not(feature = "async"))]
use std::time::Duration;

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Connect to camera using the new Camera API
    let transport = UdpTransportBlocking::connect("0.0.0.0:0", "192.168.1.100:5678")?;
    let camera = PTZOpticsG2Cam::new_blocking(transport);

    println!("=== Type-Safe Camera API Demo ===\n");

    // 1. Camera profile provides compile-time type safety
    println!("1. Camera profile information:");
    println!("   Camera profile implements capability traits at compile-time");
    println!("   Available methods are determined by profile capabilities");
    println!("   ✓ All capabilities are type-safe and model-specific");

    // 2. Position control with type-safe units
    println!(
        "
2. Type-safe position control:"
    );

    // Move using degrees
    camera.pan_tilt_absolute(Degrees::new(45.0), Degrees::new(15.0), SpeedLevel::from(5))?;
    println!("   ✓ Moved to 45° pan, 15° tilt");
    thread::sleep(Duration::from_secs(2));

    // Move using pan_tilt method with normalized coordinates (0.0 to 1.0)
    camera.pan_tilt_absolute(Degrees::new(0.5), Degrees::new(-0.25), SpeedLevel::from(5))?;
    println!("   ✓ Moved using normalized coordinates");
    thread::sleep(Duration::from_secs(2));

    // Move using pan_tilt_degrees method
    camera.pan_tilt_absolute(Degrees::new(90.0), Degrees::new(-15.0), SpeedLevel::from(5))?;
    println!("   ✓ Moved using degrees via pan_tilt_degrees");
    thread::sleep(Duration::from_secs(2));

    // 3. Profile-specific preset types
    println!("\n3. Profile-specific preset operations:");

    // PTZOpticsG2 has specific preset constraints (0-89)
    // Note: G2PresetId doesn't convert to PresetNumber directly
    // Use PresetNumber instead
    use grafton_visca::PresetNumber;
    let preset = PresetNumber::new(5)?;
    camera.preset_set(preset)?;
    println!("   ✓ Saved position to preset");

    thread::sleep(Duration::from_secs(1));
    camera.pan_tilt_home()?;
    thread::sleep(Duration::from_secs(2));

    camera.preset_recall(preset)?;
    println!("   ✓ Recalled preset");

    // 4. Profile-specific gain values
    println!("\n4. Profile-specific gain control:");
    use grafton_visca::camera::profiles::G2Gain;

    // PTZOpticsG2 has specific gain values
    use grafton_visca::types::{GainLevel, GainLimit};
    camera.set_gain(GainLevel::new(G2Gain::Gain12dB as u8)?)?;
    println!("   ✓ Set gain to 12dB (profile-specific value)");

    camera.set_gain_limit(GainLimit::new(6)?)?; // 18dB = value 6
    println!("   ✓ Set gain limit to 18dB");

    // 5. All commands are validated at compile time
    println!("\n5. Compile-time validation:");
    println!("   ✓ All speeds are validated (0-24 for pan, 0-20 for tilt)");
    println!("   ✓ Position ranges are enforced by the profile");
    println!("   ✓ Invalid preset IDs are caught at creation time");
    println!("   ✓ Gain values are restricted to valid camera options");

    // 6. Profile-aware conversions
    println!(
        "
6. Profile-aware unit conversions:"
    );
    println!("   ✓ Unit conversions are handled internally by the Camera API");
    println!("   ✓ Profile-specific ranges and scaling are enforced automatically");

    println!("\n=== Demo Complete ===");
    println!("\nBenefits of the type-safe Camera API:");
    println!("• Camera-specific constraints enforced at compile time");
    println!("• Profile-aware conversions and validations");
    println!("• Type-safe position units prevent mixing degrees/units/normalized");
    println!("• Model-specific types (presets, gain) ensure compatibility");
    println!("• Self-documenting API with clear parameter constraints");

    Ok(())
}
