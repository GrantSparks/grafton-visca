//! Example demonstrating how to use the blocking transport API without any async runtime.
//!
//! This shows the primary blocking-first API that requires no async dependencies.

#[cfg(feature = "async")]
fn main() {
    eprintln!("This example demonstrates the blocking API. Run without async features:");
    eprintln!("cargo run --example blocking_no_runtime");
}

#[cfg(not(feature = "async"))]
use grafton_visca::camera::units::Degrees;
#[cfg(not(feature = "async"))]
use grafton_visca::{
    camera::{Camera, PTZOpticsG2},
    command::pan_tilt::PanTiltDirection,
    transport::blocking::create,
};
#[cfg(not(feature = "async"))]
use std::time::Duration;

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Camera IP address with port
    let camera_ip = std::env::var("CAMERA_IP").unwrap_or_else(|_| "192.168.1.100:5678".to_string());
    println!("Connecting to camera at {}", camera_ip);

    // Create transport using the blocking API
    let transport = create::tcp(&camera_ip)?;

    // Create camera with PTZOpticsG2 profile
    let mut camera = Camera::<PTZOpticsG2, _>::new(transport);

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
    std::thread::sleep(Duration::from_secs(1));
    camera.zoom_stop()?;

    // Move to specific position using degrees
    println!("Moving to position (30°, -10°)...");
    camera.set_position(Degrees(30.0), Degrees(-10.0))?;
    std::thread::sleep(Duration::from_secs(3));

    // Test continuous movement
    println!("Testing continuous movement...");
    camera.move_continuous(PanTiltDirection::Left, 10, 0)?;
    std::thread::sleep(Duration::from_secs(2));
    camera.stop()?;

    // Test preset operations
    println!("Saving position to preset 1...");
    camera.set_preset(1)?;

    // Move away
    println!("Moving to home...");
    camera.home()?;
    std::thread::sleep(Duration::from_secs(2));

    // Recall preset
    println!("Recalling preset 1...");
    camera.recall_preset(1)?;
    std::thread::sleep(Duration::from_secs(3));

    println!("\nAll operations completed successfully!");
    println!("Blocking transport API works perfectly without any async runtime!");

    Ok(())
}
