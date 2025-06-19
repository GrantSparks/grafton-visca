//! Quick start demo showcasing the Camera API
//!
//! This example demonstrates basic camera control using the new Camera API.

#[cfg(feature = "blocking-client")]
mod common;

#[cfg(feature = "blocking-client")]
use grafton_visca::{
    camera::{
        profiles::{G2PresetId, PTZOpticsG2},
        units::Degrees,
        Camera,
    },
    command::pan_tilt::PanTiltDirection,
    Error,
};
#[cfg(feature = "blocking-client")]
use std::time::Duration;

#[cfg(feature = "blocking-client")]
// Use a minimal tokio runtime for blocking execution
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(fut)
}

#[cfg(feature = "blocking-client")]
fn main() -> Result<(), Error> {
    // Initialize logging
    env_logger::init();

    // Connect to camera using UDP (async transport with block_on)
    let udp_transport = block_on(common::r#async::udp_transport("192.168.1.100:1259"))?;
    let camera = Camera::<PTZOpticsG2>::new(udp_transport);
    println!("Connected to camera via UDP");

    // Or connect using TCP
    // let tcp_transport = block_on(common::r#async::tcp_transport("192.168.1.100:5678"))?;
    // let mut camera = Camera::<PTZOpticsG2>::new(tcp_transport);

    // Display camera capabilities
    let caps = camera.capabilities();
    println!("\nCamera Capabilities:");
    println!("  Model: {}", caps.model_name);
    println!("  Pan Range: {:?} degrees", caps.pan_range_degrees);
    println!("  Tilt Range: {:?} degrees", caps.tilt_range_degrees);
    println!("  Max Pan Speed: {}", caps.max_pan_speed);
    println!("  Max Tilt Speed: {}", caps.max_tilt_speed);
    println!();

    // Power on the camera
    block_on(camera.power_on())?;
    println!("Camera powered on");

    // Wait for camera to initialize
    std::thread::sleep(Duration::from_secs(2));

    // Move to home position
    block_on(camera.home())?;
    println!("Moved to home position");

    // Save current position as preset 1
    let preset1 = G2PresetId::new(1)?;
    block_on(camera.set_preset(preset1))?;
    println!("Saved preset 1");

    // Move camera to specific position
    block_on(camera.set_position(Degrees(45.0), Degrees(-15.0)))?;
    println!("Moved to 45° pan, -15° tilt");

    // Zoom control
    println!("Zooming in...");
    block_on(camera.zoom_in())?;
    std::thread::sleep(Duration::from_secs(2));
    block_on(camera.zoom_stop())?;

    // Set specific zoom position (50% of max)
    block_on(camera.set_zoom(0x3800))?;
    println!("Set zoom to 50%");

    // Move camera continuously
    println!("Starting continuous movement...");
    block_on(camera.move_continuous(PanTiltDirection::Right, 10, 0))?;
    std::thread::sleep(Duration::from_secs(2));
    block_on(camera.stop())?;
    println!("Continuous movement demo completed");

    // Return to preset 1 (home)
    block_on(camera.recall_preset(preset1))?;
    println!("Returned to preset 1");

    // Reset zoom
    block_on(camera.set_zoom(0x0000))?;
    println!("Reset zoom to minimum");

    println!("\nDemo completed successfully!");
    Ok(())
}

#[cfg(not(feature = "blocking-client"))]
fn main() {
    println!("This example requires the 'blocking-client' feature to be enabled.");
    println!("Run with: cargo run --example quick_start_demo --features blocking-client");
}
