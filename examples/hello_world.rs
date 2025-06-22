//! Hello world example - the simplest possible VISCA camera control.
//!
//! This example demonstrates basic blocking camera control with no async features.

#[cfg(feature = "async")]
fn main() {
    eprintln!("This example demonstrates the blocking API. Run without async features:");
    eprintln!("cargo run --example hello_world --no-default-features");
}

#[cfg(not(feature = "async"))]
use grafton_visca::{
    camera::{profiles::PTZOpticsG2, Camera},
    transport::blocking::create,
};
#[cfg(not(feature = "async"))]
use std::env;

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Get camera address from command line or use default
    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    println!("Connecting to camera at {}", camera_addr);

    // Create camera with blocking TCP transport
    let transport = create::tcp(&camera_addr)?;
    let mut camera = Camera::<PTZOpticsG2, _>::new(transport);

    // Power on the camera
    println!("Powering on camera...");
    camera.power_on()?;

    // Wait for camera to initialize
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Move to home position
    println!("Moving to home position...");
    camera.home()?;

    println!("Hello from VISCA camera!");

    Ok(())
}
