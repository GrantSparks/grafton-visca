//! Enhanced API demonstration using the new v0.4.0 extension traits.
//!
//! This example showcases the enhanced API with high-level control methods.

use grafton_visca::{
    ImagePreset, UdpTransport, ViscaError, ViscaExposureExt, ViscaImageExt, ViscaPanTiltExt,
    ViscaPositionExt, ViscaPowerExt, ViscaTransportExt, ViscaWhiteBalanceExt, ViscaZoomExt,
    WhiteBalancePreset,
};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), ViscaError> {
    env_logger::init();

    // Connect to camera using UDP transport
    let mut transport = UdpTransport::new("192.168.1.100:5678")?;

    println!("=== Enhanced API Demo ===\n");

    // Check power status and ensure camera is on
    println!("Checking camera power status...");
    if !transport.is_powered_on()? {
        println!("Camera is off, powering on...");
        transport.power_on()?;
        transport.wait_for_power_on(Duration::from_secs(5), Duration::from_millis(500))?;
        println!("Camera powered on!");
    } else {
        println!("Camera is already powered on");
    }

    // Demonstrate exposure control
    println!("\n--- Exposure Control ---");
    ViscaExposureExt::set_exposure_mode(
        &mut transport,
        grafton_visca::command::exposure::ExposureMode::Auto,
    )?;
    println!("Set exposure mode to Auto");

    transport.set_backlight_compensation(true)?;
    println!("Enabled backlight compensation");

    transport.set_brightness(0x08)?;
    println!("Set brightness to default level");

    // Demonstrate white balance control
    println!("\n--- White Balance Control ---");
    transport.set_white_balance_preset(WhiteBalancePreset::Daylight)?;
    println!("Set white balance to daylight preset");
    thread::sleep(Duration::from_secs(1));

    transport.set_white_balance_preset(WhiteBalancePreset::Tungsten)?;
    println!("Set white balance to tungsten preset");
    thread::sleep(Duration::from_secs(1));

    // Demonstrate image settings
    println!("\n--- Image Settings ---");
    transport.apply_image_preset(ImagePreset::Vivid)?;
    println!("Applied vivid image preset");
    thread::sleep(Duration::from_secs(1));

    transport.set_noise_reduction_2d(Some(3))?;
    println!("Set 2D noise reduction to level 3");

    transport.set_image_flip(false, false)?;
    println!("Disabled image flip");

    // Demonstrate zoom control with magnification
    println!("\n--- Zoom Control ---");
    transport.zoom_to_magnification(1.0)?;
    println!("Set zoom to 1x (wide)");
    thread::sleep(Duration::from_secs(2));

    transport.zoom_to_magnification(5.0)?;
    println!("Set zoom to 5x");
    thread::sleep(Duration::from_secs(2));

    transport.zoom_to_normalized(0.25)?;
    println!("Set zoom to 25% (normalized)");
    thread::sleep(Duration::from_secs(2));

    let mag = transport.get_zoom_magnification()?;
    println!("Current zoom magnification: {:.1}x", mag);

    // Demonstrate position control with degrees
    println!("\n--- Position Control ---");
    transport.move_to_position(0, 0, None)?;
    println!("Moved to center position");
    thread::sleep(Duration::from_secs(2));

    transport.move_to_degrees(45.0, 15.0, Some((10, 10)))?;
    println!("Moved to 45° pan, 15° tilt");
    thread::sleep(Duration::from_secs(2));

    transport.move_to_normalized(-0.5, 0.25, None)?;
    println!("Moved to normalized position (-50% pan, +25% tilt)");
    thread::sleep(Duration::from_secs(2));

    let pos = transport.get_position_degrees()?;
    println!(
        "Current position: Pan={:.1}°, Tilt={:.1}°",
        pos.pan, pos.tilt
    );

    // Demonstrate relative movement
    println!("\n--- Relative Movement ---");
    transport.move_by_degrees(10.0, -5.0, Some((5, 5)))?;
    println!("Moved 10° right and 5° down from current position");
    thread::sleep(Duration::from_secs(1));

    // Return to default settings
    println!("\n--- Returning to Defaults ---");
    ViscaExposureExt::set_exposure_mode(
        &mut transport,
        grafton_visca::command::exposure::ExposureMode::Auto,
    )?;
    transport.set_white_balance_preset(WhiteBalancePreset::Auto)?;
    transport.apply_image_preset(ImagePreset::Default)?;
    transport.move_to_position(0, 0, None)?;
    transport.zoom_to_magnification(1.0)?;
    println!("Returned camera to default settings");

    println!("\n=== Demo Complete ===");
    Ok(())
}
