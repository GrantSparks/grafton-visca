//! # Phase E: Enhanced Error Handling Demo
//!
//! This example demonstrates the enhanced error handling capabilities implemented in Phase E,
//! including the ViscaResultExt trait and ViscaRetry utility for robust camera communication.

use grafton_visca::{
    command::{PanTiltCommand, ZoomCommand},
    ViscaClient, ViscaError, ViscaResultExt, ViscaRetry,
};
use log::{info, warn};
use std::time::Duration;

#[cfg(feature = "async-client")]
use grafton_visca::AsyncViscaClient;

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

    println!("\n✅ Phase E error handling demonstration completed successfully!");
    Ok(())
}

#[cfg(feature = "blocking-client")]
fn blocking_error_handling_examples() -> Result<(), Box<dyn std::error::Error>> {
    // NOTE: This example uses localhost:1259 which will likely fail,
    // demonstrating error handling in action
    let client = ViscaClient::connect_udp("127.0.0.1:1259")
        .or_else(|_| ViscaClient::connect_udp("192.168.1.100:5678"))
        .unwrap_or_else(|_| {
            // Create a mock client for demonstration
            warn!("Unable to connect to camera, using mock scenarios");
            // For demo purposes, we'll simulate errors manually
            ViscaClient::connect_udp("127.0.0.1:1259").unwrap_or_else(|_| panic!("Demo only"))
        });

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
    let result = ViscaRetry::retry_blocking(
        || {
            // Simulate an operation that fails twice then succeeds
            static mut ATTEMPT_COUNT: u32 = 0;
            unsafe {
                ATTEMPT_COUNT += 1;
                if ATTEMPT_COUNT < 3 {
                    println!(
                        "      Attempt {}: Simulating CameraBusy error",
                        ATTEMPT_COUNT
                    );
                    Err(ViscaError::CameraBusy)
                } else {
                    println!("      Attempt {}: Success!", ATTEMPT_COUNT);
                    Ok("Operation completed")
                }
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
    unsafe {
        static mut ATTEMPT_COUNT2: u32 = 0;
        println!("\n   b) Non-retryable error (fails immediately):");
        let result: Result<&str, ViscaError> = ViscaRetry::retry_blocking(
            || {
                ATTEMPT_COUNT2 += 1;
                println!("      Attempt {}: Simulating SyntaxError", ATTEMPT_COUNT2);
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
    }

    println!("\n3. Real Camera Operation Examples (if connected)");

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
            println!("   💡 This is expected if no camera is connected");
        }
    }

    Ok(())
}

#[cfg(feature = "async-client")]
async fn async_error_handling_examples() -> Result<(), Box<dyn std::error::Error>> {
    println!("1. Async Retry with ViscaRetry::retry_async");

    // Example with simulated retryable error
    let result = ViscaRetry::retry_async(
        || async {
            // Simulate an operation that fails twice then succeeds
            static mut ASYNC_ATTEMPT_COUNT: u32 = 0;
            unsafe {
                ASYNC_ATTEMPT_COUNT += 1;
                if ASYNC_ATTEMPT_COUNT < 3 {
                    println!(
                        "   Async attempt {}: Simulating timeout",
                        ASYNC_ATTEMPT_COUNT
                    );
                    Err(ViscaError::Timeout)
                } else {
                    println!("   Async attempt {}: Success!", ASYNC_ATTEMPT_COUNT);
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

    unsafe {
        static mut SUGGESTED_ATTEMPT_COUNT: u32 = 0;
        let result = ViscaRetry::retry_with_suggested_delay_async(
            || async {
                SUGGESTED_ATTEMPT_COUNT += 1;
                match SUGGESTED_ATTEMPT_COUNT {
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
            },
            5,
        )
        .await;

        match result {
            Ok(value) => println!("   ✅ Suggested delay retry succeeded: {}", value),
            Err(err) => println!("   ❌ Suggested delay retry failed: {}", err),
        }
    }

    println!("\n3. Exponential Backoff Example");

    unsafe {
        static mut BACKOFF_ATTEMPT_COUNT: u32 = 0;
        let result = ViscaRetry::retry_with_exponential_backoff_async(
            || async {
                BACKOFF_ATTEMPT_COUNT += 1;
                if BACKOFF_ATTEMPT_COUNT < 4 {
                    println!("   Attempt {}: CommandBufferFull", BACKOFF_ATTEMPT_COUNT);
                    Err(ViscaError::CommandBufferFull)
                } else {
                    println!("   Attempt {}: Success!", BACKOFF_ATTEMPT_COUNT);
                    Ok("Exponential backoff completed")
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
    }

    println!("\n4. Real Async Camera Operations (if connected)");

    // Try to connect to a camera for real async operations
    match AsyncViscaClient::connect_udp("127.0.0.1:1259")
        .await
        .or_else(|_| async { AsyncViscaClient::connect_udp("192.168.1.100:5678").await })
        .await
    {
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
#[allow(dead_code)]
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
