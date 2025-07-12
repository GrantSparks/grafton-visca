//! Enhanced API demonstration using the new Camera API.
//!
//! This example showcases the enhanced Camera API with high-level control methods
//! and demonstrates migration from the old Client API.

#[cfg(feature = "tokio")]
use grafton_visca::{
    camera::methods::{
        ExposureOps, ImageProcessingOps, PanTiltOps, PowerOps, WhiteBalanceOps, ZoomOps,
    },
    
    transport::tokio::Udp,
    Camera, Error, Normalized, Degrees,
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
    let transport = Udp::connect("192.168.1.100:5678").await?;
    let camera = Camera::new(transport);

    println!("=== Enhanced Camera API Demo ===\n");

    // Power on the camera
    println!("Powering on camera...");
    camera.power_on().await?;
    println!("Camera powered on");
    time::sleep(Duration::from_secs(2)).await;

    // Demonstrate exposure control
    println!("\n--- Exposure Control ---");
    camera.exposure_auto().await?;
    println!("Set exposure mode to Auto");

    // Note: set_backlight and set_brightness are not available in the current API
    println!("Note: Backlight compensation and brightness control not yet implemented");

    // Demonstrate white balance control
    println!("\n--- White Balance Control ---");
    camera.white_balance_auto().await?;
    println!("Set white balance to auto mode");
    // Note: specific white balance modes (Outdoor, Indoor) not yet implemented
    println!("Note: Specific white balance presets not yet implemented");

    // Demonstrate image settings
    println!("\n--- Image Settings ---");
    // Note: Image processing methods like set_saturation, set_contrast are not implemented
    println!("Note: Image processing controls (saturation, contrast, noise reduction) not yet implemented");

    // Image flip is available
    camera.enable_flip().await?;
    println!("Enabled image flip");

    // Demonstrate zoom control
    println!("\n--- Zoom Control ---");
    camera.zoom_absolute(Normalized(0.0)).await?; // Minimum zoom
    println!("Set zoom to minimum (1x)");
    time::sleep(Duration::from_secs(2)).await;

    camera.zoom_absolute(Normalized(0.42)).await?; // Approximately 5x for 12x camera
    println!("Set zoom to approximately 5x");
    time::sleep(Duration::from_secs(2)).await;

    camera.zoom_absolute(Normalized(0.25)).await?; // 25% of max zoom
    println!("Set zoom to 25% of maximum");
    time::sleep(Duration::from_secs(2)).await;

    // Demonstrate position control with degrees
    println!("\n--- Position Control ---");
    camera.pan_tilt_home().await?;
    println!("Moved to home position");
    time::sleep(Duration::from_secs(2)).await;

    camera.pan_tilt_absolute(Degrees(45.0), Degrees(15.0), 10.into()).await?;
    println!("Moved to 45° pan, 15° tilt");
    time::sleep(Duration::from_secs(3)).await;

    // Using absolute position with degrees (assuming ±170° pan, -30° to +90° tilt for PTZOpticsG2)
    camera.pan_tilt_absolute(Degrees(-85.0), Degrees(15.0), 10.into()).await?;
    println!("Moved to position (-85° pan, +15° tilt)");
    time::sleep(Duration::from_secs(3)).await;

    // Demonstrate relative movement
    println!("\n--- Relative Movement ---");
    // Use relative movement to move the camera
    camera.pan_tilt_relative(Degrees(10.0), Degrees(5.0), 10.into()).await?;
    println!("Moved camera relative: +10° pan, +5° tilt");
    time::sleep(Duration::from_secs(2)).await;

    // Return to default settings
    println!("\n--- Returning to Defaults ---");
    camera.exposure_auto().await?;
    camera.white_balance_auto().await?;
    // Note: Cannot reset image settings as they're not implemented
    camera.pan_tilt_home().await?;
    camera.zoom_absolute(Normalized(0.0)).await?; // Minimum zoom
    println!("Returned camera to default settings");

    println!("\n=== Demo Complete ===");
    Ok(())
}

#[cfg(not(feature = "tokio"))]
fn main() {
    eprintln!("This example requires the 'tokio' feature to be enabled.");
    eprintln!("Run with: cargo run --example enhanced_api_demo --features tokio");
}
