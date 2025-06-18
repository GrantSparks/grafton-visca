//! Example demonstrating thread-safe usage of Camera with PTZOpticsG2 profile
//!
//! This example shows how to use the `Camera<P>` API to control a camera
//! from multiple threads using Arc<Mutex<Camera>> for thread safety.
//!
//! Run with: cargo run --example thread_safe_client [CAMERA_IP:PORT]
//! Default camera address: 192.168.1.100:5678

use grafton_visca::{
    camera::{Camera, CameraExtension, PTZOpticsG2},
    command::{
        pan_tilt::{PanSpeed, PanTiltDirection, TiltSpeed},
        PanTiltCommand, Power, PowerCommand, ZoomCommand,
    },
    transport::{BlockingAdapter, UdpTransport},
    Error,
};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Get camera address from command line or use default
    let args: Vec<String> = std::env::args().collect();
    let camera_addr = args
        .get(1)
        .map(|s| s.as_str())
        .unwrap_or("192.168.1.100:5678");

    println!("Connecting to camera at {}...", camera_addr);

    // Create UDP transport and Camera with PTZOpticsG2 profile
    let transport = UdpTransport::new(camera_addr)?;
    let camera = Camera::<PTZOpticsG2>::new(BlockingAdapter(transport));

    // Wrap the camera in Arc<Mutex> for thread-safe access
    let camera = Arc::new(Mutex::new(camera));

    println!("Connected! Starting multi-threaded demo...");

    // Power on the camera
    println!("Powering on camera...");
    {
        let cam = camera.lock().unwrap();
        cam.send_raw(&PowerCommand { power: Power::On })?;
    }
    thread::sleep(Duration::from_secs(2));

    // Spawn thread 1: Pan/Tilt control
    let camera1 = Arc::clone(&camera);
    let handle1 = thread::spawn(move || -> Result<(), Error> {
        println!("[Thread 1] Starting pan/tilt movements...");

        // Move up-right
        {
            let cam = camera1.lock().unwrap();
            cam.send_raw(&PanTiltCommand::Move {
                direction: PanTiltDirection::UpRight,
                pan_speed: PanSpeed::new(0x10)?,
                tilt_speed: TiltSpeed::new(0x10)?,
            })?;
        }
        thread::sleep(Duration::from_secs(2));

        // Stop movement
        {
            let cam = camera1.lock().unwrap();
            cam.send_raw(&PanTiltCommand::Move {
                direction: PanTiltDirection::Stop,
                pan_speed: PanSpeed::new(0)?,
                tilt_speed: TiltSpeed::new(0)?,
            })?;
        }
        thread::sleep(Duration::from_millis(500));

        // Move down-left
        {
            let cam = camera1.lock().unwrap();
            cam.send_raw(&PanTiltCommand::Move {
                direction: PanTiltDirection::DownLeft,
                pan_speed: PanSpeed::new(0x10)?,
                tilt_speed: TiltSpeed::new(0x10)?,
            })?;
        }
        thread::sleep(Duration::from_secs(2));

        // Stop and return home
        {
            let cam = camera1.lock().unwrap();
            cam.send_raw(&PanTiltCommand::Move {
                direction: PanTiltDirection::Stop,
                pan_speed: PanSpeed::new(0)?,
                tilt_speed: TiltSpeed::new(0)?,
            })?;
        }
        thread::sleep(Duration::from_millis(500));
        {
            let mut cam = camera1.lock().unwrap();
            block_on(cam.transport_mut().send_command(&PanTiltCommand::Home))?;
        }

        println!("[Thread 1] Pan/tilt complete");
        Ok(())
    });

    // Spawn thread 2: Zoom control
    let camera2 = Arc::clone(&camera);
    let handle2 = thread::spawn(move || -> Result<(), Error> {
        println!("[Thread 2] Starting zoom operations...");

        // Wait a bit to demonstrate concurrent operation
        thread::sleep(Duration::from_millis(500));

        // Zoom in
        {
            let cam = camera2.lock().unwrap();
            cam.send_raw(&ZoomCommand::ZoomInStandard)?;
        }
        thread::sleep(Duration::from_secs(2));

        // Stop zoom (using ZoomStop command)
        {
            let cam = camera2.lock().unwrap();
            cam.send_raw(&ZoomCommand::Stop)?;
        }

        // Zoom out
        {
            let cam = camera2.lock().unwrap();
            cam.send_raw(&ZoomCommand::ZoomOutStandard)?;
        }
        thread::sleep(Duration::from_secs(2));

        // Stop zoom
        {
            let cam = camera2.lock().unwrap();
            cam.send_raw(&ZoomCommand::Stop)?;
        }

        println!("[Thread 2] Zoom complete");
        Ok(())
    });

    // Main thread: Demonstrate concurrent access with proper locking
    println!("[Main] Attempting concurrent command...");
    thread::sleep(Duration::from_secs(1));

    // Try to access the camera from the main thread
    {
        let mut cam = camera.lock().unwrap();
        match block_on(cam.send_raw(&ZoomCommand::ZoomInStandard)) {
            Ok(_) => println!("[Main] Successfully sent command"),
            Err(e) => println!("[Main] Error: {:?}", e),
        }
    }

    // Wait for threads to complete
    handle1.join().unwrap()?;
    handle2.join().unwrap()?;

    println!("Multi-threaded demo complete!");

    // Demonstrate `Camera<P>` specific features
    println!("\nCamera profile information:");
    {
        let cam = camera.lock().unwrap();
        let capabilities = cam.capabilities();
        println!("Model: {}", capabilities.model_name);
        println!("Pan range: {:?} degrees", capabilities.pan_range_degrees);
        println!("Tilt range: {:?} degrees", capabilities.tilt_range_degrees);
        println!("Max pan speed: {}", capabilities.max_pan_speed);
        println!("Max tilt speed: {}", capabilities.max_tilt_speed);
        println!("Preset count: {}", capabilities.preset_count);
        println!(
            "Supports digital zoom: {}",
            capabilities.supports_digital_zoom
        );
    }

    // Power off (using Standby since there's no Off)
    println!("\nSetting camera to standby...");
    {
        let cam = camera.lock().unwrap();
        cam.send_raw(&PowerCommand {
            power: Power::Standby,
        })?;
    }

    Ok(())
}
