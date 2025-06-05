//! Example demonstrating the async control API with ViscaClient.

use grafton_visca::command::{
    pan_tilt::{PanSpeed, PanTiltDirection, TiltSpeed},
    preset::{PresetAction, PresetNumber},
    FocusCommand, InquiryCommand, PanTiltCommand, PresetCommand, ZoomCommand,
};
use grafton_visca::{ViscaClient, ViscaError, ViscaResponse};
use std::env;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), ViscaError> {
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
    let client = ViscaClient::connect_udp_async(camera_addr).await?;

    println!("\n=== Async Camera Control Demo ===\n");

    // Concurrent Operations Example
    println!("1. Concurrent Operations");
    println!("   - Executing multiple queries concurrently...");

    // Start multiple operations concurrently
    let power_future = client.send_async(&InquiryCommand::Power);
    let position_future = client.send_async(&InquiryCommand::PanTiltPosition);
    let zoom_future = client.send_async(&InquiryCommand::ZoomPosition);

    let (power_result, position_result, zoom_result) =
        tokio::join!(power_future, position_future, zoom_future);

    // Handle power response
    if let Ok(ViscaResponse::InquiryResponse(grafton_visca::ViscaInquiryResponse::Power { on })) =
        power_result
    {
        println!("   - Power: {}", if on { "ON" } else { "OFF" });
    }

    // Handle position response
    if let Ok(ViscaResponse::InquiryResponse(
        grafton_visca::ViscaInquiryResponse::PanTiltPosition { pan, tilt },
    )) = position_result
    {
        println!("   - Position: pan={}, tilt={}", pan, tilt);
    }

    // Handle zoom response
    if let Ok(ViscaResponse::InquiryResponse(grafton_visca::ViscaInquiryResponse::ZoomPosition {
        position,
    })) = zoom_result
    {
        println!("   - Zoom: {:02X?}", position);
    }

    // Sequential Control Operations
    println!("\n2. Sequential Control Operations");

    println!("   - Moving to home position...");
    client.send_async(&PanTiltCommand::Home).await?;
    sleep(Duration::from_secs(3)).await;

    println!("   - Setting up shot 1...");
    client
        .send_async(&PanTiltCommand::AbsolutePosition {
            pan: 800,
            tilt: -200,
            pan_speed: PanSpeed::new(15)?,
            tilt_speed: TiltSpeed::new(15)?,
        })
        .await?;
    client.send_async(&ZoomCommand::Direct(0x1800)).await?;
    sleep(Duration::from_secs(2)).await;

    println!("   - Saving as preset 10...");
    client
        .send_async(&PresetCommand {
            action: PresetAction::Set,
            preset_number: PresetNumber::new(10)?,
        })
        .await?;
    sleep(Duration::from_millis(500)).await;

    println!("   - Setting up shot 2...");
    client
        .send_async(&PanTiltCommand::AbsolutePosition {
            pan: -600,
            tilt: 400,
            pan_speed: PanSpeed::new(10)?,
            tilt_speed: TiltSpeed::new(10)?,
        })
        .await?;
    client.send_async(&ZoomCommand::Direct(0x3000)).await?;
    sleep(Duration::from_secs(2)).await;

    println!("   - Saving as preset 11...");
    client
        .send_async(&PresetCommand {
            action: PresetAction::Set,
            preset_number: PresetNumber::new(11)?,
        })
        .await?;
    sleep(Duration::from_millis(500)).await;

    // Smooth Movement Example
    println!("\n3. Smooth Movement Sequence");

    println!("   - Starting smooth pan...");
    client
        .send_async(&PanTiltCommand::Move {
            direction: PanTiltDirection::Right,
            pan_speed: PanSpeed::new(8)?,
            tilt_speed: TiltSpeed::new(0)?,
        })
        .await?;

    sleep(Duration::from_secs(2)).await;

    println!("   - Starting diagonal movement...");
    client
        .send_async(&PanTiltCommand::Move {
            direction: PanTiltDirection::UpRight,
            pan_speed: PanSpeed::new(8)?,
            tilt_speed: TiltSpeed::new(5)?,
        })
        .await?;

    sleep(Duration::from_secs(2)).await;

    println!("   - Stopping movement...");
    client
        .send_async(&PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0)?,
            tilt_speed: TiltSpeed::new(0)?,
        })
        .await?;

    // Focus Operations
    println!("\n4. Focus Control");

    println!("   - Setting manual focus...");
    client.send_async(&FocusCommand::Manual).await?;

    println!("   - Focus operations...");
    client.send_async(&FocusCommand::FarVariable(2)).await?;
    sleep(Duration::from_secs(1)).await;
    client.send_async(&FocusCommand::Stop).await?;

    println!("   - Restoring auto focus...");
    client.send_async(&FocusCommand::Auto).await?;

    // Preset Recall Demo
    println!("\n5. Preset Recall Demo");

    for preset_num in [10, 11] {
        println!("   - Recalling preset {}...", preset_num);
        client
            .send_async(&PresetCommand {
                action: PresetAction::Recall,
                preset_number: PresetNumber::new(preset_num)?,
            })
            .await?;
        sleep(Duration::from_secs(3)).await;
    }

    println!("   - Returning to home...");
    client.send_async(&PanTiltCommand::Home).await?;

    // Clean up presets
    println!("\n6. Cleanup");
    client
        .send_async(&PresetCommand {
            action: PresetAction::Reset,
            preset_number: PresetNumber::new(10)?,
        })
        .await?;
    client
        .send_async(&PresetCommand {
            action: PresetAction::Reset,
            preset_number: PresetNumber::new(11)?,
        })
        .await?;

    println!("\nDemo completed successfully!");
    Ok(())
}
