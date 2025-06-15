//! Example demonstrating position conversion with the new Camera<P> API
//!
//! This example shows how to:
//! - Use the type-safe Camera<P> API with camera profiles
//! - Convert between different position units (VISCA, degrees, normalized)
//! - Leverage camera-specific constants and ranges
//! - Query and display camera capabilities
//!
//! Run with:
//! ```bash
//! cargo run --example position_conversion --features async-client [CAMERA_IP]
//! ```
//!
//! Default camera IP is 192.168.0.110:1259 if not specified.

use grafton_visca::{
    camera::{
        units::{Degrees, Normalized, ViscaUnits},
        Camera, CameraProfile, PTZOpticsG2,
    },
    transport::AsyncTcpTransport,
};
use log::info;
use std::env;
use tokio;

fn get_camera_address() -> String {
    let args: Vec<String> = env::args().collect();
    let ip_address = if args.len() > 1 {
        &args[1]
    } else {
        "192.168.0.110"
    };
    format!("{ip_address}:1259")
}

async fn display_camera_profile<P: CameraProfile>(camera: &Camera<P>) {
    let profile_name = P::MODEL_NAME;
    info!("\nCamera Profile: {}", profile_name);
    info!("Constants for {}:", profile_name);
    info!("  Pan range: {:?} VISCA units", P::PAN_RANGE);
    info!("  Tilt range: {:?} VISCA units", P::TILT_RANGE);
    info!("  Zoom range: {:?} VISCA units", P::ZOOM_RANGE);
    info!("  Focus range: {:?} VISCA units", P::FOCUS_RANGE);

    // Get degree ranges using the profile
    let capabilities = camera.capabilities();
    info!("  Pan degrees: {:?}", capabilities.pan_range_degrees);
    info!("  Tilt degrees: {:?}", capabilities.tilt_range_degrees);
    info!("  Max pan speed: {}", capabilities.max_pan_speed);
    info!("  Max tilt speed: {}", capabilities.max_tilt_speed);
    info!(
        "  Digital zoom supported: {}",
        capabilities.supports_digital_zoom
    );
    info!("  Number of presets: {}", capabilities.preset_count);
}

fn demonstrate_position_conversions_static() {
    info!("\nDemonstrating position conversions with PTZOpticsG2 profile:");

    // Create a profile instance for conversions
    let profile = PTZOpticsG2::default();

    // Example VISCA positions
    let example_pan = 1000_i16;
    let example_tilt = 500_i16;

    info!(
        "Example VISCA position: pan={}, tilt={}",
        example_pan, example_tilt
    );

    // Convert VISCA units to degrees
    let pan_degrees = profile.pan_units_to_degrees(example_pan);
    let tilt_degrees = profile.tilt_units_to_degrees(example_tilt);

    info!("\nPosition conversions:");
    info!("  VISCA units: pan={}, tilt={}", example_pan, example_tilt);
    info!(
        "  Degrees: pan={:.1}°, tilt={:.1}°",
        pan_degrees, tilt_degrees
    );

    // Convert to normalized coordinates (-1.0 to 1.0)
    let pan_norm = (example_pan as f32 - *PTZOpticsG2::PAN_RANGE.start() as f32)
        / (PTZOpticsG2::PAN_RANGE.end() - PTZOpticsG2::PAN_RANGE.start()) as f32
        * 2.0
        - 1.0;
    let tilt_norm = (example_tilt as f32 - *PTZOpticsG2::TILT_RANGE.start() as f32)
        / (PTZOpticsG2::TILT_RANGE.end() - PTZOpticsG2::TILT_RANGE.start()) as f32
        * 2.0
        - 1.0;

    info!("  Normalized: pan={:.3}, tilt={:.3}", pan_norm, tilt_norm);

    // Demonstrate reverse conversions
    info!("\nReverse conversions:");
    let from_degrees_pan = profile.pan_degrees_to_units(pan_degrees);
    let from_degrees_tilt = profile.tilt_degrees_to_units(tilt_degrees);
    info!(
        "  Degrees -> VISCA: pan={}, tilt={}",
        from_degrees_pan, from_degrees_tilt
    );

    // Convert normalized back to VISCA
    let from_norm_pan = ((pan_norm + 1.0) / 2.0
        * (PTZOpticsG2::PAN_RANGE.end() - PTZOpticsG2::PAN_RANGE.start()) as f32
        + *PTZOpticsG2::PAN_RANGE.start() as f32)
        .round() as i16;
    let from_norm_tilt = ((tilt_norm + 1.0) / 2.0
        * (PTZOpticsG2::TILT_RANGE.end() - PTZOpticsG2::TILT_RANGE.start()) as f32
        + *PTZOpticsG2::TILT_RANGE.start() as f32)
        .round() as i16;
    info!(
        "  Normalized -> VISCA: pan={}, tilt={}",
        from_norm_pan, from_norm_tilt
    );
}

async fn demonstrate_type_safe_positioning(
    camera: &mut Camera<PTZOpticsG2>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("\nDemonstrating type-safe position units:");

    // Using Degrees type
    let target_pan_deg = Degrees::new(45.0);
    let target_tilt_deg = Degrees::new(15.0);

    info!(
        "Moving to position using Degrees type: pan={:.1}°, tilt={:.1}°",
        target_pan_deg.value(),
        target_tilt_deg.value()
    );

    camera.set_position(target_pan_deg, target_tilt_deg).await?;
    info!("Move command sent successfully");

    // Wait for movement to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Using ViscaUnits type
    let target_pan_units = ViscaUnits::new(1000_i16);
    let target_tilt_units = ViscaUnits::new(500_i16);

    info!(
        "\nMoving to position using ViscaUnits type: pan={}, tilt={}",
        target_pan_units.value(),
        target_tilt_units.value()
    );

    camera
        .set_position_units(target_pan_units, target_tilt_units)
        .await?;
    info!("Move command sent successfully");

    // Wait for movement to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Using Normalized type
    let norm_pan = Normalized::new(0.25_f32); // 25% to the right of center
    let norm_tilt = Normalized::new(-0.5_f32); // 50% below center

    info!(
        "\nMoving to position using Normalized type: pan={:.2}, tilt={:.2}",
        norm_pan.value(),
        norm_tilt.value()
    );

    camera.set_position_normalized(norm_pan, norm_tilt).await?;
    info!("Move command sent successfully");

    Ok(())
}

fn demonstrate_range_validation() {
    info!("\nDemonstrating range validation:");

    // Valid pan position
    let valid_pan = 1000;
    if PTZOpticsG2::PAN_RANGE.contains(&valid_pan) {
        info!("  Pan position {} is valid for PTZOptics G2", valid_pan);
    }

    // Invalid pan position
    let invalid_pan = 5000;
    if !PTZOpticsG2::PAN_RANGE.contains(&invalid_pan) {
        info!(
            "  Pan position {} is outside valid range ({:?})",
            invalid_pan,
            PTZOpticsG2::PAN_RANGE
        );
    }

    // Check tilt ranges
    let valid_tilt = 0;
    if PTZOpticsG2::TILT_RANGE.contains(&valid_tilt) {
        info!("  Tilt position {} is valid for PTZOptics G2", valid_tilt);
    }

    // Convert extreme positions
    let profile = PTZOpticsG2::default();
    let max_pan_deg = profile.pan_units_to_degrees(*PTZOpticsG2::PAN_RANGE.end());
    let min_tilt_deg = profile.tilt_units_to_degrees(*PTZOpticsG2::TILT_RANGE.start());

    info!("\nExtreme positions:");
    info!(
        "  Maximum pan: {} units = {:.1}°",
        PTZOpticsG2::PAN_RANGE.end(),
        max_pan_deg
    );
    info!(
        "  Minimum tilt: {} units = {:.1}°",
        PTZOpticsG2::TILT_RANGE.start(),
        min_tilt_deg
    );
}

async fn display_capability_summary(camera: &Camera<PTZOpticsG2>) {
    info!("\nCapability Summary:");
    let summary = camera.capability_summary();

    info!("Movement capabilities:");
    info!("  - Continuous movement: {}", summary.movement.continuous);
    info!("  - Absolute positioning: {}", summary.movement.absolute);
    info!("  - Relative positioning: {}", summary.movement.relative);
    info!("  - Pan range: {:?} degrees", summary.movement.pan_range);
    info!("  - Tilt range: {:?} degrees", summary.movement.tilt_range);
    info!("  - Max pan speed: {}", summary.movement.max_pan_speed);
    info!("  - Max tilt speed: {}", summary.movement.max_tilt_speed);

    info!("\nZoom capabilities:");
    info!("  - Optical zoom range: {:?}", summary.zoom.optical_range);
    info!("  - Digital zoom: {}", summary.zoom.digital_zoom);
    info!("  - Speed levels: {}", summary.zoom.speed_levels);

    info!("\nFocus capabilities:");
    info!("  - Auto-focus: {}", summary.focus.auto_focus);
    info!("  - Manual focus: {}", summary.focus.manual_focus);
    info!("  - Focus range: {:?}", summary.focus.range);
    info!("  - Speed levels: {}", summary.focus.speed_levels);

    info!("\nExposure capabilities:");
    info!("  - Auto-exposure: {}", summary.exposure.auto_exposure);
    info!("  - Manual exposure: {}", summary.exposure.manual_exposure);
    info!(
        "  - Wide dynamic range: {}",
        summary.exposure.wide_dynamic_range
    );

    info!("\nImage processing capabilities:");
    info!("  - White balance: {}", summary.image.white_balance);
    info!("  - Image flip: {}", summary.image.image_flip);
    info!("  - Noise reduction: {}", summary.image.noise_reduction);
    info!("  - Low-light mode: {}", summary.image.low_light_mode);
}

fn demonstrate_unit_types() {
    info!("\nDemonstrating the type-safe unit system:");

    // Create different unit types
    let degrees = Degrees::new(90.0_f32);
    let visca = ViscaUnits::new(2448_i16);
    let normalized = Normalized::new(0.5_f32);

    info!("Unit types:");
    info!("  Degrees: {:.1}°", degrees.value());
    info!("  VISCA units: {}", visca.value());
    info!(
        "  Normalized: {:.2} (range -1.0 to 1.0)",
        normalized.value()
    );

    // Show how units prevent mistakes
    info!("\nType safety benefits:");
    info!("  - Can't accidentally mix degrees and VISCA units");
    info!("  - Clear API intent with specific unit types");
    info!("  - Compile-time safety for unit conversions");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("Position conversion example with Camera<P> API");

    let address = get_camera_address();
    let transport = AsyncTcpTransport::new(&address).await?;

    // Create a type-safe camera instance with PTZOptics G2 profile
    let mut camera = Camera::<PTZOpticsG2>::new(transport);
    info!("Created Camera<PTZOpticsG2> instance for {}", address);

    // Display camera profile information
    display_camera_profile(&camera).await;

    // Demonstrate position conversions (static examples)
    demonstrate_position_conversions_static();

    // Demonstrate the unit type system
    demonstrate_unit_types();

    // Demonstrate type-safe positioning
    demonstrate_type_safe_positioning(&mut camera).await?;

    // Demonstrate range validation
    demonstrate_range_validation();

    // Display capability summary
    display_capability_summary(&camera).await;

    info!("\nExample completed successfully!");
    info!("The new Camera<P> API provides:");
    info!("  - Type-safe camera profiles with compile-time guarantees");
    info!("  - Automatic range validation based on camera model");
    info!("  - Type-safe unit conversions (Degrees, ViscaUnits, Normalized)");
    info!("  - Camera-specific capabilities and constants");

    Ok(())
}
