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
//! cargo run --example concurrent_control --features runtime-tokio -- [camera_ip[:port]]
//! ```

mod support;

use std::{env, fmt::Display, io, sync::Arc};

use tokio::{
    sync::mpsc,
    time::{sleep, Duration},
};

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, Connect},
    types::SpeedLevel,
    units::UnitInterval,
    Error, PanTiltDirection, PresetNumber, TokioRuntime,
};
use support::finish_session;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let camera_addr = camera_address()?;

    println!("=== Concurrent Camera Control Demo ===\n");
    println!("Camera address: {camera_addr}\n");

    // Example 1: Parallel operations on single camera
    parallel_operations(&camera_addr).await?;

    // Example 2: Producer-consumer pattern
    producer_consumer_pattern(&camera_addr).await?;

    println!("\n=== Demo completed ===");

    Ok(())
}

fn camera_address() -> Result<String, io::Error> {
    let mut values = env::args().skip(1);
    let address = match values.next() {
        Some(value) if value.starts_with('-') => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown option `{value}`"),
            ));
        }
        Some(value) => value,
        None => env::var("VISCA_CAMERA_ADDR").unwrap_or_else(|_| "192.168.0.110".to_string()),
    };

    if let Some(extra) = values.next() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "unexpected extra argument `{extra}`\nusage: cargo run --example concurrent_control --features runtime-tokio -- [address]"
            ),
        ));
    }

    Ok(address)
}

async fn parallel_operations(camera_addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 1: Parallel Operations ---");

    let runtime = TokioRuntime::from_current()?;
    let camera = Connect::open_tcp_async::<PtzOpticsG2, _>(camera_addr, runtime).await?;

    let operation_result = async {
        println!("Querying multiple states in parallel...");

        // Query multiple camera states concurrently
        // Note: accessors must be bound before passing to tokio::join!
        let power_acc = camera.power();
        let pan_tilt_acc = camera.pan_tilt();
        let zoom_acc = camera.zoom();
        let focus_acc = camera.focus();

        let (power, pan_tilt, zoom, focus) = tokio::try_join!(
            power_acc.state(),
            pan_tilt_acc.position(),
            zoom_acc.position(),
            focus_acc.position()
        )?;

        println!("Power: {power:?}");
        println!("Pan/Tilt: {pan_tilt:?}");
        println!("Zoom: {zoom:?}");
        println!("Focus: {focus:?}");

        println!("\nExecuting coordinated movements...");

        // Dispatch both starts and observe both results before issuing either stop.
        let zoom = camera.zoom();
        let pan_tilt = camera.pan_tilt();
        let (zoom_start, pan_start) = tokio::join!(
            zoom.tele(),
            pan_tilt.move_direction(
                PanTiltDirection::UpRight,
                SpeedLevel::Slow.into(),
                SpeedLevel::Slow.into(),
            )
        );

        // Let movements run briefly
        sleep(Duration::from_millis(100)).await;

        // Attempt both stops even if one start or stop reported an error. A timed-out
        // start may still have reached the camera, so stopping is the safe cleanup.
        let zoom = camera.zoom();
        let pan_tilt = camera.pan_tilt();
        let (zoom_stop, pan_stop) = tokio::join!(zoom.stop(), pan_tilt.stop());

        report_result("zoom start", &zoom_start);
        report_result("pan/tilt start", &pan_start);
        report_result("zoom STOP", &zoom_stop);
        report_result("pan/tilt STOP", &pan_stop);

        zoom_start?;
        pan_start?;
        zoom_stop?;
        pan_stop?;

        // Wait for movements to settle
        tokio::try_join!(
            camera.await_zoom_idle(Duration::from_secs(5)),
            camera.await_pan_tilt_idle(Duration::from_secs(5))
        )?;

        println!("✓ Parallel operations completed");

        // Return to home
        println!("Returning to home position...");
        camera.pan_tilt().home().await?;
        camera.await_idle(Duration::from_secs(5)).await?;
        println!("✓ Camera returned to home\n");

        Ok::<(), Error>(())
    }
    .await;
    let close_result = camera.close().await;

    finish_session(operation_result, close_result)?;
    Ok(())
}

async fn producer_consumer_pattern(camera_addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 2: Producer-Consumer Pattern ---");

    let runtime = TokioRuntime::from_current()?;
    let camera = Arc::new(Connect::open_tcp_async::<PtzOpticsG2, _>(camera_addr, runtime).await?);

    let (tx, mut rx) = mpsc::channel(10);

    // Consumer task - executes commands sequentially
    let consumer = {
        let cam = camera.clone();
        tokio::spawn(async move {
            let max_preset = cam.camera().capabilities().max_presets;

            while let Some(cmd) = rx.recv().await {
                match cmd {
                    Command::Home => {
                        println!("  Executing: Home");
                        cam.pan_tilt().home().await?;
                        cam.await_pan_tilt_idle(Duration::from_secs(5)).await?;
                    }
                    Command::Preset(preset) => {
                        let preset = preset.value();
                        println!("  Executing: Preset {preset}");
                        if preset > max_preset {
                            return Err(Error::ParameterOutOfRange {
                                parameter: "preset_number",
                                value: i32::from(preset),
                                min: 0,
                                max: i32::from(max_preset),
                            });
                        }
                        cam.presets().recall(preset).await?;
                        cam.await_idle(Duration::from_secs(5)).await?;
                    }
                    Command::Zoom(level) => {
                        println!("  Executing: Zoom to {:.0}%", level.value() * 100.0);
                        cam.zoom().set_normalized(level).await?;
                        cam.await_zoom_idle(Duration::from_secs(5)).await?;
                    }
                }
            }

            Ok::<(), Error>(())
        })
    };

    let preset_1 = PresetNumber::new(1)?;
    let preset_2 = PresetNumber::new(2)?;
    let preset_3 = PresetNumber::new(3)?;
    let zoom_half = UnitInterval::new(0.5)?;
    let zoom_three_quarters = UnitInterval::new(0.75)?;

    // Producer 1 - sends a sequence of commands
    let producer1 = {
        let tx = tx.clone();
        tokio::spawn(async move {
            tx.send(Command::Home).await?;
            tx.send(Command::Preset(preset_1)).await?;
            tx.send(Command::Zoom(zoom_half)).await?;
            Ok::<(), mpsc::error::SendError<Command>>(())
        })
    };

    // Producer 2 - sends another sequence
    let producer2 = {
        let tx = tx.clone();
        tokio::spawn(async move {
            tx.send(Command::Preset(preset_2)).await?;
            tx.send(Command::Zoom(zoom_three_quarters)).await?;
            tx.send(Command::Preset(preset_3)).await?;
            Ok::<(), mpsc::error::SendError<Command>>(())
        })
    };

    // Wait for producers to finish sending
    let (producer1_result, producer2_result) = tokio::join!(producer1, producer2);

    // Close channel and wait for consumer to finish
    drop(tx);
    let consumer_result = consumer.await;
    let camera = Arc::try_unwrap(camera)
        .map_err(|_| io::Error::other("camera still has outstanding task owners"))?;
    let close_result = camera.close().await;

    report_task_result("producer 1", &producer1_result);
    report_task_result("producer 2", &producer2_result);
    report_task_result("consumer", &consumer_result);
    let task_failed = task_failed(&producer1_result)
        || task_failed(&producer2_result)
        || task_failed(&consumer_result);
    if task_failed {
        if let Err(error) = &close_result {
            eprintln!("The camera session also failed to close cleanly: {error}");
        }
    }

    producer1_result??;
    producer2_result??;
    consumer_result??;
    close_result?;

    println!("✓ Producer-consumer pattern completed\n");

    Ok(())
}

#[derive(Debug)]
enum Command {
    Home,
    Preset(PresetNumber),
    Zoom(UnitInterval),
}

fn report_result(label: &str, result: &Result<(), Error>) {
    if let Err(error) = result {
        eprintln!("{label} failed: {error}");
    }
}

fn report_task_result<E>(label: &str, result: &Result<Result<(), E>, tokio::task::JoinError>)
where
    E: Display,
{
    match result {
        Ok(Err(error)) => eprintln!("{label} failed: {error}"),
        Err(error) => eprintln!("{label} task failed: {error}"),
        Ok(Ok(())) => {}
    }
}

fn task_failed<E>(result: &Result<Result<(), E>, tokio::task::JoinError>) -> bool {
    !matches!(result, Ok(Ok(())))
}
