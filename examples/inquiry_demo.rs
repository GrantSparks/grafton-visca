//! Example program

//! Example demonstrating VISCA inquiry commands.
//!
//! This example shows how to use the high-level inquiry methods
//! provided by the InquiryMethodsExt trait.

#[cfg(not(feature = "async"))]
use grafton_visca::{prelude::blocking::*, transport::TcpTransportBlocking, Error};
#[cfg(not(feature = "async"))]
use std::env;

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Error> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Get camera address from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        let prog_name = &args[0];
        eprintln!("Usage: {prog_name} <camera_ip:port>");
        eprintln!("Example: {prog_name} 192.168.1.100:5678");
        std::process::exit(1);
    }

    // Connect to camera using blocking transport
    let camera_addr = &args[1];
    println!("Connecting to camera at {camera_addr}...");
    let transport = TcpTransportBlocking::connect(camera_addr)?;
    let mut camera = GenericViscaCam::new_blocking(transport);

    // Run inquiries using blocking methods
    run_inquiries(&mut camera)?;

    Ok(())
}

#[cfg(not(feature = "async"))]
fn run_inquiries<P, T>(camera: &mut grafton_visca::Camera<P, T>) -> Result<(), Error>
where
    P: grafton_visca::capabilities::Profile,
    T: grafton_visca::transport::UnifiedTransport,
{
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
    let power_state = if power_on { "ON" } else { "OFF" };
    println!("   Power is: {power_state}");

    if !power_on {
        println!("Camera is powered off. Some queries may not work.");
    }

    // Position
    let (pan_pos, tilt_pos) = camera.get_pan_tilt_position()?;
    println!("Position: pan=0x{pan_pos:04X}, tilt=0x{tilt_pos:04X}");

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
    // Note: Luminance inquiry not yet implemented
    let luminance = 0;
    println!("Luminance: {luminance}");

    // Note: Contrast inquiry not yet implemented
    let contrast = 0;
    println!("Contrast: {contrast}");

    // Note: Sharpness inquiry not yet implemented
    let sharpness = 0;
    println!("Sharpness: {sharpness}");

    // Note: Saturation inquiry not yet implemented
    let saturation = 0;
    println!("Saturation: {saturation}");

    // Note: Hue inquiry not yet implemented
    let hue = 0;
    println!("Hue: {hue}");

    // Advanced Settings
    println!("\n=== Advanced Settings ===");
    // Note: Image flip inquiry not implemented in current API

    // Note: Backlight inquiry not yet implemented
    let backlight = false;
    println!(
        "Backlight Compensation: {}",
        if backlight { "ON" } else { "OFF" }
    );

    // Note: B&W mode inquiry not yet implemented
    let bw_mode = false;
    let bw_state = if bw_mode { "ON" } else { "OFF" };
    println!("Black & White Mode: {bw_state}");

    println!("\nInquiry demo completed successfully!");
    Ok(())
}

#[cfg(feature = "async")]
fn main() {
    eprintln!("This example requires the blocking mode (default) feature.");
    eprintln!("Run with: cargo run --example inquiry_demo --features blocking-client");
}
