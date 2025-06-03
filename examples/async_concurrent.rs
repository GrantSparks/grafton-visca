//! Example demonstrating concurrent async command execution with grafton-visca
//! 
//! This example shows how to:
//! - Connect to a camera using async API
//! - Send multiple commands concurrently
//! - Handle the two-socket limitation
//! - Process responses asynchronously

use grafton_visca::{AsyncViscaClient, ViscaResponse};
use grafton_visca::command::{
    PanTiltCommand, pan_tilt::{PanTiltDirection, PanSpeed, TiltSpeed, PanTiltStop},
    ZoomCommand, FocusCommand, focus::{Focus}, 
    PresetCommand, preset::Preset,
    InquiryCommand,
};
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    
    // Connect to camera (replace with your camera's IP)
    let camera_ip = std::env::var("CAMERA_IP")
        .unwrap_or_else(|_| "192.168.0.100:5678".to_string());
    
    println!("Connecting to camera at {}...", camera_ip);
    let camera = AsyncViscaClient::connect_udp(&camera_ip).await?;
    
    // Example 1: Send two commands concurrently
    println!("\n=== Concurrent Command Execution ===");
    let start = Instant::now();
    
    // Start moving the camera and zooming at the same time
    let move_fut = camera.send(&PanTiltCommand::Move {
        direction: PanTiltDirection::UpRight,
        pan_speed: PanSpeed::new(0x10)?,
        tilt_speed: TiltSpeed::new(0x10)?,
    });
    
    let zoom_fut = camera.send(&ZoomCommand::TeleStandard);
    
    // Wait for both to complete
    let (move_result, zoom_result) = tokio::join!(move_fut, zoom_fut);
    
    match move_result {
        Ok(ViscaResponse::Completion) => println!("Pan/Tilt move completed"),
        Ok(resp) => println!("Pan/Tilt move response: {:?}", resp),
        Err(e) => println!("Pan/Tilt move error: {:?}", e),
    }
    
    match zoom_result {
        Ok(ViscaResponse::Completion) => println!("Zoom completed"),
        Ok(resp) => println!("Zoom response: {:?}", resp),
        Err(e) => println!("Zoom error: {:?}", e),
    }
    
    println!("Both commands completed in {:?}", start.elapsed());
    
    // Stop movement
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    let _ = camera.send(&PanTiltStop).await;
    let _ = camera.send(&ZoomCommand::Stop).await;
    
    // Example 2: Demonstrate the two-socket limitation
    println!("\n=== Two-Socket Limitation Demo ===");
    let start = Instant::now();
    
    // Try to send three commands at once
    let cmd1 = camera.send(&PresetCommand { preset: Preset::Set { preset_number: 1 } });
    let cmd2 = camera.send(&FocusCommand { focus: Focus::Near });
    let cmd3 = camera.send(&ZoomCommand::WideStandard);
    
    println!("Sending 3 commands concurrently (only 2 will execute at once)...");
    let (res1, res2, res3) = tokio::join!(cmd1, cmd2, cmd3);
    
    println!("Command 1 (Preset Set): {:?}", res1);
    println!("Command 2 (Focus Near): {:?}", res2);
    println!("Command 3 (Zoom Wide): {:?}", res3);
    println!("All commands completed in {:?}", start.elapsed());
    
    // Example 3: Query camera status concurrently
    println!("\n=== Concurrent Status Queries ===");
    
    let zoom_pos = camera.send(&InquiryCommand::ZoomPosition);
    let focus_pos = camera.send(&InquiryCommand::FocusPosition);
    let pan_tilt_pos = camera.send(&InquiryCommand::PanTiltPosition);
    
    let (zoom_res, focus_res, pt_res) = tokio::join!(zoom_pos, focus_pos, pan_tilt_pos);
    
    if let Ok(ViscaResponse::InquiryResponse(resp)) = zoom_res {
        println!("Zoom position: {:?}", resp);
    }
    
    if let Ok(ViscaResponse::InquiryResponse(resp)) = focus_res {
        println!("Focus position: {:?}", resp);
    }
    
    if let Ok(ViscaResponse::InquiryResponse(resp)) = pt_res {
        println!("Pan/Tilt position: {:?}", resp);
    }
    
    // Example 4: Error handling with concurrent commands
    println!("\n=== Error Handling Demo ===");
    
    // Clone the client for concurrent use
    let camera_clone = camera.clone();
    
    // Spawn multiple tasks
    let task1 = tokio::spawn(async move {
        match camera_clone.send(&PanTiltCommand::Home).await {
            Ok(_) => println!("Task 1: Home command succeeded"),
            Err(e) => println!("Task 1: Home command failed: {:?}", e),
        }
    });
    
    let camera_clone = camera.clone();
    let task2 = tokio::spawn(async move {
        match camera_clone.send(&FocusCommand { focus: Focus::Auto }).await {
            Ok(_) => println!("Task 2: Auto focus succeeded"),
            Err(e) => println!("Task 2: Auto focus failed: {:?}", e),
        }
    });
    
    // Wait for all tasks
    let _ = tokio::join!(task1, task2);
    
    println!("\n=== Example completed successfully ===");
    Ok(())
}