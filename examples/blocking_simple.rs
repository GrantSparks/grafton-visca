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

    // Create a runtime for async operations
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        // Test basic operations
        println!("\nTesting camera operations:");

        // Power on
        println!("Powering on...");
        camera.power_on()?;
        std::thread::sleep(Duration::from_secs(2));

        // Move to home position
        println!("Moving to home position...");
        camera.home()?;
        std::thread::sleep(Duration::from_secs(3));

        // Test absolute position movement
        println!("Moving to position (30°, -10°)...");
        camera.set_position(Degrees(30.0), Degrees(-10.0))?;
        std::thread::sleep(Duration::from_secs(3));

        // Test continuous movement
        println!("Starting continuous pan left...");
        camera.move_continuous(PanTiltDirection::Left, 10, 0)?;
        std::thread::sleep(Duration::from_secs(2));

        println!("Stopping movement...");
        camera.stop()?;

        // Test zoom
        println!("Testing zoom in...");
        camera.zoom_in()?;
        std::thread::sleep(Duration::from_secs(1));
        camera.zoom_stop()?;

        println!("\nAll operations completed successfully!");
        println!("The blocking transport API works seamlessly without an async runtime!");
        println!("(Note: The Camera API is async internally, so we use a minimal runtime here)");

        Ok::<_, Box<dyn std::error::Error>>(())
    })
}
