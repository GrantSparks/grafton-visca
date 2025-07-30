//! Hello world example - the simplest possible VISCA camera control.
//!
//! This example demonstrates basic camera control with both blocking and async APIs.
//!
//! Run with:
//! - Blocking: cargo run --example hello_world <camera_ip:port>
//! - Async: cargo run --example hello_world --features tokio <camera_ip:port>

#[cfg(not(feature = "tokio"))]
use grafton_visca::prelude::blocking::*;
#[cfg(feature = "tokio")]
use grafton_visca::prelude::r#async::*;
use std::env;

#[cfg(not(feature = "tokio"))]
use grafton_visca::transport::blocking::Tcp;

#[cfg(feature = "tokio")]
use grafton_visca::transport::tokio::Tcp;

#[cfg(not(feature = "tokio"))]
fn main() -> Result<(), Error> {
    // Initialize logging
    env_logger::init();

    // Get camera address from command line or use default
    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    println!("Connecting to camera at {camera_addr} (blocking mode)");

    // Create camera with blocking TCP transport
    let transport = Tcp::connect(&camera_addr)?;
    let camera = PTZOpticsG2Cam::new(transport);

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
async fn main() -> Result<(), Error> {
    // Initialize logging
    env_logger::init();

    // Get camera address from command line or use default
    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    println!("Connecting to camera at {camera_addr} (async mode)");

    // Create camera with async TCP transport
    let transport = Tcp::connect(&camera_addr).await?;
    let camera = PTZOpticsG2Cam::new(transport);

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
