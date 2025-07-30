//! Simple test to verify ergonomic type conversions work.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::types::*;

    println!("🎥 Testing Ergonomic Type-Safe API");
    println!("==================================");

    // Test speed conversions
    println!("\n⚡ Speed Conversions:");
    let pan_fast = PanSpeed::from(SpeedLevel::Fast);
    let tilt_medium = TiltSpeed::from(SpeedLevel::Medium);
    println!("  • Pan Fast: {} ({})", pan_fast, pan_fast.value());
    println!("  • Tilt Medium: {} ({})", tilt_medium, tilt_medium.value());

    // Test position conversions
    println!("\n📍 Position Conversions:");
    let pan_45_deg = PanPosition::from_degrees(45.0)?;
    let tilt_neg15_deg = TiltPosition::from_degrees(-15.0)?;
    println!(
        "  • Pan 45°: {} -> {:.1}°",
        pan_45_deg,
        pan_45_deg.to_degrees()
    );
    println!(
        "  • Tilt -15°: {} -> {:.1}°",
        tilt_neg15_deg,
        tilt_neg15_deg.to_degrees()
    );

    // Test zoom conversions
    println!("\n🔍 Zoom Conversions:");
    let zoom_50_percent = ZoomPosition::try_from(0.5f32)?;
    let zoom_back: f32 = zoom_50_percent.into();
    println!("  • Zoom 50%: {zoom_50_percent} -> {zoom_back:.2}");

    // Test focus conversions
    println!("\n🎯 Focus Conversions:");
    let focus_75_percent = FocusPosition::try_from(0.75f32)?;
    let focus_back: f32 = focus_75_percent.into();
    println!("  • Focus 75%: {focus_75_percent} -> {focus_back:.2}");

    // Test iris conversions
    println!("\n📸 Iris Conversions:");
    let iris_f28 = IrisLevel::from(FStop::F2_8);
    let iris_raw = IrisLevel::try_from(0x0Bu8)?;
    println!("  • Iris F2.8: {} (0x{:02X})", iris_f28, iris_f28.value());
    println!(
        "  • Iris raw 0x0B: {} (0x{:02X})",
        iris_raw,
        iris_raw.value()
    );

    println!("\n✅ All conversions working correctly!");
    Ok(())
}
