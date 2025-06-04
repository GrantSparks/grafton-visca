//! Example demonstrating the high-level control API for camera operations.

use grafton_visca::{
    PanTiltDirection, UdpTransport, ViscaError, ViscaFocusExt, ViscaPanTiltExt, ViscaPresetExt,
    ViscaZoomExt,
};
use std::{env, thread, time::Duration};

fn main() -> Result<(), ViscaError> {
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
    println!("Connecting to camera at {}...", camera_addr);
    let mut transport = UdpTransport::new(camera_addr)?;

    println!("\n=== Camera Control Demo ===\n");

    // Pan/Tilt Control Examples
    println!("1. Pan/Tilt Control");
    println!("   - Moving to home position...");
    transport.go_home()?;
    thread::sleep(Duration::from_secs(3));

    println!("   - Moving to position (1000, -500) at default speed...");
    transport.move_to_position(1000, -500, None)?;
    thread::sleep(Duration::from_secs(2));

    println!("   - Moving relative (-500, 250) with custom speed...");
    transport.move_relative(-500, 250, Some((20, 15)))?;
    thread::sleep(Duration::from_secs(2));

    println!("   - Starting continuous movement (up-right)...");
    transport.start_moving(PanTiltDirection::UpRight, 10, 10)?;
    thread::sleep(Duration::from_millis(1500));

    println!("   - Stopping movement...");
    transport.stop_movement()?;
    thread::sleep(Duration::from_millis(500));

    // Zoom Control Examples
    println!("\n2. Zoom Control");
    println!("   - Zooming to wide angle (0x0000)...");
    transport.zoom_to(0x0000)?;
    thread::sleep(Duration::from_secs(2));

    println!("   - Zooming in at default speed...");
    transport.zoom_in(None)?;
    thread::sleep(Duration::from_secs(1));
    transport.stop_zoom()?;

    println!("   - Zooming to mid-range (0x2000)...");
    transport.zoom_to(0x2000)?;
    thread::sleep(Duration::from_secs(2));

    println!("   - Zooming out at slow speed (2)...");
    transport.zoom_out(Some(2))?;
    thread::sleep(Duration::from_millis(1500));
    transport.stop_zoom()?;

    // Focus Control Examples
    println!("\n3. Focus Control");
    println!("   - Enabling auto-focus...");
    transport.set_auto_focus(true)?;
    thread::sleep(Duration::from_secs(1));

    println!("   - Switching to manual focus...");
    transport.set_auto_focus(false)?;

    println!("   - Focusing near at default speed...");
    transport.focus_near(None)?;
    thread::sleep(Duration::from_millis(800));
    transport.stop_focus()?;

    println!("   - Setting focus to position 0x8000...");
    transport.focus_to(0x8000)?;
    thread::sleep(Duration::from_secs(1));

    println!("   - Triggering one-push auto-focus...");
    transport.trigger_one_push_focus()?;
    thread::sleep(Duration::from_secs(2));

    // Preset Management Examples
    println!("\n4. Preset Management");
    println!("   - Saving current position to preset 1...");
    transport.save_preset(1)?;
    thread::sleep(Duration::from_millis(500));

    println!("   - Moving camera to a different position...");
    transport.move_to_position(-1000, 300, None)?;
    transport.zoom_to(0x3000)?;
    thread::sleep(Duration::from_secs(3));

    println!("   - Saving this position to preset 2...");
    transport.save_preset(2)?;
    thread::sleep(Duration::from_millis(500));

    println!("   - Returning to home...");
    transport.go_home()?;
    thread::sleep(Duration::from_secs(2));

    println!("   - Recalling preset 1...");
    transport.recall_preset(1)?;
    thread::sleep(Duration::from_secs(3));

    println!("   - Recalling preset 2...");
    transport.recall_preset(2)?;
    thread::sleep(Duration::from_secs(3));

    println!("\n=== Demo Complete ===");
    println!("All control operations executed successfully!");

    Ok(())
}
