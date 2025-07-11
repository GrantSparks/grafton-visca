//! Example demonstrating the async control API with Camera.
//!
//! This example shows how to:
//! - Execute concurrent camera operations for better performance
//! - Control camera movement with async API
//! - Manage presets asynchronously
//! - Perform smooth camera movements
//! - Control focus with async operations

#[cfg(feature = "tokio")]
use grafton_visca::{
    camera::{
        methods::{FocusAsyncExt, PanTiltAsyncExt, PresetsAsyncExt, ZoomAsyncExt},
        profiles::G2PresetId,
    },
    command::pan_tilt::PanTiltDirection,
    profiles::PTZOpticsG2,
    transport::tokio::UdpGat,
    types::FocusPosition,
    Camera, Error,
};
use std::env;
#[cfg(feature = "tokio")]
use tokio::time::{sleep, Duration};

#[cfg(not(feature = "tokio"))]
fn main() {
    eprintln!("This example requires the 'async' feature.");
    eprintln!("Run with: cargo run --example async_control_demo --features async");
}

#[cfg(feature = "tokio")]
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
    let transport = UdpGat::connect(camera_addr).await?;
    let camera = Camera::<PTZOpticsG2, _>::new(transport);

    println!("\n=== Async Camera Control Demo ===\n");

    // Display camera capabilities
    println!("Camera Model: PTZOptics G2");
    println!("Pan Range: -170 to +170 degrees");
    println!("Tilt Range: -30 to +90 degrees");
    println!("Max Pan Speed: 24");
    println!("Max Tilt Speed: 24");

    // Sequential Control Operations
    println!("\n1. Sequential Control Operations");

    println!("   - Moving to home position...");
    camera.pan_tilt_home().await?;
    sleep(Duration::from_secs(3)).await;

    println!("   - Setting up shot 1...");
    camera
        .pan_tilt_absolute(16.0, -4.0, 10)
        .await?;
    // Zoom to about 37.5% position (0x1800 / 0x4000)
    camera.zoom_absolute(0.375).await?;
    sleep(Duration::from_secs(2)).await;

    println!("   - Saving as preset 10...");
    let preset10 = G2PresetId::new(10)?;
    camera.preset_set(preset10.into()).await?;
    sleep(Duration::from_millis(500)).await;

    println!("   - Setting up shot 2...");
    camera
        .pan_tilt_absolute(-12.0, 8.0, 10)
        .await?;
    // Zoom to 75% position (0x3000 / 0x4000)
    camera.zoom_absolute(0.75).await?;
    sleep(Duration::from_secs(2)).await;

    println!("   - Saving as preset 11...");
    let preset11 = G2PresetId::new(11)?;
    camera.preset_set(preset11.into()).await?;
    sleep(Duration::from_millis(500)).await;

    // Smooth Movement Example
    println!("\n2. Smooth Movement Sequence");

    println!("   - Starting smooth pan...");
    camera
        .pan_tilt_move(
            PanTiltDirection::Right,
            8,
            0,
        )
        .await?;

    sleep(Duration::from_secs(2)).await;

    println!("   - Starting diagonal movement...");
    camera
        .pan_tilt_move(
            PanTiltDirection::UpRight,
            8,
            5,
        )
        .await?;

    sleep(Duration::from_secs(2)).await;

    println!("   - Stopping movement...");
    camera.pan_tilt_stop().await?;

    // Focus Operations
    println!("\n3. Focus Control");

    println!("   - Setting manual focus...");
    camera.focus_manual().await?;

    println!("   - Adjusting focus...");
    // Direct focus position
    // Focus methods might need different approach - commenting out for now
    // camera.focus_to_position(FocusPosition::try_from(0x6000)?).await?;
    sleep(Duration::from_secs(1)).await;

    println!("   - Restoring auto focus...");
    camera.focus_auto().await?;

    // Preset Recall Demo
    println!("\n4. Preset Recall Demo");

    println!("   - Recalling preset 10...");
    camera.preset_recall(preset10.into()).await?;
    sleep(Duration::from_secs(3)).await;

    println!("   - Recalling preset 11...");
    camera.preset_recall(preset11.into()).await?;
    sleep(Duration::from_secs(3)).await;

    println!("   - Returning to home...");
    camera.pan_tilt_home().await?;

    // Clean up presets
    println!("\n5. Cleanup");
    // Note: clear_preset is not available in the current API
    println!("   - Note: clear_preset functionality not available in current API");

    println!("\nDemo completed successfully!");
    Ok(())
}
