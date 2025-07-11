//! Quick start demo showcasing the Camera API
//!
//! This example demonstrates basic camera control using the new Camera API.

#[cfg(not(feature = "async"))]
use grafton_visca::transport::blocking::{TcpGat, UdpGat};
#[cfg(not(feature = "async"))]
use grafton_visca::{
    camera::{
        methods::{PanTiltBlockingExt, PowerBlockingExt, PresetBlockingExt, ZoomBlockingExt},
        profiles::{G2PresetId, PTZOpticsG2},
    },
    command::pan_tilt::PanTiltDirection,
    types::{PanSpeed, TiltSpeed, ZoomPosition},
    units::Degrees,
    CameraBlocking, Error,
};
#[cfg(not(feature = "async"))]
use std::time::Duration;

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Error> {
    // Initialize logging
    env_logger::init();

    // Connect to camera using UDP
    let udp_transport = UdpGat::connect("192.168.1.100:1259")?;
    let mut camera = CameraBlocking::<PTZOpticsG2, _>::new(udp_transport);
    println!("Connected to camera via UDP");

    // Or connect using TCP
    // let tcp_transport = TcpGat::connect("192.168.1.100:5678")?;
    // let mut camera = CameraBlocking::<PTZOpticsG2, _>::new(tcp_transport);

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
    let preset1 = G2PresetId::new(1)?;
    camera.preset_set(preset1.into())?;
    println!("Saved preset 1");

    // Move camera to specific position
    camera.pan_tilt_absolute(45.0, -15.0, 5)?;
    println!("Moved to 45° pan, -15° tilt");

    // Zoom control
    println!("Zooming in...");
    camera.zoom_in()?;
    std::thread::sleep(Duration::from_secs(2));
    camera.zoom_stop()?;

    // Set specific zoom position (50% of max)
    camera.set_zoom(ZoomPosition::try_from(0.5f32)?)?;
    println!("Set zoom to 50%");

    // Move camera continuously
    println!("Starting continuous movement...");
    camera.pan_tilt_move(
        PanTiltDirection::Right,
        PanSpeed::new(10)?,
        TiltSpeed::new(0)?,
    )?;
    std::thread::sleep(Duration::from_secs(2));
    camera.pan_tilt_stop()?;
    println!("Continuous movement demo completed");

    // Return to preset 1 (home)
    camera.preset_recall(preset1.into())?;
    println!("Returned to preset 1");

    // Reset zoom
    camera.set_zoom(ZoomPosition::MIN)?;
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
