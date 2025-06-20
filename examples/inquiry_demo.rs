//! Example program

//! Example demonstrating the high-level inquiry API for querying camera state.
//!
//! This example now uses the new `Camera<P>` API with full inquiry support!

use grafton_visca::{
    camera::{Camera, PTZOpticsG2},
    transport::blocking::create,
    Error,
};
use std::env;

#[cfg(not(not(feature = "async")))]
fn main() {
    eprintln!("This example requires the blocking mode (default) feature.");
    eprintln!("Run with: cargo run --example inquiry_demo --features blocking-client");
}

#[cfg(not(feature = "async"))]
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

    // Connect to camera using blocking transport
    let camera_addr = &args[1];
    println!("Connecting to camera at {camera_addr}...");
    let transport = create::tcp(camera_addr)?;
    let mut camera = Camera::<PTZOpticsG2>::new(transport);

    // Run inquiries using blocking methods
    run_inquiries(&mut camera)?;

    Ok(())
}

#[cfg(not(feature = "async"))]
fn run_inquiries(camera: &mut Camera<PTZOpticsG2>) -> Result<(), Error> {
    // Query individual camera settings
    println!("\n=== Individual Camera Queries ===");

    // Power state
    let power = camera.get_power_state()?;
    println!("Power: {}", if power { "ON" } else { "OFF" });

    if !power {
        println!("Camera is powered off. Some queries may not work.");
    }

    // Position in degrees
    let (pan_deg, tilt_deg) = camera.get_position()?;
    println!(
        "Position (degrees): pan={:.1}°, tilt={:.1}°",
        pan_deg.0, tilt_deg.0
    );

    // Position in VISCA units
    let (pan_units, tilt_units) = camera.get_position_units()?;
    println!(
        "Position (units): pan={}, tilt={}",
        pan_units.0, tilt_units.0
    );

    // Zoom
    let zoom = camera.get_zoom_position()?;
    println!("Zoom Position: 0x{zoom:04X}");

    // Focus
    let focus = camera.get_focus_position()?;
    println!("Focus Position: 0x{focus:04X}");

    // Exposure
    let exposure_mode = camera.get_exposure_mode()?;
    println!("Exposure Mode: {exposure_mode:?}");

    if camera.get_exposure_compensation_enabled()? {
        let compensation = camera.get_exposure_compensation()?;
        println!("Exposure Compensation: {compensation:+} EV");
    } else {
        println!("Exposure Compensation: Disabled");
    }

    // White Balance
    let wb_mode = camera.get_white_balance_mode()?;
    println!("White Balance Mode: {wb_mode:?}");

    // Image Settings
    println!("\n=== Image Settings ===");
    let luminance = camera.get_luminance()?;
    println!("Luminance: {luminance}");

    let contrast = camera.get_contrast()?;
    println!("Contrast: {contrast}");

    let sharpness = camera.get_sharpness()?;
    println!("Sharpness: {sharpness}");

    let saturation = camera.get_saturation()?;
    println!("Saturation: {saturation}");

    let hue = camera.get_hue()?;
    println!("Hue: {hue}");

    // Advanced Settings
    println!("\n=== Advanced Settings ===");
    let (vertical_flip, horizontal_flip) = camera.get_image_flip()?;
    println!("Image Flip: Vertical={vertical_flip}, Horizontal={horizontal_flip}");

    let backlight = camera.get_backlight_status()?;
    println!(
        "Backlight Compensation: {}",
        if backlight { "ON" } else { "OFF" }
    );

    let bw_mode = camera.get_black_white_mode()?;
    println!("Black & White Mode: {}", if bw_mode { "ON" } else { "OFF" });

    // Get complete camera state
    println!("\n=== Complete Camera State ===");
    println!("Querying all camera settings...");
    let state = camera.get_camera_state()?;

    println!("\nCamera State Summary:");
    println!("  Power: {}", if state.power { "ON" } else { "OFF" });
    println!(
        "  Position: pan={}, tilt={} (units)",
        state.position.pan, state.position.tilt
    );
    println!(
        "  Position: pan={:.1}°, tilt={:.1}° (degrees)",
        state.position.pan_degrees, state.position.tilt_degrees
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
