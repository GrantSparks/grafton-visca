//! Example program

//! Example demonstrating VISCA inquiry commands.
//!
//! This example shows how to use the high-level inquiry methods
//! provided by the InquiryMethodsExt trait.

#[cfg(not(feature = "async"))]
use grafton_visca::{
    camera::methods::{InquiryOps, PanTiltInquiryOps},
    transport::blocking::Tcp,
    Camera, Error,
};
#[cfg(not(feature = "async"))]
use std::env;

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
    let transport = Tcp::connect(camera_addr)?;
    let mut camera = Camera::new_blocking(transport);

    // Run inquiries using blocking methods
    run_inquiries(&mut camera)?;

    Ok(())
}

#[cfg(not(feature = "async"))]
fn run_inquiries(camera: &mut Camera) -> Result<(), Error> {
    println!(
        "
=== VISCA Inquiry Command Demo ==="
    );
    println!("This demonstrates using the high-level inquiry methods.");
    println!(
        "All commands are sent and parsed automatically.
"
    );

    // Example 1: Query power state
    println!("1. Querying power state...");
    let power_on = camera.get_power_state_blocking()?;
    println!("   Power is: {}", if power_on { "ON" } else { "OFF" });

    if !power_on {
        println!("Camera is powered off. Some queries may not work.");
    }

    // Position in degrees
    let (pan_deg, tilt_deg) = camera.get_position_degrees_blocking()?;
    println!(
        "Position (degrees): pan={:.1}°, tilt={:.1}°",
        pan_deg.0, tilt_deg.0
    );

    // Zoom
    let zoom = camera.get_zoom_position_blocking()?;
    println!("Zoom Position: 0x{zoom:04X}");

    // Focus
    let focus = camera.get_focus_position_blocking()?;
    println!("Focus Position: 0x{focus:04X}");

    // Exposure
    let exposure_mode = camera.get_exposure_mode_blocking()?;
    println!("Exposure Mode: {exposure_mode:?}");

    if camera.get_exposure_compensation_enabled_blocking()? {
        let compensation = camera.get_exposure_compensation_blocking()?;
        println!("Exposure Compensation: {compensation:+} EV");
    } else {
        println!("Exposure Compensation: Disabled");
    }

    // White Balance
    let wb_mode = camera.get_white_balance_mode_blocking()?;
    println!("White Balance Mode: {wb_mode:?}");

    // Image Settings
    println!("\n=== Image Settings ===");
    let luminance = camera.get_luminance_blocking()?;
    println!("Luminance: {luminance}");

    let contrast = camera.get_contrast_blocking()?;
    println!("Contrast: {contrast}");

    let sharpness = camera.get_sharpness_blocking()?;
    println!("Sharpness: {sharpness}");

    let saturation = camera.get_saturation_blocking()?;
    println!("Saturation: {saturation}");

    let hue = camera.get_hue_blocking()?;
    println!("Hue: {hue}");

    // Advanced Settings
    println!("\n=== Advanced Settings ===");
    // Note: Image flip inquiry not implemented in current API

    let backlight = camera.get_backlight_blocking()?;
    println!(
        "Backlight Compensation: {}",
        if backlight { "ON" } else { "OFF" }
    );

    let bw_mode = camera.get_black_white_blocking()?;
    println!("Black & White Mode: {}", if bw_mode { "ON" } else { "OFF" });

    println!("\nInquiry demo completed successfully!");
    Ok(())
}

#[cfg(feature = "async")]
fn main() {
    eprintln!("This example requires the blocking mode (default) feature.");
    eprintln!("Run with: cargo run --example inquiry_demo --features blocking-client");
}
