//! Concurrent camera control demonstration.
//!
//! This example shows thread-safe concurrent operations:
//! - Controlling multiple cameras simultaneously
//! - Parallel command execution on single camera
//! - Synchronization patterns
//! - Resource sharing with Arc/Mutex
//! - Async task spawning
//!
//! Run with: cargo run --example concurrent_control --features tokio

#[cfg(not(feature = "rt-tokio"))]
fn main() {
    println!("This example requires the 'tokio' feature.");
    println!("Run with: cargo run --example concurrent_control --features tokio");
}

#[cfg(feature = "rt-tokio")]
use grafton_visca::{
    camera::{
        methods::{
            inquiry::{InquiryControl, PanTiltInquiryControl},
            pan_tilt::PanTiltControl,
            presets::PresetsControl,
            zoom::ZoomControl,
        },
        profiles::PtzOpticsG2,
    },
    prelude::r#async::*,
    transport::tokio::tcp::Tcp,
    types::SpeedLevel,
    CameraBuilder, PanTiltDirection, PresetNumber, Result,
};
#[cfg(feature = "rt-tokio")]
use tokio::time::{sleep, Duration};

#[cfg(feature = "rt-tokio")]
use std::sync::Arc;

#[cfg(feature = "rt-tokio")]
#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("=== Concurrent Camera Control Demo ===\n");

    // Example 1: Control multiple cameras simultaneously
    multi_camera_control().await?;

    // Example 2: Parallel operations on single camera
    parallel_single_camera().await?;

    // Example 3: Producer-consumer pattern
    producer_consumer_pattern().await?;

    // Example 4: Synchronized multi-camera movements
    synchronized_movement().await?;

    Ok(())
}

#[cfg(feature = "rt-tokio")]
async fn multi_camera_control() -> Result<()> {
    println!("--- Example 1: Multiple Cameras Simultaneously ---");

    // Create multiple cameras using best practices
    // First, establish connections with proper error handling
    let transport1 = Tcp::connect_timeout("192.168.0.109:5678", Duration::from_secs(5))
        .await
        .map_err(|e| {
            eprintln!("Failed to connect to camera 1: {e}");
            e
        })?;

    let transport2 = Tcp::connect_timeout("192.168.0.110:5678", Duration::from_secs(5))
        .await
        .map_err(|e| {
            eprintln!("Failed to connect to camera 2: {e}");
            e
        })?;

    let transport3 = Tcp::connect_timeout("192.168.0.111:5678", Duration::from_secs(5))
        .await
        .map_err(|e| {
            eprintln!("Failed to connect to camera 3: {e}");
            e
        })?;

    // Build cameras with the Tokio executor using the builder pattern
    let cam1 = Arc::new(
        CameraBuilder::tokio()?
            .build_async::<PtzOpticsG2, _>(transport1)
            .await?,
    );

    let cam2 = Arc::new(
        CameraBuilder::tokio()?
            .build_async::<PtzOpticsG2, _>(transport2)
            .await?,
    );

    let cam3 = Arc::new(
        CameraBuilder::tokio()?
            .build_async::<PtzOpticsG2, _>(transport3)
            .await?,
    );

    // Spawn concurrent tasks for each camera
    let handle1 = {
        let cam = cam1.clone();
        tokio::spawn(async move {
            println!("Camera 1: Starting preset tour");
            for i in 1..=3 {
                if let Ok(preset) = PresetNumber::new(i) {
                    cam.preset_recall(preset).await?;
                    cam.await_idle(Duration::from_secs(5)).await?;
                }
            }
            Ok::<(), grafton_visca::Error>(())
        })
    };

    let handle2 = {
        let cam = cam2.clone();
        tokio::spawn(async move {
            println!("Camera 2: Performing pan sweep");
            cam.pan_tilt_home().await?;
            cam.await_pan_tilt_idle(Duration::from_secs(5)).await?;
            cam.pan_tilt_move(
                PanTiltDirection::Right,
                SpeedLevel::Medium.into(),
                SpeedLevel::Slowest.into(),
            )
            .await?;
            sleep(Duration::from_millis(100)).await;
            cam.pan_tilt_stop().await?;
            cam.await_pan_tilt_idle(Duration::from_secs(5)).await?;
            Ok::<(), grafton_visca::Error>(())
        })
    };

    let handle3 = {
        let cam = cam3.clone();
        tokio::spawn(async move {
            println!("Camera 3: Zoom demonstration");
            cam.zoom_absolute(Normalized::new(0.0)).await?;
            cam.await_zoom_idle(Duration::from_secs(3)).await?;
            cam.zoom_absolute(Normalized::new(0.5)).await?;
            cam.await_zoom_idle(Duration::from_secs(3)).await?;
            cam.zoom_absolute(Normalized::new(1.0)).await?;
            Ok::<(), grafton_visca::Error>(())
        })
    };

    // Wait for all operations to complete
    let (r1, r2, r3) = tokio::join!(handle1, handle2, handle3);

    if r1.is_ok() && r2.is_ok() && r3.is_ok() {
        println!("✓ All cameras operated successfully in parallel");
    }

    // Return cameras to home position
    println!("Returning cameras to home position...");
    let _ = tokio::join!(
        cam1.pan_tilt_home(),
        cam2.pan_tilt_home(),
        cam3.pan_tilt_home(),
    );
    println!("✓ All cameras returned to home\n");

    Ok(())
}

#[cfg(feature = "rt-tokio")]
async fn parallel_single_camera() -> Result<()> {
    println!("--- Example 2: Parallel Operations on Single Camera ---");

    // Create camera with proper error handling
    let transport = Tcp::connect_timeout("192.168.0.110:5678", Duration::from_secs(5))
        .await
        .map_err(|e| {
            eprintln!("Failed to connect to camera: {e}");
            e
        })?;

    let camera = Arc::new(
        CameraBuilder::tokio()?
            .build_async::<PtzOpticsG2, _>(transport)
            .await?,
    );

    println!("Querying multiple states in parallel...");

    let (power, pan_tilt, zoom, focus) = tokio::join!(
        camera.get_power_state(),
        camera.get_pan_tilt_position(),
        camera.get_zoom_position(),
        camera.get_focus_position()
    );

    println!("Power: {power:?}");
    println!("Pan/Tilt: {pan_tilt:?}");
    println!("Zoom: {zoom:?}");
    println!("Focus: {focus:?}");

    println!("\nExecuting coordinated movements...");

    let zoom_task = {
        let cam = camera.clone();
        tokio::spawn(async move { cam.zoom_tele_std().await })
    };

    let pan_task = {
        let cam = camera.clone();
        tokio::spawn(async move {
            cam.pan_tilt_move(
                PanTiltDirection::UpRight,
                SpeedLevel::Slow.into(),
                SpeedLevel::Slow.into(),
            )
            .await
        })
    };

    sleep(Duration::from_millis(100)).await;

    camera.zoom_stop().await?;
    camera.pan_tilt_stop().await?;
    let _ = tokio::join!(
        camera.await_zoom_idle(Duration::from_secs(5)),
        camera.await_pan_tilt_idle(Duration::from_secs(5))
    );

    let _ = tokio::join!(zoom_task, pan_task);

    println!("✓ Parallel operations completed");

    // Return to home
    println!("Returning to home position...");
    camera.pan_tilt_home().await?;
    camera.await_idle(Duration::from_secs(5)).await?;
    println!("✓ Camera returned to home\n");

    Ok(())
}

#[cfg(feature = "rt-tokio")]
async fn producer_consumer_pattern() -> Result<()> {
    println!("--- Example 3: Producer-Consumer Pattern ---");

    use tokio::sync::mpsc;

    let transport = Tcp::connect("192.168.0.110").await?;
    let camera = Arc::new(
        CameraBuilder::tokio()?
            .build_async::<PtzOpticsG2, _>(transport)
            .await?,
    );

    // Save initial state
    let (tx, mut rx) = mpsc::channel(10);

    // Consumer task - executes commands
    let consumer = {
        let cam = camera.clone();
        tokio::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    Command::Home => {
                        println!("Executing: Home");
                        let _ = cam.pan_tilt_home().await;
                        let _ = cam.await_pan_tilt_idle(Duration::from_secs(5)).await;
                    }
                    Command::Preset(n) => {
                        println!("Executing: Preset {n}");
                        if let Ok(preset) = PresetNumber::new(n) {
                            let _ = cam.preset_recall(preset).await;
                            let _ = cam.await_idle(Duration::from_secs(5)).await;
                        }
                    }
                    Command::Zoom(level) => {
                        println!("Executing: Zoom to {level}");
                        let _ = cam.zoom_absolute(Normalized::new(level)).await;
                        let _ = cam.await_zoom_idle(Duration::from_secs(5)).await;
                    }
                }
            }
        })
    };

    // Producer tasks - generate commands
    let producer1 = {
        let tx = tx.clone();
        tokio::spawn(async move {
            tx.send(Command::Home).await.unwrap();
            tx.send(Command::Preset(1)).await.unwrap();
            tx.send(Command::Zoom(0.5)).await.unwrap();
        })
    };

    let producer2 = {
        let tx = tx.clone();
        tokio::spawn(async move {
            tx.send(Command::Preset(2)).await.unwrap();
            tx.send(Command::Zoom(0.75)).await.unwrap();
            tx.send(Command::Preset(3)).await.unwrap();
        })
    };

    // Wait for producers to finish
    let _ = tokio::join!(producer1, producer2);

    // Close channel and wait for consumer
    drop(tx);
    let _ = consumer.await;

    println!("✓ Producer-consumer pattern completed");

    // Restore initial state
    println!("Restoring camera state...");
    println!("✓ Camera restored to initial state");
    println!();

    Ok(())
}

#[cfg(feature = "rt-tokio")]
async fn synchronized_movement() -> Result<()> {
    println!("--- Example 4: Synchronized Multi-Camera Movement ---");

    use tokio::sync::Barrier;

    // Create cameras
    let camera_addrs = vec!["192.168.0.109", "192.168.0.110", "192.168.0.111"];
    let mut cameras = vec![];
    let mut initial_states = vec![];

    for addr in camera_addrs {
        let transport = Tcp::connect(addr).await?;
        let camera = Arc::new(
            CameraBuilder::tokio()?
                .build_async::<PtzOpticsG2, _>(transport)
                .await?,
        );

        // Save initial state
        let state = camera.get_pan_tilt_position().await.ok();
        initial_states.push(state);
        cameras.push(camera);
    }

    let barrier = Arc::new(Barrier::new(cameras.len()));

    // Spawn synchronized tasks
    let mut handles = vec![];

    let cameras_for_movement = cameras.clone();
    for (i, camera) in cameras_for_movement.into_iter().enumerate() {
        let barrier = barrier.clone();

        let handle = tokio::spawn(async move {
            println!("Camera {}: Ready", i + 1);

            barrier.wait().await;
            println!("Camera {}: Moving to home", i + 1);
            camera.pan_tilt_home().await?;

            camera.await_pan_tilt_idle(Duration::from_secs(10)).await?;
            println!("Camera {}: Home position reached", i + 1);

            barrier.wait().await;
            println!("Camera {}: Recalling preset 1", i + 1);
            if let Ok(preset) = PresetNumber::new(1) {
                camera.preset_recall(preset).await?;
                camera.await_idle(Duration::from_secs(10)).await?;
                println!("Camera {}: Preset 1 reached", i + 1);
            }

            Ok::<(), grafton_visca::Error>(())
        });

        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        if let Ok(result) = handle.await {
            result?;
        }
    }

    println!("✓ Synchronized movement completed");

    // Restore all cameras to initial states
    println!("Restoring all camera states...");
    let mut restore_handles = vec![];
    for (camera, state) in cameras.iter().zip(initial_states.iter()) {
        let cam = camera.clone();
        let state = *state;
        let handle = tokio::spawn(async move {
            if let Some((_pan, _tilt)) = state {
                // Move back to initial position
                cam.pan_tilt_home().await?;
            }
            Ok::<(), grafton_visca::Error>(())
        });
        restore_handles.push(handle);
    }

    // Wait for all restorations
    for handle in restore_handles {
        let _ = handle.await;
    }
    println!("✓ All cameras restored to initial states\n");

    Ok(())
}

#[cfg(feature = "rt-tokio")]
#[derive(Debug)]
enum Command {
    Home,
    Preset(u8),
    Zoom(f32),
}
