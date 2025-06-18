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

#[cfg(not(feature = "async-client"))]
use grafton_visca::{
    camera::{
        profiles::{G2PresetId, PTZOpticsG2},
        units::Degrees,
        Camera,
    },
    command::pan_tilt::PanTiltDirection,
    transport::{BlockingAdapter, UdpTransport},
};

#[cfg(not(feature = "async-client"))]
use std::thread;

#[cfg(not(feature = "async-client"))]
use std::time::Duration;

// Use a minimal tokio runtime for blocking execution
#[cfg(not(feature = "async-client"))]
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(fut)
}

#[cfg(not(feature = "async-client"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== PTZ Builder Pattern Demo with Camera<P> API ===");

    // Create a type-safe camera instance with PTZOpticsG2 profile
    let udp_transport = UdpTransport::new("192.168.1.100:52381")?;
    let transport = BlockingAdapter(udp_transport);
    let mut camera = Camera::<PTZOpticsG2>::new(transport);
    println!("Connected to PTZOptics G2 camera");

    // Demonstrate sequential command execution with type-safe units
    println!("\n1. Type-safe position control with Degrees");
    block_on(camera.home())?;
    println!("   ✓ Moved to home position");
    thread::sleep(Duration::from_secs(2));

    // Set position using degrees - compiler enforces correct units
    block_on(camera.set_position(Degrees(45.0), Degrees(-15.0)))?;
    println!("   ✓ Set position to pan=45°, tilt=-15°");
    thread::sleep(Duration::from_secs(2));

    // Demonstrate complex movement sequence
    println!("\n2. Sequential movement operations");

    // Move to preset position first
    block_on(camera.home())?;
    thread::sleep(Duration::from_secs(1));

    // Continuous movement
    block_on(camera.move_continuous(PanTiltDirection::UpRight, 12, 10))?;
    thread::sleep(Duration::from_millis(500));
    block_on(camera.stop())?;
    println!("   ✓ Executed: Home → Move UpRight → Stop");

    // Zoom operations
    block_on(camera.set_zoom(0x4000))?;
    println!("   ✓ Set zoom to 0x4000");
    thread::sleep(Duration::from_secs(1));

    // Focus control
    block_on(camera.focus_manual())?;
    block_on(camera.set_focus(0x8000))?;
    println!("   ✓ Set manual focus to 0x8000");

    // Demonstrate absolute positioning with validation
    println!("\n3. Profile-aware absolute positioning");

    // The `Camera<PTZOpticsG2>` knows the valid ranges for this model
    let pan_degrees = 90.0; // PTZOpticsG2 supports ±170°
    let tilt_degrees = 30.0; // PTZOpticsG2 supports -30° to +90°

    match block_on(camera.set_position(Degrees(pan_degrees), Degrees(tilt_degrees))) {
        Ok(_) => println!(
            "   ✓ Set position to pan={}°, tilt={}°",
            pan_degrees, tilt_degrees
        ),
        Err(e) => println!("   ✗ Position out of range: {}", e),
    }
    thread::sleep(Duration::from_secs(2));

    // Demonstrate preset operations (type-safe preset IDs)
    println!("\n4. Type-safe preset operations");

    // Save current position to preset using G2-specific preset ID
    let preset_id = G2PresetId::new(1)?;
    block_on(camera.set_preset(preset_id))?;
    println!("   ✓ Saved current position to preset {}", preset_id);

    // Move to a different position
    block_on(camera.set_position(Degrees(-45.0), Degrees(0.0)))?;
    thread::sleep(Duration::from_secs(2));

    // Recall the saved preset
    block_on(camera.recall_preset(preset_id))?;
    println!("   ✓ Recalled preset {}", preset_id);
    thread::sleep(Duration::from_secs(2));

    // Demonstrate capability queries
    println!("\n5. Camera capability summary");
    let capabilities = camera.capabilities();
    println!("   Model: {}", capabilities.model_name);
    println!("   Pan range: {:?} degrees", capabilities.pan_range_degrees);
    println!(
        "   Tilt range: {:?} degrees",
        capabilities.tilt_range_degrees
    );
    println!("   Zoom steps: {}", capabilities.zoom_steps);
    println!("   Max pan speed: {}", capabilities.max_pan_speed);
    println!("   Max tilt speed: {}", capabilities.max_tilt_speed);
    println!("   Digital zoom: {}", capabilities.supports_digital_zoom);

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
#[cfg(not(feature = "async-client"))]
fn perform_scan_sequence(
    camera: &mut Camera<PTZOpticsG2>,
) -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::camera::units::Degrees;

    // Return to home
    block_on(camera.home())?;
    thread::sleep(Duration::from_secs(1));

    // Scan left
    block_on(camera.move_continuous(PanTiltDirection::Left, 8, 0))?;
    thread::sleep(Duration::from_secs(2));
    block_on(camera.stop())?;

    // Scan right
    block_on(camera.move_continuous(PanTiltDirection::Right, 8, 0))?;
    thread::sleep(Duration::from_secs(4));
    block_on(camera.stop())?;

    // Return to center
    block_on(camera.set_position(Degrees(0.0), Degrees(0.0)))?;

    Ok(())
}

#[cfg(feature = "async-client")]
use grafton_visca::{
    camera::{
        profiles::{G2PresetId, PTZOpticsG2},
        units::Degrees,
        Camera,
    },
    command::pan_tilt::PanTiltDirection,
    transport::AsyncUdpTransport,
};

#[cfg(feature = "async-client")]
use std::time::Duration;

#[cfg(feature = "async-client")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== Async PTZ Pattern Demo with Camera<P> API ===");

    // Create async camera with type-safe profile
    let transport = AsyncUdpTransport::new("192.168.1.100:52381").await?;
    let mut camera = Camera::<PTZOpticsG2>::new(transport);
    println!("Connected to PTZOptics G2 camera (async)");

    // Demonstrate async operations with type-safe units
    println!("\n1. Async type-safe position control");
    camera.home().await?;
    println!("   ✓ Moved to home position");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Async position control with degrees
    camera.set_position(Degrees(45.0), Degrees(-15.0)).await?;
    println!("   ✓ Set position to pan=45°, tilt=-15°");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Demonstrate async sequential operations
    println!("\n2. Async sequential operations");
    perform_async_scan_sequence(&mut camera).await?;
    println!("   ✓ Completed async scan sequence");

    // Demonstrate complex sequential operations
    println!("\n3. Complex sequential operations");

    // Sequential operations that would typically be concurrent
    // Start moving and zooming (simulating concurrent behavior)
    camera
        .move_continuous(PanTiltDirection::Right, 8, 0)
        .await?;
    camera.zoom_in().await?;
    println!("   → Started movement and zoom");

    // Let both operations run
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Stop both operations
    camera.stop().await?;
    camera.zoom_stop().await?;
    println!("   ✓ Completed movement and zoom sequence");

    // Demonstrate preset patrol pattern
    println!("\n4. Async preset patrol");

    // Save positions with G2-specific preset IDs
    camera.set_position(Degrees(-80.0), Degrees(0.0)).await?;
    let preset1 = G2PresetId::new(1)?;
    camera.set_preset(preset1).await?;

    camera.set_position(Degrees(0.0), Degrees(45.0)).await?;
    let preset2 = G2PresetId::new(2)?;
    camera.set_preset(preset2).await?;

    camera.set_position(Degrees(80.0), Degrees(0.0)).await?;
    let preset3 = G2PresetId::new(3)?;
    camera.set_preset(preset3).await?;

    // Patrol between presets
    for _ in 0..2 {
        for preset in &[preset1, preset2, preset3] {
            camera.recall_preset(*preset).await?;
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
    println!("   ✓ Completed preset patrol");

    // Reset to neutral
    println!("\n5. Reset to neutral position");
    camera.home().await?;
    camera.set_zoom(0x0000).await?;
    camera.focus_auto().await?;
    println!("   ✓ Reset camera to neutral state");

    println!("\n=== Async Camera<P> API demo completed! ===");
    Ok(())
}

// Async helper function for scan sequence
#[cfg(feature = "async-client")]
async fn perform_async_scan_sequence(
    camera: &mut Camera<PTZOpticsG2>,
) -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::camera::units::Degrees;

    // Return to home
    camera.home().await?;
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Scan pattern with position feedback
    let scan_positions = vec![
        Degrees(-90.0),
        Degrees(-45.0),
        Degrees(0.0),
        Degrees(45.0),
        Degrees(90.0),
        Degrees(0.0),
    ];

    for pan_pos in scan_positions {
        camera.set_position(pan_pos, Degrees(0.0)).await?;
        println!("      → Scanning at pan={:?}", pan_pos);
        tokio::time::sleep(Duration::from_millis(800)).await;
    }

    Ok(())
}
