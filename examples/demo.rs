//! Demo application showcasing grafton-visca library features
//!
//! This example demonstrates various camera control features including:
//! - Power control
//! - Pan/Tilt movement
//! - Zoom operations
//! - Focus control
//! - Exposure settings
//! - Color adjustments
//! - Inquiry commands

use grafton_visca::command::pan_tilt::{PanSpeed, TiltSpeed};
use grafton_visca::command::*;
use grafton_visca::{send_command_and_wait, UdpTransport, ViscaResponse};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Get camera IP from environment or use default
    let camera_ip = std::env::var("CAMERA_IP").unwrap_or_else(|_| "192.168.0.100:5678".to_string());

    println!(
        "🎥 Grafton VISCA Demo - Connecting to camera at {}",
        camera_ip
    );
    println!("{}", "=".repeat(50));

    // Create transport
    let mut transport = UdpTransport::new(&camera_ip)?;

    // Demo 1: Power Control
    println!("\n📍 Demo 1: Power Control");
    println!("Powering on camera...");
    match send_command_and_wait(&mut transport, &PowerCommand { power: Power::On })? {
        ViscaResponse::Completion => println!("✅ Camera powered on successfully"),
        ViscaResponse::Error(e) => println!("❌ Power on error: {:?}", e),
        _ => println!("⚠️  Unexpected response"),
    }
    thread::sleep(Duration::from_secs(2));

    // Demo 2: Pan/Tilt Movement
    println!("\n📍 Demo 2: Pan/Tilt Movement");

    println!("Moving to home position...");
    send_command_and_wait(&mut transport, &PanTiltCommand::Home)?;
    thread::sleep(Duration::from_secs(2));

    println!("Moving camera up-right...");
    let move_cmd = PanTiltCommand::Move {
        direction: PanTiltDirection::UpRight,
        pan_speed: PanSpeed::new(0x10)?,
        tilt_speed: TiltSpeed::new(0x10)?,
    };
    send_command_and_wait(&mut transport, &move_cmd)?;
    thread::sleep(Duration::from_secs(1));

    println!("Stopping movement...");
    let stop_cmd = PanTiltCommand::Move {
        direction: PanTiltDirection::Stop,
        pan_speed: PanSpeed::new(0)?,
        tilt_speed: TiltSpeed::new(0)?,
    };
    send_command_and_wait(&mut transport, &stop_cmd)?;

    // Demo 3: Zoom Control
    println!("\n📍 Demo 3: Zoom Control");

    println!("Zooming in...");
    send_command_and_wait(&mut transport, &ZoomCommand::TeleStandard)?;
    thread::sleep(Duration::from_secs(1));

    send_command_and_wait(&mut transport, &ZoomCommand::Stop)?;

    println!("Zooming out...");
    send_command_and_wait(&mut transport, &ZoomCommand::WideStandard)?;
    thread::sleep(Duration::from_secs(1));

    send_command_and_wait(&mut transport, &ZoomCommand::Stop)?;

    // Demo 4: Focus Control
    println!("\n📍 Demo 4: Focus Control");

    println!("Setting auto focus...");
    send_command_and_wait(&mut transport, &FocusCommand::Auto)?;
    thread::sleep(Duration::from_secs(1));

    // Demo 5: Preset Positions
    println!("\n📍 Demo 5: Preset Positions");

    println!("Saving current position as preset 1...");
    let preset_set = PresetCommand {
        action: PresetAction::Set,
        preset_number: 1,
    };
    send_command_and_wait(&mut transport, &preset_set)?;

    thread::sleep(Duration::from_secs(1));

    println!("Moving camera to a different position...");
    let move_cmd = PanTiltCommand::Move {
        direction: PanTiltDirection::DownLeft,
        pan_speed: PanSpeed::new(0x10)?,
        tilt_speed: TiltSpeed::new(0x10)?,
    };
    send_command_and_wait(&mut transport, &move_cmd)?;
    thread::sleep(Duration::from_secs(1));
    send_command_and_wait(&mut transport, &stop_cmd)?;

    println!("Recalling preset 1...");
    let preset_recall = PresetCommand {
        action: PresetAction::Recall,
        preset_number: 1,
    };
    send_command_and_wait(&mut transport, &preset_recall)?;
    thread::sleep(Duration::from_secs(2));

    // Demo 6: Exposure Control
    println!("\n📍 Demo 6: Exposure Control");

    println!("Setting manual exposure mode...");
    send_command_and_wait(
        &mut transport,
        &ExposureCommand {
            mode: ExposureMode::Manual,
        },
    )?;

    println!("Adjusting iris to F4.0...");
    send_command_and_wait(&mut transport, &IrisCommand::Direct(0x06))?;

    println!("Setting shutter speed...");
    send_command_and_wait(&mut transport, &ShutterCommand::Direct(0x0A))?;

    // Demo 7: Color Adjustments
    println!("\n📍 Demo 7: Color Adjustments");

    println!("Setting white balance to auto...");
    send_command_and_wait(
        &mut transport,
        &WhiteBalanceCommand {
            mode: WhiteBalanceMode::Auto,
        },
    )?;

    println!("Adjusting saturation...");
    send_command_and_wait(&mut transport, &SaturationCommand { level: 0x08 })?;

    println!("Adjusting hue...");
    send_command_and_wait(&mut transport, &HueCommand { level: 0x07 })?;

    // Demo 8: Image Quality Settings
    println!("\n📍 Demo 8: Image Quality Settings");

    println!("Setting luminance...");
    send_command_and_wait(&mut transport, &LuminanceCommand { value: 0x08 })?;

    println!("Setting contrast...");
    send_command_and_wait(&mut transport, &ContrastCommand { value: 0x08 })?;

    println!("Setting sharpness...");
    send_command_and_wait(&mut transport, &SharpnessCommand::Direct { value: 0x08 })?;

    // Demo 9: Inquiry Commands
    println!("\n📍 Demo 9: Inquiry Commands");

    println!("Querying camera status...");

    // Query zoom position
    match send_command_and_wait(&mut transport, &InquiryCommand::ZoomPosition)? {
        ViscaResponse::InquiryResponse(ViscaInquiryResponse::ZoomPosition { position }) => {
            println!("  Zoom position: 0x{:04X}", position);
        }
        _ => println!("  Failed to get zoom position"),
    }

    // Query pan/tilt position
    match send_command_and_wait(&mut transport, &InquiryCommand::PanTiltPosition)? {
        ViscaResponse::InquiryResponse(ViscaInquiryResponse::PanTiltPosition { pan, tilt }) => {
            println!("  Pan/Tilt position: Pan={}, Tilt={}", pan, tilt);
        }
        _ => println!("  Failed to get pan/tilt position"),
    }

    // Query exposure mode
    match send_command_and_wait(&mut transport, &InquiryCommand::ExposureMode)? {
        ViscaResponse::InquiryResponse(ViscaInquiryResponse::ExposureMode { mode }) => {
            println!("  Exposure mode: {:?}", mode);
        }
        _ => println!("  Failed to get exposure mode"),
    }

    // Query white balance mode
    match send_command_and_wait(&mut transport, &InquiryCommand::WhiteBalanceMode)? {
        ViscaResponse::InquiryResponse(ViscaInquiryResponse::WhiteBalance { mode }) => {
            println!("  White balance mode: {:?}", mode);
        }
        _ => println!("  Failed to get white balance mode"),
    }

    // Demo 10: Advanced Features
    println!("\n📍 Demo 10: Advanced Features");

    println!("Setting 2D noise reduction...");
    send_command_and_wait(&mut transport, &NoiseReduction2DCommand::Level(3))?;

    println!("Setting backlight compensation...");
    send_command_and_wait(&mut transport, &BacklightCommand { status: true })?;

    println!("Setting image flip (horizontal)...");
    send_command_and_wait(
        &mut transport,
        &ImageFlipCombinedCommand {
            mode: ImageFlipMode::Horizontal,
        },
    )?;
    thread::sleep(Duration::from_secs(1));

    println!("Resetting image flip...");
    send_command_and_wait(
        &mut transport,
        &ImageFlipCombinedCommand {
            mode: ImageFlipMode::Off,
        },
    )?;

    // Return to home position
    println!("\n🏁 Demo complete! Returning to home position...");
    send_command_and_wait(&mut transport, &PanTiltCommand::Home)?;

    println!("\n✨ All demos completed successfully!");
    println!("{}", "=".repeat(50));

    Ok(())
}
