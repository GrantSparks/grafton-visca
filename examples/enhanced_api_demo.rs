//! Enhanced API demonstration using the new Camera API.
//!
//! This example showcases the enhanced Camera API with high-level control methods
//! and demonstrates migration from the old Client API.

mod common;
use common::blocking::UdpTransport;

use grafton_visca::{
    camera::{
        profiles::PTZOpticsG2,
        units::{Degrees, Normalized},
        Camera,
    },
    command::{exposure::ExposureMode, white_balance::WhiteBalanceMode},
    transport::BlockingAdapter,
    Error,
};
use std::thread;
use std::time::Duration;

// Use a minimal tokio runtime for blocking execution
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(fut)
}

fn main() -> Result<(), Error> {
    env_logger::init();

    // Connect to camera using new Camera API with UDP transport
    let transport = UdpTransport::new("192.168.1.100:5678")?;
    let camera = Camera::<PTZOpticsG2>::new(BlockingAdapter(transport));

    println!("=== Enhanced Camera API Demo ===\n");

    // Power on the camera
    println!("Powering on camera...");
    block_on(camera.power_on())?;
    println!("Camera powered on");
    thread::sleep(Duration::from_secs(2));

    // Demonstrate exposure control
    println!("\n--- Exposure Control ---");
    block_on(camera.set_exposure_mode(ExposureMode::Auto))?;
    println!("Set exposure mode to Auto");

    block_on(camera.backlight_on())?;
    println!("Enabled backlight compensation");

    block_on(camera.set_brightness(0x08))?;
    println!("Set brightness to default level");

    // Demonstrate white balance control
    println!("\n--- White Balance Control ---");
    block_on(camera.set_white_balance_mode(WhiteBalanceMode::Outdoor))?;
    println!("Set white balance to outdoor preset");
    thread::sleep(Duration::from_secs(1));

    block_on(camera.set_white_balance_mode(WhiteBalanceMode::Indoor))?;
    println!("Set white balance to indoor preset");
    thread::sleep(Duration::from_secs(1));

    // Demonstrate image settings
    println!("\n--- Image Settings ---");
    // The new API doesn't have image presets, so we'll set individual parameters
    block_on(camera.set_saturation(10))?; // Higher saturation for "vivid"
    block_on(camera.set_contrast(9))?; // Higher contrast
    println!("Applied vivid image settings");
    thread::sleep(Duration::from_secs(1));

    block_on(camera.set_noise_reduction_2d(3))?;
    println!("Set 2D noise reduction to level 3");

    block_on(camera.set_image_flip(grafton_visca::command::image::ImageFlipMode::Off))?;
    println!("Disabled image flip");

    // Demonstrate zoom control
    println!("\n--- Zoom Control ---");
    block_on(camera.set_zoom(0x0000))?; // Minimum zoom
    println!("Set zoom to minimum (1x)");
    thread::sleep(Duration::from_secs(2));

    // For PTZOpticsG2, zoom range is 0x0000-0x4000 for 12x zoom
    // 5x would be approximately 0x1555
    block_on(camera.set_zoom(0x1555))?;
    println!("Set zoom to approximately 5x");
    thread::sleep(Duration::from_secs(2));

    // 25% of max zoom
    block_on(camera.set_zoom(0x1000))?;
    println!("Set zoom to 25% of maximum");
    thread::sleep(Duration::from_secs(2));

    // Demonstrate position control with degrees
    println!("\n--- Position Control ---");
    block_on(camera.home())?;
    println!("Moved to home position");
    thread::sleep(Duration::from_secs(2));

    block_on(camera.set_position(Degrees(45.0), Degrees(15.0)))?;
    println!("Moved to 45° pan, 15° tilt");
    thread::sleep(Duration::from_secs(3));

    block_on(camera.set_position_normalized(Normalized(-0.5), Normalized(0.25)))?;
    println!("Moved to normalized position (-50% pan, +25% tilt)");
    thread::sleep(Duration::from_secs(3));

    // Demonstrate relative movement
    println!("\n--- Relative Movement ---");
    // The new API doesn't have direct relative movement, but we can demonstrate
    // continuous movement instead
    block_on(camera.move_continuous(
        grafton_visca::command::pan_tilt::PanTiltDirection::UpRight,
        5,
        5,
    ))?;
    println!("Moving camera up-right...");
    thread::sleep(Duration::from_secs(1));
    block_on(camera.stop())?;
    println!("Stopped movement");

    // Return to default settings
    println!("\n--- Returning to Defaults ---");
    block_on(camera.set_exposure_mode(ExposureMode::Auto))?;
    block_on(camera.set_white_balance_mode(WhiteBalanceMode::Auto))?;
    // Reset image settings to defaults
    block_on(camera.set_saturation(7))?; // Default values
    block_on(camera.set_contrast(8))?;
    block_on(camera.set_sharpness(8))?;
    block_on(camera.set_brightness(8))?;
    block_on(camera.home())?;
    block_on(camera.set_zoom(0x0000))?; // Minimum zoom
    println!("Returned camera to default settings");

    println!("\n=== Demo Complete ===");
    Ok(())
}
