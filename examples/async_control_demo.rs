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
        methods::{FocusMethodsExt, PanTiltMethodsExt},
        profiles::G2PresetId,
        Camera,
    },
    command::pan_tilt::PanTiltDirection,
    profiles::PTZOpticsG2,
    transport::create,
    types::{FocusPosition, PanSpeed, TiltSpeed, ZoomPosition},
    units::Degrees,
    Error,
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
    let transport = create::udp(camera_addr).await?;
    let camera = Camera::<PTZOpticsG2, _>::new(transport);

    println!("\n=== Async Camera Control Demo ===\n");

    // Display camera capabilities
    let caps = camera.capabilities();
    println!("Camera Model: {}", caps.model_name);
    println!("Pan Range: {:?} degrees", caps.pan_range_degrees);
    println!("Tilt Range: {:?} degrees", caps.tilt_range_degrees);
    println!("Max Pan Speed: {}", caps.max_pan_speed);
    println!("Max Tilt Speed: {}", caps.max_tilt_speed);

    // Sequential Control Operations
    println!("\n1. Sequential Control Operations");

    println!("   - Moving to home position...");
    camera.pan_tilt_home().await?;
    sleep(Duration::from_secs(3)).await;

    println!("   - Setting up shot 1...");
    camera
        .pan_tilt_absolute(Degrees(16.0), Degrees(-4.0))
        .await?;
    camera.set_zoom(ZoomPosition::try_from(0x1800)?).await?;
    sleep(Duration::from_secs(2)).await;

    println!("   - Saving as preset 10...");
    let preset10 = G2PresetId::new(10)?;
    camera.preset_set(preset10.into()).await?;
    sleep(Duration::from_millis(500)).await;

    println!("   - Setting up shot 2...");
    camera
        .pan_tilt_absolute(Degrees(-12.0), Degrees(8.0))
        .await?;
    camera.set_zoom(ZoomPosition::try_from(0x3000)?).await?;
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
            PanSpeed::new(8)?,
            TiltSpeed::new(0)?,
        )
        .await?;

    sleep(Duration::from_secs(2)).await;

    println!("   - Starting diagonal movement...");
    camera
        .pan_tilt_move(
            PanTiltDirection::UpRight,
            PanSpeed::new(8)?,
            TiltSpeed::new(5)?,
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
    camera.set_focus(FocusPosition::try_from(0x6000)?).await?;
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
