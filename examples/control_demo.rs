//! Example program

//! Example demonstrating the high-level control API for camera operations.

#[cfg(feature = "async")]
fn main() {
    eprintln!("This example demonstrates the blocking control API.");
    eprintln!("Run without async features: cargo run --example control_demo");
}

#[cfg(not(feature = "async"))]
use grafton_visca::{
    camera::methods::{FocusBlockingExt, PanTiltBlockingExt, PresetBlockingExt, ZoomBlockingExt},
    profiles::PTZOpticsG2,
    transport::blocking::UdpGat,
    CameraBlocking, Error,
};
#[cfg(not(feature = "async"))]
use std::{env, time::Duration};

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Error> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Get camera address from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <camera_ip:port>", args[0]);
        eprintln!("Example: {} 192.168.1.100:5678", args[0]);
        std::process::exit(1);
    }

    // Connect to camera
    let camera_addr = &args[1];
    println!("Connecting to camera at {camera_addr}...");
    let transport = UdpGat::connect(camera_addr)?;
    let mut camera = CameraBlocking::<PTZOpticsG2, _>::new(transport);

    println!("\n=== Camera Control Demo ===\n");

    {
        // Pan/Tilt Control Examples
        println!("1. Pan/Tilt Control");
        println!("   - Moving to home position...");
        camera.pan_tilt_home()?;
        std::thread::sleep(Duration::from_secs(3));

        println!("   - Moving to absolute position (20°, -10°)...");
        camera.pan_tilt_absolute(20.0, -10.0, 10)?;
        std::thread::sleep(Duration::from_secs(2));

        println!("   - Moving to another position (10°, -5°)...");
        camera.pan_tilt_absolute(10.0, -5.0, 10)?;
        std::thread::sleep(Duration::from_secs(2));

        println!("   - Relative movement (+5°, +2°)...");
        camera.pan_tilt_relative(5.0, 2.0, 10)?;
        std::thread::sleep(Duration::from_millis(1500));

        println!("   - Stopping movement...");
        camera.pan_tilt_stop()?;
        std::thread::sleep(Duration::from_millis(500));

        // Zoom Control Examples
        println!("\n2. Zoom Control");
        println!("   - Zooming to minimum (wide)...");
        camera.zoom_absolute(0.0)?;
        std::thread::sleep(Duration::from_secs(2));

        println!("   - Zooming in at default speed...");
        camera.zoom_in()?;
        std::thread::sleep(Duration::from_secs(1));
        camera.zoom_stop()?;

        println!("   - Zooming to mid-range (50%)...");
        camera.zoom_absolute(0.5)?;
        std::thread::sleep(Duration::from_secs(2));

        println!("   - Zooming out at slow speed (2)...");
        // Note: zoom speed control requires using extension traits with older API
        // For now, using standard speed zoom
        camera.zoom_out()?;
        std::thread::sleep(Duration::from_millis(1500));
        camera.zoom_stop()?;

        // Focus Control Examples
        println!("\n3. Focus Control");
        println!("   - Enabling auto-focus...");
        camera.focus_auto()?;
        std::thread::sleep(Duration::from_secs(1));

        println!("   - Switching to manual focus...");
        camera.focus_manual()?;

        println!("   - Focusing near at default speed...");
        camera.focus_near(5)?;
        std::thread::sleep(Duration::from_secs(1));
        camera.focus_stop()?;

        println!("   - Triggering one-push auto focus...");
        camera.focus_one_push()?;
        std::thread::sleep(Duration::from_secs(2));

        // Preset Management Examples
        println!("\n4. Preset Management");
        println!("   - Saving current position to preset 1...");
        camera.preset_set(1)?;
        std::thread::sleep(Duration::from_millis(500));

        println!("   - Moving camera to a different position...");
        camera.pan_tilt_absolute(-20.0, 6.0, 10)?;
        camera.zoom_absolute(0.75)?; // 75% zoom
        std::thread::sleep(Duration::from_secs(3));

        println!("   - Saving this position to preset 2...");
        camera.preset_set(2)?;
        std::thread::sleep(Duration::from_millis(500));

        println!("   - Returning to home...");
        camera.pan_tilt_home()?;
        std::thread::sleep(Duration::from_secs(2));

        println!("   - Recalling preset 1...");
        camera.preset_recall(1)?;
        std::thread::sleep(Duration::from_secs(3));

        println!("   - Recalling preset 2...");
        camera.preset_recall(2)?;
        std::thread::sleep(Duration::from_secs(3));

        println!("\n=== Demo Complete ===");
        println!("All control operations executed successfully!");

        Ok::<_, Error>(())
    }
}
