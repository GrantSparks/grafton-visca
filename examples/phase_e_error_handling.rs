//! # Phase E: Enhanced Error Handling Demo
//!
//! This example demonstrates the enhanced error handling capabilities implemented in Phase E,
//! including the ViscaResultExt trait and ViscaRetry utility for robust camera communication.

use grafton_visca::ViscaError;
use std::time::Duration;

#[cfg(feature = "blocking-client")]
use grafton_visca::{ViscaClient, ViscaResultExt, ViscaRetry};

#[cfg(feature = "async-client")]
use grafton_visca::{AsyncViscaClient, ViscaRetry};

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use grafton_visca::command::{PanTiltCommand, ZoomCommand};

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use log::{info, warn};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🚀 Phase E: Enhanced Error Handling Demo");
    println!("==========================================");

    // Demonstrate blocking error handling if feature is enabled
    #[cfg(feature = "blocking-client")]
    {
        println!("\n📋 Blocking Error Handling Examples:");
        blocking_error_handling_examples()?;
    }

    // Demonstrate async error handling if feature is enabled
    #[cfg(feature = "async-client")]
    {
        println!("\n⚡ Async Error Handling Examples:");
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async_error_handling_examples())?;
    }

    // Show message when no features are enabled
    #[cfg(not(any(feature = "blocking-client", feature = "async-client")))]
    {
        println!("\n⚠️  No client features enabled");
        println!("💡 Enable with one of:");
        println!("   cargo run --example phase_e_error_handling --features blocking-client");
        println!("   cargo run --example phase_e_error_handling --features async-client");
    }

    // Demonstrate error classification (no features required)
    demonstrate_error_classification();

    println!("\n✅ Phase E error handling demonstration completed successfully!");
    Ok(())
}

#[cfg(feature = "blocking-client")]
fn blocking_error_handling_examples() -> Result<(), Box<dyn std::error::Error>> {
    println!("1. Basic Error Context with ViscaResultExt");
    println!("   Using with_retry_context() to add logging to retryable errors");

    // Simulate a result that would benefit from retry context
    let simulated_busy_error: Result<(), ViscaError> = Err(ViscaError::CameraBusy);
    match simulated_busy_error.with_retry_context() {
        Ok(_) => println!("   ✅ Operation succeeded"),
        Err(err) => {
            println!("   ⚠️  Retryable error detected: {}", err);
            if err.is_retryable() {
                println!("   💡 Suggestion: Use ViscaRetry utilities for automatic retry");
                if let Some(delay) = err.suggested_retry_delay() {
                    println!("   ⏱️  Suggested retry delay: {:?}", delay);
                }
            }
        }
    }

    println!("\n2. ViscaRetry::retry_blocking Examples");

    // Example 1: Retry with custom parameters
    println!("   a) Retry with custom base delay:");
    use std::cell::Cell;
    let attempt_count = Cell::new(0);
    let result = ViscaRetry::retry_blocking(
        || {
            // Simulate an operation that fails twice then succeeds
            let count = attempt_count.get() + 1;
            attempt_count.set(count);
            if count < 3 {
                println!("      Attempt {}: Simulating CameraBusy error", count);
                Err(ViscaError::CameraBusy)
            } else {
                println!("      Attempt {}: Success!", count);
                Ok("Operation completed")
            }
        },
        3,                         // max attempts
        Duration::from_millis(50), // base delay
    );

    match result {
        Ok(value) => println!("   ✅ Retry succeeded: {}", value),
        Err(err) => println!("   ❌ Retry failed: {}", err),
    }

    // Reset counter for next example
    println!("\n   b) Non-retryable error (fails immediately):");
    let attempt_count2 = Cell::new(0);
    let result: Result<&str, ViscaError> = ViscaRetry::retry_blocking(
        || {
            let count = attempt_count2.get() + 1;
            attempt_count2.set(count);
            println!("      Attempt {}: Simulating SyntaxError", count);
            Err(ViscaError::SyntaxError)
        },
        3,
        Duration::from_millis(50),
    );

    match result {
        Ok(_) => println!("   ✅ Unexpected success"),
        Err(err) => {
            println!("   ❌ Failed as expected: {}", err);
            println!("   💡 SyntaxError is not retryable, so only 1 attempt was made");
        }
    }

    println!("\n3. Real Camera Operation Examples (if connected)");

    // Try to connect to a camera
    match ViscaClient::connect_udp("127.0.0.1:1259") {
        Ok(client) => {
            info!("   📹 Connected to camera, testing real operations");

            // Try actual camera operations with retry
            let pan_tilt_result = ViscaRetry::retry_blocking(
                || client.send(&PanTiltCommand::Home),
                3,
                Duration::from_millis(100),
            );

            match pan_tilt_result {
                Ok(_) => {
                    info!("   ✅ Pan/Tilt Home command succeeded");

                    // Try zoom operation with different retry strategy
                    let zoom_result = ViscaRetry::retry_blocking(
                        || client.send(&ZoomCommand::TeleStandard),
                        5,
                        Duration::from_millis(200),
                    );

                    match zoom_result {
                        Ok(_) => info!("   ✅ Zoom command succeeded"),
                        Err(err) => warn!("   ⚠️  Zoom command failed: {}", err),
                    }
                }
                Err(err) => {
                    warn!("   ⚠️  Pan/Tilt command failed: {}", err);
                }
            }
        }
        Err(err) => {
            warn!("   ⚠️  Could not connect to camera: {}", err);
            println!("   💡 This is expected if no camera is connected");
            println!("   💡 To test with a real camera, see the control_demo example");
        }
    }

    Ok(())
}

#[cfg(feature = "async-client")]
async fn async_error_handling_examples() -> Result<(), Box<dyn std::error::Error>> {
    println!("1. Async Retry with ViscaRetry::retry_async");

    // Example with simulated retryable error
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    let async_attempt_count = Arc::new(AtomicU32::new(0));
    let async_attempt_count_clone = async_attempt_count.clone();

    let result = ViscaRetry::retry_async(
        move || {
            let count = async_attempt_count_clone.clone();
            async move {
                let current = count.fetch_add(1, Ordering::SeqCst) + 1;
                if current < 3 {
                    println!("   Async attempt {}: Simulating timeout", current);
                    Err(ViscaError::Timeout)
                } else {
                    println!("   Async attempt {}: Success!", current);
                    Ok("Async operation completed")
                }
            }
        },
        4,                          // max attempts
        Duration::from_millis(100), // base delay
    )
    .await;

    match result {
        Ok(value) => println!("   ✅ Async retry succeeded: {}", value),
        Err(err) => println!("   ❌ Async retry failed: {}", err),
    }

    println!("\n2. Retry with Suggested Delays");

    let suggested_attempt_count = Arc::new(AtomicU32::new(0));
    let suggested_attempt_count_clone = suggested_attempt_count.clone();

    let result = ViscaRetry::retry_with_suggested_delay_async(
        move || {
            let count = suggested_attempt_count_clone.clone();
            async move {
                let current = count.fetch_add(1, Ordering::SeqCst) + 1;
                match current {
                    1 => {
                        println!("   Attempt 1: CameraBusy (suggested delay: 100ms)");
                        Err(ViscaError::CameraBusy)
                    }
                    2 => {
                        println!("   Attempt 2: CameraMoving (suggested delay: 500ms)");
                        Err(ViscaError::CameraMoving { pan: 100, tilt: 50 })
                    }
                    3 => {
                        println!("   Attempt 3: CommandTimeout (suggested delay: 1s)");
                        Err(ViscaError::CommandTimeout {
                            duration: Duration::from_secs(5),
                            command: "test".to_string(),
                        })
                    }
                    _ => {
                        println!("   Attempt 4: Success!");
                        Ok("Operation completed with suggested delays")
                    }
                }
            }
        },
        5,
    )
    .await;

    match result {
        Ok(value) => println!("   ✅ Suggested delay retry succeeded: {}", value),
        Err(err) => println!("   ❌ Suggested delay retry failed: {}", err),
    }

    println!("\n3. Exponential Backoff Example");

    let backoff_attempt_count = Arc::new(AtomicU32::new(0));
    let backoff_attempt_count_clone = backoff_attempt_count.clone();

    let result = ViscaRetry::retry_with_exponential_backoff_async(
        move || {
            let count = backoff_attempt_count_clone.clone();
            async move {
                let current = count.fetch_add(1, Ordering::SeqCst) + 1;
                if current < 4 {
                    println!("   Attempt {}: CommandBufferFull", current);
                    Err(ViscaError::CommandBufferFull)
                } else {
                    println!("   Attempt {}: Success!", current);
                    Ok("Exponential backoff completed")
                }
            }
        },
        5,                          // max attempts
        Duration::from_millis(25),  // initial delay: 25ms
        Duration::from_millis(400), // max delay: 400ms
    )
    .await;
    // Delays will be: 25ms, 50ms, 100ms, 200ms, 400ms...

    match result {
        Ok(value) => println!("   ✅ Exponential backoff succeeded: {}", value),
        Err(err) => println!("   ❌ Exponential backoff failed: {}", err),
    }

    println!("\n4. Real Async Camera Operations (if connected)");

    // Try to connect to a camera for real async operations
    let client_result = match AsyncViscaClient::connect_udp("127.0.0.1:1259").await {
        Ok(client) => Ok(client),
        Err(_) => AsyncViscaClient::connect_udp("192.168.1.100:5678").await,
    };

    match client_result {
        Ok(client) => {
            info!("   📹 Connected to camera, testing real operations");

            // Test concurrent operations with retry
            let pan_home = ViscaRetry::retry_async(
                || async { client.send(&PanTiltCommand::Home).await },
                3,
                Duration::from_millis(100),
            );

            let zoom_tele = ViscaRetry::retry_with_suggested_delay_async(
                || async { client.send(&ZoomCommand::TeleStandard).await },
                3,
            );

            let (pan_result, zoom_result) = tokio::join!(pan_home, zoom_tele);

            match pan_result {
                Ok(_) => info!("   ✅ Async pan/tilt home succeeded"),
                Err(err) => warn!("   ⚠️  Async pan/tilt home failed: {}", err),
            }

            match zoom_result {
                Ok(_) => info!("   ✅ Async zoom operation succeeded"),
                Err(err) => warn!("   ⚠️  Async zoom operation failed: {}", err),
            }
        }
        Err(err) => {
            warn!("   ⚠️  Could not connect to camera: {}", err);
            println!("   💡 This is expected if no camera is available");
        }
    }

    Ok(())
}

/// Demonstrate error classification and retry decisions
fn demonstrate_error_classification() {
    println!("\n📊 Error Classification Examples:");

    let errors = vec![
        ViscaError::CameraBusy,
        ViscaError::CameraMoving { pan: 100, tilt: 50 },
        ViscaError::CommandTimeout {
            duration: Duration::from_secs(5),
            command: "test".to_string(),
        },
        ViscaError::CommandBufferFull,
        ViscaError::Timeout,
        ViscaError::SyntaxError,
        ViscaError::CommandNotExecutable,
        ViscaError::PresetNotFound { id: 5 },
        ViscaError::FeatureNotSupported {
            feature: "advanced_zoom".to_string(),
        },
    ];

    for error in errors {
        println!("   Error: {}", error);
        println!("     Retryable: {}", error.is_retryable());
        if let Some(delay) = error.suggested_retry_delay() {
            println!("     Suggested delay: {:?}", delay);
        }
        println!();
    }
}
