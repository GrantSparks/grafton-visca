//! Simple blocking example demonstrating the blocking transport API.
//!
//! This example shows how to use the library without any async runtime,
//! using only the blocking transport and API when the async feature is disabled.

#[cfg(feature = "async")]
fn main() {
    eprintln!("This example demonstrates the blocking API. Run without async features:");
    eprintln!("cargo run --example blocking_simple --no-default-features");
}

#[cfg(not(feature = "async"))]
use grafton_visca::camera::units::Degrees;
#[cfg(not(feature = "async"))]
use grafton_visca::transport::blocking::create;
#[cfg(not(feature = "async"))]
use grafton_visca::{
    camera::{profiles::PTZOpticsG2, Camera},
    command::pan_tilt::PanTiltDirection,
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

    // Create blocking transport - no async runtime needed!
    let transport = create::tcp(&camera_ip)?;

    // Create camera with PTZOpticsG2 profile
    let mut camera = Camera::<PTZOpticsG2, _>::new(transport);

    println!("Camera created successfully with blocking transport!");
    println!("Running without any async runtime - pure blocking I/O!");

    // Display camera capabilities
    let caps = camera.capabilities();
    println!("\nCamera Capabilities:");
    println!("  Model: {}", caps.model_name);
    println!("  Pan Range: {:?} degrees", caps.pan_range_degrees);
    println!("  Tilt Range: {:?} degrees", caps.tilt_range_degrees);
    println!("  Max Pan Speed: {}", caps.max_pan_speed);
    println!("  Max Tilt Speed: {}", caps.max_tilt_speed);

    // Test basic operations - all blocking, no async runtime
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

    // Test inquiry operations
    println!("\nTesting inquiry operations:");

    // Get power status
    if let Ok(power_status) = camera.get_power_state() {
        println!("Power status: {:?}", power_status);
    }

    // Get current position
    if let Ok((pan, tilt)) = camera.get_position() {
        println!("Current position: Pan={:?}, Tilt={:?}", pan, tilt);
    }

    // Get zoom position
    if let Ok(zoom_pos) = camera.get_zoom_position() {
        println!("Zoom position: {:?}", zoom_pos);
    }

    println!("\nAll operations completed successfully!");
    println!("Pure blocking I/O works perfectly without any async runtime!");

    Ok(())
}
