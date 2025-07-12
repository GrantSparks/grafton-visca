//! Example program

//! Example demonstrating VISCA inquiry commands.
//!
//! This example shows how to use the high-level inquiry methods
//! provided by the InquiryMethodsExt trait.

#[cfg(not(feature = "async"))]
use grafton_visca::{
    camera::methods::InquiryOps, profiles::PTZOpticsG2, transport::blocking::Tcp,
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
    let mut camera = Camera::<PTZOpticsG2, _>::new(transport);

    // Run inquiries using blocking methods
    run_inquiries(&mut camera)?;

    Ok(())
}

#[cfg(not(feature = "async"))]
fn run_inquiries<T: grafton_visca::transport::core::Transport>(
    camera: &mut Camera<PTZOpticsG2, T>,
) -> Result<(), Error> {
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

    // Position in VISCA units (available for all cameras)
    let (pan_units, tilt_units) = camera.get_position()?;
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

    let backlight = camera.get_backlight()?;
    println!(
        "Backlight Compensation: {}",
        if backlight { "ON" } else { "OFF" }
    );

    let bw_mode = camera.get_black_white()?;
    println!("Black & White Mode: {}", if bw_mode { "ON" } else { "OFF" });

    println!("\nInquiry demo completed successfully!");
    Ok(())
}

#[cfg(feature = "async")]
fn main() {
    eprintln!("This example requires the blocking mode (default) feature.");
    eprintln!("Run with: cargo run --example inquiry_demo --features blocking-client");
}
