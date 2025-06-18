//! Example demonstrating the blocking API without any async runtime.
//!
//! This shows that the blocking API can work without tokio or any async dependencies.
//!
//! Note: This example requires that ONLY the blocking-client feature is enabled.
//! If async-client is also enabled, the async API takes precedence.

#[cfg(feature = "async-client")]
compile_error!("This example requires only the blocking-client feature. Please run with: cargo run --example blocking_no_runtime --no-default-features --features blocking-client");

use grafton_visca::{
    camera::{profiles::G2PresetId, units::Degrees, Camera, PTZOpticsG2},
    command::pan_tilt::PanTiltDirection,
    transport::{BlockingAdapter, TcpTransport},
};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Camera IP address
    let camera_ip = std::env::var("CAMERA_IP").unwrap_or_else(|_| "192.168.1.100".to_string());
    println!("Connecting to camera at {}", camera_ip);

    // Create a blocking TCP transport
    let tcp_transport = TcpTransport::new(&camera_ip)?;

    // Wrap it in a BlockingAdapter to make it compatible with the Camera struct
    let transport = BlockingAdapter(tcp_transport);

    // Create camera with PTZOpticsG2 profile
    let camera = Camera::<PTZOpticsG2>::new(transport);

    println!("Camera created successfully!");

    // Test basic operations
    println!("\nTesting blocking API without async runtime:");

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
    println!("The blocking API works without any async runtime!");

    Ok(())
}
