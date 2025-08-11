//! Runtime-agnostic async example demonstrating custom runtime usage.
//!
//! This example shows how to use the library with a custom runtime implementation.
//! While the transport layer still requires tokio features, the camera operations
//! use a generic runtime abstraction for timeouts and delays.
//!
//! Run with:
//! ```sh
//! cargo run --example runtime_agnostic --features rt-tokio [camera_ip[:port]]
//! ```

#[cfg(feature = "rt-tokio")]
use grafton_visca::{
    camera::{helpers::MovementOps, profiles::PTZOpticsG2},
    executor::{Sleep, SpawnableFuture, Spawner},
    prelude::r#async::*,
    runtime::{GenericRuntime, SharedRuntime},
    transport::tokio::Tcp,
    types::{PanSpeed, TiltSpeed},
    Camera, Error,
};

#[cfg(feature = "rt-tokio")]
use std::{env, pin::Pin, sync::Arc, time::Duration};

/// A simple sleep implementation using async-io timer.
#[cfg(feature = "rt-tokio")]
#[derive(Debug, Clone)]
struct CustomSleep;

#[cfg(feature = "rt-tokio")]
impl Sleep for CustomSleep {
    fn sleep(
        &self,
        duration: Duration,
    ) -> Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            async_io::Timer::after(duration).await;
        })
    }
}

/// A simple spawner implementation using async-executor.
#[cfg(feature = "rt-tokio")]
#[derive(Debug, Clone)]
struct CustomSpawner {
    executor: Arc<async_executor::Executor<'static>>,
}

#[cfg(feature = "rt-tokio")]
impl Spawner for CustomSpawner {
    fn spawn(&self, task: SpawnableFuture) {
        self.executor.spawn(task).detach();
    }
}

#[cfg(feature = "rt-tokio")]
async fn camera_demo() -> Result<(), Error> {
    // Initialize logging (set RUST_LOG=debug for verbose output)
    env_logger::init();

    // Get camera address from command line or use default
    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110".to_string());

    println!("🎥 Runtime-Agnostic Camera Control Demo");
    println!("========================================");
    println!("Using custom runtime implementation");
    println!("Connecting to camera at {camera_addr}");
    println!();

    // Create a custom runtime using our Sleep and Spawner implementations
    let executor = Arc::new(async_executor::Executor::new());
    let runtime: SharedRuntime = Arc::new(GenericRuntime::new(
        CustomSleep,
        CustomSpawner {
            executor: executor.clone(),
        },
    ));

    // Create camera with custom runtime
    // We need to create the transport directly since we're not using tokio
    let transport = Tcp::connect(&camera_addr).await?;
    let camera = Camera::<PTZOpticsG2, _>::new(transport).with_runtime(runtime.clone());

    println!("✅ Connected successfully with custom runtime!");
    println!();

    // === BASIC OPERATIONS ===
    println!("═══ Basic Camera Operations ═══");

    // Check power state
    match camera.get_power_state().await {
        Ok(true) => println!("✓ Camera is powered on"),
        Ok(false) => {
            println!("Camera is off, powering on...");
            camera.power_on().await?;
            println!("✓ Camera powered on");
        }
        Err(e) => println!("⚠ Could not check power state: {e}"),
    }

    // Get current position
    match camera.get_pan_tilt_degrees().await {
        Ok((pan, tilt)) => {
            println!("✓ Current position: Pan={:.1}°, Tilt={:.1}°", pan.0, tilt.0);
        }
        Err(e) => println!("⚠ Could not get position: {e}"),
    }

    // Get zoom level
    match camera.get_zoom_position().await {
        Ok(zoom) => {
            let zoom_pct = (zoom as f32 / 0x4000 as f32) * 100.0;
            println!("✓ Current zoom: {zoom_pct:.1}%");
        }
        Err(e) => println!("⚠ Could not get zoom: {e}"),
    }

    println!();

    // === MOVEMENT DEMO ===
    println!("═══ Movement Demo ═══");

    // Pan left
    println!("Panning left...");
    camera
        .pan_tilt_move(
            grafton_visca::PanTiltDirection::Left,
            PanSpeed::new(12)?,
            TiltSpeed::new(0)?,
        )
        .await?;

    // Use our custom runtime for sleep
    runtime.sleep(Duration::from_secs(1)).await;

    camera.pan_tilt_stop().await?;
    println!("✓ Pan completed");

    // Tilt up
    println!("Tilting up...");
    camera
        .pan_tilt_move(
            grafton_visca::PanTiltDirection::Up,
            PanSpeed::new(0)?,
            TiltSpeed::new(10)?,
        )
        .await?;

    runtime.sleep(Duration::from_secs(1)).await;

    camera.pan_tilt_stop().await?;
    println!("✓ Tilt completed");

    // Return to home
    println!("Returning to home position...");
    camera.pan_tilt_home().await?;

    // Wait for movement to complete
    // Using await_idle instead of polling
    camera.await_idle(Duration::from_secs(10)).await?;
    println!("✓ Returned to home");

    println!();

    // === ZOOM DEMO ===
    println!("═══ Zoom Demo ═══");

    println!("Zooming in...");
    camera.zoom_in().await?;
    runtime.sleep(Duration::from_secs(1)).await;
    camera.zoom_stop().await?;

    match camera.get_zoom_position().await {
        Ok(zoom) => {
            let zoom_pct = (zoom as f32 / 0x4000 as f32) * 100.0;
            println!("✓ Zoomed to {zoom_pct:.1}%");
        }
        Err(e) => println!("⚠ Could not get zoom: {e}"),
    }

    println!("Zooming out...");
    camera.zoom_out().await?;
    runtime.sleep(Duration::from_secs(1)).await;
    camera.zoom_stop().await?;

    match camera.get_zoom_position().await {
        Ok(zoom) => {
            let zoom_pct = (zoom as f32 / 0x4000 as f32) * 100.0;
            println!("✓ Zoomed to {zoom_pct:.1}%");
        }
        Err(e) => println!("⚠ Could not get zoom: {e}"),
    }

    println!();
    println!("✅ Demo completed successfully!");
    println!("   Demonstrated runtime-agnostic async operations");

    Ok(())
}

#[cfg(feature = "rt-tokio")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    camera_demo().await?;
    Ok(())
}

#[cfg(not(feature = "rt-tokio"))]
fn main() {
    eprintln!("This example requires the 'rt-tokio' feature to be enabled.");
    eprintln!("Run with: cargo run --example runtime_agnostic --features rt-tokio");
    std::process::exit(1);
}
