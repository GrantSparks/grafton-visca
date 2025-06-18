//! Example demonstrating the new Camera API with type-safe profiles.

use grafton_visca::{
    camera::{
        profiles::{G2PresetId, PTZOpticsG2},
        units::{Degrees, Normalized},
        Camera,
    },
    transport::AsyncTcpTransport,
    Error,
};

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize logging
    env_logger::init();

    // Create a G2 camera with TCP transport
    let transport = AsyncTcpTransport::new("192.168.1.100:5678").await?;
    let camera = Camera::<PTZOpticsG2>::new(transport);

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
    camera
        .set_position_normalized(Normalized(0.5), Normalized(-0.25))
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Set and recall a preset (G2 supports presets 0-89)
    println!("Setting preset 10...");
    let preset = G2PresetId::new(10)?;
    camera.set_preset(preset).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Move somewhere else
    println!("Moving to different position...");
    camera.set_position(Degrees(-30.0), Degrees(15.0)).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Recall the preset
    println!("Recalling preset 10...");
    camera.recall_preset(preset).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Zoom operations
    println!("Zooming in...");
    camera.zoom_in().await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    println!("Stopping zoom...");
    camera.zoom_stop().await?;

    // Set specific zoom position
    println!("Setting zoom to 50%...");
    let zoom_50_percent = 0x7000 / 2; // Half of max zoom for G2
    camera.set_zoom(zoom_50_percent).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Focus control
    println!("Setting focus to auto...");
    camera.focus_auto().await?;

    println!("Demo complete!");
    Ok(())
}
