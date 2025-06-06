//! Demonstration of the extension traits with direct transport usage.
//!
//! This example shows how to use the high-level extension traits with
//! UDP or TCP transports directly.

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

    println!("=== Transport Extension Traits Demo ===\n");

    // Power control
    println!("--- Power Control ---");
    if !transport.is_powered_on()? {
        println!("Camera is off, powering on...");
        transport.power_on()?;
        transport.wait_for_power_on(Duration::from_secs(5), Duration::from_millis(500))?;
    }

    // Exposure control
    println!("\n--- Exposure Control ---");
    ViscaExposureExt::set_iris(&mut transport, 0x0C)?;
    println!("Set iris to F5.6");

    transport.set_shutter_speed(0x10)?;
    println!("Set shutter speed");

    // White balance
    println!("\n--- White Balance ---");
    transport.set_white_balance_preset(WhiteBalancePreset::Daylight)?;
    println!("Set white balance to daylight");

    // Image settings
    println!("\n--- Image Settings ---");
    transport.apply_image_preset(ImagePreset::Cinema)?;
    println!("Applied cinema image preset");

    // Zoom control
    println!("\n--- Zoom Control ---");
    transport.zoom_to_magnification(2.0)?;
    println!("Set zoom to 2x");
    thread::sleep(Duration::from_secs(2));

    let mag = transport.get_zoom_magnification()?;
    println!("Current zoom: {:.1}x", mag);

    // Position control
    println!("\n--- Position Control ---");
    transport.move_to_degrees(30.0, 10.0, Some((10, 10)))?;
    println!("Moved to 30° pan, 10° tilt");
    thread::sleep(Duration::from_secs(2));

    let pos = transport.get_position_degrees()?;
    println!(
        "Current position: Pan={:.1}°, Tilt={:.1}°",
        pos.pan, pos.tilt
    );

    // Return to defaults
    println!("\n--- Returning to Defaults ---");
    transport.move_to_position(0, 0, None)?;
    transport.zoom_to_magnification(1.0)?;
    println!("Returned to home position");

    println!("\n=== Demo Complete ===");
    Ok(())
}
