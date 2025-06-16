//! Example demonstrating position conversion with the new Camera<P> API
//!
//! This example shows how to:
//! - Use the type-safe Camera<P> API with camera profiles
//! - Convert between different position units (VISCA, degrees, normalized)
//! - Leverage compile-time safety with camera-specific constants
//! - Query camera capabilities
//!
//! Run with:
//! ```sh
//! cargo run --example position_conversion --features async-client [camera_ip:port]
//! ```
//!
//! Default camera IP is 192.168.0.110:1259 if not specified.

#[cfg(feature = "async-client")]
use grafton_visca::{
    camera::{
        units::{Degrees, Normalized, ViscaUnits},
        Camera, CameraProfile, PTZOpticsG2,
    },
    transport::AsyncTcpTransport,
};
#[cfg(feature = "async-client")]
use log::info;
#[cfg(feature = "async-client")]
use std::env;

#[cfg(feature = "async-client")]
fn get_camera_address() -> String {
    let args: Vec<String> = env::args().collect();
    let ip_address = if args.len() > 1 {
        &args[1]
    } else {
        "192.168.0.110"
    };
    format!("{ip_address}:1259")
}

#[cfg(feature = "async-client")]
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
    info!("  Zoom steps: {}", capabilities.zoom_steps);
    info!("  Number of presets: {}", capabilities.preset_count);
}

#[cfg(feature = "async-client")]
fn demonstrate_position_conversions_static() {
    info!("\nDemonstrating position conversions with PTZOpticsG2 profile:");

    // Create a profile instance for conversions
    let profile = PTZOpticsG2;

    // Example VISCA units
    let example_pan: i16 = 1224; // Half of max pan range for G2
    let example_tilt: i16 = 432; // A quarter of the way through tilt range

    info!("\nConverting VISCA units to degrees using profile methods:");
    let pan_degrees = profile.pan_units_to_degrees(example_pan);
    let tilt_degrees = profile.tilt_units_to_degrees(example_tilt);

    info!("\nPosition conversions:");
    info!("  VISCA units: pan={}, tilt={}", example_pan, example_tilt);
    info!(
        "  Degrees: pan={:.1}°, tilt={:.1}°",
        pan_degrees, tilt_degrees
    );

    // Convert to normalized coordinates (-1.0 to 1.0)
    let pan_norm = example_pan as f32 / *PTZOpticsG2::PAN_RANGE.end() as f32;
    let tilt_norm = example_tilt as f32 / *PTZOpticsG2::TILT_RANGE.end() as f32;
    info!("  Normalized: pan={:.3}, tilt={:.3}", pan_norm, tilt_norm);

    // Reverse conversions
    info!("\nReverse conversions:");
    let pan_from_degrees = profile.pan_degrees_to_units(pan_degrees);
    let tilt_from_degrees = profile.tilt_degrees_to_units(tilt_degrees);
    info!(
        "  From degrees back to VISCA: pan={}, tilt={}",
        pan_from_degrees, tilt_from_degrees
    );

    // Converting between units manually
    let pan_from_norm = (pan_norm * *PTZOpticsG2::PAN_RANGE.end() as f32) as i16;
    let tilt_from_norm = (tilt_norm * *PTZOpticsG2::TILT_RANGE.end() as f32) as i16;
    info!(
        "  From normalized back to VISCA: pan={}, tilt={}",
        pan_from_norm, tilt_from_norm
    );
}

#[cfg(feature = "async-client")]
async fn demonstrate_type_safe_positioning(
    camera: &mut Camera<PTZOpticsG2>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("\nDemonstrating type-safe positioning with new Camera<P> API:");

    // 1. Position using degrees (most intuitive for users)
    info!("\n1. Setting position using degrees:");
    let pan_deg = Degrees(45.0); // 45 degrees to the right
    let tilt_deg = Degrees(-15.0); // 15 degrees down
    info!("  Moving to: pan={:?}, tilt={:?}", pan_deg, tilt_deg);
    camera.set_position(pan_deg, tilt_deg).await?;
    info!("  ✓ Position set using degrees");

    // Wait for movement to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // 2. Position using VISCA units (when you need precise control)
    info!("\n2. Setting position using VISCA units:");
    let pan_units = ViscaUnits(1000); // Specific VISCA unit position
    let tilt_units = ViscaUnits(300);
    info!("  Moving to: pan={:?}, tilt={:?}", pan_units, tilt_units);
    camera.set_position_units(pan_units, tilt_units).await?;
    info!("  ✓ Position set using VISCA units");

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // 3. Position using normalized coordinates (useful for UI controls)
    info!("\n3. Setting position using normalized coordinates:");
    let norm_pan = Normalized(0.25); // 25% to the right of center
    let norm_tilt = Normalized(-0.5); // 50% below center
    info!("  Moving to: pan={:?}, tilt={:?}", norm_pan, norm_tilt);
    camera.set_position_normalized(norm_pan, norm_tilt).await?;
    info!("  ✓ Position set using normalized coordinates");

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Return to home
    info!("\n4. Returning to home position:");
    camera.home().await?;
    info!("  ✓ Returned to home");

    Ok(())
}

#[cfg(feature = "async-client")]
fn demonstrate_range_validation() {
    info!("\nDemonstrating range validation with camera profiles:");

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
    let profile = PTZOpticsG2;
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

#[cfg(feature = "async-client")]
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

#[cfg(feature = "async-client")]
fn demonstrate_unit_types() {
    info!("\nDemonstrating the type-safe unit system:");

    // Create unit types
    let degrees = Degrees::new(90.0_f32);
    let visca = ViscaUnits::new(2448_i16);
    let normalized = Normalized::new(0.5_f32);

    info!("  Degrees: {:?}", degrees);
    info!("  VISCA Units: {:?}", visca);
    info!("  Normalized: {:?}", normalized);

    // These types prevent accidental mixing of units
    // For example, you can't pass VISCA units where degrees are expected
    info!("\nType safety benefits:");
    info!("  - Can't accidentally mix degrees and VISCA units");
    info!("  - Clear API intent with specific unit types");
    info!("  - Compile-time safety for unit conversions");
}

#[cfg(feature = "async-client")]
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

#[cfg(not(feature = "async-client"))]
fn main() {
    eprintln!("This example requires the async-client feature.");
    eprintln!("Run with: cargo run --example position_conversion --features async-client");
}

