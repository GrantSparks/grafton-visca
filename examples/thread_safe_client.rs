//! Example demonstrating thread-safe usage of Camera with PTZOpticsG2 profile
//!
//! This example shows how to use the `Camera<P>` API to control a camera
//! from multiple threads using Arc<Mutex<Camera>> for thread safety.
//!
//! Run with: cargo run --example thread_safe_client --no-default-features --features blocking-client [CAMERA_IP:PORT]
//! Default camera address: 192.168.1.100:5678

#[cfg(all(feature = "blocking-client", not(feature = "async-client")))]
use grafton_visca::{
    camera::{Camera, CameraProfile, PTZOpticsG2},
    command::pan_tilt::PanTiltDirection,
    transport::{BlockingAdapter, UdpTransport},
    Error,
};
#[cfg(all(feature = "blocking-client", not(feature = "async-client")))]
use std::sync::{Arc, Mutex};
#[cfg(all(feature = "blocking-client", not(feature = "async-client")))]
use std::thread;
#[cfg(all(feature = "blocking-client", not(feature = "async-client")))]
use std::time::Duration;

#[cfg(all(feature = "blocking-client", not(feature = "async-client")))]
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
        cam.power_on()?;
    }
    thread::sleep(Duration::from_secs(2));

    // Spawn thread 1: Pan/Tilt control
    let camera1 = Arc::clone(&camera);
    let handle1 = thread::spawn(move || -> Result<(), Error> {
        println!("[Thread 1] Starting pan/tilt movements...");

        // Move up-right
        {
            let cam = camera1.lock().unwrap();
            cam.move_continuous(PanTiltDirection::UpRight, 16, 16)?;
        }
        thread::sleep(Duration::from_secs(2));

        // Stop movement
        {
            let cam = camera1.lock().unwrap();
            cam.stop()?;
        }
        thread::sleep(Duration::from_millis(500));

        // Move down-left
        {
            let cam = camera1.lock().unwrap();
            cam.move_continuous(PanTiltDirection::DownLeft, 16, 16)?;
        }
        thread::sleep(Duration::from_secs(2));

        // Stop and return home
        {
            let cam = camera1.lock().unwrap();
            cam.stop()?;
        }
        thread::sleep(Duration::from_millis(500));
        {
            let cam = camera1.lock().unwrap();
            cam.home()?;
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
            cam.zoom_in()?;
        }
        thread::sleep(Duration::from_secs(2));

        // Stop zoom
        {
            let cam = camera2.lock().unwrap();
            cam.zoom_stop()?;
        }

        // Zoom out
        {
            let cam = camera2.lock().unwrap();
            cam.zoom_out()?;
        }
        thread::sleep(Duration::from_secs(2));

        // Stop zoom
        {
            let cam = camera2.lock().unwrap();
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
        let cam = camera.lock().unwrap();
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
    println!(
        "Pan range: {:?} degrees",
        PTZOpticsG2::pan_degree_range(&PTZOpticsG2)
    );
    println!(
        "Tilt range: {:?} degrees",
        PTZOpticsG2::tilt_degree_range(&PTZOpticsG2)
    );
    println!("Max pan speed: {}", PTZOpticsG2::MAX_PAN_SPEED);
    println!("Max tilt speed: {}", PTZOpticsG2::MAX_TILT_SPEED);
    println!("Preset count: {}", PTZOpticsG2::max_preset_id() + 1);
    println!(
        "Supports digital zoom: {}",
        PTZOpticsG2::digital_zoom_supported(&PTZOpticsG2)
    );

    // Power off
    println!("\nPowering off camera...");
    {
        let cam = camera.lock().unwrap();
        cam.power_off()?;
    }

    Ok(())
}

#[cfg(not(all(feature = "blocking-client", not(feature = "async-client"))))]
fn main() {
    eprintln!("This example requires only the blocking-client feature.");
    eprintln!("Run with: cargo run --example thread_safe_client --no-default-features --features blocking-client");
}
