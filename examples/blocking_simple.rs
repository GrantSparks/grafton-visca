//! Simple blocking example demonstrating the new blocking-first transport API.
//!
//! This example shows how to use the library without any async runtime,
//! using only the blocking transport and API.

use grafton_visca::camera::units::Degrees;
use grafton_visca::{
    camera::{profiles::PTZOpticsG2, Camera},
    command::pan_tilt::PanTiltDirection,
    transport::blocking::create,
};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Camera IP address with port
    let camera_ip = std::env::var("CAMERA_IP").unwrap_or_else(|_| "192.168.1.100:5678".to_string());
    println!("Connecting to camera at {}", camera_ip);

    // Create blocking transport - no async runtime needed!
    let transport = create::tcp(&camera_ip)?;

    // Create camera with PTZOpticsG2 profile
    let mut camera = Camera::<PTZOpticsG2>::new(transport);

    println!("Camera created successfully with blocking transport!");

    // Display camera capabilities
    let caps = camera.capabilities();
    println!("\nCamera Capabilities:");
    println!("  Model: {}", caps.model_name);
    println!("  Pan Range: {:?} degrees", caps.pan_range_degrees);
    println!("  Tilt Range: {:?} degrees", caps.tilt_range_degrees);
    println!("  Max Pan Speed: {}", caps.max_pan_speed);
    println!("  Max Tilt Speed: {}", caps.max_tilt_speed);

    // Test basic operations
    println!("\nTesting camera operations:");

    // Power on
    println!("Powering on...");
    camera.power_on()?;
    thread::sleep(Duration::from_secs(2));

    // Move to home position
    println!("Moving to home position...");
    camera.home()?;
    thread::sleep(Duration::from_secs(3));

    // Test zoom
    println!("Testing zoom in...");
    camera.zoom_in()?;
    thread::sleep(Duration::from_millis(500));

    println!("Stopping zoom...");
    camera.zoom_stop()?;

    // Test pan/tilt movement
    println!("Testing pan/tilt movement...");

    // Move right
    println!("Moving right...");
    camera.move_continuous(PanTiltDirection::Right, 5, 0)?;
    thread::sleep(Duration::from_millis(500));

    // Stop movement
    println!("Stopping movement...");
    camera.stop()?;

    // Move to specific position
    println!("Moving to position (30°, 15°)...");
    camera.set_position(Degrees(30.0), Degrees(15.0))?;
    thread::sleep(Duration::from_secs(2));

    // Move to a different position
    println!("Moving to position (-30°, -15°)...");
    camera.set_position(Degrees(-30.0), Degrees(-15.0))?;
    thread::sleep(Duration::from_secs(2));

    // Return to home
    println!("Returning to home position...");
    camera.home()?;
    thread::sleep(Duration::from_secs(2));

    println!("\nAll tests completed successfully!");
    println!("Blocking transport works without any async runtime!");

    Ok(())
}
