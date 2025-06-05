//! Example demonstrating Phase D ergonomic APIs.
//!
//! This example shows the new PTZ builder pattern and AsyncViscaExt trait
//! introduced in Phase D of the v0.4.0 refactoring.

use std::sync::Arc;
use std::time::Duration;

use grafton_visca::{
    command::pan_tilt::{PanSpeed, PanTiltDirection, TiltSpeed},
    ViscaClient, ViscaClientPtzExt, ViscaError,
};

// Async features
#[cfg(feature = "async-client")]
use grafton_visca::{AsyncViscaExt, PanScanDirection};

#[cfg(all(feature = "blocking-client", not(feature = "async-client")))]
fn main() -> Result<(), ViscaError> {
    env_logger::init();

    println!("=== Phase D Ergonomic APIs Demo (Blocking) ===");

    // Connect to camera
    let client = Arc::new(ViscaClient::connect_udp("192.168.1.100:5678")?);
    println!("Connected to camera");

    // Demonstrate PTZ Builder with sequential execution
    println!("\n1. PTZ Builder - Sequential execution");
    Arc::clone(&client)
        .ptz()
        .pan_tilt_home()
        .zoom_in(3)
        .focus_auto()
        .execute_sequential()?;
    println!("   ✓ Executed: Home → Zoom In → Auto Focus");

    // Demonstrate complex movement sequence
    println!("\n2. PTZ Builder - Complex movement");
    Arc::clone(&client)
        .ptz()
        .pan_tilt_move(
            PanTiltDirection::UpRight,
            PanSpeed::new(12).unwrap(),
            TiltSpeed::new(10).unwrap(),
        )
        .zoom_direct(0x4000)
        .focus_manual()
        .execute_sequential()?;
    println!("   ✓ Executed: Move UpRight → Direct Zoom → Manual Focus");

    // Demonstrate absolute positioning
    println!("\n3. PTZ Builder - Absolute positioning");
    Arc::clone(&client)
        .ptz()
        .pan_tilt_absolute(1000, -500, 0x18, 0x14)
        .zoom_direct(0x8000)
        .execute_sequential()?;
    println!("   ✓ Executed: Absolute position + zoom");

    // Demonstrate builder reuse and clearing
    println!("\n4. PTZ Builder - Reuse and clear");
    let builder = Arc::clone(&client).ptz().pan_tilt_home().zoom_stop();
    println!("   Builder length: {}", builder.len());

    let cleared_builder = builder.clear();
    println!("   Cleared builder length: {}", cleared_builder.len());

    println!("\n=== Blocking demo completed! ===");
    Ok(())
}

#[cfg(all(feature = "async-client", not(feature = "blocking-client")))]
#[tokio::main]
async fn main() -> Result<(), ViscaError> {
    env_logger::init();

    println!("=== Phase D Ergonomic APIs Demo (Async) ===");

    // Connect to camera
    let client = Arc::new(ViscaClient::connect_udp_async("192.168.1.100:5678").await?);
    println!("Connected to camera");

    // Demonstrate PTZ Builder with sequential execution
    println!("\n1. PTZ Builder - Sequential execution");
    Arc::clone(&client)
        .ptz()
        .pan_tilt_home()
        .zoom_in(3)
        .focus_auto()
        .execute_sequential_async()
        .await?;
    println!("   ✓ Executed: Home → Zoom In → Auto Focus");

    // Demonstrate PTZ Builder with concurrent execution
    println!("\n2. PTZ Builder - Concurrent execution");
    Arc::clone(&client)
        .ptz()
        .pan_tilt_move(
            PanTiltDirection::UpRight,
            PanSpeed::new(12).unwrap(),
            TiltSpeed::new(10).unwrap(),
        )
        .zoom_direct(0x4000)
        .focus_manual()
        .execute_concurrent()
        .await?;
    println!("   ✓ Executed: Movement + Zoom + Focus (concurrent)");

    // Demonstrate AsyncViscaExt high-level operations
    println!("\n3. AsyncViscaExt - Setup shot");
    client.setup_shot(0.3, -0.2, 0x6000).await?;
    println!("   ✓ Set up shot: pan=30%, tilt=-20%, zoom=0x6000");

    println!("\n4. AsyncViscaExt - Save and recall position");
    client.save_current_position(5).await?;
    println!("   ✓ Saved current position to preset 5");

    // Move to different position
    client.setup_shot(-0.5, 0.4, 0x2000).await?;
    println!("   ✓ Moved to different position");

    // Recall saved position with zoom adjustment
    client.recall_position_with_zoom(5, Some(0x8000)).await?;
    println!("   ✓ Recalled preset 5 with zoom adjustment");

    println!("\n5. AsyncViscaExt - Pan scan");
    client
        .smooth_pan_scan(8, Duration::from_secs(3), PanScanDirection::Right)
        .await?;
    println!("   ✓ Performed smooth pan scan to the right");

    println!("\n6. AsyncViscaExt - Frame subject");
    client.frame_subject(0x5000, true).await?;
    println!("   ✓ Framed subject with auto-focus");

    println!("\n7. AsyncViscaExt - Patrol positions");
    // First save a few preset positions
    client.setup_shot(-0.8, 0.0, 0x3000).await?;
    client.save_current_position(1).await?;

    client.setup_shot(0.0, 0.8, 0x4000).await?;
    client.save_current_position(2).await?;

    client.setup_shot(0.8, 0.0, 0x5000).await?;
    client.save_current_position(3).await?;

    // Patrol between the positions
    client
        .patrol_positions(&[1, 2, 3], Some(Duration::from_millis(1000)))
        .await?;
    println!("   ✓ Patrolled between presets 1, 2, 3");

    println!("\n8. AsyncViscaExt - Movement health check");
    let healthy = client.movement_health_check().await?;
    println!(
        "   ✓ Movement health check: {}",
        if healthy { "PASS" } else { "FAIL" }
    );

    println!("\n9. AsyncViscaExt - Reset to neutral");
    client.reset_to_neutral().await?;
    println!("   ✓ Reset camera to neutral state");

    println!("\n=== Async demo completed! ===");
    Ok(())
}

#[cfg(all(feature = "async-client", feature = "blocking-client"))]
#[tokio::main]
async fn main() -> Result<(), ViscaError> {
    env_logger::init();

    println!("=== Phase D Ergonomic APIs Demo (Both Features) ===");

    // Demonstrate that both blocking and async work in the same build
    let client = Arc::new(ViscaClient::connect_udp("192.168.1.100:5678")?);
    println!("Connected to camera using blocking constructor");

    // Use blocking PTZ builder
    println!("\n1. Blocking PTZ Builder");
    Arc::clone(&client)
        .ptz()
        .pan_tilt_home()
        .zoom_in(2)
        .execute_sequential()?;
    println!("   ✓ Blocking execution completed");

    // Use async PTZ builder
    println!("\n2. Async PTZ Builder");
    Arc::clone(&client)
        .ptz()
        .pan_tilt_move(
            PanTiltDirection::Left,
            PanSpeed::new(8).unwrap(),
            TiltSpeed::new(0).unwrap(),
        )
        .zoom_out(3)
        .execute_sequential_async()
        .await?;
    println!("   ✓ Async execution completed");

    // Use AsyncViscaExt trait
    println!("\n3. AsyncViscaExt operations");
    client.setup_shot(0.0, 0.0, 0x4000).await?;
    println!("   ✓ AsyncViscaExt setup_shot completed");

    let healthy = client.movement_health_check().await?;
    println!(
        "   ✓ Health check: {}",
        if healthy { "PASS" } else { "FAIL" }
    );

    println!("\n=== Dual-mode demo completed! ===");
    Ok(())
}

#[cfg(not(any(feature = "async-client", feature = "blocking-client")))]
fn main() {
    println!("This example requires either 'blocking-client' or 'async-client' feature.");
    println!("Run with: cargo run --example phase_d_ergonomic_apis --features blocking-client");
    println!("Or with:  cargo run --example phase_d_ergonomic_apis --features async-client");
}
