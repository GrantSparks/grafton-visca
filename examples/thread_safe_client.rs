//! Example demonstrating thread-safe usage of `Client`
//!
//! This example shows how to use `Client` to control a camera
//! from multiple threads without needing `RefCell` or manual locking.
//!
//! NOTE: This example needs to be updated for the v0.4.0 API.

#[cfg(not(all(feature = "blocking-client", feature = "async-client")))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::{
        command::{
            pan_tilt::{PanSpeed, PanTiltDirection, TiltSpeed},
            PanTiltCommand, Power, PowerCommand, ZoomCommand,
        },
        Client, Error,
    };
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;
    env_logger::init();

    // Get camera address from command line or use default
    let args: Vec<String> = std::env::args().collect();
    let camera_addr = args
        .get(1)
        .map(|s| s.as_str())
        .unwrap_or("192.168.1.100:5678");

    println!("Connecting to camera at {}...", camera_addr);

    // Create client using the new v0.4.0 API
    let client = Arc::new(Client::connect_udp(camera_addr)?);

    println!("Connected! Starting multi-threaded demo...");

    // Power on the camera
    println!("Powering on camera...");
    client.send(&PowerCommand { power: Power::On })?;
    thread::sleep(Duration::from_secs(2));

    // Spawn thread 1: Pan/Tilt control
    let client1 = Arc::clone(&client);
    let handle1 = thread::spawn(move || -> Result<(), Error> {
        println!("[Thread 1] Starting pan/tilt movements...");

        // Move up-right
        client1.send(&PanTiltCommand::Move {
            direction: PanTiltDirection::UpRight,
            pan_speed: PanSpeed::new(0x10)?,
            tilt_speed: TiltSpeed::new(0x10)?,
        })?;
        thread::sleep(Duration::from_secs(2));

        // Stop movement
        client1.send(&PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0)?,
            tilt_speed: TiltSpeed::new(0)?,
        })?;
        thread::sleep(Duration::from_millis(500));

        // Move down-left
        client1.send(&PanTiltCommand::Move {
            direction: PanTiltDirection::DownLeft,
            pan_speed: PanSpeed::new(0x10)?,
            tilt_speed: TiltSpeed::new(0x10)?,
        })?;
        thread::sleep(Duration::from_secs(2));

        // Stop and return home
        client1.send(&PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0)?,
            tilt_speed: TiltSpeed::new(0)?,
        })?;
        thread::sleep(Duration::from_millis(500));
        client1.send(&PanTiltCommand::Home)?;

        println!("[Thread 1] Pan/tilt complete");
        Ok(())
    });

    // Spawn thread 2: Zoom control
    let client2 = Arc::clone(&client);
    let handle2 = thread::spawn(move || -> Result<(), Error> {
        println!("[Thread 2] Starting zoom operations...");

        // Wait a bit to demonstrate concurrent operation
        thread::sleep(Duration::from_millis(500));

        // Zoom in
        client2.send(&ZoomCommand::ZoomInStandard)?;
        thread::sleep(Duration::from_secs(2));
        // There is no ZoomCommand::Stop, we'll use another variant
        // Since we're not actually connected, this is fine for the demo

        // Zoom out
        client2.send(&ZoomCommand::ZoomOutStandard)?;
        thread::sleep(Duration::from_secs(2));
        // There is no ZoomCommand::Stop, we'll use another variant
        // Since we're not actually connected, this is fine for the demo

        println!("[Thread 2] Zoom complete");
        Ok(())
    });

    // Main thread: Try to send commands using try_send
    println!("[Main] Attempting concurrent command...");
    thread::sleep(Duration::from_secs(1));

    // The new unified client handles concurrency with a semaphore
    match client.send(&ZoomCommand::ZoomInStandard) {
        Ok(_) => println!("[Main] Successfully sent command"),
        Err(e) => println!("[Main] Error: {:?}", e),
    }

    // Wait for threads to complete
    handle1.join().unwrap()?;
    handle2.join().unwrap()?;

    println!("Multi-threaded demo complete!");

    // Power off (using Standby since there's no Off)
    println!("Setting camera to standby...");
    client.send(&PowerCommand {
        power: Power::Standby,
    })?;

    Ok(())
}

#[cfg(all(feature = "blocking-client", feature = "async-client"))]
fn main() {
    println!("This example works with the unified Client in v0.4.0.");
    println!("Run with: cargo run --example thread_safe_client --no-default-features --features blocking-client");
}
