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

#[cfg(not(feature = "tokio"))]
fn main() {
    println!("This example requires the 'tokio' feature.");
    println!("Run with: cargo run --example concurrent_control --features tokio");
}

#[cfg(feature = "tokio")]
use grafton_visca::{
    camera::profiles::PTZOpticsG2, prelude::r#async::*, types::SpeedLevel, CameraBuilder,
    Normalized, PanTiltDirection, PresetNumber, Result,
};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[cfg(feature = "tokio")]
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

#[cfg(feature = "tokio")]
async fn multi_camera_control() -> Result<()> {
    println!("--- Example 1: Multiple Cameras Simultaneously ---");

    // Create multiple cameras
    let cam1 = Arc::new(
        CameraBuilder::tokio_tcp("192.168.0.110")
            .profile::<PTZOpticsG2>()
            .build()
            .await?,
    );

    let cam2 = Arc::new(
        CameraBuilder::tokio_tcp("192.168.0.111")
            .profile::<PTZOpticsG2>()
            .build()
            .await?,
    );

    let cam3 = Arc::new(
        CameraBuilder::tokio_tcp("192.168.0.112")
            .profile::<PTZOpticsG2>()
            .build()
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
                    sleep(Duration::from_secs(2)).await;
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
            sleep(Duration::from_millis(500)).await;
            cam.pan_tilt_move(
                PanTiltDirection::Right,
                SpeedLevel::Medium.into(),
                SpeedLevel::Slowest.into(),
            )
            .await?;
            sleep(Duration::from_secs(3)).await;
            cam.pan_tilt_stop().await?;
            Ok::<(), grafton_visca::Error>(())
        })
    };

    let handle3 = {
        let cam = cam3.clone();
        tokio::spawn(async move {
            println!("Camera 3: Zoom demonstration");
            cam.zoom_absolute(Normalized::new(0.0)).await?;
            sleep(Duration::from_millis(500)).await;
            cam.zoom_absolute(Normalized::new(0.5)).await?;
            sleep(Duration::from_millis(500)).await;
            cam.zoom_absolute(Normalized::new(1.0)).await?;
            Ok::<(), grafton_visca::Error>(())
        })
    };

    // Wait for all operations to complete
    let (r1, r2, r3) = tokio::join!(handle1, handle2, handle3);

    if r1.is_ok() && r2.is_ok() && r3.is_ok() {
        println!("✓ All cameras operated successfully in parallel\n");
    }

    Ok(())
}

#[cfg(feature = "tokio")]
async fn parallel_single_camera() -> Result<()> {
    println!("--- Example 2: Parallel Operations on Single Camera ---");

    let camera = Arc::new(
        CameraBuilder::tokio_tcp("192.168.0.110")
            .profile::<PTZOpticsG2>()
            .build()
            .await?,
    );

    // Note: Some operations can be done in parallel, others must be sequential
    // Query operations can be parallel
    println!("Querying multiple states in parallel...");

    let (power, position, zoom, focus) = tokio::join!(
        camera.get_power_state(),
        camera.get_pan_tilt_position(),
        camera.get_zoom_position(),
        camera.get_focus_position()
    );

    println!("Power: {:?}", power);
    println!("Position: {:?}", position);
    println!("Zoom: {:?}", zoom);
    println!("Focus: {:?}", focus);

    // Control operations should be coordinated
    println!("\nExecuting coordinated movements...");

    // Start zoom and pan/tilt simultaneously (if camera supports it)
    let zoom_task = {
        let cam = camera.clone();
        tokio::spawn(async move { cam.zoom_in().await })
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

    // Let them run for a bit
    sleep(Duration::from_secs(2)).await;

    // Stop both operations
    camera.zoom_stop().await?;
    camera.pan_tilt_stop().await?;

    let _ = tokio::join!(zoom_task, pan_task);

    println!("✓ Parallel operations completed\n");

    Ok(())
}

#[cfg(feature = "tokio")]
async fn producer_consumer_pattern() -> Result<()> {
    println!("--- Example 3: Producer-Consumer Pattern ---");

    use tokio::sync::mpsc;

    let camera = Arc::new(
        CameraBuilder::tokio_tcp("192.168.0.110")
            .profile::<PTZOpticsG2>()
            .build()
            .await?,
    );

    // Create command channel
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
                    }
                    Command::Preset(n) => {
                        println!("Executing: Preset {n}");
                        if let Ok(preset) = PresetNumber::new(n) {
                            let _ = cam.preset_recall(preset).await;
                        }
                    }
                    Command::Zoom(level) => {
                        println!("Executing: Zoom to {level}");
                        let _ = cam.zoom_absolute(Normalized::new(level)).await;
                    }
                }
                sleep(Duration::from_millis(500)).await;
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

    println!("✓ Producer-consumer pattern completed\n");

    Ok(())
}

#[cfg(feature = "tokio")]
async fn synchronized_movement() -> Result<()> {
    println!("--- Example 4: Synchronized Multi-Camera Movement ---");

    use tokio::sync::Barrier;

    // Create cameras
    let cameras = vec![
        Arc::new(
            CameraBuilder::tokio_tcp("192.168.0.110")
                .profile::<PTZOpticsG2>()
                .build()
                .await?,
        ),
        Arc::new(
            CameraBuilder::tokio_tcp("192.168.0.111")
                .profile::<PTZOpticsG2>()
                .build()
                .await?,
        ),
        Arc::new(
            CameraBuilder::tokio_tcp("192.168.0.112")
                .profile::<PTZOpticsG2>()
                .build()
                .await?,
        ),
    ];

    let barrier = Arc::new(Barrier::new(cameras.len()));

    // Spawn synchronized tasks
    let mut handles = vec![];

    for (i, camera) in cameras.into_iter().enumerate() {
        let barrier = barrier.clone();

        let handle = tokio::spawn(async move {
            println!("Camera {}: Ready", i + 1);

            // Wait for all cameras to be ready
            barrier.wait().await;

            // All cameras move simultaneously
            println!("Camera {}: Moving to home", i + 1);
            camera.pan_tilt_home().await?;

            // Wait for all to complete home
            barrier.wait().await;

            // All cameras recall preset 1 simultaneously
            println!("Camera {}: Recalling preset 1", i + 1);
            if let Ok(preset) = PresetNumber::new(1) {
                camera.preset_recall(preset).await?;
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

    println!("✓ Synchronized movement completed\n");

    Ok(())
}

#[cfg(feature = "tokio")]
#[derive(Debug)]
enum Command {
    Home,
    Preset(u8),
    Zoom(f32),
}
