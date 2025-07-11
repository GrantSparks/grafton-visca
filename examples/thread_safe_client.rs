//! Example demonstrating thread-safe usage of Camera with PTZOpticsG2 profile
//!
//! This example shows how to use the `Camera<P>` API to control a camera
//! from multiple threads using Arc<Mutex<Camera>> for thread safety.
//!
//! Run with: cargo run --example thread_safe_client --no-default-features --features blocking-client [CAMERA_IP:PORT]
//! Default camera address: 192.168.1.100:5678

#[cfg(not(feature = "async"))]
use grafton_visca::transport::blocking::Udp;
#[cfg(not(feature = "async"))]
use grafton_visca::{
    camera::methods::{PanTiltBlockingExt, PowerBlockingExt, ZoomBlockingExt},
    command::pan_tilt::PanTiltDirection,
    profiles::PTZOpticsG2,
    types::{PanSpeed, TiltSpeed},
    CameraBlocking, Error,
};
#[cfg(not(feature = "async"))]
use std::sync::{Arc, Mutex};
#[cfg(not(feature = "async"))]
use std::thread;
#[cfg(not(feature = "async"))]
use std::time::Duration;

#[cfg(not(feature = "async"))]
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
    let transport = Udp::connect(camera_addr)?;
    let camera = CameraBlocking::<PTZOpticsG2, _>::new(transport);

    // Wrap the camera in Arc<Mutex> for thread-safe access
    let camera = Arc::new(Mutex::new(camera));

    println!("Connected! Starting multi-threaded demo...");

    // Power on the camera
    println!("Powering on camera...");
    {
        let mut cam = camera.lock().unwrap();
        cam.power_on()?;
    }
    thread::sleep(Duration::from_secs(2));

    // Spawn thread 1: Pan/Tilt control
    let camera1 = Arc::clone(&camera);
    let handle1 = thread::spawn(move || -> Result<(), Error> {
        println!("[Thread 1] Starting pan/tilt movements...");

        // Move up-right
        {
            let mut cam = camera1.lock().unwrap();
            cam.pan_tilt_move(
                PanTiltDirection::UpRight,
                PanSpeed::try_from(16)?,
                TiltSpeed::try_from(16)?,
            )?;
        }
        thread::sleep(Duration::from_secs(2));

        // Stop movement
        {
            let mut cam = camera1.lock().unwrap();
            cam.pan_tilt_stop()?;
        }
        thread::sleep(Duration::from_millis(500));

        // Move down-left
        {
            let mut cam = camera1.lock().unwrap();
            cam.pan_tilt_move(
                PanTiltDirection::DownLeft,
                PanSpeed::try_from(16)?,
                TiltSpeed::try_from(16)?,
            )?;
        }
        thread::sleep(Duration::from_secs(2));

        // Stop and return home
        {
            let mut cam = camera1.lock().unwrap();
            cam.pan_tilt_stop()?;
        }
        thread::sleep(Duration::from_millis(500));
        {
            let mut cam = camera1.lock().unwrap();
            cam.pan_tilt_home()?;
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
            let mut cam = camera2.lock().unwrap();
            cam.zoom_in()?;
        }
        thread::sleep(Duration::from_secs(2));

        // Stop zoom
        {
            let mut cam = camera2.lock().unwrap();
            cam.zoom_stop()?;
        }

        // Zoom out
        {
            let mut cam = camera2.lock().unwrap();
            cam.zoom_out()?;
        }
        thread::sleep(Duration::from_secs(2));

        // Stop zoom
        {
            let mut cam = camera2.lock().unwrap();
            cam.zoom_stop()?;
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
        match cam.zoom_in() {
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
    println!("Model: {}", PTZOpticsG2::MODEL_NAME);
    println!("Pan range: -170 to +170 degrees");
    println!("Tilt range: -30 to +90 degrees");
    println!("Max pan speed: 24");
    println!("Max tilt speed: 20");
    println!("Preset count: 128");
    println!("Supports digital zoom: true");

    // Power off
    println!("\nPowering off camera...");
    {
        let mut cam = camera.lock().unwrap();
        cam.power_off()?;
    }

    Ok(())
}

#[cfg(feature = "async")]
fn main() {
    eprintln!("This example requires only the blocking-client feature.");
    eprintln!("Run with: cargo run --example thread_safe_client --no-default-features --features blocking-client");
}
