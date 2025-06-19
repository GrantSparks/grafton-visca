//! Example demonstrating how to use the new async transport API
//! in a blocking context using a minimal runtime.
//!
//! The new transport API is async-first, but this shows how to use it
//! in blocking code with a minimal tokio runtime.

use grafton_visca::{
    camera::{profiles::G2PresetId, Camera, PTZOpticsG2},
    command::pan_tilt::PanTiltDirection,
    transport::create,
};

// Import Degrees from the correct path
use grafton_visca::camera::units::Degrees;
use std::time::Duration;

// Use a minimal tokio runtime for blocking execution
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(fut)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Camera IP address with port
    let camera_ip = std::env::var("CAMERA_IP").unwrap_or_else(|_| "192.168.1.100:5678".to_string());
    println!("Connecting to camera at {}", camera_ip);

    // Create transport using the new clean API
    let transport = block_on(create::tcp(&camera_ip))?;

    // Create camera with PTZOpticsG2 profile
    let camera = Camera::<PTZOpticsG2>::new(transport);

    println!("Camera created successfully with new transport API!");

    // Test basic operations
    println!("\nTesting new async transport API in blocking context:");

    // Power on
    println!("Powering on...");
    block_on(camera.power_on())?;

    // Wait a moment for camera to initialize
    std::thread::sleep(Duration::from_secs(2));

    // Move to home position
    println!("Moving to home position...");
    block_on(camera.home())?;

    // Wait for movement to complete
    std::thread::sleep(Duration::from_secs(3));

    // Test zoom
    println!("Testing zoom in...");
    block_on(camera.zoom_in())?;
    std::thread::sleep(Duration::from_millis(500));

    println!("Stopping zoom...");
    block_on(camera.zoom_stop())?;

    // Test pan/tilt movement
    println!("Testing pan/tilt movement...");

    // Move right
    println!("Moving right...");
    block_on(camera.move_continuous(PanTiltDirection::Right, 5, 0))?;
    std::thread::sleep(Duration::from_millis(500));

    // Stop movement
    println!("Stopping movement...");
    block_on(camera.stop())?;

    // Move to specific position
    println!("Moving to position (30°, 15°)...");
    block_on(camera.set_position(Degrees(30.0), Degrees(15.0)))?;

    // Wait for movement
    std::thread::sleep(Duration::from_secs(2));

    // Test presets
    println!("Setting preset 1...");
    let preset1 = G2PresetId::new(1)?;
    block_on(camera.set_preset(preset1))?;

    // Move to a different position
    println!("Moving to position (-30°, -15°)...");
    block_on(camera.set_position(Degrees(-30.0), Degrees(-15.0)))?;
    std::thread::sleep(Duration::from_secs(2));

    // Recall preset
    println!("Recalling preset 1...");
    block_on(camera.recall_preset(preset1))?;

    std::thread::sleep(Duration::from_secs(2));

    println!("\nAll tests completed successfully!");
    println!("The new async transport API works in blocking context with minimal runtime!");

    Ok(())
}

// The new transport API is async-first but can be used in any context with block_on
