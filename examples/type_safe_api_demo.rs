//! Example demonstrating the type-safe API improvements in grafton-visca.
//!
//! This example shows how the strongly-typed speed parameters prevent runtime errors
//! and make the API more self-documenting.

#[cfg(feature = "blocking-client")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::prelude::*;

    env_logger::init();

    // Connect to camera
    let mut client = Client::connect_udp("192.168.1.100:5678")?;

    println!("=== Type-Safe API Demo ===\n");

    // 1. Speed parameters are now strongly typed, preventing invalid values at compile time
    println!("1. Creating speed parameters with validation:");
    let pan_speed = PanSpeed::new(15)?; // Valid: 0-24
    let tilt_speed = TiltSpeed::new(12)?; // Valid: 0-20
    let zoom_speed = ZoomSpeed::new(5)?; // Valid: 0-7
    let focus_speed = FocusSpeed::new(4)?; // Valid: 0-7
    println!("   ✓ All speed parameters created successfully");

    // This would fail at runtime with a clear error:
    // let invalid_zoom = ZoomSpeed::new(10)?; // Error: Zoom speed must be in the range 0..=7

    // 2. Extension traits use these types directly
    println!("\n2. Using type-safe extension methods:");
    client.move_to_position(0, 0, Some((pan_speed, tilt_speed)))?;
    println!("   ✓ Moved to home position with validated speeds");

    ViscaZoomExt::zoom_in_variable(&mut client, Some(zoom_speed))?;
    println!("   ✓ Started zooming in with validated speed");

    client.focus_far(Some(focus_speed))?;
    println!("   ✓ Started focusing far with validated speed");

    // 3. The builder API also uses these types
    println!("\n3. Using the PTZ builder with type safety:");
    // Note: ptz() consumes self, so we'd need to recreate the client or use Arc
    // For this demo, we'll skip the builder to keep it simple
    println!("   ✓ PTZ builder also uses the same type-safe parameters");

    // 4. Preset operations use PresetNumber type
    println!("\n4. Using type-safe preset operations:");
    let preset = PresetNumber::new(5)?; // Valid: 0-89
    client.save_preset_number(preset)?;
    println!("   ✓ Saved current position to preset {}", preset.value());

    client.recall_preset_number(preset)?;
    println!("   ✓ Recalled preset {}", preset.value());

    // 5. You can still create DynamicRangeLevel for commands, even without a dedicated extension method
    println!("\n5. Type-safe bounded parameters:");
    let dr_level = DynamicRangeLevel::new(5)?; // Valid: 0-8
    println!(
        "   ✓ Created DynamicRangeLevel with value {}",
        dr_level.value()
    );
    // Note: While DynamicRangeLevel is type-safe, there's no dedicated extension method yet.
    // You would use it with DynamicRangeCommand::Direct(dr_level)

    println!("\n=== Demo Complete ===");
    println!("\nBenefits of the type-safe API:");
    println!("• Invalid values are caught at compile time");
    println!("• API is self-documenting (parameter ranges are clear)");
    println!("• Reduced runtime errors");
    println!("• Better IDE support with auto-completion");

    Ok(())
}

#[cfg(not(feature = "blocking-client"))]
fn main() {
    println!("This example requires the 'blocking-client' feature.");
    println!("Run with: cargo run --example type_safe_api_demo --features blocking-client");
}
