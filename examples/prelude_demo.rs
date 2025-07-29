//! Demonstrates the use of the prelude module for clean imports.
//!
//! The prelude module provides a convenient way to import all camera operation traits
//! without having to list them individually.

use std::env;

#[cfg(not(feature = "tokio"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::prelude::blocking::*;
    use grafton_visca::transport::blocking::Tcp;

    // Initialize logging
    env_logger::init();

    // Get camera address from command line or use default
    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    println!("Connecting to camera at {}...", camera_addr);

    // Create camera - all operation traits are available through prelude
    let transport = Tcp::connect(&camera_addr)?;
    let camera = PTZOpticsG2Cam::new(transport);

    println!("Using PTZOptics G2 camera");

    // All camera operations are available without individual trait imports:

    // Power operations (PowerOps trait)
    println!("Powering on...");
    camera.power_on()?;

    // Zoom operations (ZoomOps trait)
    println!("Testing zoom...");
    camera.zoom_stop()?;

    // Pan/Tilt operations (PanTiltOps trait)
    println!("Moving to home position...");
    camera.pan_tilt_home()?;

    // Focus operations (FocusOps trait)
    println!("Setting auto focus...");
    camera.focus_auto()?;

    // Preset operations (PresetsOps trait)
    println!("Recalling preset 1...");
    camera.preset_recall(PresetNumber::new(1)?)?;

    // White balance operations (WhiteBalanceOps trait)
    println!("Setting white balance to auto...");
    camera.white_balance_auto()?;

    println!("All operations completed successfully!");

    Ok(())
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::prelude::r#async::*;
    use grafton_visca::transport::tokio::Tcp;

    // Initialize logging
    env_logger::init();

    // Get camera address from command line or use default
    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    println!("Connecting to camera at {} (async mode)...", camera_addr);

    let transport = Tcp::connect(&camera_addr).await?;
    let camera = PTZOpticsG2Cam::new(transport);

    // All async operations available
    println!("Powering on...");
    camera.power_on().await?;

    println!("Testing zoom...");
    camera.zoom_stop().await?;

    println!("Moving to home position...");
    camera.pan_tilt_home().await?;

    println!("All operations completed successfully!");

    Ok(())
}
