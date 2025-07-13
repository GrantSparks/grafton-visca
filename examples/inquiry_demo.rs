//! Example program

//! Example demonstrating VISCA inquiry commands.
//!
//! This example shows how to use the high-level inquiry methods
//! provided by the InquiryMethodsExt trait.

#[cfg(not(feature = "async"))]
use grafton_visca::{
    blocking::{Camera, InquiryOps, PanTiltInquiryOps},
    transport::blocking::Tcp,
    Error,
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
    let camera = grafton_visca::Camera::new(transport).blocking();

    // Run inquiries using blocking methods
    run_inquiries(&mut camera)?;

    Ok(())
}

#[cfg(not(feature = "async"))]
fn run_inquiries(camera: &mut grafton_visca::blocking::Camera) -> Result<(), Error> {
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
    let power_on = camera.get_power_state()?;
    println!("   Power is: {}", if power_on { "ON" } else { "OFF" });

    if !power_on {
        println!("Camera is powered off. Some queries may not work.");
    }

    // Position in degrees
    let (pan_deg, tilt_deg) = camera.get_$1()?;
    println!(
        "Position (degrees): pan={:.1}°, tilt={:.1}°",
        pan_deg.0, tilt_deg.0
    );

    // Zoom
    let zoom = camera.get_$1()?;
    println!("Zoom Position: 0x{zoom:04X}");

    // Focus
    let focus = camera.get_$1()?;
    println!("Focus Position: 0x{focus:04X}");

    // Exposure
    let exposure_mode = camera.get_$1()?;
    println!("Exposure Mode: {exposure_mode:?}");

    if camera.get_$1()? {
        let compensation = camera.get_$1()?;
        println!("Exposure Compensation: {compensation:+} EV");
    } else {
        println!("Exposure Compensation: Disabled");
    }

    // White Balance
    let wb_mode = camera.get_$1()?;
    println!("White Balance Mode: {wb_mode:?}");

    // Image Settings
    println!("\n=== Image Settings ===");
    let luminance = camera.get_$1()?;
    println!("Luminance: {luminance}");

    let contrast = camera.get_$1()?;
    println!("Contrast: {contrast}");

    let sharpness = camera.get_$1()?;
    println!("Sharpness: {sharpness}");

    let saturation = camera.get_$1()?;
    println!("Saturation: {saturation}");

    let hue = camera.get_$1()?;
    println!("Hue: {hue}");

    // Advanced Settings
    println!("\n=== Advanced Settings ===");
    // Note: Image flip inquiry not implemented in current API

    let backlight = camera.get_$1()?;
    println!(
        "Backlight Compensation: {}",
        if backlight { "ON" } else { "OFF" }
    );

    let bw_mode = camera.get_$1()?;
    println!("Black & White Mode: {}", if bw_mode { "ON" } else { "OFF" });

    println!("\nInquiry demo completed successfully!");
    Ok(())
}

#[cfg(feature = "async")]
fn main() {
    eprintln!("This example requires the blocking mode (default) feature.");
    eprintln!("Run with: cargo run --example inquiry_demo --features blocking-client");
}
