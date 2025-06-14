//! Example program

//! Example demonstrating the high-level inquiry API for querying camera state.

use grafton_visca::{Client, Error, InquiryExt};
use std::env;

fn main() -> Result<(), Error> {
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
    println!("Connecting to camera at {camera_addr}...");
    let mut client = Client::connect_udp(camera_addr)?;

    // Query individual camera settings
    println!("\n=== Individual Camera Queries ===");

    // Power state
    let power = client.get_power_state()?;
    println!("Power: {}", if power { "ON" } else { "OFF" });

    if !power {
        println!("Camera is powered off. Some queries may not work.");
    }

    // Position
    let (pan, tilt) = client.get_pan_tilt_position()?;
    println!("Pan/Tilt Position: pan={pan}, tilt={tilt}");

    // Zoom
    let zoom = client.get_zoom_position()?;
    println!("Zoom Position: 0x{zoom:04X}");

    // Focus
    let focus = client.get_focus_position()?;
    println!("Focus Position: 0x{focus:04X}");

    // Exposure
    let exposure_mode = client.get_exposure_mode()?;
    println!("Exposure Mode: {exposure_mode:?}");

    if client.get_exposure_compensation_enabled()? {
        let compensation = client.get_exposure_compensation()?;
        println!("Exposure Compensation: {compensation:+} EV");
    } else {
        println!("Exposure Compensation: Disabled");
    }

    // White Balance
    let wb_mode = client.get_white_balance_mode()?;
    println!("White Balance Mode: {wb_mode:?}");

    // Image Settings
    println!("\n=== Image Settings ===");
    let luminance = client.get_luminance()?;
    println!("Luminance: {luminance}");

    let contrast = client.get_contrast()?;
    println!("Contrast: {contrast}");

    let sharpness = client.get_sharpness()?;
    println!("Sharpness: {sharpness}");

    let saturation = client.get_saturation()?;
    println!("Saturation: {saturation}");

    let hue = client.get_hue()?;
    println!("Hue: {hue}");

    // Advanced Settings
    println!("\n=== Advanced Settings ===");
    let (vertical_flip, horizontal_flip) = client.get_image_flip()?;
    println!("Image Flip: Vertical={vertical_flip}, Horizontal={horizontal_flip}");

    let backlight = client.get_backlight_status()?;
    println!(
        "Backlight Compensation: {}",
        if backlight { "ON" } else { "OFF" }
    );

    let bw_mode = client.get_black_white_mode()?;
    println!("Black & White Mode: {}", if bw_mode { "ON" } else { "OFF" });

    // Get complete camera state
    println!("\n=== Complete Camera State ===");
    println!("Querying all camera settings...");
    let state = client.get_camera_state()?;

    println!("\nCamera State Summary:");
    println!("  Power: {}", if state.power { "ON" } else { "OFF" });
    println!(
        "  Position: pan={}, tilt={}",
        state.position.pan, state.position.tilt
    );
    println!(
        "  Optics: zoom=0x{:04X}, focus=0x{:04X}",
        state.optics.zoom, state.optics.focus
    );
    println!(
        "  Exposure: mode={:?}, compensation={:?}",
        state.exposure.mode, state.exposure.compensation
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
