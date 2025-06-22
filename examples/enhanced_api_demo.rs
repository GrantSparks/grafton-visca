//! Enhanced API demonstration using the new Camera API.
//!
//! This example showcases the enhanced Camera API with high-level control methods
//! and demonstrates migration from the old Client API.

#[cfg(feature = "tokio")]
use grafton_visca::{
    camera::{profiles::PTZOpticsG2, Camera},
    command::{exposure::ExposureMode, white_balance::WhiteBalanceMode},
    transport::create,
    types::{
        BrightnessLevel, ContrastLevel, NoiseReduction2DLevel, SaturationLevel, SharpnessLevel,
    },
    units::Degrees,
    Error,
};
#[cfg(feature = "tokio")]
use std::time::Duration;
#[cfg(feature = "tokio")]
use tokio::time;

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    // Connect to camera using new Camera API with UDP transport
    let transport = create::udp("192.168.1.100:5678").await?;
    let camera = Camera::<PTZOpticsG2, _>::new(transport);

    println!("=== Enhanced Camera API Demo ===\n");

    // Power on the camera
    println!("Powering on camera...");
    camera.power_on().await?;
    println!("Camera powered on");
    time::sleep(Duration::from_secs(2)).await;

    // Demonstrate exposure control
    println!("\n--- Exposure Control ---");
    camera.set_exposure_mode(ExposureMode::Auto).await?;
    println!("Set exposure mode to Auto");

    camera.set_backlight(true).await?;
    println!("Enabled backlight compensation");

    camera.set_brightness(BrightnessLevel::new(0x08)?).await?;
    println!("Set brightness to default level");

    // Demonstrate white balance control
    println!("\n--- White Balance Control ---");
    camera
        .set_white_balance_mode(WhiteBalanceMode::Outdoor)
        .await?;
    println!("Set white balance to outdoor preset");
    time::sleep(Duration::from_secs(1)).await;

    camera
        .set_white_balance_mode(WhiteBalanceMode::Indoor)
        .await?;
    println!("Set white balance to indoor preset");
    time::sleep(Duration::from_secs(1)).await;

    // Demonstrate image settings
    println!("\n--- Image Settings ---");
    // The new API doesn't have image presets, so we'll set individual parameters
    camera.set_saturation(SaturationLevel::new(10)?).await?; // Higher saturation for "vivid"
    camera.set_contrast(ContrastLevel::new(9)?).await?; // Higher contrast
    println!("Applied vivid image settings");
    time::sleep(Duration::from_secs(1)).await;

    camera
        .set_noise_reduction_2d(NoiseReduction2DLevel::new(3)?)
        .await?;
    println!("Set 2D noise reduction to level 3");

    // Note: set_image_flip is not available in the current API
    println!("Note: Image flip control not available in current API");

    // Demonstrate zoom control
    println!("\n--- Zoom Control ---");
    camera.set_zoom(0x0000).await?; // Minimum zoom
    println!("Set zoom to minimum (1x)");
    time::sleep(Duration::from_secs(2)).await;

    // For PTZOpticsG2, zoom range is 0x0000-0x4000 for 12x zoom
    // 5x would be approximately 0x1555
    camera.set_zoom(0x1555).await?;
    println!("Set zoom to approximately 5x");
    time::sleep(Duration::from_secs(2)).await;

    // 25% of max zoom
    camera.set_zoom(0x1000).await?;
    println!("Set zoom to 25% of maximum");
    time::sleep(Duration::from_secs(2)).await;

    // Demonstrate position control with degrees
    println!("\n--- Position Control ---");
    camera.home().await?;
    println!("Moved to home position");
    time::sleep(Duration::from_secs(2)).await;

    camera.set_position(Degrees(45.0), Degrees(15.0)).await?;
    println!("Moved to 45° pan, 15° tilt");
    time::sleep(Duration::from_secs(3)).await;

    // Note: set_position_normalized is not available in the current API
    // Using set_position with degrees instead (assuming ±170° pan, -30° to +90° tilt for PTZOpticsG2)
    camera.set_position(Degrees(-85.0), Degrees(15.0)).await?;
    println!("Moved to position (-85° pan, +15° tilt)");
    time::sleep(Duration::from_secs(3)).await;

    // Demonstrate relative movement
    println!("\n--- Relative Movement ---");
    // The new API doesn't have direct relative movement, but we can demonstrate
    // continuous movement instead
    camera
        .move_continuous(
            grafton_visca::command::pan_tilt::PanTiltDirection::UpRight,
            5,
            5,
        )
        .await?;
    println!("Moving camera up-right...");
    time::sleep(Duration::from_secs(1)).await;
    camera.stop().await?;
    println!("Stopped movement");

    // Return to default settings
    println!("\n--- Returning to Defaults ---");
    camera.set_exposure_mode(ExposureMode::Auto).await?;
    camera
        .set_white_balance_mode(WhiteBalanceMode::Auto)
        .await?;
    // Reset image settings to defaults
    camera.set_saturation(SaturationLevel::new(7)?).await?; // Default values
    camera.set_contrast(ContrastLevel::new(8)?).await?;
    camera.set_sharpness(SharpnessLevel::new(8)?).await?;
    camera.set_brightness(BrightnessLevel::new(8)?).await?;
    camera.home().await?;
    camera.set_zoom(0x0000).await?; // Minimum zoom
    println!("Returned camera to default settings");

    println!("\n=== Demo Complete ===");
    Ok(())
}

#[cfg(not(feature = "tokio"))]
fn main() {
    eprintln!("This example requires the 'tokio' feature to be enabled.");
    eprintln!("Run with: cargo run --example enhanced_api_demo --features tokio");
}
