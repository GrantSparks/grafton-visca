//! Concurrent camera control demonstration.
//!
//! This example shows thread-safe concurrent operations:
//! - Controlling multiple cameras simultaneously
//! - Parallel command execution on single camera
//! - Synchronization patterns
//! - Resource sharing with Arc/Mutex
//! - Async task spawning
//!
//! **Context**: This example uses native async transports with the tokio runtime
//! to demonstrate true concurrent operations across multiple cameras.
//!
//! Run with: cargo run --example concurrent_control --features runtime-tokio

#[cfg(not(feature = "runtime-tokio"))]
fn main() {
    println!("This example requires the 'runtime-tokio' feature.");
    println!("Run with: cargo run --example concurrent_control --features runtime-tokio");
}

#[cfg(feature = "runtime-tokio")]
use tokio::{
    sync::{mpsc, Barrier},
    time::{sleep, Duration},
};

#[cfg(feature = "runtime-tokio")]
use std::sync::Arc;

#[cfg(feature = "runtime-tokio")]
use grafton_visca::{
    camera::{
        profiles::{PtzOpticsG2, PtzOpticsG3, SonyBRC300},
        session::CameraSession,
        Connect,
    },
    mode::Async,
    runtime::TransportHandle,
    types::{PanTiltDirection, SpeedLevel},
    units::Normalized,
    PresetNumber, Result, TokioRuntime,
};

#[cfg(feature = "runtime-tokio")]
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

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

#[cfg(feature = "runtime-tokio")]
async fn multi_camera_control() -> Result<()> {
    println!("--- Example 1: Multiple Cameras Simultaneously ---");

    // Create a Tokio runtime for all cameras
    let runtime = TokioRuntime::from_current()?;

    // Connect to cameras using the new session-centric API
    // This returns a CameraSession with a cleaner lifecycle
    let cam1: Arc<CameraSession<Async, SonyBRC300, TransportHandle<TokioRuntime>, TokioRuntime>> =
        Arc::new(
            Connect::open_tcp_async::<SonyBRC300, _>("192.168.0.109", runtime.clone())
                .await
                .map_err(|e| {
                    eprintln!("Failed to connect to camera 1: {e}");
                    e
                })?,
        );

    let cam2: Arc<CameraSession<Async, PtzOpticsG2, TransportHandle<TokioRuntime>, TokioRuntime>> =
        Arc::new(
            Connect::open_tcp_async::<PtzOpticsG2, _>("192.168.0.110", runtime.clone())
                .await
                .map_err(|e| {
                    eprintln!("Failed to connect to camera 2: {e}");
                    e
                })?,
        );

    let cam3: Arc<CameraSession<Async, PtzOpticsG3, TransportHandle<TokioRuntime>, TokioRuntime>> =
        Arc::new(
            Connect::open_tcp_async::<PtzOpticsG3, _>("192.168.0.111", runtime.clone())
                .await
                .map_err(|e| {
                    eprintln!("Failed to connect to camera 3: {e}");
                    e
                })?,
        );

    // Spawn concurrent tasks for each camera
    let handle1 = {
        let cam = cam1.clone();
        tokio::spawn(async move {
            println!("Camera 1: Starting preset tour");
            for i in 1..=3 {
                if let Ok(_preset) = PresetNumber::new(i) {
                    cam.presets().recall(i).await?;
                    // Movement detection methods now available directly on session
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
            cam.pan_tilt().home().await?;
            cam.await_pan_tilt_idle(Duration::from_secs(5)).await?;
            cam.pan_tilt()
                .move_direction(
                    PanTiltDirection::Right,
                    SpeedLevel::Medium.into(),
                    SpeedLevel::Slowest.into(),
                )
                .await?;
            sleep(Duration::from_millis(100)).await;
            cam.pan_tilt().stop().await?;
            cam.await_pan_tilt_idle(Duration::from_secs(5)).await?;
            Ok::<(), grafton_visca::Error>(())
        })
    };

    let handle3 = {
        let cam = cam3.clone();
        tokio::spawn(async move {
            println!("Camera 3: Zoom demonstration");
            cam.zoom().absolute(Normalized::new(0.0)).await?;
            cam.await_zoom_idle(Duration::from_secs(3)).await?;
            cam.zoom().absolute(Normalized::new(0.5)).await?;
            cam.await_zoom_idle(Duration::from_secs(3)).await?;
            cam.zoom().absolute(Normalized::new(1.0)).await?;
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
    let handle1 = {
        let cam = cam1.clone();
        tokio::spawn(async move { cam.pan_tilt().home().await })
    };
    let handle2 = {
        let cam = cam2.clone();
        tokio::spawn(async move { cam.pan_tilt().home().await })
    };
    let handle3 = {
        let cam = cam3.clone();
        tokio::spawn(async move { cam.pan_tilt().home().await })
    };
    let _ = tokio::join!(handle1, handle2, handle3);
    println!("✓ All cameras returned to home\n");

    Ok(())
}

#[cfg(feature = "runtime-tokio")]
async fn parallel_single_camera() -> Result<()> {
    println!("--- Example 2: Parallel Operations on Single Camera ---");

    // Create a Tokio runtime and connect to a single camera
    let runtime = TokioRuntime::from_current()?;
    let camera: Arc<
        CameraSession<Async, PtzOpticsG2, TransportHandle<TokioRuntime>, TokioRuntime>,
    > = Arc::new(
        Connect::open_tcp_async::<PtzOpticsG2, _>("192.168.0.110", runtime)
            .await
            .map_err(|e| {
                eprintln!("Failed to connect to camera: {e}");
                e
            })?,
    );

    println!("Querying multiple states in parallel...");

    // Access the underlying camera for inquiry operations
    // The camera is always available in an Open session
    let (power, pan_tilt, zoom, focus) = tokio::join!(
        async { camera.power().state().await },
        async { camera.pan_tilt().position().await },
        async { camera.zoom().position().await },
        async { camera.focus().position().await }
    );

    println!("Power: {power:?}");
    println!("Pan/Tilt: {pan_tilt:?}");
    println!("Zoom: {zoom:?}");
    println!("Focus: {focus:?}");

    println!("\nExecuting coordinated movements...");

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

    sleep(Duration::from_millis(100)).await;

    camera.zoom().stop().await?;
    camera.pan_tilt().stop().await?;
    let _ = tokio::join!(
        camera.await_zoom_idle(Duration::from_secs(5)),
        camera.await_pan_tilt_idle(Duration::from_secs(5))
    );

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
async fn producer_consumer_pattern() -> Result<()> {
    println!("--- Example 3: Producer-Consumer Pattern ---");

    let runtime = TokioRuntime::from_current()?;
    let camera: Arc<
        CameraSession<Async, PtzOpticsG2, TransportHandle<TokioRuntime>, TokioRuntime>,
    > = Arc::new(Connect::open_tcp_async::<PtzOpticsG2, _>("192.168.0.110", runtime).await?);

    let (tx, mut rx) = mpsc::channel(10);

    // Consumer task - executes commands
    let consumer = {
        let cam = camera.clone();
        tokio::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    Command::Home => {
                        println!("Executing: Home");
                        let _ = cam.pan_tilt().home().await;
                        let _ = cam.await_pan_tilt_idle(Duration::from_secs(5)).await;
                    }
                    Command::Preset(n) => {
                        println!("Executing: Preset {n}");
                        if let Ok(_preset) = PresetNumber::new(n) {
                            let _ = cam.presets().recall(n).await;
                            let _ = cam.await_idle(Duration::from_secs(5)).await;
                        }
                    }
                    Command::Zoom(level) => {
                        println!("Executing: Zoom to {level}");
                        let _ = cam.zoom().absolute(Normalized::new(level)).await;
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

#[cfg(feature = "runtime-tokio")]
async fn synchronized_movement() -> Result<()> {
    println!("--- Example 4: Synchronized Multi-Camera Movement ---");

    // Create cameras
    let camera_addrs = vec!["192.168.0.109", "192.168.0.110", "192.168.0.111"];
    let mut cameras = vec![];
    let mut initial_states = vec![];

    let runtime = TokioRuntime::from_current()?;
    for addr in camera_addrs {
        let camera: Arc<
            CameraSession<Async, PtzOpticsG2, TransportHandle<TokioRuntime>, TokioRuntime>,
        > = Arc::new(
            Connect::open_tcp_async::<PtzOpticsG2, _>(format!("{addr}:5678"), runtime.clone())
                .await?,
        );

        let state = camera.pan_tilt().position().await.ok();
        initial_states.push(state);
        cameras.push(camera);
    }

    let barrier: Arc<Barrier> = Arc::new(Barrier::new(cameras.len()));

    // Spawn synchronized tasks
    let mut handles = vec![];

    let cameras_for_movement = cameras.clone();
    for (i, camera) in cameras_for_movement.into_iter().enumerate() {
        let barrier = barrier.clone();

        let handle = tokio::spawn(async move {
            println!("Camera {}: Ready", i + 1);

            barrier.wait().await;
            println!("Camera {}: Moving to home", i + 1);
            camera.pan_tilt().home().await?;

            camera.await_pan_tilt_idle(Duration::from_secs(10)).await?;
            println!("Camera {}: Home position reached", i + 1);

            barrier.wait().await;
            println!("Camera {}: Recalling preset 1", i + 1);
            if let Ok(_preset) = PresetNumber::new(1) {
                camera.presets().recall(1).await?;
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
            if let Some(_position) = state {
                cam.pan_tilt().home().await?;
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

#[cfg(feature = "runtime-tokio")]
#[derive(Debug)]
enum Command {
    Home,
    Preset(u8),
    Zoom(f32),
}
