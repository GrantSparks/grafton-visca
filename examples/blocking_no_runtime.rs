//! Example demonstrating how to use the blocking transport API without any async runtime.
//!
//! This shows the primary blocking-first API that requires no async dependencies.

use grafton_visca::{
    camera::{profiles::G2PresetId, Camera, PTZOpticsG2},
    command::pan_tilt::PanTiltDirection,
    transport::blocking::create,
};

// Import Degrees from the correct path
use grafton_visca::camera::units::Degrees;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Camera IP address with port
    let camera_ip = std::env::var("CAMERA_IP").unwrap_or_else(|_| "192.168.1.100:5678".to_string());
    println!("Connecting to camera at {}", camera_ip);

    // Create transport using the blocking API
    let transport = create::tcp(&camera_ip)?;

    // Create camera with PTZOpticsG2 profile
    let mut camera = Camera::<PTZOpticsG2>::new(transport);

    println!("Camera created successfully with blocking transport API!");

    // Test basic operations
    println!("\nTesting blocking transport API:");

    // Power on
    println!("Powering on...");
    camera.power_on()?;

    // Wait a moment for camera to initialize
    std::thread::sleep(Duration::from_secs(2));

    // Move to home position
    println!("Moving to home position...");
    camera.home()?;

    // Wait for movement to complete
    std::thread::sleep(Duration::from_secs(3));

    // Test zoom
    println!("Testing zoom in...");
    camera.zoom_in()?;
    std::thread::sleep(Duration::from_millis(500));

    println!("Stopping zoom...");
    camera.zoom_stop()?;

    // Test pan/tilt movement
    println!("Testing pan/tilt movement...");

    // Move right
    println!("Moving right...");
    camera.move_continuous(PanTiltDirection::Right, 5, 0)?;
    std::thread::sleep(Duration::from_millis(500));

    // Stop movement
    println!("Stopping movement...");
    camera.stop()?;

    // Move to specific position
    println!("Moving to position (30°, 15°)...");
    camera.set_position(Degrees(30.0), Degrees(15.0))?;

    // Wait for movement
    std::thread::sleep(Duration::from_secs(2));

    // Test presets
    println!("Setting preset 1...");
    let preset1 = G2PresetId::new(1)?;
    camera.set_preset(preset1)?;

    // Move to a different position
    println!("Moving to position (-30°, -15°)...");
    camera.set_position(Degrees(-30.0), Degrees(-15.0))?;
    std::thread::sleep(Duration::from_secs(2));

    // Recall preset
    println!("Recalling preset 1...");
    camera.recall_preset(preset1)?;

    std::thread::sleep(Duration::from_secs(2));

    println!("\nAll tests completed successfully!");
    println!("The blocking transport API works without any async runtime!");

    Ok(())
}
