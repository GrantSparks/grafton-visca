//! Example demonstrating Sony encapsulation protocol.
//!
//! This example shows how to use the library with Sony cameras that require
//! the Sony encapsulated protocol format. The library automatically handles
//! the 8-byte header encapsulation when using Sony camera profiles.
//!
//! Run with:
//! ```sh
//! cargo run --example sony_encapsulation [camera_ip[:port]]
//! ```

#[cfg(not(feature = "async"))]
use std::{env, thread::sleep, time::Duration};

#[cfg(not(feature = "async"))]
use grafton_visca::{
    camera::profiles::SonyFR7,
    command::zoom::ZoomSpeed,
    prelude::blocking::*,
    types::{PanPosition, PanSpeed, TiltPosition, TiltSpeed},
    CameraBuilder, Error,
};

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110".to_string());

    println!("🎥 Sony Encapsulation Protocol Demo");
    println!("===================================");
    println!("This example demonstrates the Sony encapsulated protocol format.");
    println!("The library automatically handles the 8-byte header when using Sony profiles.\n");

    println!("Connecting to Sony camera at {camera_addr}");
    println!("Using profile: Sony FR7 (with sequence numbers)\n");

    // Build camera with Sony FR7 profile
    // This automatically configures the transport to use Sony encapsulation
    let camera = CameraBuilder::tcp(&camera_addr)
        .profile::<SonyFR7>() // Sony FR7 uses encapsulation with sequence numbers
        .open()?;

    println!("✅ Connected successfully!");
    println!("Protocol: Sony Encapsulated (8-byte header with sequence tracking)\n");

    // The rest of the API is identical - the encapsulation is handled transparently
    println!("═══ Testing Basic Commands ═══\n");

    // Power status inquiry
    println!("Checking power status...");
    match camera.get_power_state() {
        Ok(is_on) => {
            println!("  Power is: {}", if is_on { "ON" } else { "OFF" });
            if !is_on {
                println!("  Turning camera ON...");
                camera.power_on()?;
                sleep(Duration::from_secs(3));
                println!("  Camera powered on successfully");
            }
        }
        Err(e) => println!("  Could not check power: {}", e),
    }
    println!();

    // Get current position
    println!("Getting current camera position...");
    match camera.get_pan_tilt_degrees() {
        Ok((pan, tilt)) => {
            println!("  Current position: Pan={}, Tilt={}", pan, tilt);
        }
        Err(e) => println!("  Could not get position: {}", e),
    }

    match camera.get_zoom_position() {
        Ok(zoom) => {
            println!("  Current zoom: {:?}", zoom);
        }
        Err(e) => println!("  Could not get zoom: {}", e),
    }
    println!();

    // Test movement commands
    println!("═══ Testing Movement Commands ═══\n");

    println!("Moving to home position...");
    camera.pan_tilt_home()?;
    camera.await_idle(Duration::from_secs(10))?;
    println!("  ✓ Moved to home position");

    println!("Testing pan/tilt movement...");
    camera.pan_tilt_absolute(
        PanPosition::new((30.0 * 614.4) as i16)?, // Convert 30 degrees to units
        TiltPosition::new((10.0 * 614.4) as i16)?, // Convert 10 degrees to units
        PanSpeed::from(SpeedLevel::Fast),
        TiltSpeed::from(SpeedLevel::Fast),
    )?;
    camera.await_idle(Duration::from_secs(10))?;
    println!("  ✓ Moved to Pan=30°, Tilt=10°");

    println!("Testing zoom...");
    camera.zoom_tele_variable(ZoomSpeed::new(4)?)?;
    sleep(Duration::from_millis(500));
    camera.zoom_stop()?;
    camera.await_idle(Duration::from_secs(10))?;
    println!("  ✓ Zoomed in");

    // Return to home
    println!("\nReturning to home position...");
    camera.pan_tilt_home()?;
    camera.zoom_wide_variable(ZoomSpeed::new(4)?)?;
    sleep(Duration::from_millis(500));
    camera.zoom_stop()?;
    camera.await_idle(Duration::from_secs(10))?;
    println!("  ✓ Returned to home");

    println!("\n═══ Protocol Details ═══");
    println!("All commands were automatically wrapped in Sony 8-byte headers:");
    println!("- Byte 0: 0x01 (Command type)");
    println!("- Byte 1: Payload length");
    println!("- Bytes 2-3: Sequence number (incremented for each command)");
    println!("- Bytes 4-7: Reserved (0x00)");
    println!("- Followed by: VISCA command bytes");
    println!("\nResponses were automatically extracted from Sony encapsulation.");

    println!("\n✅ Demo completed successfully!");

    Ok(())
}

#[cfg(feature = "rt-tokio")]
use grafton_visca::{
    camera::controls::{
        inquiry::{InquiryControl, PanTiltInquiryControl},
        pan_tilt::PanTiltControl,
        power::PowerControl,
        zoom::ZoomControl,
    },
    camera::profiles::SonyFR7,
    runtime_adapters::tokio::TcpTransport as Tcp,
    types::SpeedLevel,
    units::Degrees,
    CameraBuilder, Error,
};

#[cfg(feature = "rt-tokio")]
use tokio::time::{sleep, Duration};

#[cfg(feature = "rt-tokio")]
use std::env;

#[cfg(feature = "rt-tokio")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110".to_string());

    println!("🎥 Sony Encapsulation Protocol Demo (Async)");
    println!("===========================================");
    println!("This example demonstrates the Sony encapsulated protocol format.");
    println!("The library automatically handles the 8-byte header when using Sony profiles.\n");

    println!("Connecting to Sony camera at {camera_addr}");
    println!("Using profile: Sony FR7 (with sequence numbers)\n");

    // Build camera with Sony FR7 profile
    // This automatically configures the transport to use Sony encapsulation
    let transport = Tcp::connect(&camera_addr).await?;
    let camera = CameraBuilder::tokio()?
        .open_async::<SonyFR7, _>(transport)
        .await?;

    println!("✅ Connected successfully!");
    println!("Protocol: Sony Encapsulated (8-byte header with sequence tracking)\n");

    // The rest of the API is identical - the encapsulation is handled transparently
    println!("═══ Testing Basic Commands ═══\n");

    // Power status inquiry
    println!("Checking power status...");
    match camera.get_power_state().await {
        Ok(is_on) => {
            println!("  Power is: {}", if is_on { "ON" } else { "OFF" });
            if !is_on {
                println!("  Turning camera ON...");
                camera.power_on().await?;
                sleep(Duration::from_secs(3)).await;
                println!("  Camera powered on successfully");
            }
        }
        Err(e) => println!("  Could not check power: {}", e),
    }
    println!();

    // Get current position
    println!("Getting current camera position...");
    match camera.get_pan_tilt_position().await {
        Ok(position) => {
            println!(
                "  Current position: Pan={}, Tilt={}",
                position.pan, position.tilt
            );
        }
        Err(e) => println!("  Could not get position: {}", e),
    }

    match camera.get_zoom_position().await {
        Ok(zoom) => {
            println!("  Current zoom: {:?}", zoom);
        }
        Err(e) => println!("  Could not get zoom: {}", e),
    }
    println!();

    // Test movement commands
    println!("═══ Testing Movement Commands ═══\n");

    println!("Moving to home position...");
    camera.pan_tilt_home().await?;
    camera.await_idle(Duration::from_secs(10)).await?;
    println!("  ✓ Moved to home position");

    println!("Testing pan/tilt movement...");
    camera
        .pan_tilt_absolute(Degrees(30.0), Degrees(10.0), SpeedLevel::Fast)
        .await?;
    camera.await_idle(Duration::from_secs(10)).await?;
    println!("  ✓ Moved to Pan=30°, Tilt=10°");

    println!("Testing zoom...");
    camera.zoom_tele_std().await?;
    sleep(Duration::from_millis(500)).await;
    camera.zoom_stop().await?;
    camera.await_idle(Duration::from_secs(10)).await?;
    println!("  ✓ Zoomed in");

    // Demonstrate concurrent operations (async advantage!)
    println!("\n═══ Concurrent Operations (Async Advantage!) ═══\n");
    println!("Executing pan and zoom simultaneously...");

    let (pan_result, zoom_result): (Result<(), Error>, Result<(), Error>) = tokio::join!(
        camera.pan_tilt_absolute(Degrees(0.0), Degrees(0.0), SpeedLevel::Fast),
        async {
            camera.zoom_wide_std().await?;
            sleep(Duration::from_millis(500)).await;
            camera.zoom_stop().await
        }
    );

    pan_result?;
    zoom_result?;
    camera.await_idle(Duration::from_secs(10)).await?;
    println!("  ✓ All movements completed concurrently");

    println!("\n═══ Protocol Details ═══");
    println!("All commands were automatically wrapped in Sony 8-byte headers:");
    println!("- Byte 0: 0x01 (Command type)");
    println!("- Byte 1: Payload length");
    println!("- Bytes 2-3: Sequence number (incremented for each command)");
    println!("- Bytes 4-7: Reserved (0x00)");
    println!("- Followed by: VISCA command bytes");
    println!("\nResponses were automatically extracted from Sony encapsulation.");

    println!("\n✅ Demo completed successfully!");

    Ok(())
}

// Provide a stub main when neither blocking nor tokio runtime is available
#[cfg(all(feature = "async", not(feature = "rt-tokio")))]
fn main() {
    eprintln!(
        "This example requires either no features (for blocking) or --features rt-tokio for async"
    );
    eprintln!("Try: cargo run --example sony_encapsulation");
    eprintln!("  or: cargo run --example sony_encapsulation --features rt-tokio");
}
