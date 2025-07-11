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
use grafton_visca::transport::blocking::Tcp;
#[cfg(not(feature = "async"))]
use grafton_visca::{
    camera::methods::{PanTiltBlockingExt, PowerBlockingExt, ZoomBlockingExt},
    profiles::PTZOpticsG2,
    CameraBlocking,
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
    let transport = Tcp::connect(&camera_ip)?;

    // Create camera with PTZOpticsG2 profile
    let mut camera = CameraBlocking::<PTZOpticsG2, _>::new(transport);

    println!("Camera created successfully with blocking transport!");
    println!("Running without any async runtime - pure blocking I/O!");

    // Note: The capabilities() method is not available in the current API
    // Camera capabilities are defined by the profile (PTZOpticsG2) at compile time
    println!("\nUsing PTZOpticsG2 camera profile");

    // Test basic operations - all blocking, no async runtime
    println!("\nTesting camera operations:");

    // Power on
    println!("Powering on...");
    camera.power_on()?;
    std::thread::sleep(Duration::from_secs(2));

    // Move to home position
    println!("Moving to home position...");
    camera.pan_tilt_home()?;
    std::thread::sleep(Duration::from_secs(3));

    // Test absolute position movement
    println!("Moving to position (30°, -10°)...");
    camera.pan_tilt_absolute(30.0, -10.0, 10)?;
    std::thread::sleep(Duration::from_secs(3));

    // Test relative movement
    println!("Moving relative: pan left 20°...");
    camera.pan_tilt_relative(-20.0, 0.0, 10)?;
    std::thread::sleep(Duration::from_secs(2));

    // Test zoom
    println!("Testing zoom in...");
    camera.zoom_in()?;
    std::thread::sleep(Duration::from_secs(1));
    camera.zoom_stop()?;

    // Note: Inquiry methods are not available in the current API
    // The API focuses on control commands rather than status queries

    println!("\nAll operations completed successfully!");
    println!("Pure blocking I/O works perfectly without any async runtime!");

    Ok(())
}
