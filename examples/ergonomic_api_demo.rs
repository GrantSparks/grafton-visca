//! Demonstrates the new ergonomic type-safe API improvements.
//!
//! This example showcases how the API now accepts multiple parameter types
//! for common camera operations, making it more user-friendly while maintaining type safety.

use grafton_visca::{
    types::{
        FStop, FocusPosition, PanPosition, PanSpeed, SpeedLevel, TiltPosition, TiltSpeed,
        ZoomPosition,
    },
    Error,
};

fn main() -> Result<(), Error> {
    // Note: This is a demonstration of API usage patterns
    // In a real application, you would connect to actual camera hardware

    println!("🎥 Demonstrating Ergonomic Type-Safe API");
    println!("==========================================");

    // --- Speed Type Conversions ---
    println!("\n⚡ Speed Type Conversions:");

    // Create speed types from different sources
    let pan_speed_from_enum = PanSpeed::from(SpeedLevel::Fast);
    let tilt_speed_from_enum = TiltSpeed::from(SpeedLevel::Medium);
    let pan_speed_from_raw = PanSpeed::try_from(15)?;
    let tilt_speed_from_raw = TiltSpeed::try_from(12)?;

    println!("  • Pan speed from enum: {}", pan_speed_from_enum);
    println!("  • Tilt speed from enum: {}", tilt_speed_from_enum);
    println!("  • Pan speed from raw: {}", pan_speed_from_raw);
    println!("  • Tilt speed from raw: {}", tilt_speed_from_raw);

    // --- Position Type Conversions ---
    println!("\n📍 Position Type Conversions:");

    // Positions can be created from degrees
    let pan_pos = PanPosition::from_degrees(45.0)?;
    let tilt_pos = TiltPosition::from_degrees(-15.0)?;
    println!("  • Pan position: {} (from 45.0°)", pan_pos);
    println!("  • Tilt position: {} (from -15.0°)", tilt_pos);

    // Convert back to degrees
    println!("  • Pan back to degrees: {:.1}°", pan_pos.to_degrees());
    println!("  • Tilt back to degrees: {:.1}°", tilt_pos.to_degrees());

    // --- Zoom Type Conversions ---
    println!("\n🔍 Zoom Type Conversions:");

    // Traditional typed parameter
    let zoom_typed = ZoomPosition::new(0x2000)?;
    println!("  • Zoom from raw value: {}", zoom_typed);

    // Normalized value (0.0-1.0)
    let zoom_normalized = ZoomPosition::try_from(0.5f32)?;
    println!("  • Zoom from normalized (50%): {}", zoom_normalized);
    let normalized_back: f32 = zoom_normalized.into();
    println!("  • Zoom back to normalized: {:.2}", normalized_back);

    // --- Focus Type Conversions ---
    println!("\n🎯 Focus Type Conversions:");

    // Traditional typed parameter
    let focus_typed = FocusPosition::new(0x8000)?;
    println!("  • Focus from raw value: {}", focus_typed);

    // Normalized value (75% to near focus)
    let focus_normalized = FocusPosition::try_from(0.75f32)?;
    println!("  • Focus from normalized (75%): {}", focus_normalized);
    let focus_back: f32 = focus_normalized.into();
    println!("  • Focus back to normalized: {:.2}", focus_back);

    // --- Iris/Aperture Type Conversions ---
    println!("\n📸 Iris Type Conversions:");

    // F-stop enum (intuitive)
    let iris_from_fstop = grafton_visca::types::IrisLevel::from(FStop::F2_8);
    println!("  • Iris from F-stop (F2.8): {}", iris_from_fstop);

    // Raw value (validated)
    let iris_from_raw = grafton_visca::types::IrisLevel::try_from(0x0Bu8)?; // F2.0
    println!("  • Iris from raw (0x0B): {}", iris_from_raw);

    // --- Type Safety Demonstrations ---
    println!("\n🛡️  Type Safety Examples:");

    // These cause runtime validation errors:
    match ZoomPosition::try_from(0x8000u16) {
        Err(e) => println!("  • Zoom validation caught: {}", e),
        Ok(_) => println!("  • Unexpected success"),
    }

    match grafton_visca::types::IrisLevel::try_from(0x20u8) {
        Err(e) => println!("  • Iris validation caught: {}", e),
        Ok(_) => println!("  • Unexpected success"),
    }

    // --- Summary ---
    println!("\n✨ Summary:");
    println!("   The ergonomic API provides multiple ways to specify parameters:");
    println!("   • Raw values (with validation)");
    println!("   • Typed wrappers (compile-time safety)");
    println!("   • Intuitive enums (user-friendly)");
    println!("   • Normalized values (0.0-1.0 ranges)");
    println!("   • Degree-based positioning");
    println!("\n   Choose the method that best fits your use case!");

    Ok(())
}
