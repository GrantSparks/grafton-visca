//! Simple demo showcasing the new unified ViscaClient API
//!
//! This example demonstrates basic camera control using the new v0.5.0 API.

use grafton_visca::{
    command::pan_tilt::PanTiltDirection, ViscaClient, ViscaError, ViscaPositionExt,
    ViscaTransportExt, ViscaZoomExt,
};
use std::time::Duration;

fn main() -> Result<(), ViscaError> {
    // Initialize logging
    env_logger::init();

    // Connect to camera using UDP
    let mut client = ViscaClient::connect_udp("192.168.1.100:1259")?;
    println!("Connected to camera via UDP");

    // Or connect using TCP
    // let mut client = ViscaClient::connect_tcp("192.168.1.100:5678")?;

    // Power on the camera
    client.power_on()?;
    println!("Camera powered on");

    // Wait for camera to initialize
    std::thread::sleep(Duration::from_secs(2));

    // Move to home position
    client.home()?;
    println!("Moved to home position");

    // Save current position as preset 1 using ViscaTransportExt
    <ViscaClient as ViscaTransportExt>::save_preset(&mut client, 1)?;
    println!("Saved preset 1");

    // Move camera using high-level API
    client.move_to_degrees(45.0, -15.0, Some((10, 10)))?;
    println!("Moved to 45° pan, -15° tilt");

    // Zoom in
    client.zoom_to_magnification(5.0)?;
    println!("Zoomed to 5x magnification");

    // Move camera continuously
    client.move_start(PanTiltDirection::Right, 10, 0)?;
    std::thread::sleep(Duration::from_secs(2));
    client.move_stop()?;
    println!("Continuous movement demo completed");

    // Return to preset 1 (home) using ViscaTransportExt
    <ViscaClient as ViscaTransportExt>::recall_preset(&mut client, 1)?;
    println!("Returned to preset 1");

    // Reset zoom
    client.zoom_to_magnification(1.0)?;
    println!("Reset zoom to 1x");

    println!("\nDemo completed successfully!");
    Ok(())
}
