//! Example demonstrating the high-level inquiry API for querying camera state.

use grafton_visca::{UdpTransport, ViscaInquiryExt, ViscaError};
use std::env;

fn main() -> Result<(), ViscaError> {
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
    let mut transport = UdpTransport::new(camera_addr)?;

    // Query individual camera settings
    println!("\n=== Individual Camera Queries ===");
    
    // Power state
    let power = transport.get_power_state()?;
    println!("Power: {}", if power { "ON" } else { "OFF" });

    if !power {
        println!("Camera is powered off. Some queries may not work.");
    }

    // Position
    let (pan, tilt) = transport.get_pan_tilt_position()?;
    println!("Pan/Tilt Position: pan={}, tilt={}", pan, tilt);

    // Zoom
    let zoom = transport.get_zoom_position()?;
    println!("Zoom Position: 0x{:04X}", zoom);

    // Focus
    let focus = transport.get_focus_position()?;
    println!("Focus Position: 0x{:04X}", focus);

    // Exposure
    let exposure_mode = transport.get_exposure_mode()?;
    println!("Exposure Mode: {:?}", exposure_mode);

    if transport.get_exposure_compensation_enabled()? {
        let compensation = transport.get_exposure_compensation()?;
        println!("Exposure Compensation: {:+} EV", compensation);
    } else {
        println!("Exposure Compensation: Disabled");
    }

    // White Balance
    let wb_mode = transport.get_white_balance_mode()?;
    println!("White Balance Mode: {:?}", wb_mode);

    // Image Settings
    println!("\n=== Image Settings ===");
    let luminance = transport.get_luminance()?;
    println!("Luminance: {}", luminance);

    let contrast = transport.get_contrast()?;
    println!("Contrast: {}", contrast);

    let sharpness = transport.get_sharpness()?;
    println!("Sharpness: {}", sharpness);

    let saturation = transport.get_saturation()?;
    println!("Saturation: {}", saturation);

    let hue = transport.get_hue()?;
    println!("Hue: {}", hue);

    // Advanced Settings
    println!("\n=== Advanced Settings ===");
    let (vertical_flip, horizontal_flip) = transport.get_image_flip()?;
    println!("Image Flip: Vertical={}, Horizontal={}", vertical_flip, horizontal_flip);

    let backlight = transport.get_backlight_status()?;
    println!("Backlight Compensation: {}", if backlight { "ON" } else { "OFF" });

    let bw_mode = transport.get_black_white_mode()?;
    println!("Black & White Mode: {}", if bw_mode { "ON" } else { "OFF" });

    // Get complete camera state
    println!("\n=== Complete Camera State ===");
    println!("Querying all camera settings...");
    let state = transport.get_camera_state()?;
    
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

    println!("\nInquiry demo completed successfully!");
    Ok(())
}