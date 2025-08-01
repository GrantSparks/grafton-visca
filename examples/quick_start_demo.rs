//! Quick start demo showcasing the Camera API
//!
//! This example demonstrates basic camera control using the new Camera API.

#[cfg(not(feature = "async"))]
use grafton_visca::transport::UdpTransportBlocking;
#[cfg(not(feature = "async"))]
use grafton_visca::{
    prelude::blocking::*,
    types::{PanSpeed, SpeedLevel, TiltSpeed},
    units::{Degrees, Normalized},
    Error, PanTiltDirection, PresetNumber,
};
#[cfg(not(feature = "async"))]
use std::time::Duration;

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Error> {
    // Initialize logging
    env_logger::init();

    // Connect to camera using UDP
    let udp_transport = UdpTransportBlocking::connect("0.0.0.0:0", "192.168.1.100:1259")?;
    let camera = PTZOpticsG2Cam::new_blocking(udp_transport);
    println!("Connected to camera via UDP");

    // Or connect using TCP
    // let tcp_transport = TcpTransport::connect("192.168.1.100:5678")?;
    // let mut camera = PTZOpticsG2Cam::new(tcp_transport);

    // Camera capabilities are now checked at compile time through the PTZOpticsG2 profile
    println!("\nUsing PTZOpticsG2 camera profile");
    println!("  Pan Range: -170 to +170 degrees");
    println!("  Tilt Range: -30 to +90 degrees");
    println!("  Max optical zoom: 20x");
    println!();

    // Power on the camera
    camera.power_on()?;
    println!("Camera powered on");

    // Wait for camera to initialize
    std::thread::sleep(Duration::from_secs(2));

    // Move to home position
    camera.pan_tilt_home()?;
    println!("Moved to home position");

    // Save current position as preset 1
    // Use PresetNumber directly instead of G2PresetId
    camera.preset_set(PresetNumber::new(1)?)?;
    println!("Saved preset 1");

    // Move camera to specific position
    camera.pan_tilt_absolute(Degrees::new(45.0), Degrees::new(-15.0), SpeedLevel::from(5))?;
    println!("Moved to 45° pan, -15° tilt");

    // Zoom control
    println!("Zooming in...");
    camera.zoom_in()?;
    std::thread::sleep(Duration::from_secs(2));
    camera.zoom_stop()?;

    // Set specific zoom position (50% of max)
    camera.zoom_absolute(Normalized::new(0.5))?;
    println!("Set zoom to 50%");

    // Move camera continuously
    println!("Starting continuous movement...");
    camera.pan_tilt_move(
        PanTiltDirection::Right,
        PanSpeed::try_from(10)?,
        TiltSpeed::try_from(0)?,
    )?;
    std::thread::sleep(Duration::from_secs(2));
    camera.pan_tilt_stop()?;
    println!("Continuous movement demo completed");

    // Return to preset 1 (home)
    camera.preset_recall(PresetNumber::new(1)?)?;
    println!("Returned to preset 1");

    // Reset zoom
    camera.zoom_absolute(Normalized::new(0.0))?;
    println!("Reset zoom to minimum");

    println!("\nDemo completed successfully!");
    Ok(())
}

#[cfg(feature = "async")]
fn main() {
    eprintln!("This example is designed for blocking mode only.");
    eprintln!("To run in blocking mode, disable async features:");
    eprintln!("  cargo run --example quick_start_demo --no-default-features");
}
