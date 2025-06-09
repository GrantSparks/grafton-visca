// TODO: Update this example for v0.5.0 - async support is not yet available
fn main() {
    println!("This example needs to be updated for v0.5.0");
    println!("Async support is not yet available in the current version");
}

/*
//! Example demonstrating the async inquiry API with Client.

use grafton_visca::command::InquiryCommand;
use grafton_visca::{Client, Error, ViscaInquiryResponse, ViscaResponse};
use std::env;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Error> {
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
    let client = Client::connect_udp_async(camera_addr).await?;

    println!("\n=== Camera Inquiry Demo ===\n");

    // Basic camera status
    println!("1. Basic Camera Status");

    let power = client.send_async(&InquiryCommand::Power).await?;
    if let ViscaResponse::InquiryResponse(ViscaInquiryResponse::Power { on }) = power {
        println!("   - Power: {}", if on { "ON" } else { "OFF" });
    }

    println!("\n2. Camera Position and Zoom");

    let position = client.send_async(&InquiryCommand::PanTiltPosition).await?;
    if let ViscaResponse::InquiryResponse(ViscaInquiryResponse::PanTiltPosition { pan, tilt }) =
        position
    {
        println!("   - Pan/Tilt Position: pan={}, tilt={}", pan, tilt);
    }

    let zoom = client.send_async(&InquiryCommand::ZoomPosition).await?;
    if let ViscaResponse::InquiryResponse(ViscaInquiryResponse::ZoomPosition { position }) = zoom {
        println!("   - Zoom Position: {:02X?}", position);
    }

    let focus = client.send_async(&InquiryCommand::FocusPosition).await?;
    if let ViscaResponse::InquiryResponse(ViscaInquiryResponse::FocusPosition { position }) = focus
    {
        println!("   - Focus Position: {:02X?}", position);
    }

    println!("\n3. Image Settings");

    // Execute multiple inquiries concurrently for better performance
    let exposure_future = client.send_async(&InquiryCommand::ExposureMode);
    let wb_future = client.send_async(&InquiryCommand::WhiteBalanceMode);
    let luminance_future = client.send_async(&InquiryCommand::Luminance);

    let (exposure_result, wb_result, luminance_result) =
        tokio::join!(exposure_future, wb_future, luminance_future);

    if let Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::ExposureMode { mode })) =
        exposure_result
    {
        println!("   - Exposure Mode: {:?}", mode);
    }

    if let Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::WhiteBalance { mode })) =
        wb_result
    {
        println!("   - White Balance: {:?}", mode);
    }

    if let Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::Luminance(level))) =
        luminance_result
    {
        println!("   - Luminance: {}", level);
    }

    println!("\n4. Additional Image Parameters");

    let contrast = client.send_async(&InquiryCommand::Contrast).await?;
    if let ViscaResponse::InquiryResponse(ViscaInquiryResponse::Contrast(level)) = contrast {
        println!("   - Contrast: {}", level);
    }

    let gain = client.send_async(&InquiryCommand::Gain).await?;
    if let ViscaResponse::InquiryResponse(ViscaInquiryResponse::Gain { gain }) = gain {
        println!("   - Gain: {}", gain);
    }

    println!("\n5. Comprehensive Status Report");

    // Create a comprehensive status report
    let status_futures = vec![
        client.send_async(&InquiryCommand::Power),
        client.send_async(&InquiryCommand::PanTiltPosition),
        client.send_async(&InquiryCommand::ZoomPosition),
        client.send_async(&InquiryCommand::FocusPosition),
        client.send_async(&InquiryCommand::ExposureMode),
        client.send_async(&InquiryCommand::WhiteBalanceMode),
    ];

    let results = futures_util::future::try_join_all(status_futures).await?;

    println!("   Complete Camera Status:");
    for result in results {
        if let ViscaResponse::InquiryResponse(inquiry) = result {
            println!("     - {:?}", inquiry);
        }
    }

    println!("\n6. Monitoring Example (5 readings)");

    for i in 1..=5 {
        println!("   Reading #{}", i);

        let current_zoom = client.send_async(&InquiryCommand::ZoomPosition).await?;
        if let ViscaResponse::InquiryResponse(ViscaInquiryResponse::ZoomPosition { position }) =
            current_zoom
        {
            println!("     - Current Zoom: {:02X?}", position);
        }

        sleep(Duration::from_secs(1)).await;
    }

    println!("\nInquiry demo completed successfully!");
    Ok(())
}
*/
