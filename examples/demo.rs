use grafton_visca::command::color::{HueCommand, SaturationCommand};
use grafton_visca::command::exposure::{
    ExposureCommand, ExposureMode, IrisCommand, ShutterCommand,
};
use grafton_visca::command::focus::FocusCommand;
use grafton_visca::command::inquiry::InquiryCommand;
use grafton_visca::command::luminance_contrast_sharpness::{
    ContrastCommand, LuminanceCommand, SharpnessCommand,
};
use grafton_visca::command::pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed};
use grafton_visca::command::power::{Power, PowerCommand};
use grafton_visca::command::preset::{PresetAction, PresetCommand, PresetNumber};
use grafton_visca::command::white_balance::{WhiteBalanceCommand, WhiteBalanceMode};
use grafton_visca::command::zoom::ZoomCommand;
use grafton_visca::command::{
    BacklightCommand, ImageFlipCombinedCommand, ImageFlipMode, NoiseReduction2DCommand,
};
use grafton_visca::{ContrastLevel, IrisLevel, LuminanceLevel, ShutterSpeed};
use grafton_visca::{Client, Transport, ViscaInquiryResponse, Response};
use std::thread;
use std::time::Duration;

fn demo_power_control(client: &mut Client) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📍 Demo 1: Power Control");
    println!("Powering on camera...");
    match client.execute_command(&PowerCommand { power: Power::On })? {
        Response::Completion => println!("✅ Camera powered on successfully"),
        Response::Error(e) => println!("❌ Power on error: {e:?}"),
        _ => println!("⚠️  Unexpected response"),
    }
    thread::sleep(Duration::from_secs(2));
    Ok(())
}

fn demo_pan_tilt_movement(client: &mut Client) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📍 Demo 2: Pan/Tilt Movement");

    println!("Moving to home position...");
    client.execute_command(&PanTiltCommand::Home)?;
    thread::sleep(Duration::from_secs(2));

    println!("Moving camera up-right...");
    let move_cmd = PanTiltCommand::Move {
        direction: PanTiltDirection::UpRight,
        pan_speed: PanSpeed::new(0x10)?,
        tilt_speed: TiltSpeed::new(0x10)?,
    };
    client.execute_command(&move_cmd)?;
    thread::sleep(Duration::from_secs(1));

    println!("Stopping movement...");
    let stop_cmd = PanTiltCommand::Move {
        direction: PanTiltDirection::Stop,
        pan_speed: PanSpeed::new(0)?,
        tilt_speed: TiltSpeed::new(0)?,
    };
    client.execute_command(&stop_cmd)?;
    Ok(())
}

fn demo_zoom_control(client: &mut Client) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📍 Demo 3: Zoom Control");

    println!("Zooming in...");
    client.execute_command(&ZoomCommand::TeleStandard)?;
    thread::sleep(Duration::from_secs(1));

    client.execute_command(&ZoomCommand::Stop)?;

    println!("Zooming out...");
    client.execute_command(&ZoomCommand::WideStandard)?;
    thread::sleep(Duration::from_secs(1));

    client.execute_command(&ZoomCommand::Stop)?;
    Ok(())
}

fn demo_focus_control(client: &mut Client) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📍 Demo 4: Focus Control");

    println!("Setting auto focus...");
    client.execute_command(&FocusCommand::Auto)?;
    thread::sleep(Duration::from_secs(1));
    Ok(())
}

fn demo_preset_positions(client: &mut Client) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📍 Demo 5: Preset Positions");

    println!("Saving current position as preset 1...");
    let preset_set = PresetCommand {
        action: PresetAction::Set,
        preset_number: PresetNumber::new(1)?,
    };
    client.execute_command(&preset_set)?;

    thread::sleep(Duration::from_secs(1));

    println!("Moving camera to a different position...");
    let move_cmd = PanTiltCommand::Move {
        direction: PanTiltDirection::DownLeft,
        pan_speed: PanSpeed::new(0x10)?,
        tilt_speed: TiltSpeed::new(0x10)?,
    };
    client.execute_command(&move_cmd)?;
    thread::sleep(Duration::from_secs(1));

    let stop_cmd = PanTiltCommand::Move {
        direction: PanTiltDirection::Stop,
        pan_speed: PanSpeed::new(0)?,
        tilt_speed: TiltSpeed::new(0)?,
    };
    client.execute_command(&stop_cmd)?;

    println!("Recalling preset 1...");
    let preset_recall = PresetCommand {
        action: PresetAction::Recall,
        preset_number: PresetNumber::new(1)?,
    };
    client.execute_command(&preset_recall)?;
    thread::sleep(Duration::from_secs(2));
    Ok(())
}

fn demo_exposure_control(client: &mut Client) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📍 Demo 6: Exposure Control");

    println!("Setting manual exposure mode...");
    client.execute_command(&ExposureCommand {
        mode: ExposureMode::Manual,
    })?;

    println!("Adjusting iris to F4.0...");
    client.execute_command(&IrisCommand::Direct(IrisLevel::new(0x06).unwrap()))?;

    println!("Setting shutter speed...");
    client.execute_command(&ShutterCommand::Direct(ShutterSpeed::new(0x0A).unwrap()))?;
    Ok(())
}

fn demo_color_adjustments(client: &mut Client) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📍 Demo 7: Color Adjustments");

    println!("Setting white balance to auto...");
    client.execute_command(&WhiteBalanceCommand {
        mode: WhiteBalanceMode::Auto,
    })?;

    println!("Adjusting saturation...");
    client.execute_command(&SaturationCommand { level: 0x08 })?;

    println!("Adjusting hue...");
    client.execute_command(&HueCommand { level: 0x07 })?;
    Ok(())
}

fn demo_image_quality(client: &mut Client) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📍 Demo 8: Image Quality Settings");

    println!("Setting luminance...");
    client.execute_command(&LuminanceCommand {
        value: LuminanceLevel::new(0x08).unwrap(),
    })?;

    println!("Setting contrast...");
    client.execute_command(&ContrastCommand {
        value: ContrastLevel::new(0x08).unwrap(),
    })?;

    println!("Setting sharpness...");
    client.execute_command(&SharpnessCommand::Direct { value: 0x08 })?;
    Ok(())
}

fn demo_inquiry_commands(client: &mut Client) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📍 Demo 9: Inquiry Commands");

    println!("Querying camera status...");

    // Query zoom position
    match client.execute_command(&InquiryCommand::ZoomPosition)? {
        Response::InquiryResponse(ViscaInquiryResponse::ZoomPosition { position }) => {
            println!("  Zoom position: 0x{position:04X}");
        }
        _ => println!("  Failed to get zoom position"),
    }

    // Query pan/tilt position
    match client.execute_command(&InquiryCommand::PanTiltPosition)? {
        Response::InquiryResponse(ViscaInquiryResponse::PanTiltPosition { pan, tilt }) => {
            println!("  Pan/Tilt position: Pan={pan}, Tilt={tilt}");
        }
        _ => println!("  Failed to get pan/tilt position"),
    }

    // Query exposure mode
    match client.execute_command(&InquiryCommand::ExposureMode)? {
        Response::InquiryResponse(ViscaInquiryResponse::ExposureMode { mode }) => {
            println!("  Exposure mode: {mode:?}");
        }
        _ => println!("  Failed to get exposure mode"),
    }

    // Query white balance mode
    match client.execute_command(&InquiryCommand::WhiteBalanceMode)? {
        Response::InquiryResponse(ViscaInquiryResponse::WhiteBalance { mode }) => {
            println!("  White balance mode: {mode:?}");
        }
        _ => println!("  Failed to get white balance mode"),
    }
    Ok(())
}

fn demo_advanced_features(client: &mut Client) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📍 Demo 10: Advanced Features");

    println!("Setting 2D noise reduction...");
    client.execute_command(&NoiseReduction2DCommand::Level(
        grafton_visca::NoiseReduction2DLevel::new(3).unwrap(),
    ))?;

    println!("Setting backlight compensation...");
    client.execute_command(&BacklightCommand { status: true })?;

    println!("Setting image flip (horizontal)...");
    client.execute_command(&ImageFlipCombinedCommand {
        mode: ImageFlipMode::Horizontal,
    })?;
    thread::sleep(Duration::from_secs(1));

    println!("Resetting image flip...");
    client.execute_command(&ImageFlipCombinedCommand {
        mode: ImageFlipMode::Off,
    })?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Get camera IP from environment or use default
    let camera_ip = std::env::var("CAMERA_IP").unwrap_or_else(|_| "192.168.0.100:5678".to_string());

    println!("🎥 Grafton VISCA Demo - Connecting to camera at {camera_ip}");
    println!("{}", "=".repeat(50));

    // Create client
    let mut client = Client::connect_udp(&camera_ip)?;

    // Run all demos
    demo_power_control(&mut client)?;
    demo_pan_tilt_movement(&mut client)?;
    demo_zoom_control(&mut client)?;
    demo_focus_control(&mut client)?;
    demo_preset_positions(&mut client)?;
    demo_exposure_control(&mut client)?;
    demo_color_adjustments(&mut client)?;
    demo_image_quality(&mut client)?;
    demo_inquiry_commands(&mut client)?;
    demo_advanced_features(&mut client)?;

    // Return to home position
    println!("\n🏁 Demo complete! Returning to home position...");
    client.execute_command(&PanTiltCommand::Home)?;

    println!("\n✨ All demos completed successfully!");
    println!("{}", "=".repeat(50));

    Ok(())
}
