//! PTZ builder pattern demonstration using the new Camera API
//!
//! This example demonstrates the type-safe `Camera<P>` API introduced
//! in the camera profiles feature, showcasing:
//! - Type-safe position units (Degrees, ViscaUnits, Normalized)
//! - Camera profile-aware methods
//! - Sequential command execution patterns
//! - Builder-like patterns for complex operations
//!
//! Note: While the original PTZ builder (Client::ptz()) is no longer available
//! in the new API, this example shows how to achieve similar functionality
//! using the `Camera<P>` API with helper functions and sequential operations.

#[cfg(not(feature = "async"))]
use grafton_visca::transport::blocking::Udp;

#[cfg(not(feature = "async"))]
use grafton_visca::{
    prelude::blocking::*,
    types::{FocusPosition, PanSpeed, SpeedLevel, TiltSpeed},
    units::{Degrees, Normalized},
    PanTiltDirection, PresetNumber,
};

#[cfg(not(feature = "async"))]
use std::thread;
#[cfg(not(feature = "async"))]
use std::time::Duration;

#[cfg(all(feature = "async", not(feature = "tokio")))]
fn main() {
    eprintln!("This example requires the 'tokio' feature to be enabled.");
    eprintln!("Run with: cargo run --example ptz_builder_demo --features tokio");
}

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== PTZ Builder Pattern Demo with Camera<P> API ===");

    // Create a type-safe camera instance with PTZOpticsG2 profile
    let camera_ip = std::env::var("CAMERA_IP").unwrap_or_else(|_| "192.168.1.100:5678".to_string());
    let transport = Udp::connect(&camera_ip)?;
    let mut camera = PTZOpticsG2Cam::new(transport);
    println!("Connected to PTZOptics G2 camera");

    // Demonstrate sequential command execution with type-safe units
    println!("\n1. Type-safe position control with Degrees");
    camera.pan_tilt_home()?;
    println!("   ✓ Moved to home position");
    thread::sleep(Duration::from_secs(2));

    // Set position using degrees - compiler enforces correct units
    camera.pan_tilt_absolute(Degrees::new(45.0), Degrees::new(-15.0), SpeedLevel::Medium)?;
    println!("   ✓ Set position to pan=45°, tilt=-15°");
    thread::sleep(Duration::from_secs(2));

    // Demonstrate complex movement sequence
    println!("\n2. Sequential movement operations");

    // Move to preset position first
    camera.pan_tilt_home()?;
    thread::sleep(Duration::from_secs(1));

    // Continuous movement
    camera.pan_tilt_move(
        PanTiltDirection::UpRight,
        PanSpeed::new(12)?,
        TiltSpeed::new(10)?,
    )?;
    thread::sleep(Duration::from_millis(500));
    camera.pan_tilt_stop()?;
    println!("   ✓ Executed: Home → Move UpRight → Stop");

    // Zoom operations
    camera.zoom_absolute(Normalized(0.5))?;
    println!("   ✓ Set zoom to 50%");
    thread::sleep(Duration::from_secs(1));

    // Focus control
    camera.focus_manual()?;
    let focus_pos = FocusPosition::try_from(Normalized(0.5))?;
    camera.set_focus(focus_pos)?;
    println!("   ✓ Set manual focus to 50%");

    // Demonstrate absolute positioning with validation
    println!("\n3. Profile-aware absolute positioning");

    // The `Camera<PTZOpticsG2>` knows the valid ranges for this model
    let pan_degrees = 90.0; // PTZOpticsG2 supports ±170°
    let tilt_degrees = 30.0; // PTZOpticsG2 supports -30° to +90°

    match camera.pan_tilt_absolute(
        Degrees::new(pan_degrees),
        Degrees::new(tilt_degrees),
        SpeedLevel::Medium,
    ) {
        Ok(_) => println!("   ✓ Set position to pan={pan_degrees}°, tilt={tilt_degrees}°"),
        Err(e) => println!("   ✗ Position out of range: {e}"),
    }
    thread::sleep(Duration::from_secs(2));

    // Demonstrate preset operations (type-safe preset IDs)
    println!("\n4. Type-safe preset operations");

    // Save current position to preset
    let preset_id = 1;
    camera.preset_set(PresetNumber::new(preset_id)?)?;
    println!("   ✓ Saved current position to preset {preset_id}");

    // Move to a different position
    camera.pan_tilt_absolute(Degrees::new(-45.0), Degrees::new(0.0), SpeedLevel::Medium)?;
    thread::sleep(Duration::from_secs(2));

    // Recall the saved preset
    camera.preset_recall(PresetNumber::new(preset_id)?)?;
    println!("   ✓ Recalled preset {preset_id}");
    thread::sleep(Duration::from_secs(2));

    // Demonstrate capability queries
    println!("\n5. Camera capability summary");
    println!("   Model: PTZOptics G2");
    println!("   Pan range: -170 to +170 degrees");
    println!("   Tilt range: -30 to +90 degrees");
    println!("   Zoom steps: 0x0000 to 0x4000");
    println!("   Max pan speed: 24");
    println!("   Max tilt speed: 20");

    // Demonstrate builder-like pattern for complex operations
    println!("\n6. Builder-like pattern for complex sequences");

    // While the new API doesn't have a PTZ builder per se, you can
    // create helper functions that chain operations
    perform_scan_sequence(&mut camera)?;
    println!("   ✓ Completed scan sequence");

    println!("\n=== Camera<P> API demo completed! ===");
    Ok(())
}

// Helper function demonstrating sequential command patterns
#[cfg(not(feature = "async"))]
fn perform_scan_sequence<P, T>(
    camera: &mut grafton_visca::Camera<P, T>,
) -> Result<(), Box<dyn std::error::Error>>
where
    P: grafton_visca::capabilities::Profile,
    T: grafton_visca::transport::UnifiedTransport,
{
    // Return to home
    camera.pan_tilt_home()?;
    thread::sleep(Duration::from_secs(1));

    // Scan left
    camera.pan_tilt_move(
        PanTiltDirection::Left,
        PanSpeed::new(8)?,
        TiltSpeed::new(0)?,
    )?;
    thread::sleep(Duration::from_secs(2));
    camera.pan_tilt_stop()?;

    // Scan right
    camera.pan_tilt_move(
        PanTiltDirection::Right,
        PanSpeed::new(8)?,
        TiltSpeed::new(0)?,
    )?;
    thread::sleep(Duration::from_secs(4));
    camera.pan_tilt_stop()?;

    // Return to center
    camera.pan_tilt_absolute(Degrees::new(0.0), Degrees::new(0.0), SpeedLevel::Medium)?;

    Ok(())
}

#[cfg(feature = "tokio")]
use grafton_visca::{
    camera::profiles::G2PresetId,
    capabilities::Profile,
    prelude::r#async::PTZOpticsG2Cam,
    r#async::prelude::*,
    transport::tokio::Udp,
    transport::UnifiedTransport,
    types::{PanSpeed, SpeedLevel, TiltSpeed},
    units::{Degrees, Normalized},
    Camera, PanTiltDirection, PresetNumber,
};

#[cfg(feature = "tokio")]
use std::time::Duration;

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== Async PTZ Pattern Demo with Camera<P> API ===");

    // Create async camera with type-safe profile
    let transport = Udp::connect("192.168.1.100:52381").await?;
    let camera = PTZOpticsG2Cam::new(transport);
    println!("Connected to PTZOptics G2 camera (async)");

    // Demonstrate async operations with type-safe units
    println!("\n1. Async type-safe position control");
    camera.pan_tilt_home().await?;
    println!("   ✓ Moved to home position");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Async position control with degrees
    camera
        .pan_tilt_absolute(Degrees::new(45.0), Degrees::new(-15.0), SpeedLevel::Medium)
        .await?;
    println!("   ✓ Set position to pan=45°, tilt=-15°");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Demonstrate async sequential operations
    println!("\n2. Async sequential operations");
    perform_async_scan_sequence(&camera).await?;
    println!("   ✓ Completed async scan sequence");

    // Demonstrate complex sequential operations
    println!("\n3. Complex sequential operations");

    // Sequential operations that would typically be concurrent
    // Start moving and zooming (simulating concurrent behavior)
    camera
        .pan_tilt_move(
            PanTiltDirection::Right,
            PanSpeed::new(8)?,
            TiltSpeed::new(0)?,
        )
        .await?;
    camera.zoom_in().await?;
    println!("   → Started movement and zoom");

    // Let both operations run
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Stop both operations
    camera.pan_tilt_stop().await?;
    camera.zoom_stop().await?;
    println!("   ✓ Completed movement and zoom sequence");

    // Demonstrate preset patrol pattern
    println!("\n4. Async preset patrol");

    // Save positions with G2-specific preset IDs
    camera
        .pan_tilt_absolute(Degrees::new(-80.0), Degrees::new(0.0), SpeedLevel::Medium)
        .await?;
    let preset1 = G2PresetId::new(1)?;
    camera
        .preset_set(PresetNumber::new(preset1.into())?)
        .await?;

    camera
        .pan_tilt_absolute(Degrees::new(0.0), Degrees::new(45.0), SpeedLevel::Medium)
        .await?;
    let preset2 = G2PresetId::new(2)?;
    camera
        .preset_set(PresetNumber::new(preset2.into())?)
        .await?;

    camera
        .pan_tilt_absolute(Degrees::new(80.0), Degrees::new(0.0), SpeedLevel::Medium)
        .await?;
    let preset3 = G2PresetId::new(3)?;
    camera
        .preset_set(PresetNumber::new(preset3.into())?)
        .await?;

    // Patrol between presets
    for _ in 0..2 {
        for preset in &[preset1, preset2, preset3] {
            camera
                .preset_recall(PresetNumber::new((*preset).into())?)
                .await?;
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
    println!("   ✓ Completed preset patrol");

    // Reset to neutral
    println!("\n5. Reset to neutral position");
    camera.pan_tilt_home().await?;
    camera.zoom_absolute(Normalized::new(0.0)).await?;
    camera.focus_auto().await?;
    println!("   ✓ Reset camera to neutral state");

    println!("\n=== Async Camera<P> API demo completed! ===");
    Ok(())
}

// Async helper function for scan sequence
#[cfg(feature = "tokio")]
async fn perform_async_scan_sequence<P, T>(
    camera: &Camera<P, T>,
) -> Result<(), Box<dyn std::error::Error>>
where
    P: Profile,
    T: UnifiedTransport,
{
    // Return to home
    camera.pan_tilt_home().await?;
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Scan pattern with position feedback
    let scan_positions = vec![-90.0, -45.0, 0.0, 45.0, 90.0, 0.0];

    for pan_pos in scan_positions {
        camera
            .pan_tilt_absolute(Degrees::new(pan_pos), Degrees::new(0.0), SpeedLevel::Medium)
            .await?;
        println!("      → Scanning at pan={pan_pos}°");
        tokio::time::sleep(Duration::from_millis(800)).await;
    }

    Ok(())
}
