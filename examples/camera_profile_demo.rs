//! Example demonstrating the new Camera API with type-safe profiles.

#[cfg(feature = "tokio")]
use grafton_visca::{
    camera::profiles::G2PresetId,
    prelude::r#async::PTZOpticsG2Cam,
    r#async::prelude::*,
    transport::tokio::Tcp,
    types::SpeedLevel,
    units::{Degrees, Normalized},
    Error, PresetNumber,
};
#[cfg(feature = "tokio")]
use std::time::Duration;

#[cfg(not(feature = "tokio"))]
fn main() {
    eprintln!("This example requires the 'tokio' feature.");
    eprintln!("Run with: cargo run --example camera_profile_demo --features tokio");
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize logging
    env_logger::init();

    // Create a G2 camera with TCP transport
    let transport = Tcp::connect_timeout("192.168.1.100:5678", Duration::from_secs(5)).await?;
    let camera = PTZOpticsG2Cam::new(transport);

    // Display camera model (compile-time known)
    println!("Camera Model: PTZOptics G2");
    println!("All capabilities are compile-time verified:");
    println!("  ✓ Pan/Tilt");
    println!("  ✓ Zoom");
    println!("  ✓ Presets");
    println!("  ✓ Focus");
    println!("  ✓ Exposure");
    println!("  ✓ White Balance");
    println!();

    // Power on the camera
    println!("Powering on camera...");
    camera.power_on().await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Move to home position
    println!("Moving to home position...");
    camera.pan_tilt_home().await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Move to specific position in degrees
    println!("Moving to 45° pan, 30° tilt...");
    camera
        .pan_tilt_absolute(Degrees::new(45.0), Degrees::new(30.0), SpeedLevel::Fastest)
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Move using normalized coordinates
    println!("Moving to normalized position (0.5, -0.25)...");
    // Convert normalized coordinates to degrees
    // 0.5 * 180° = 90 degrees pan, -0.25 * 90° = -22.5 degrees tilt
    camera
        .pan_tilt_absolute(Degrees::new(90.0), Degrees::new(-22.5), SpeedLevel::Fastest)
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Set and recall a preset (G2 supports presets 0-89)
    println!("Setting preset 10...");
    let preset = G2PresetId::new(10)?;
    camera.preset_set(PresetNumber::new(preset.into())?).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Move somewhere else
    println!("Moving to different position...");
    camera
        .pan_tilt_absolute(Degrees::new(-30.0), Degrees::new(15.0), SpeedLevel::Fastest)
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Recall the preset
    println!("Recalling preset 10...");
    camera
        .preset_recall(PresetNumber::new(preset.into())?)
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Zoom operations
    println!("Zooming in...");
    camera.zoom_in().await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    println!("Stopping zoom...");
    camera.zoom_stop().await?;

    // Set specific zoom position
    println!("Setting zoom to 50%...");
    camera.zoom_absolute(Normalized::new(0.5)).await?; // 50% zoom
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Focus control
    println!("Setting focus to auto...");
    camera.focus_auto().await?;

    println!("Demo complete!");
    Ok(())
}
