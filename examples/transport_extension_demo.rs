//! Demonstration of the extension traits with direct transport usage.
//!
//! This example shows how to use the high-level extension traits with
//! UDP or TCP transports directly.

use grafton_visca::{
    command::pan_tilt::{PanSpeed, TiltSpeed},
    ImagePreset, Client, Error, ViscaExposureExt, ViscaImageExt, ViscaPanTiltExt,
    ViscaPositionExt, ViscaPowerExt, ViscaTransportExt, ViscaWhiteBalanceExt, ViscaZoomExt,
    WhiteBalancePreset,
};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Error> {
    env_logger::init();

    // Connect to camera using UDP client
    let mut client = Client::connect_udp("192.168.1.100:5678")?;

    println!("=== Transport Extension Traits Demo ===\n");

    // Power control
    println!("--- Power Control ---");
    if !client.is_powered_on()? {
        println!("Camera is off, powering on...");
        client.power_on()?;
        client.wait_for_power_on(Duration::from_secs(5), Duration::from_millis(500))?;
    }

    // Exposure control
    println!("\n--- Exposure Control ---");
    ViscaExposureExt::set_iris(&mut client, 0x0C)?;
    println!("Set iris to F5.6");

    client.set_shutter(0x10)?;
    println!("Set shutter speed");

    // White balance
    println!("\n--- White Balance ---");
    client.set_white_balance_preset(WhiteBalancePreset::Daylight)?;
    println!("Set white balance to daylight");

    // Image settings
    println!("\n--- Image Settings ---");
    client.apply_image_preset(ImagePreset::Cinema)?;
    println!("Applied cinema image preset");

    // Zoom control
    println!("\n--- Zoom Control ---");
    ViscaZoomExt::zoom_to_magnification(&mut client, 2.0)?;
    println!("Set zoom to 2x");
    thread::sleep(Duration::from_secs(2));

    let mag = ViscaZoomExt::get_zoom_magnification(&mut client)?;
    println!("Current zoom: {mag:.1}x");

    // Position control
    println!("\n--- Position Control ---");
    ViscaPositionExt::move_to_degrees(
        &mut client,
        30.0,
        10.0,
        Some((PanSpeed::new(10)?, TiltSpeed::new(10)?)),
    )?;
    println!("Moved to 30° pan, 10° tilt");
    thread::sleep(Duration::from_secs(2));

    let pos = ViscaPositionExt::get_position_degrees(&mut client)?;
    println!(
        "Current position: Pan={:.1}°, Tilt={:.1}°",
        pos.pan, pos.tilt
    );

    // Return to defaults
    println!("\n--- Returning to Defaults ---");
    ViscaPanTiltExt::move_to_position(&mut client, 0, 0, None)?;
    ViscaZoomExt::zoom_to_magnification(&mut client, 1.0)?;
    println!("Returned to home position");

    println!("\n=== Demo Complete ===");
    Ok(())
}
