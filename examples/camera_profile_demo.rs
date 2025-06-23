//! Example demonstrating the new Camera API with type-safe profiles.

#[cfg(feature = "tokio")]
use grafton_visca::{
    camera::{
        profiles::{G2PresetId, PTZOpticsG2},
        Camera,
    },
    transport::tokio::Tcp,
    types::ZoomPosition,
    units::Degrees,
    units::Raw,
    Error,
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
    let camera = Camera::<PTZOpticsG2, _>::new(transport);

    // Display camera capabilities
    let caps = camera.capabilities();
    println!("Camera Model: {}", caps.model_name);
    println!("Pan Range: {:?} degrees", caps.pan_range_degrees);
    println!("Tilt Range: {:?} degrees", caps.tilt_range_degrees);
    println!("Max Pan Speed: {}", caps.max_pan_speed);
    println!("Max Tilt Speed: {}", caps.max_tilt_speed);
    println!("Preset Count: {}", caps.preset_count);
    println!();

    // Power on the camera
    println!("Powering on camera...");
    camera.power_on().await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Move to home position
    println!("Moving to home position...");
    camera.home().await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Move to specific position in degrees
    println!("Moving to 45° pan, 30° tilt...");
    camera.set_position(Degrees(45.0), Degrees(30.0)).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Move using normalized coordinates
    println!("Moving to normalized position (0.5, -0.25)...");
    // Convert normalized coordinates to degrees
    let pan_deg = Degrees(0.5 * 180.0); // 90 degrees
    let tilt_deg = Degrees(-0.25 * 90.0); // -22.5 degrees
    camera.set_position(pan_deg, tilt_deg).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Set and recall a preset (G2 supports presets 0-89)
    println!("Setting preset 10...");
    let preset = G2PresetId::new(10)?;
    camera.set_preset(preset.into()).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Move somewhere else
    println!("Moving to different position...");
    camera.set_position(Degrees(-30.0), Degrees(15.0)).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Recall the preset
    println!("Recalling preset 10...");
    camera.recall_preset(preset.into()).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Zoom operations
    println!("Zooming in...");
    camera.zoom_in().await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    println!("Stopping zoom...");
    camera.zoom_stop().await?;

    // Set specific zoom position
    println!("Setting zoom to 50%...");
    let _zoom_50_percent = ZoomPosition::new(0x7000 / 2)?; // Half of max zoom for G2
    camera.set_zoom(Raw(0x7000u16 / 2).into()).await?; // Direct VISCA value
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Focus control
    println!("Setting focus to auto...");
    camera.focus_auto().await?;

    println!("Demo complete!");
    Ok(())
}
