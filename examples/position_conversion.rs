//! Example demonstrating position conversion and camera constants usage
//!
//! This example shows how to:
//! - Use camera-specific constants
//! - Convert between different position units (VISCA, degrees, normalized)
//! - Validate camera parameters
//! - Detect camera model (placeholder functionality)

use grafton_visca::{
    command::{InquiryCommand, PanTiltCommand},
    constants::{
        self, CameraConstants, CameraModel, DegreePosition, PositionConversion,
        ViscaPosition,
    },
    send_command_and_wait, CameraDetection, UdpTransport, ViscaInquiryResponse, ViscaResponse,
};
use log::{error, info};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("Position conversion and constants example");

    // Get camera address from command line or use default
    let args: Vec<String> = env::args().collect();
    let ip_address = if args.len() > 1 {
        &args[1]
    } else {
        "192.168.0.110"
    };
    let address = format!("{}:1259", ip_address);

    // Connect to camera
    let mut transport = UdpTransport::new(&address)?;
    info!("Connected to camera at {}", address);

    // Try to detect camera model (currently returns Unknown)
    let model = transport.detect_camera_model()?;
    info!("Detected camera model: {:?}", model);

    // For this example, we'll assume PTZOptics G2
    let model = CameraModel::PTZOpticsG2;
    info!("Using camera model: {:?}", model);

    // Display camera constants
    info!("\nCamera Constants for {:?}:", model);
    info!("  Pan range: {:?} VISCA units", model.pan_range());
    info!("  Tilt range: {:?} VISCA units", model.tilt_range());
    info!("  Zoom range: {:?} VISCA units", model.zoom_range());
    info!("  Pan degrees: {} degrees", model.pan_degrees());
    info!("  Tilt degrees: {} degrees", model.tilt_degrees());
    info!("  Max pan speed: {}", model.max_pan_speed());
    info!("  Max tilt speed: {}", model.max_tilt_speed());

    // Get current position
    info!("\nQuerying current camera position...");
    let response = send_command_and_wait(&mut transport, &InquiryCommand::PanTiltPosition)?;
    
    if let ViscaResponse::InquiryResponse(ViscaInquiryResponse::PanTiltPosition { pan, tilt }) = response {
        let visca_pos = ViscaPosition { pan, tilt };
        info!("Current VISCA position: pan={}, tilt={}", pan, tilt);

        // Convert to different units
        let degrees = visca_pos.to_degrees(model);
        let normalized = visca_pos.to_normalized(model);

        info!("\nPosition conversions:");
        info!("  VISCA units: pan={}, tilt={}", visca_pos.pan, visca_pos.tilt);
        info!("  Degrees: pan={:.1}°, tilt={:.1}°", degrees.pan, degrees.tilt);
        info!("  Normalized: pan={:.3}, tilt={:.3}", normalized.pan, normalized.tilt);

        // Demonstrate reverse conversions
        info!("\nReverse conversions:");
        let from_degrees = degrees.to_visca(model);
        info!("  Degrees -> VISCA: pan={}, tilt={}", from_degrees.pan, from_degrees.tilt);
        
        let from_normalized = normalized.to_visca(model);
        info!("  Normalized -> VISCA: pan={}, tilt={}", from_normalized.pan, from_normalized.tilt);
    } else {
        error!("Failed to get current position");
    }

    // Demonstrate validation
    info!("\nValidation examples:");
    
    // Valid pan position
    match constants::validate_pan_position(1000, model) {
        Ok(pos) => info!("  Pan position {} is valid", pos),
        Err(e) => error!("  Pan validation error: {}", e),
    }
    
    // Invalid pan position
    match constants::validate_pan_position(5000, model) {
        Ok(pos) => info!("  Pan position {} is valid", pos),
        Err(e) => info!("  Pan validation error (expected): {}", e),
    }
    
    // Valid preset ID
    match constants::validate_preset_id(50) {
        Ok(id) => info!("  Preset ID {} is valid", id),
        Err(e) => error!("  Preset validation error: {}", e),
    }
    
    // Invalid preset ID
    match constants::validate_preset_id(150) {
        Ok(id) => info!("  Preset ID {} is valid", id),
        Err(e) => info!("  Preset validation error (expected): {}", e),
    }

    // Demonstrate moving to a position specified in degrees
    info!("\nMoving to position specified in degrees...");
    let target_degrees = DegreePosition { pan: 45.0, tilt: 15.0 };
    let target_visca = target_degrees.to_visca(model);
    
    info!("Target position: {:.1}° pan, {:.1}° tilt", target_degrees.pan, target_degrees.tilt);
    info!("Converted to VISCA: {} pan, {} tilt", target_visca.pan, target_visca.tilt);
    
    // Validate before sending
    if let (Ok(_), Ok(_)) = (
        constants::validate_pan_position(target_visca.pan, model),
        constants::validate_tilt_position(target_visca.tilt, model),
    ) {
        let command = PanTiltCommand::AbsolutePosition {
            pan: target_visca.pan,
            tilt: target_visca.tilt,
            pan_speed: constants::speed::PAN_SPEED_DEFAULT,
            tilt_speed: constants::speed::TILT_SPEED_DEFAULT,
        };
        
        match send_command_and_wait(&mut transport, &command) {
            Ok(_) => info!("Successfully moved to target position"),
            Err(e) => error!("Failed to move: {}", e),
        }
    } else {
        error!("Target position is out of range");
    }

    // Using constants for network and timing
    info!("\nOther useful constants:");
    info!("  Default VISCA port: {}", constants::network::VISCA_DEFAULT_PORT);
    info!("  Command timeout: {} ms", constants::timing::COMMAND_TIMEOUT_MS);
    info!("  Preset recall timeout: {} ms", constants::timing::PRESET_RECALL_TIMEOUT_MS);
    info!("  Max preset ID: {}", constants::preset::PRESET_ID_MAX);

    Ok(())
}