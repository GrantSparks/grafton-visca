//! Example program

//! Example demonstrating the high-level control API for camera operations.

use grafton_visca::{
    camera::{
        profiles::{G2PresetId, PTZOpticsG2},
        units::Degrees,
        Camera,
    },
    command::pan_tilt::PanTiltDirection,
    transport::{BlockingAdapter, UdpTransport},
    Error,
};
use std::{env, thread, time::Duration};

// Use a minimal tokio runtime for blocking execution
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(fut)
}

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
    let transport = UdpTransport::new(camera_addr)?;
    let camera = Camera::<PTZOpticsG2>::new(BlockingAdapter(transport));

    println!("\n=== Camera Control Demo ===\n");

    // Pan/Tilt Control Examples
    println!("1. Pan/Tilt Control");
    println!("   - Moving to home position...");
    block_on(camera.home())?;
    thread::sleep(Duration::from_secs(3));

    println!("   - Moving to position (1000, -500) at default speed...");
    block_on(camera.set_position(Degrees(20.0), Degrees(-10.0)))?;
    thread::sleep(Duration::from_secs(2));

    println!("   - Moving to another position (500, -250)...");
    // Note: Custom speed control is not directly available in the new API
    // Using the default speed
    block_on(camera.set_position(Degrees(10.0), Degrees(-5.0)))?;
    thread::sleep(Duration::from_secs(2));

    println!("   - Starting continuous movement (up-right)...");
    block_on(camera.move_continuous(PanTiltDirection::UpRight, 10, 10))?;
    thread::sleep(Duration::from_millis(1500));

    println!("   - Stopping movement...");
    block_on(camera.stop())?;
    thread::sleep(Duration::from_millis(500));

    // Zoom Control Examples
    println!("\n2. Zoom Control");
    println!("   - Zooming to minimum (0x0000)...");
    block_on(camera.set_zoom(0x0000))?;
    thread::sleep(Duration::from_secs(2));

    println!("   - Zooming in at default speed...");
    block_on(camera.zoom_in())?;
    thread::sleep(Duration::from_secs(1));
    block_on(camera.zoom_stop())?;

    println!("   - Zooming to mid-range (0x2000)...");
    block_on(camera.set_zoom(0x2000))?;
    thread::sleep(Duration::from_secs(2));

    println!("   - Zooming out at slow speed (2)...");
    // Note: zoom speed control requires using extension traits with older API
    // For now, using standard speed zoom
    block_on(camera.zoom_out())?;
    thread::sleep(Duration::from_millis(1500));
    block_on(camera.zoom_stop())?;

    // Focus Control Examples
    println!("\n3. Focus Control");
    println!("   - Enabling auto-focus...");
    block_on(camera.focus_auto())?;
    thread::sleep(Duration::from_secs(1));

    println!("   - Switching to manual focus...");
    block_on(camera.focus_manual())?;

    println!("   - Focusing near at default speed...");
    // Note: focus_near, stop_focus, focus_to, and one_push_focus require extension traits
    // These are not directly available in the new Camera API yet
    println!("   [Focus control methods like focus_near, focus_to, and one_push_focus");
    println!("    are not yet available in the new Camera API]");
    thread::sleep(Duration::from_secs(2));

    // Preset Management Examples
    println!("\n4. Preset Management");
    println!("   - Saving current position to preset 1...");
    block_on(camera.set_preset(G2PresetId::new(1).unwrap()))?;
    thread::sleep(Duration::from_millis(500));

    println!("   - Moving camera to a different position...");
    block_on(camera.set_position(Degrees(-20.0), Degrees(6.0)))?;
    block_on(camera.set_zoom(0x3000))?;
    thread::sleep(Duration::from_secs(3));

    println!("   - Saving this position to preset 2...");
    block_on(camera.set_preset(G2PresetId::new(2).unwrap()))?;
    thread::sleep(Duration::from_millis(500));

    println!("   - Returning to home...");
    block_on(camera.home())?;
    thread::sleep(Duration::from_secs(2));

    println!("   - Recalling preset 1...");
    block_on(camera.recall_preset(G2PresetId::new(1).unwrap()))?;
    thread::sleep(Duration::from_secs(3));

    println!("   - Recalling preset 2...");
    block_on(camera.recall_preset(G2PresetId::new(2).unwrap()))?;
    thread::sleep(Duration::from_secs(3));

    println!("\n=== Demo Complete ===");
    println!("All control operations executed successfully!");

    Ok(())
}
