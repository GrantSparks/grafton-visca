//! Example demonstrating concurrent async command execution with grafton-visca
//!
//! This example shows how to:
//! - Connect to a camera using async API
//! - Send multiple commands concurrently
//! - Handle the two-socket limitation gracefully
//! - Process responses asynchronously
//! - Maximize throughput with concurrent operations

use grafton_visca::command::{
    pan_tilt::{PanSpeed, PanTiltDirection, TiltSpeed},
    preset::{PresetAction, PresetNumber},
    FocusCommand, InquiryCommand, PanTiltCommand, PresetCommand, Response, ZoomCommand,
};
use grafton_visca::{Client, Error};
use std::time::Instant;

#[cfg(not(feature = "async-client"))]
fn main() {
    eprintln!("This example requires the 'async-client' feature.");
    eprintln!("Run with: cargo run --example async_concurrent --features async-client");
}

#[cfg(feature = "async-client")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Get camera address from command line or environment
    let camera_addr = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("CAMERA_IP").ok())
        .unwrap_or_else(|| "192.168.0.100:5678".to_string());

    println!("Connecting to camera at {}...", camera_addr);
    let camera = Client::connect_udp_async(&camera_addr).await?;

    // Example 1: Send two commands concurrently
    println!("\n=== Concurrent Command Execution ===");
    let start = Instant::now();

    // Start moving the camera and zooming at the same time
    let move_cmd = PanTiltCommand::Move {
        direction: PanTiltDirection::UpRight,
        pan_speed: PanSpeed::new(0x10)?,
        tilt_speed: TiltSpeed::new(0x10)?,
    };
    let move_fut = camera.send_async(&move_cmd);
    let zoom_fut = camera.send_async(&ZoomCommand::ZoomInStandard);

    // Wait for both to complete
    let (move_result, zoom_result) = tokio::join!(move_fut, zoom_fut);

    println!("Commands completed in {:?}", start.elapsed());
    println!("Move result: {:?}", move_result);
    println!("Zoom result: {:?}", zoom_result);

    // Wait a moment then stop both
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let stop_move = PanTiltCommand::Move {
        direction: PanTiltDirection::Stop,
        pan_speed: PanSpeed::new(0)?,
        tilt_speed: TiltSpeed::new(0)?,
    };
    let stop_move_fut = camera.send_async(&stop_move);
    let stop_zoom_fut = camera.send_async(&ZoomCommand::Stop);

    let _ = tokio::join!(stop_move_fut, stop_zoom_fut);

    // Example 2: Concurrent inquiries
    println!("\n=== Concurrent Inquiries ===");
    let start = Instant::now();

    // Query multiple camera states at once
    let power_fut = camera.send_async(&InquiryCommand::Power);
    let position_fut = camera.send_async(&InquiryCommand::PanTiltPosition);
    let zoom_pos_fut = camera.send_async(&InquiryCommand::ZoomPosition);
    let focus_mode_fut = camera.send_async(&InquiryCommand::FocusPosition);

    let (power, position, zoom_pos, focus_mode) =
        tokio::join!(power_fut, position_fut, zoom_pos_fut, focus_mode_fut);

    println!("All inquiries completed in {:?}", start.elapsed());

    if let Ok(Response::InquiryResponse(ref resp)) = power {
        println!("Power status: {:?}", resp);
    }
    if let Ok(Response::InquiryResponse(ref resp)) = position {
        println!("Position: {:?}", resp);
    }
    if let Ok(Response::InquiryResponse(ref resp)) = zoom_pos {
        println!("Zoom position: {:?}", resp);
    }
    if let Ok(Response::InquiryResponse(ref resp)) = focus_mode {
        println!("Focus mode: {:?}", resp);
    }

    // Example 3: Complex concurrent sequence
    println!("\n=== Complex Concurrent Sequence ===");

    // Save current position as preset while also getting camera info
    let save_preset = PresetCommand {
        action: PresetAction::Set,
        preset_number: PresetNumber::new(1)?,
    };

    let save_fut = camera.send_async(&save_preset);
    let wb_fut = camera.send_async(&InquiryCommand::WhiteBalanceMode);
    let exposure_fut = camera.send_async(&InquiryCommand::ExposureMode);

    let (save_result, wb_result, exposure_result) = tokio::join!(save_fut, wb_fut, exposure_fut);

    println!("Preset saved: {:?}", save_result.is_ok());
    if let Ok(Response::InquiryResponse(ref resp)) = wb_result {
        println!("White balance: {:?}", resp);
    }
    if let Ok(Response::InquiryResponse(ref resp)) = exposure_result {
        println!("Exposure mode: {:?}", resp);
    }

    // Example 4: Maximizing throughput with many operations
    println!("\n=== Maximum Throughput Test ===");
    let start = Instant::now();

    // Create many inquiry futures
    let mut futures = Vec::new();
    for _ in 0..10 {
        futures.push(camera.send_async(&InquiryCommand::ZoomPosition));
    }

    // Execute them all concurrently (limited by the 2-socket constraint)
    let results = futures_util::future::join_all(futures).await;

    let elapsed = start.elapsed();
    let successful = results.iter().filter(|r| r.is_ok()).count();

    println!("Sent 10 commands in {:?}", elapsed);
    println!("Successful: {}/10", successful);
    println!("Average time per command: {:?}", elapsed / 10);

    // Example 5: Movement coordination
    println!("\n=== Coordinated Movement ===");

    // Move to home while setting focus to auto
    let home_fut = camera.send_async(&PanTiltCommand::Home);
    let focus_fut = camera.send_async(&FocusCommand::Auto);

    let (home_result, focus_result) = tokio::join!(home_fut, focus_fut);

    println!("Home command: {:?}", home_result.is_ok());
    println!("Auto focus: {:?}", focus_result.is_ok());

    // Wait for movement to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Now do a complex movement pattern
    println!("\nExecuting movement pattern...");

    // Pan right while zooming in
    let pan_right = PanTiltCommand::Move {
        direction: PanTiltDirection::Right,
        pan_speed: PanSpeed::new(0x08)?,
        tilt_speed: TiltSpeed::new(0)?,
    };

    let pan_fut = camera.send_async(&pan_right);

    let zoom_cmd = if let Ok(speed) = grafton_visca::command::zoom::ZoomSpeed::new(3) {
        ZoomCommand::ZoomInVariable(speed)
    } else {
        ZoomCommand::ZoomInStandard
    };
    let zoom_in_fut = camera.send_async(&zoom_cmd);

    let _ = tokio::join!(pan_fut, zoom_in_fut);
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Stop all movement
    let stop_pan = PanTiltCommand::Move {
        direction: PanTiltDirection::Stop,
        pan_speed: PanSpeed::new(0)?,
        tilt_speed: TiltSpeed::new(0)?,
    };

    let stop_pan_fut = camera.send_async(&stop_pan);
    let stop_zoom_fut = camera.send_async(&ZoomCommand::Stop);

    let _ = tokio::join!(stop_pan_fut, stop_zoom_fut);

    println!("\nConcurrent operations demo completed!");
    Ok(())
}
