//! Example demonstrating the async high-level inquiry API for querying camera state.

use grafton_visca::{AsyncViscaClient, AsyncUdpTransport, ViscaError};
use std::env;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), ViscaError> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Get camera address from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <camera_ip:port>", args[0]);
        eprintln!("Example: {} 192.168.1.100:5678", args[0]);
        std::process::exit(1);
    }

    // Connect to camera
    let camera_addr = &args[1];
    println!("Connecting to camera at {}...", camera_addr);
    let transport = AsyncUdpTransport::new(camera_addr).await?;
    let client = AsyncViscaClient::new(Arc::new(transport));

    // Query individual camera settings
    println!("\n=== Individual Camera Queries (Async) ===");
    
    // Power state
    let power = client.get_power_state().await?;
    println!("Power: {}", if power { "ON" } else { "OFF" });

    if !power {
        println!("Camera is powered off. Some queries may not work.");
    }

    // Position
    let (pan, tilt) = client.get_pan_tilt_position().await?;
    println!("Pan/Tilt Position: pan={}, tilt={}", pan, tilt);

    // Zoom
    let zoom = client.get_zoom_position().await?;
    println!("Zoom Position: 0x{:04X}", zoom);

    // Focus
    let focus = client.get_focus_position().await?;
    println!("Focus Position: 0x{:04X}", focus);

    // Concurrent queries example
    println!("\n=== Concurrent Queries ===");
    println!("Querying multiple settings concurrently...");
    
    let exposure_future = client.get_exposure_mode();
    let wb_future = client.get_white_balance_mode();
    let luminance_future = client.get_luminance();
    let contrast_future = client.get_contrast();
    
    // Execute all queries concurrently (respecting VISCA's 2-socket limit)
    let (exposure_mode, wb_mode, luminance, contrast) = tokio::join!(
        exposure_future,
        wb_future,
        luminance_future,
        contrast_future
    );
    
    println!("Exposure Mode: {:?}", exposure_mode?);
    println!("White Balance Mode: {:?}", wb_mode?);
    println!("Luminance: {}", luminance?);
    println!("Contrast: {}", contrast?);

    // Get complete camera state
    println!("\n=== Complete Camera State (Async) ===");
    println!("Querying all camera settings...");
    let state = client.get_camera_state().await?;
    
    println!("\nCamera State Summary:");
    println!("  Power: {}", if state.power { "ON" } else { "OFF" });
    println!("  Position: pan={}, tilt={}", state.position.pan, state.position.tilt);
    println!("  Optics: zoom=0x{:04X}, focus=0x{:04X}", state.optics.zoom, state.optics.focus);
    println!("  Exposure: mode={:?}, compensation={:?}", 
        state.exposure.mode, 
        state.exposure.compensation
    );
    println!("  White Balance: {:?}", state.white_balance.mode);
    println!("  Image Quality:");
    println!("    - Luminance: {}", state.image.luminance);
    println!("    - Contrast: {}", state.image.contrast);
    println!("    - Sharpness: {}", state.image.sharpness);
    println!("    - Saturation: {}", state.image.saturation);
    println!("    - Hue: {}", state.image.hue);

    // Demonstrate continuous monitoring
    println!("\n=== Continuous Monitoring Example ===");
    println!("Monitoring zoom position for 5 seconds...");
    
    let start = tokio::time::Instant::now();
    let mut last_zoom = 0u16;
    
    while start.elapsed() < tokio::time::Duration::from_secs(5) {
        let current_zoom = client.get_zoom_position().await?;
        if current_zoom != last_zoom {
            println!("Zoom changed: 0x{:04X} -> 0x{:04X}", last_zoom, current_zoom);
            last_zoom = current_zoom;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    println!("\nAsync inquiry demo completed successfully!");
    Ok(())
}