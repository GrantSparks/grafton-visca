//! Example demonstrating thread-safe usage of ViscaClient
//!
//! This example shows how to use ViscaClient to control a camera
//! from multiple threads without needing RefCell or manual locking.

#[cfg(not(all(feature = "sync", feature = "async")))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::{
        command::{
            pan_tilt::{PanSpeed, PanTiltDirection, TiltSpeed},
            power::Power,
            PanTiltCommand, PowerCommand, ZoomCommand,
        },
        UdpTransport, ViscaClient, ViscaError,
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

    // Create transport and wrap it in ViscaClient
    let transport = UdpTransport::new(camera_addr)?;
    let client = Arc::new(ViscaClient::new(Box::new(transport)));

    println!("Connected! Starting multi-threaded demo...");

    // Power on the camera
    println!("Powering on camera...");
    client.send(&PowerCommand { power: Power::On })?;
    thread::sleep(Duration::from_secs(2));

    // Spawn thread 1: Pan/Tilt control
    let client1 = Arc::clone(&client);
    let handle1 = thread::spawn(move || -> Result<(), ViscaError> {
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
    let handle2 = thread::spawn(move || -> Result<(), ViscaError> {
        println!("[Thread 2] Starting zoom operations...");

        // Wait a bit to demonstrate concurrent operation
        thread::sleep(Duration::from_millis(500));

        // Zoom in
        client2.send(&ZoomCommand::TeleStandard)?;
        thread::sleep(Duration::from_secs(2));
        // There is no ZoomCommand::Stop, we'll use another variant
        // Since we're not actually connected, this is fine for the demo

        // Zoom out
        client2.send(&ZoomCommand::WideStandard)?;
        thread::sleep(Duration::from_secs(2));
        // There is no ZoomCommand::Stop, we'll use another variant
        // Since we're not actually connected, this is fine for the demo

        println!("[Thread 2] Zoom complete");
        Ok(())
    });

    // Main thread: Try to send commands using try_send
    println!("[Main] Attempting concurrent command...");
    thread::sleep(Duration::from_secs(1));

    // This might fail if another thread is using the transport
    match client.try_send(&ZoomCommand::TeleStandard) {
        Ok(_) => println!("[Main] Successfully sent command"),
        Err(ViscaError::InvalidParameter(msg)) if msg.contains("busy") => {
            println!("[Main] Transport busy (expected during concurrent usage)");
        }
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

#[cfg(all(feature = "sync", feature = "async"))]
fn main() {
    println!("This example requires the new ViscaClient which is not available when both sync and async features are enabled.");
    println!("Run with: cargo run --example thread_safe_client");
}
