//! Example demonstrating `Camera<P>` inquiry methods.
//!
//! This example shows how to:
//! - Query camera state using the type-safe Camera API
//! - Get position information in both degrees and VISCA units
//! - Retrieve various camera settings
//! - Use profile-aware unit conversions

use grafton_visca::{
    camera::{Camera, PTZOpticsG2},
    transport::create,
    Error,
};

// Include the transport implementation from the example file

#[cfg(not(feature = "async"))]
fn main() {
    eprintln!("This example requires the 'async' feature.");
    eprintln!("Run with: cargo run --example camera_inquiry_demo --features async");
}

#[cfg(feature = "async")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=== Camera Inquiry Demo ===\n");

    // Get camera address from command line or use default
    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    // Connect to camera
    println!("Connecting to camera at {}...", camera_addr);
    let transport = create::tcp(&camera_addr).await?;
    let camera = Camera::<PTZOpticsG2, _>::new(transport);

    // Query power state
    println!("\n--- Power State ---");
    match camera.get_power_state().await {
        Ok(is_on) => println!("Power: {}", if is_on { "ON" } else { "OFF" }),
        Err(e) => println!("Failed to get power state: {}", e),
    }

    // Query position in degrees
    println!("\n--- Position (Degrees) ---");
    match camera.get_position().await {
        Ok((pan, tilt)) => {
            println!("Pan: {:.1}°", pan.0);
            println!("Tilt: {:.1}°", tilt.0);
        }
        Err(e) => println!("Failed to get position: {}", e),
    }

    // Query position in VISCA units
    println!("\n--- Position (VISCA Units) ---");
    match camera.get_position_units().await {
        Ok((pan, tilt)) => {
            println!("Pan: {} units", pan.0);
            println!("Tilt: {} units", tilt.0);
        }
        Err(e) => println!("Failed to get position units: {}", e),
    }

    // Query zoom and focus
    println!("\n--- Optics ---");
    match camera.get_zoom_position().await {
        Ok(zoom) => println!("Zoom: 0x{:04X}", zoom),
        Err(e) => println!("Failed to get zoom: {}", e),
    }

    match camera.get_focus_position().await {
        Ok(focus) => println!("Focus: 0x{:04X}", focus),
        Err(e) => println!("Failed to get focus: {}", e),
    }

    // Query exposure settings
    println!("\n--- Exposure ---");
    match camera.get_exposure_mode().await {
        Ok(mode) => println!("Exposure Mode: {:?}", mode),
        Err(e) => println!("Failed to get exposure mode: {}", e),
    }

    if let Ok(true) = camera.get_exposure_compensation_enabled().await {
        match camera.get_exposure_compensation().await {
            Ok(comp) => println!("Exposure Compensation: {:+} EV", comp),
            Err(e) => println!("Failed to get exposure compensation: {}", e),
        }
    }

    // Query white balance
    println!("\n--- White Balance ---");
    match camera.get_white_balance_mode().await {
        Ok(mode) => println!("White Balance Mode: {:?}", mode),
        Err(e) => println!("Failed to get WB mode: {}", e),
    }

    // Query image quality settings
    println!("\n--- Image Quality ---");
    match camera.get_luminance().await {
        Ok(level) => println!("Luminance: {}", level),
        Err(e) => println!("Failed to get luminance: {}", e),
    }

    match camera.get_contrast().await {
        Ok(level) => println!("Contrast: {}", level),
        Err(e) => println!("Failed to get contrast: {}", e),
    }

    match camera.get_sharpness().await {
        Ok(level) => println!("Sharpness: {}", level),
        Err(e) => println!("Failed to get sharpness: {}", e),
    }

    match camera.get_saturation().await {
        Ok(level) => println!("Saturation: {}", level),
        Err(e) => println!("Failed to get saturation: {}", e),
    }

    match camera.get_hue().await {
        Ok(level) => println!("Hue: {}", level),
        Err(e) => println!("Failed to get hue: {}", e),
    }

    // Query complete camera state
    println!("\n--- Complete Camera State ---");
    println!("Querying all camera settings...");

    match camera.get_camera_state().await {
        Ok(state) => {
            println!("\nCamera State Summary:");
            println!("  Power: {}", if state.power { "ON" } else { "OFF" });
            println!(
                "  Position: {:.1}° pan, {:.1}° tilt",
                state.position.pan_degrees, state.position.tilt_degrees
            );
            println!(
                "  Optics: zoom=0x{:04X}, focus=0x{:04X}",
                state.optics.zoom, state.optics.focus
            );
            println!("  Exposure: mode={:?}", state.exposure.mode);
            println!("  White Balance: {:?}", state.white_balance.mode);
            println!("  Image Quality:");
            println!("    - Luminance: {}", state.image.luminance);
            println!("    - Contrast: {}", state.image.contrast);
            println!("    - Sharpness: {}", state.image.sharpness);
            println!("    - Saturation: {}", state.image.saturation);
            println!("    - Hue: {}", state.image.hue);
        }
        Err(e) => println!("Failed to get complete camera state: {}", e),
    }

    println!("\nInquiry demo completed!");
    Ok(())
}
