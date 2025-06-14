//! Example program

//! Demonstrates the unified extension trait system.
//!
//! This example shows how the same API works seamlessly in both
//! sync and async contexts without code duplication.

use grafton_visca::command::pan_tilt::{PanSpeed, PanTiltDirection, TiltSpeed};
use grafton_visca::{CameraExt, Client, Error, InquiryExt, PowerExt, PresetExt, ZoomExt};
use std::time::Duration;

#[cfg(feature = "async-client")]
use grafton_visca::AsyncCameraExt;

/// Demonstrates blocking usage of the unified extension traits.
#[cfg(feature = "blocking-client")]
fn blocking_example() -> Result<(), Error> {
    println!("=== Blocking Extension Traits Example ===\n");

    // Connect to camera
    let mut camera = Client::connect_udp("192.168.1.100:5678")?;

    // Extension traits provide a clean, unified API
    println!("1. Checking camera power status...");
    if camera.is_powered_on()? {
        println!("   ✅ Camera is powered on");
    } else {
        println!("   ⚡ Camera is off, powering on...");
        camera.power_on()?;
    }

    println!("\n2. Getting camera positions...");
    let zoom_pos = camera.get_zoom_position()?;
    let (pan, tilt) = camera.pan_tilt_position()?;
    println!("   📍 Current position:");
    println!("      Zoom: 0x{:04X}", zoom_pos);
    println!("      Pan:  {} degrees", pan as f32 * 0.1);
    println!("      Tilt: {} degrees", tilt as f32 * 0.1);

    println!("\n3. Testing camera movements...");

    // Move home
    println!("   🏠 Moving to home position");
    camera.move_home()?;
    std::thread::sleep(Duration::from_secs(2));

    // Test zoom
    println!("   🔍 Testing zoom");
    camera.zoom_to(0x3000)?;
    std::thread::sleep(Duration::from_millis(1000));

    // Test pan/tilt
    println!("   🎯 Testing pan/tilt");
    camera.start_moving(
        PanTiltDirection::UpRight,
        PanSpeed::new(0x10)?,
        TiltSpeed::new(0x10)?,
    )?;
    std::thread::sleep(Duration::from_millis(500));
    camera.stop_moving()?;

    // Save preset
    println!("   💾 Saving current position as preset 1");
    camera.set_preset(1)?;

    println!("\n✅ Blocking example completed successfully!");
    Ok(())
}

/// Demonstrates async usage of the unified extension traits.
#[cfg(feature = "async-client")]
async fn async_example() -> Result<(), Error> {
    println!("=== Async Extension Traits Example ===\n");

    // Connect to camera
    let mut camera = Client::connect_udp_async("192.168.1.100:5678").await?;

    // Extension traits work in async context!
    println!("1. Checking camera power status...");
    if camera.is_powered_on()? {
        println!("   ✅ Camera is powered on");
    } else {
        println!("   ⚡ Camera is off, powering on...");
        camera.power_on()?;

        // AsyncCameraExt provides async-specific operations
        println!("   ⏳ Waiting for camera to power on...");
        camera
            .wait_for_power_on(Duration::from_secs(30), Duration::from_millis(500))
            .await?;
    }

    println!("\n2. Testing async-specific operations...");

    // Move to position and wait
    println!("   🎯 Moving to position and waiting for completion");
    camera
        .move_to_and_wait(
            100, // pan: 10 degrees
            50,  // tilt: 5 degrees
            PanSpeed::new(0x10)?,
            TiltSpeed::new(0x10)?,
            Duration::from_secs(10),
        )
        .await?;

    // Zoom and wait
    println!("   🔍 Zooming to position and waiting");
    camera
        .zoom_to_and_wait(0x4000, Duration::from_secs(5))
        .await?;

    // Save some presets
    println!("\n3. Setting up preset positions...");
    for i in 1..=3 {
        println!("   💾 Saving preset {}", i);
        camera.set_preset(i)?;

        // Move camera between saves
        if i < 3 {
            camera.start_moving(
                PanTiltDirection::Right,
                PanSpeed::new(0x08)?,
                TiltSpeed::new(0x08)?,
            )?;
            tokio::time::sleep(Duration::from_millis(500)).await;
            camera.stop_moving()?;
        }
    }

    // Demonstrate patrol
    println!("\n4. Starting preset patrol (press Ctrl+C to stop)...");
    tokio::select! {
        result = camera.patrol_presets(&[1, 2, 3], Duration::from_secs(3)) => {
            result?;
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\n   🛑 Patrol stopped by user");
        }
    }

    println!("\n✅ Async example completed successfully!");
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🚀 Unified Extension Traits Demo");
    println!("================================\n");

    #[cfg(feature = "blocking-client")]
    {
        if let Err(e) = blocking_example() {
            println!("❌ Blocking example failed: {}", e);
        }
        println!();
    }

    #[cfg(feature = "async-client")]
    {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            if let Err(e) = async_example().await {
                println!("❌ Async example failed: {}", e);
            }
        });
    }

    #[cfg(not(any(feature = "blocking-client", feature = "async-client")))]
    {
        println!("⚠️  No client features enabled!");
        println!("   Enable with:");
        println!("   cargo run --example unified_extensions_demo --features blocking-client");
        println!("   cargo run --example unified_extensions_demo --features async-client");
        println!("   cargo run --example unified_extensions_demo --features full");
    }

    println!("\n✨ Key benefits of unified extension traits:");
    println!("   1. Same methods work in both sync and async contexts");
    println!("   2. No '_async' suffix confusion");
    println!("   3. Clean, intuitive API");
    println!("   4. AsyncCameraExt adds async-specific operations");
    println!("   5. Works with Arc<Client> for shared ownership");

    Ok(())
}
