//! Hello world example - the simplest possible VISCA camera control.
//!
//! This example demonstrates basic camera control with both blocking and async APIs
//! using the new profile-centric connection helpers.
//!
//! Run with:
//! - Blocking: cargo run --example hello_world <camera_ip:port>
//! - Async: cargo run --example hello_world --features tokio <camera_ip:port>

use grafton_visca::{camera::profiles::PTZOpticsG2, CameraBuilder, Result};
use std::env;

// Import operation traits for blocking operations
#[cfg(not(feature = "tokio"))]
use grafton_visca::blocking::{PanTiltOps, PowerOps};

// Import operation traits for async operations
#[cfg(feature = "tokio")]
use grafton_visca::r#async::{PanTiltOps, PowerOps};

#[cfg(not(feature = "tokio"))]
fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();

    // Get camera address from command line or use default
    // Note: Port is now optional! The builder will automatically add the correct
    // default port based on the camera profile (5678 for PTZOptics TCP)
    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110".to_string());

    println!("Connecting to camera at {camera_addr} (blocking mode)");

    // Create camera using the new builder pattern
    // The builder automatically adds port 5678 for PTZOpticsG2 TCP if not specified
    let camera = CameraBuilder::tcp(&camera_addr)
        .profile::<PTZOpticsG2>()
        .build()?;

    // Power on the camera
    println!("Powering on camera...");
    camera.power_on()?;

    // Wait for camera to initialize
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Move to home position
    println!("Moving to home position...");
    camera.pan_tilt_home()?;

    println!("Hello from VISCA camera!");

    Ok(())
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();

    // Get camera address from command line or use default
    // Note: Port is now optional! The builder will automatically add the correct
    // default port based on the camera profile (5678 for PTZOptics TCP)
    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110".to_string());

    println!("Connecting to camera at {camera_addr} (async mode)");

    // Create camera using the new builder pattern
    // The builder automatically adds port 5678 for PTZOpticsG2 TCP if not specified
    let camera = CameraBuilder::tokio_tcp(&camera_addr)
        .profile::<PTZOpticsG2>()
        .build()
        .await?;

    // Power on the camera
    println!("Powering on camera...");
    camera.power_on().await?;

    // Wait for camera to initialize
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Move to home position
    println!("Moving to home position...");
    camera.pan_tilt_home().await?;

    println!("Hello from VISCA camera!");

    Ok(())
}
