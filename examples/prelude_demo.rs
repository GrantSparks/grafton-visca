//! Demonstrates the use of the prelude module for clean imports.
//!
//! The prelude module provides a convenient way to import all camera operation traits
//! without having to list them individually.

use grafton_visca::prelude::*;
use grafton_visca::transport::blocking::Tcp;
use std::env;

fn main() -> Result<(), Error> {
    // Initialize logging
    env_logger::init();

    // Get camera address from command line or use default
    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    println!("Connecting to camera at {}...", camera_addr);

    // Create camera - all operation traits are available through prelude
    let transport = Tcp::connect(&camera_addr)?;
    let camera = Camera::with_profile(CameraModel::PTZOpticsG2, transport);

    println!("Camera model: {}", camera.model_name());

    // The blocking wrapper gives us access to all blocking trait methods
    let camera = camera.blocking();

    // All camera operations are available without individual trait imports:

    // Power operations
    camera.power_on()?;

    // Zoom operations
    camera.zoom_stop()?;

    // Pan/Tilt operations
    camera.pan_tilt_home()?;

    // Focus operations
    camera.focus_auto()?;

    // Preset operations
    camera.preset_recall(PresetNumber::new(1).unwrap())?;

    // Exposure operations
    camera.exposure_auto()?;

    // And many more...

    println!("All operations completed successfully!");

    Ok(())
}

#[cfg(feature = "tokio")]
#[tokio::main]
#[allow(dead_code)]
async fn async_example() -> Result<(), Error> {
    use grafton_visca::transport::tokio::Tcp;

    // For async operations, use the async prelude
    use grafton_visca::r#async::prelude::*;

    let transport = Tcp::connect("192.168.1.100:5678").await?;
    let camera = Camera::with_profile(CameraModel::PTZOpticsG2, transport).r#async();

    // All async operations available
    camera.power_on().await?;
    camera.zoom_stop().await?;
    camera.pan_tilt_home().await?;

    Ok(())
}
