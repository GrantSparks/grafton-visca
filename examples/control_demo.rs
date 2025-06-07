//! Example demonstrating the high-level control API for camera operations.

use grafton_visca::{
    command::{
        pan_tilt::{PanSpeed, PanTiltDirection, TiltSpeed},
        preset::PresetNumber,
        zoom::ZoomSpeed,
    },
    ViscaClient, ViscaError, ViscaFocusExt, ViscaPanTiltExt, ViscaPresetExt, ViscaTransportExt,
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
    println!("Connecting to camera at {camera_addr}...");
    let mut client = ViscaClient::connect_udp(camera_addr)?;

    println!("\n=== Camera Control Demo ===\n");

    // Pan/Tilt Control Examples
    println!("1. Pan/Tilt Control");
    println!("   - Moving to home position...");
    client.home()?;
    thread::sleep(Duration::from_secs(3));

    println!("   - Moving to position (1000, -500) at default speed...");
    ViscaPanTiltExt::move_to_position(&mut client, 1000, -500, None)?;
    thread::sleep(Duration::from_secs(2));

    println!("   - Moving relative (-500, 250) with custom speed...");
    // Note: move_relative is not available in current API
    // Using absolute position instead
    ViscaPanTiltExt::move_to_position(
        &mut client,
        500,
        -250,
        Some((PanSpeed::new(20)?, TiltSpeed::new(15)?)),
    )?;
    thread::sleep(Duration::from_secs(2));

    println!("   - Starting continuous movement (up-right)...");
    client.start_moving(
        PanTiltDirection::UpRight,
        PanSpeed::new(10)?,
        TiltSpeed::new(10)?,
    )?;
    thread::sleep(Duration::from_millis(1500));

    println!("   - Stopping movement...");
    client.move_stop()?;
    thread::sleep(Duration::from_millis(500));

    // Zoom Control Examples
    println!("\n2. Zoom Control");
    println!("   - Zooming to wide angle (0x0000)...");
    client.zoom_to(0x0000)?;
    thread::sleep(Duration::from_secs(2));

    println!("   - Zooming in at default speed...");
    ViscaZoomExt::zoom_in(&mut client, None)?;
    thread::sleep(Duration::from_secs(1));
    client.stop_zoom()?;

    println!("   - Zooming to mid-range (0x2000)...");
    client.zoom_to(0x2000)?;
    thread::sleep(Duration::from_secs(2));

    println!("   - Zooming out at slow speed (2)...");
    ViscaZoomExt::zoom_out(&mut client, Some(ZoomSpeed::new(2)?))?;
    thread::sleep(Duration::from_millis(1500));
    client.stop_zoom()?;

    // Focus Control Examples
    println!("\n3. Focus Control");
    println!("   - Enabling auto-focus...");
    client.set_focus_auto()?;
    thread::sleep(Duration::from_secs(1));

    println!("   - Switching to manual focus...");
    client.set_focus_manual()?;

    println!("   - Focusing near at default speed...");
    ViscaFocusExt::focus_near(&mut client, None)?;
    thread::sleep(Duration::from_millis(800));
    client.stop_focus()?;

    println!("   - Setting focus to position 0x8000...");
    client.focus_to(0x8000)?;
    thread::sleep(Duration::from_secs(1));

    println!("   - Triggering one-push auto-focus...");
    ViscaFocusExt::trigger_one_push_focus(&mut client)?;
    thread::sleep(Duration::from_secs(2));

    // Preset Management Examples
    println!("\n4. Preset Management");
    println!("   - Saving current position to preset 1...");
    ViscaPresetExt::save_preset(&mut client, PresetNumber::new(1)?)?;
    thread::sleep(Duration::from_millis(500));

    println!("   - Moving camera to a different position...");
    ViscaPanTiltExt::move_to_position(&mut client, -1000, 300, None)?;
    client.zoom_to(0x3000)?;
    thread::sleep(Duration::from_secs(3));

    println!("   - Saving this position to preset 2...");
    ViscaPresetExt::save_preset(&mut client, PresetNumber::new(2)?)?;
    thread::sleep(Duration::from_millis(500));

    println!("   - Returning to home...");
    client.home()?;
    thread::sleep(Duration::from_secs(2));

    println!("   - Recalling preset 1...");
    ViscaPresetExt::recall_preset(&mut client, PresetNumber::new(1)?)?;
    thread::sleep(Duration::from_secs(3));

    println!("   - Recalling preset 2...");
    ViscaPresetExt::recall_preset(&mut client, PresetNumber::new(2)?)?;
    thread::sleep(Duration::from_secs(3));

    println!("\n=== Demo Complete ===");
    println!("All control operations executed successfully!");

    Ok(())
}
