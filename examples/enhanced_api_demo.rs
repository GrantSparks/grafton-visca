//! Example program

//! Enhanced API demonstration using the new v0.4.0 extension traits.
//!
//! This example showcases the enhanced API with high-level control methods.

use grafton_visca::{
    command::pan_tilt::{PanSpeed, TiltSpeed},
    Client, Error, ExposureExt, ImageExt, ImagePreset, PanTiltExt, PositionExt, PowerExt,
    WhiteBalanceExt, WhiteBalancePreset, ZoomExt,
};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Error> {
    env_logger::init();

    // Connect to camera using UDP client
    let mut client = Client::connect_udp("192.168.1.100:5678")?;

    println!("=== Enhanced API Demo ===\n");

    // Check power status and ensure camera is on
    println!("Checking camera power status...");
    let was_already_on = client.ensure_powered_on()?;
    if !was_already_on {
        println!("Camera was powered off, now powered on");
        client.wait_for_power_on(Duration::from_secs(5), Duration::from_millis(500))?;
    } else {
        println!("Camera is already powered on");
    }

    // Demonstrate exposure control
    println!("\n--- Exposure Control ---");
    ExposureExt::set_exposure_mode(
        &mut client,
        grafton_visca::command::exposure::ExposureMode::Auto,
    )?;
    println!("Set exposure mode to Auto");

    ExposureExt::set_backlight(&mut client, true)?;
    println!("Enabled backlight compensation");

    client.set_brightness(0x08)?;
    println!("Set brightness to default level");

    // Demonstrate white balance control
    println!("\n--- White Balance Control ---");
    client.set_white_balance_preset(WhiteBalancePreset::Daylight)?;
    println!("Set white balance to daylight preset");
    thread::sleep(Duration::from_secs(1));

    client.set_white_balance_preset(WhiteBalancePreset::Tungsten)?;
    println!("Set white balance to tungsten preset");
    thread::sleep(Duration::from_secs(1));

    // Demonstrate image settings
    println!("\n--- Image Settings ---");
    client.apply_image_preset(ImagePreset::Vivid)?;
    println!("Applied vivid image preset");
    thread::sleep(Duration::from_secs(1));

    client.set_noise_reduction_2d(Some(3))?;
    println!("Set 2D noise reduction to level 3");

    client.set_image_flip(false, false)?;
    println!("Disabled image flip");

    // Demonstrate zoom control with magnification
    println!("\n--- Zoom Control ---");
    client.zoom_to_magnification(1.0)?;
    println!("Set zoom to 1x (minimum zoom)");
    thread::sleep(Duration::from_secs(2));

    client.zoom_to_magnification(5.0)?;
    println!("Set zoom to 5x");
    thread::sleep(Duration::from_secs(2));

    client.zoom_to_normalized(0.25)?;
    println!("Set zoom to 25% (normalized)");
    thread::sleep(Duration::from_secs(2));

    let mag = client.get_zoom_magnification()?;
    println!("Current zoom magnification: {mag:.1}x");

    // Demonstrate position control with degrees
    println!("\n--- Position Control ---");
    client.move_to_position(0, 0, None)?;
    println!("Moved to center position");
    thread::sleep(Duration::from_secs(2));

    client.move_to_degrees(45.0, 15.0, Some((PanSpeed::new(10)?, TiltSpeed::new(10)?)))?;
    println!("Moved to 45° pan, 15° tilt");
    thread::sleep(Duration::from_secs(2));

    client.move_to_normalized(-0.5, 0.25, None)?;
    println!("Moved to normalized position (-50% pan, +25% tilt)");
    thread::sleep(Duration::from_secs(2));

    let pos = client.get_position_degrees()?;
    println!(
        "Current position: Pan={:.1}°, Tilt={:.1}°",
        pos.pan, pos.tilt
    );

    // Demonstrate relative movement
    println!("\n--- Relative Movement ---");
    PositionExt::move_by_degrees(
        &mut client,
        10.0,
        -5.0,
        Some((PanSpeed::new(5)?, TiltSpeed::new(5)?)),
    )?;
    println!("Moved 10° right and 5° down from current position");
    thread::sleep(Duration::from_secs(1));

    // Return to default settings
    println!("\n--- Returning to Defaults ---");
    ExposureExt::set_exposure_mode(
        &mut client,
        grafton_visca::command::exposure::ExposureMode::Auto,
    )?;
    client.set_white_balance_preset(WhiteBalancePreset::Auto)?;
    client.apply_image_preset(ImagePreset::Default)?;
    PanTiltExt::move_to_position(&mut client, 0, 0, None)?;
    ZoomExt::zoom_to_magnification(&mut client, 1.0)?;
    println!("Returned camera to default settings");

    println!("\n=== Demo Complete ===");
    Ok(())
}
