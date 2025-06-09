//! Example demonstrating position conversion and camera constants usage
//!
//! This example shows how to:
//! - Use camera-specific constants
//! - Convert between different position units (VISCA, degrees, normalized)
//! - Validate camera parameters
//! - Detect camera model (placeholder functionality)

#[cfg(feature = "blocking-client")]
use grafton_visca::{
    command::pan_tilt::{PanSpeed, TiltSpeed},
    constants::{
        self, CameraConstants, CameraModel, DegreePosition, PositionConversion, ViscaPosition,
    },
    Client, InquiryExt, PanTiltExt,
};
#[cfg(feature = "blocking-client")]
use log::{error, info};
#[cfg(feature = "blocking-client")]
use std::env;

#[cfg(feature = "blocking-client")]
fn get_camera_address() -> String {
    let args: Vec<String> = env::args().collect();
    let ip_address = if args.len() > 1 {
        &args[1]
    } else {
        "192.168.0.110"
    };
    format!("{ip_address}:1259")
}

#[cfg(feature = "blocking-client")]
fn display_camera_constants(model: CameraModel) {
    info!("\nCamera Constants for {model:?}:");
    info!("  Pan range: {:?} VISCA units", model.pan_range());
    info!("  Tilt range: {:?} VISCA units", model.tilt_range());
    info!("  Zoom range: {:?} VISCA units", model.zoom_range());
    info!("  Pan degrees: {} degrees", model.pan_degrees());
    info!("  Tilt degrees: {} degrees", model.tilt_degrees());
    info!("  Max pan speed: {}", model.max_pan_speed());
    info!("  Max tilt speed: {}", model.max_tilt_speed());
}

#[cfg(feature = "blocking-client")]
fn demonstrate_position_conversions(
    client: &mut Client,
    model: CameraModel,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("\nQuerying current camera position...");
    let (pan, tilt) = client.get_pan_tilt_position()?;

    let visca_pos = ViscaPosition { pan, tilt };
    info!("Current VISCA position: pan={pan}, tilt={tilt}");

    // Convert to different units
    let degrees = visca_pos.to_degrees(model);
    let normalized = visca_pos.to_normalized(model);

    info!("\nPosition conversions:");
    info!(
        "  VISCA units: pan={}, tilt={}",
        visca_pos.pan, visca_pos.tilt
    );
    info!(
        "  Degrees: pan={:.1}°, tilt={:.1}°",
        degrees.pan, degrees.tilt
    );
    info!(
        "  Normalized: pan={:.3}, tilt={:.3}",
        normalized.pan, normalized.tilt
    );

    // Demonstrate reverse conversions
    info!("\nReverse conversions:");
    let from_degrees = degrees.to_visca(model);
    info!(
        "  Degrees -> VISCA: pan={}, tilt={}",
        from_degrees.pan, from_degrees.tilt
    );

    let from_normalized = normalized.to_visca(model);
    info!(
        "  Normalized -> VISCA: pan={}, tilt={}",
        from_normalized.pan, from_normalized.tilt
    );

    Ok(())
}

#[cfg(feature = "blocking-client")]
fn demonstrate_validation(model: CameraModel) {
    info!("\nValidation examples:");

    // Valid pan position
    match constants::validate_pan_position(1000, model) {
        Ok(pos) => info!("  Pan position {pos} is valid"),
        Err(e) => error!("  Pan validation error: {e}"),
    }

    // Invalid pan position
    match constants::validate_pan_position(5000, model) {
        Ok(pos) => info!("  Pan position {pos} is valid"),
        Err(e) => info!("  Pan validation error (expected): {e}"),
    }

    // Valid preset ID
    match constants::validate_preset_id(50) {
        Ok(id) => info!("  Preset ID {id} is valid"),
        Err(e) => error!("  Preset validation error: {e}"),
    }

    // Invalid preset ID
    match constants::validate_preset_id(150) {
        Ok(id) => info!("  Preset ID {id} is valid"),
        Err(e) => info!("  Preset validation error (expected): {e}"),
    }
}

#[cfg(feature = "blocking-client")]
fn move_to_degrees_position(client: &mut Client, model: CameraModel) {
    info!("\nMoving to position specified in degrees...");
    let target_degrees = DegreePosition {
        pan: 45.0,
        tilt: 15.0,
    };
    let target_visca = target_degrees.to_visca(model);

    info!(
        "Target position: {:.1}° pan, {:.1}° tilt",
        target_degrees.pan, target_degrees.tilt
    );
    info!(
        "Converted to VISCA: {} pan, {} tilt",
        target_visca.pan, target_visca.tilt
    );

    // Validate before sending
    if let (Ok(_), Ok(_)) = (
        constants::validate_pan_position(target_visca.pan, model),
        constants::validate_tilt_position(target_visca.tilt, model),
    ) {
        match PanTiltExt::move_to_position(
            client,
            target_visca.pan,
            target_visca.tilt,
            Some((
                PanSpeed::new(constants::speed::PAN_SPEED_DEFAULT).unwrap(),
                TiltSpeed::new(constants::speed::TILT_SPEED_DEFAULT).unwrap(),
            )),
        ) {
            Ok(()) => info!("Successfully moved to target position"),
            Err(e) => error!("Failed to move: {e}"),
        }
    } else {
        error!("Target position is out of range");
    }
}

#[cfg(feature = "blocking-client")]
fn display_other_constants() {
    info!("\nOther useful constants:");
    info!(
        "  Default VISCA port: {}",
        constants::network::VISCA_DEFAULT_PORT
    );
    info!(
        "  Command timeout: {} ms",
        constants::timing::COMMAND_TIMEOUT_MS
    );
    info!(
        "  Preset recall timeout: {} ms",
        constants::timing::PRESET_RECALL_TIMEOUT_MS
    );
    info!("  Max preset ID: {}", constants::preset::PRESET_ID_MAX);
}

#[cfg(feature = "blocking-client")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("Position conversion and constants example");

    let address = get_camera_address();
    let mut client = Client::connect_udp(&address)?;
    info!("Connected to camera at {address}");

    let model = CameraModel::PTZOpticsG2;
    info!("Using camera model: {model:?}");

    display_camera_constants(model);
    demonstrate_position_conversions(&mut client, model)?;
    demonstrate_validation(model);
    move_to_degrees_position(&mut client, model);
    display_other_constants();

    Ok(())
}

#[cfg(not(feature = "blocking-client"))]
fn main() {
    println!("This example requires the 'blocking-client' feature to be enabled.");
    println!("Run with: cargo run --example position_conversion --features blocking-client");
}
