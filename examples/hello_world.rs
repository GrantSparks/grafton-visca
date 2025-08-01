//! Hello world example - the simplest possible VISCA camera control.
//!
//! This example demonstrates basic camera control with both blocking and async APIs
//! using the new profile-centric connection helpers.
//!
//! Run with:
//! - Blocking: cargo run --example hello_world <camera_ip:port>
//! - Async: cargo run --example hello_world --features tokio <camera_ip:port>

use grafton_visca::camera::{profiles::PTZOpticsG2, Camera};
use grafton_visca::Result;
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
    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:52381".to_string());

    println!("Connecting to camera at {camera_addr} (blocking mode)");

    // Create camera using the new profile-centric connection helper
    let camera = Camera::<PTZOpticsG2, _>::connect_tcp(&camera_addr)?;

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
    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:52381".to_string());

    println!("Connecting to camera at {camera_addr} (async mode)");

    // Create camera using the new profile-centric async connection helper
    let camera = Camera::<PTZOpticsG2, _>::connect_tokio_tcp(&camera_addr).await?;

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
