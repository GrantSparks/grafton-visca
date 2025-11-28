//! Concurrent camera control demonstration.
//!
//! This example shows thread-safe concurrent operations on a single camera:
//! - Parallel command execution
//! - Producer-consumer patterns
//! - Resource sharing with Arc
//! - Async task spawning
//!
//! **Context**: This example uses native async transports with the tokio runtime
//! to demonstrate concurrent operations.
//!
//! Run with:
//! ```sh
//! cargo run --example concurrent_control --features runtime-tokio [camera_ip[:port]]
//! ```

#[cfg(not(feature = "runtime-tokio"))]
fn main() {
    println!("This example requires the 'runtime-tokio' feature.");
    println!("Run with: cargo run --example concurrent_control --features runtime-tokio");
}

#[cfg(feature = "runtime-tokio")]
use tokio::{
    sync::mpsc,
    time::{sleep, Duration},
};

#[cfg(feature = "runtime-tokio")]
use std::sync::Arc;

#[cfg(feature = "runtime-tokio")]
use grafton_visca::{
    camera::{profiles::PtzOpticsG2, session::CameraSession, Connect},
    mode::Async,
    runtime::TransportHandle,
    types::SpeedLevel,
    units::Normalized,
    PanTiltDirection, PresetNumber, Result, TokioRuntime,
};

#[cfg(feature = "runtime-tokio")]
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110".to_string());

    println!("=== Concurrent Camera Control Demo ===\n");
    println!("Camera address: {camera_addr}\n");

    // Example 1: Parallel operations on single camera
    parallel_operations(&camera_addr).await?;

    // Example 2: Producer-consumer pattern
    producer_consumer_pattern(&camera_addr).await?;

    println!("\n=== Demo completed ===");

    Ok(())
}

#[cfg(feature = "runtime-tokio")]
async fn parallel_operations(camera_addr: &str) -> Result<()> {
    println!("--- Example 1: Parallel Operations ---");

    let runtime = TokioRuntime::from_current()?;
    let camera: Arc<
        CameraSession<Async, PtzOpticsG2, TransportHandle<TokioRuntime>, TokioRuntime>,
    > = Arc::new(Connect::open_tcp_async::<PtzOpticsG2, _>(camera_addr, runtime).await?);

    println!("Querying multiple states in parallel...");

    // Query multiple camera states concurrently
    // Note: accessors must be bound before passing to tokio::join!
    let power_acc = camera.power();
    let pan_tilt_acc = camera.pan_tilt();
    let zoom_acc = camera.zoom();
    let focus_acc = camera.focus();

    let (power, pan_tilt, zoom, focus) = tokio::join!(
        power_acc.state(),
        pan_tilt_acc.position(),
        zoom_acc.position(),
        focus_acc.position()
    );

    println!("Power: {power:?}");
    println!("Pan/Tilt: {pan_tilt:?}");
    println!("Zoom: {zoom:?}");
    println!("Focus: {focus:?}");

    println!("\nExecuting coordinated movements...");

    // Launch concurrent movement commands
    let zoom_task = {
        let cam = camera.clone();
        tokio::spawn(async move { cam.zoom().tele().await })
    };

    let pan_task = {
        let cam = camera.clone();
        tokio::spawn(async move {
            cam.pan_tilt()
                .move_direction(
                    PanTiltDirection::UpRight,
                    SpeedLevel::Slow.into(),
                    SpeedLevel::Slow.into(),
                )
                .await
        })
    };

    // Let movements run briefly
    sleep(Duration::from_millis(100)).await;

    // Stop all movements
    camera.zoom().stop().await?;
    camera.pan_tilt().stop().await?;

    // Wait for movements to settle
    let _ = tokio::join!(
        camera.await_zoom_idle(Duration::from_secs(5)),
        camera.await_pan_tilt_idle(Duration::from_secs(5))
    );

    // Ensure spawned tasks complete
    let _ = tokio::join!(zoom_task, pan_task);

    println!("✓ Parallel operations completed");

    // Return to home
    println!("Returning to home position...");
    camera.pan_tilt().home().await?;
    camera.await_idle(Duration::from_secs(5)).await?;
    println!("✓ Camera returned to home\n");

    Ok(())
}

#[cfg(feature = "runtime-tokio")]
async fn producer_consumer_pattern(camera_addr: &str) -> Result<()> {
    println!("--- Example 2: Producer-Consumer Pattern ---");

    let runtime = TokioRuntime::from_current()?;
    let camera: Arc<
        CameraSession<Async, PtzOpticsG2, TransportHandle<TokioRuntime>, TokioRuntime>,
    > = Arc::new(Connect::open_tcp_async::<PtzOpticsG2, _>(camera_addr, runtime).await?);

    let (tx, mut rx) = mpsc::channel(10);

    // Consumer task - executes commands sequentially
    let consumer = {
        let cam = camera.clone();
        tokio::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    Command::Home => {
                        println!("  Executing: Home");
                        let _ = cam.pan_tilt().home().await;
                        let _ = cam.await_pan_tilt_idle(Duration::from_secs(5)).await;
                    }
                    Command::Preset(n) => {
                        println!("  Executing: Preset {n}");
                        if PresetNumber::new(n).is_ok() {
                            let _ = cam.presets().recall(n).await;
                            let _ = cam.await_idle(Duration::from_secs(5)).await;
                        }
                    }
                    Command::Zoom(level) => {
                        println!("  Executing: Zoom to {:.0}%", level * 100.0);
                        let _ = cam.zoom().absolute(Normalized::new(level)).await;
                        let _ = cam.await_zoom_idle(Duration::from_secs(5)).await;
                    }
                }
            }
        })
    };

    // Producer 1 - sends a sequence of commands
    let producer1 = {
        let tx = tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(Command::Home).await;
            let _ = tx.send(Command::Preset(1)).await;
            let _ = tx.send(Command::Zoom(0.5)).await;
        })
    };

    // Producer 2 - sends another sequence
    let producer2 = {
        let tx = tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(Command::Preset(2)).await;
            let _ = tx.send(Command::Zoom(0.75)).await;
            let _ = tx.send(Command::Preset(3)).await;
        })
    };

    // Wait for producers to finish sending
    let _ = tokio::join!(producer1, producer2);

    // Close channel and wait for consumer to finish
    drop(tx);
    let _ = consumer.await;

    println!("✓ Producer-consumer pattern completed\n");

    Ok(())
}

#[cfg(feature = "runtime-tokio")]
#[derive(Debug)]
enum Command {
    Home,
    Preset(u8),
    Zoom(f32),
}
