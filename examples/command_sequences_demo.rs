//! Command sequence building with the new CommandBuilder API.
//!
//! This shows how to create complex command sequences fluently,
//! replacing manual command-by-command execution.

use grafton_visca::{
    camera::{profiles::PTZOpticsG2, units::Degrees, Camera, CommandBuilderExt},
    command::{
        exposure::ExposureMode,
        focus::FocusSpeed,
        pan_tilt::{PanSpeed, TiltSpeed},
        preset::PresetNumber,
        white_balance::WhiteBalanceMode,
        zoom::ZoomSpeed,
    },
    transport::UdpTransport,
    Error,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== Command Sequence Builder Demo ===");
    println!("Building complex camera operations fluently\n");

    let transport = UdpTransport::new("192.168.1.100:52381")?;
    let camera = Camera::<PTZOpticsG2>::new(transport);

    // Example 1: Camera initialization sequence
    println!("Example 1: Camera Initialization Sequence");
    println!("Building: Power On → Home → Auto Settings → Clear Zoom");

    let init_results = camera
        .commands()
        .power_on()
        .pan_tilt_home()
        .zoom_stop()
        .focus_auto()
        .exposure_mode(ExposureMode::Auto)
        .white_balance_mode(WhiteBalanceMode::Auto)
        .backlight(false)
        .noise_reduction(2) // Medium noise reduction
        .execute_sequential_async()
        .await?;

    println!(
        "  ✓ Initialization complete ({} commands executed)\n",
        init_results.len()
    );

    // Example 2: Complex movement pattern
    println!("Example 2: Complex Movement Pattern");
    println!("Building: Save positions as presets while moving");

    let movement_results = camera
        .commands()
        // Position 1: Wide shot center
        .pan_tilt_to_degrees(
            0.0,
            0.0,
            PanSpeed::new(20).unwrap(),
            TiltSpeed::new(20).unwrap(),
        )?
        .zoom_to(0) // Full wide
        .preset_set(PresetNumber::new(1).unwrap())
        // Position 2: Close-up left
        .pan_tilt_to_degrees(
            -45.0,
            -10.0,
            PanSpeed::new(15).unwrap(),
            TiltSpeed::new(15).unwrap(),
        )?
        .zoom_to(16384) // 50% zoom
        .preset_set(PresetNumber::new(2).unwrap())
        // Position 3: Close-up right
        .pan_tilt_to_degrees(
            45.0,
            -10.0,
            PanSpeed::new(15).unwrap(),
            TiltSpeed::new(15).unwrap(),
        )?
        .zoom_to(16384)
        .preset_set(PresetNumber::new(3).unwrap())
        // Position 4: Audience view
        .pan_tilt_to_degrees(
            180.0,
            15.0,
            PanSpeed::new(10).unwrap(),
            TiltSpeed::new(10).unwrap(),
        )?
        .zoom_to(8192) // 25% zoom
        .preset_set(PresetNumber::new(4).unwrap())
        // Return to position 1
        .preset_recall(PresetNumber::new(1).unwrap())
        .execute_sequential_async()
        .await?;

    println!(
        "  ✓ Movement pattern complete ({} commands)\n",
        movement_results.len()
    );

    // Example 3: Image adjustment sequence
    println!("Example 3: Image Quality Adjustment Sequence");

    let image_results = camera
        .commands()
        // Set manual exposure for consistent look
        .exposure_mode(ExposureMode::Manual)
        .shutter_speed(grafton_visca::types::ShutterSpeed::Shutter1_250)
        .iris(grafton_visca::types::IrisLevel::F5_6)
        .gain(grafton_visca::types::GainLimit::Limit9dB)
        // Fine-tune image quality
        .aperture(grafton_visca::command::image::ApertureValue::new(8).unwrap())
        .white_balance_mode(WhiteBalanceMode::Indoor)
        .noise_reduction(3) // High noise reduction
        .flicker_reduction(true)
        .execute_sequential_async()
        .await?;

    println!(
        "  ✓ Image adjustments complete ({} commands)\n",
        image_results.len()
    );

    // Example 4: Concurrent status queries
    println!("Example 4: Concurrent Status Queries");
    println!("Querying multiple status values simultaneously...");

    use grafton_visca::command::inquiry::*;

    let query_results = camera
        .commands()
        .custom(PowerStateInquiry, "Power State")
        .custom(ZoomPositionInquiry, "Zoom Position")
        .custom(FocusModeInquiry, "Focus Mode")
        .custom(ExposureModeInquiry, "Exposure Mode")
        .custom(WhiteBalanceModeInquiry, "White Balance Mode")
        .custom(PanTiltPositionInquiry, "Pan/Tilt Position")
        .execute_concurrent_async()
        .await?;

    println!("  Query results:");
    for (i, result) in query_results.iter().enumerate() {
        match result {
            Ok(response) => println!("    Query {}: {:?}", i + 1, response),
            Err(e) => println!("    Query {} failed: {}", i + 1, e),
        }
    }

    // Example 5: Dynamic sequence building
    println!("\nExample 5: Dynamic Sequence Building");
    println!("Building sequence based on current camera state...");

    // Start with empty builder
    let mut builder = camera.commands();

    // Check power and add power on if needed
    match camera.power_state_async().await {
        Ok(false) => {
            println!("  Camera is off, adding power on command");
            builder = builder.power_on();
        }
        Ok(true) => println!("  Camera already on"),
        Err(_) => println!("  Could not determine power state"),
    }

    // Check position and add home if not centered
    match camera.position_async().await {
        Ok((pan, tilt)) if pan.0.abs() > 5.0 || tilt.0.abs() > 5.0 => {
            println!("  Camera not centered, adding home command");
            builder = builder.pan_tilt_home();
        }
        Ok(_) => println!("  Camera already centered"),
        Err(_) => println!("  Could not determine position"),
    }

    // Always end with a specific setup
    builder = builder
        .zoom_to(0)
        .focus_auto()
        .exposure_mode(ExposureMode::Auto);

    // Execute the dynamically built sequence
    let dynamic_results = builder.execute_sequential_async().await?;
    println!(
        "  ✓ Dynamic sequence complete ({} commands)\n",
        dynamic_results.len()
    );

    // Example 6: Using command builder for smooth transitions
    println!("Example 6: Smooth Transition Sequence");

    // Smooth zoom with coordinated pan
    let smooth_results = camera
        .commands()
        .zoom_out(ZoomSpeed::new(1).unwrap()) // Start slow zoom out
        .pan_tilt_relative_degrees(
            30.0,
            0.0, // While panning right
            PanSpeed::new(3).unwrap(),
            TiltSpeed::new(1).unwrap(),
        )
        .zoom_stop() // Stop zoom
        .focus_far(FocusSpeed::new(2).unwrap()) // Slight focus adjustment
        .focus_stop()
        .execute_sequential_async()
        .await?;

    println!(
        "  ✓ Smooth transition complete ({} commands)",
        smooth_results.len()
    );

    println!("\nDemo complete!");
    println!("\nKey benefits of CommandBuilder:");
    println!("  • Fluent, readable API");
    println!("  • Type-safe command construction");
    println!("  • Automatic validation against camera profile");
    println!("  • Sequential and concurrent execution modes");
    println!("  • Descriptive logging of operations");

    Ok(())
}
