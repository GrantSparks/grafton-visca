//! Error handling demonstration using the Camera API
//!
//! This example demonstrates error handling patterns with the Camera API,
//! including retry logic and error classification.

#[cfg(not(feature = "async"))]
use grafton_visca::transport::blocking::Tcp;
#[cfg(feature = "tokio")]
use grafton_visca::transport::tokio::Tcp;

use grafton_visca::Error;
#[cfg(any(not(feature = "async"), feature = "tokio"))]
use grafton_visca::{
    camera::{
        methods::{PanTiltMethods, PowerMethods, PresetMethods, ZoomMethods},
        profiles::G2PresetId,
        Camera,
    },
    command::pan_tilt::PanTiltDirection,
    profiles::PTZOpticsG2,
    types::{PanSpeed, TiltSpeed},
};
use std::time::Duration;
#[cfg(any(not(feature = "async"), feature = "tokio"))]
use std::time::Instant;

#[cfg(all(feature = "async", not(feature = "tokio")))]
fn main() {
    println!("=== VISCA Error Handling Demo ===\n");
    println!("This example demonstrates error handling patterns.\n");

    // Still demonstrate error classification even without a transport
    demonstrate_error_classification();

    println!("\nFor full demo with camera connection:");
    println!("Run with: cargo run --example error_handling_demo --features tokio");
}

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=== VISCA Error Handling Demo ===\n");
    println!("This example demonstrates:");
    println!("- Error handling patterns with the Camera API");
    println!("- Implementing retry logic");
    println!("- Classifying different error types\n");

    // Get camera address
    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    // Demonstrate error classification first (no camera needed)
    demonstrate_error_classification();

    // Try to connect and demonstrate error handling
    println!("\n2. Camera Connection and Error Handling:");
    match demonstrate_camera_errors(&camera_addr) {
        Ok(_) => println!("   ✓ Camera demonstration completed"),
        Err(e) => println!("   ✗ Camera demonstration failed: {}", e),
    }

    println!("\n✅ Error handling demonstration completed!");
    Ok(())
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=== VISCA Error Handling Demo ===\n");
    println!("This example demonstrates:");
    println!("- Error handling patterns with the Camera API");
    println!("- Implementing retry logic");
    println!("- Classifying different error types\n");

    // Get camera address
    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    // Demonstrate error classification first (no camera needed)
    demonstrate_error_classification();

    // Try to connect and demonstrate error handling
    println!("\n2. Camera Connection and Error Handling:");
    match demonstrate_camera_errors(&camera_addr).await {
        Ok(_) => println!("   ✓ Camera demonstration completed"),
        Err(e) => println!("   ✗ Camera demonstration failed: {}", e),
    }

    println!("\n✅ Error handling demonstration completed!");
    Ok(())
}

fn demonstrate_error_classification() {
    println!("1. Error Types and Classification:");
    println!("   Different errors require different handling strategies\n");

    // Create example errors to demonstrate
    let errors = vec![
        ("CameraBusy", Error::CameraBusy),
        ("CameraMoving", Error::CameraMoving { pan: 100, tilt: 50 }),
        (
            "CommandTimeout",
            Error::CommandTimeout {
                duration: Duration::from_secs(5),
                command: "zoom".to_string(),
            },
        ),
        ("CommandBufferFull", Error::CommandBufferFull),
        ("Timeout", Error::Timeout),
        ("SyntaxError", Error::SyntaxError),
        ("CommandNotExecutable", Error::CommandNotExecutable),
        ("PresetNotFound", Error::PresetNotFound { id: 5 }),
        (
            "FeatureNotSupported",
            Error::FeatureNotSupported {
                feature: "advanced_zoom".to_string(),
            },
        ),
    ];

    for (name, error) in errors {
        println!("   {}: {}", name, error);

        // Check if error is transient (might succeed on retry)
        let is_transient = matches!(
            error,
            Error::CameraBusy
                | Error::CameraMoving { .. }
                | Error::CommandTimeout { .. }
                | Error::CommandBufferFull
                | Error::Timeout
        );

        println!("     Transient (retryable): {}", is_transient);

        // Suggest retry delay based on error type
        let suggested_delay = match error {
            Error::CameraBusy => Some(Duration::from_millis(100)),
            Error::CameraMoving { .. } => Some(Duration::from_millis(500)),
            Error::CommandTimeout { .. } => Some(Duration::from_secs(1)),
            Error::CommandBufferFull => Some(Duration::from_millis(50)),
            Error::Timeout => Some(Duration::from_millis(200)),
            _ => None,
        };

        if let Some(delay) = suggested_delay {
            println!("     Suggested retry delay: {:?}", delay);
        }
        println!();
    }
}

#[cfg(not(feature = "async"))]
fn demonstrate_camera_errors(camera_addr: &str) -> Result<(), Error> {
    println!("   Attempting to connect to camera at {}...", camera_addr);

    // Try to create transport
    let transport = match Tcp::connect(camera_addr) {
        Ok(t) => {
            println!("   ✓ Transport created successfully");
            t
        }
        Err(e) => {
            println!("   ✗ Failed to create transport: {}", e);
            println!("   💡 This is expected if the address is invalid");
            return Err(Error::ConnectionFailed {
                addr: camera_addr.to_string(),
                source: e,
            });
        }
    };

    let mut camera = Camera::<PTZOpticsG2, _>::new(transport);

    // Demonstrate retry pattern
    println!("\n3. Retry Pattern Implementation:");
    println!("   Implementing exponential backoff for camera operations\n");

    // Example: Retry power on with exponential backoff
    let max_attempts = 3;
    let mut attempt = 0;
    let mut backoff = Duration::from_millis(100);

    let result = loop {
        attempt += 1;
        println!("   Attempt {}/{}: Power on", attempt, max_attempts);

        let start = Instant::now();
        match camera.power_on() {
            Ok(_) => {
                let elapsed = start.elapsed();
                println!("   ✓ Power on succeeded in {:?}", elapsed);
                break Ok(());
            }
            Err(e) => {
                let elapsed = start.elapsed();
                println!("   ✗ Power on failed after {:?}: {}", elapsed, e);

                // Check if error is retryable
                let is_retryable = matches!(
                    e,
                    Error::CameraBusy
                        | Error::CommandTimeout { .. }
                        | Error::CommandBufferFull
                        | Error::Timeout
                );

                if !is_retryable {
                    println!("   💡 Error is not retryable, giving up");
                    break Err(e);
                }

                if attempt >= max_attempts {
                    println!("   💡 Max attempts reached");
                    break Err(e);
                }

                println!("   ⏱️  Waiting {:?} before retry...", backoff);
                std::thread::sleep(backoff);
                backoff *= 2; // Exponential backoff
            }
        }
    };

    if result.is_err() {
        println!("\n   💡 Camera may not be connected or powered off");
        println!("   💡 The retry pattern still demonstrates proper error handling");
        return Ok(()); // Don't fail the demo
    }

    // Wait for camera initialization
    std::thread::sleep(Duration::from_secs(2));

    // Demonstrate handling specific errors
    println!("\n4. Handling Specific Error Scenarios:");

    // Scenario 1: Camera busy during movement
    println!("\n   a) Handling CameraBusy during movement:");

    // Start a movement
    match camera.move_continuous(
        PanTiltDirection::Right,
        PanSpeed::new(10)?,
        TiltSpeed::new(0)?,
    ) {
        Ok(_) => {
            println!("   ✓ Started movement");

            // Try another command immediately (might get CameraBusy)
            match camera.zoom_in() {
                Ok(_) => println!("   ✓ Zoom command accepted"),
                Err(Error::CameraBusy) => {
                    println!("   ⚠️  Camera busy (expected during movement)");
                    println!("   💡 Wait for movement to complete or stop it first");

                    // Stop movement
                    std::thread::sleep(Duration::from_millis(500));
                    camera.pan_tilt_stop()?;

                    // Retry zoom
                    match camera.zoom_in() {
                        Ok(_) => println!("   ✓ Zoom succeeded after stopping movement"),
                        Err(e) => println!("   ✗ Zoom still failed: {}", e),
                    }
                }
                Err(e) => println!("   ✗ Unexpected error: {}", e),
            }
        }
        Err(e) => println!("   ✗ Failed to start movement: {}", e),
    }

    // Scenario 2: Invalid preset
    println!("\n   b) Handling invalid preset:");



    // Try to recall a preset that might not exist
    match G2PresetId::new(99) {
        Ok(preset_id) => match camera.recall_preset(preset_id.into()) {
            Ok(_) => println!("   ✓ Preset 99 recalled successfully"),
            Err(Error::PresetNotFound { id }) => {
                println!("   ⚠️  Preset {} not found (expected)", id);
                println!("   💡 Save preset first or use a different ID");
            }
            Err(e) => println!("   ✗ Unexpected error: {}", e),
        },
        Err(_) => {
            println!("   ⚠️  Preset ID 99 is out of range for this camera");
            println!("   💡 PTZOptics G2 supports presets 0-99");
        }
    }

    // Scenario 3: Feature not supported
    println!("\n   c) Handling unsupported features:");
    println!("   💡 Some cameras don't support all VISCA features");
    println!("   💡 Use capability queries to check support");

    // Get camera capabilities (profile-based, not from camera)
    let caps = camera.capabilities();
    println!("   ✓ Camera capabilities (from profile):");
    println!("     Model: {}", caps.model_name);
    println!("     Pan range: {:?} degrees", caps.pan_range_degrees);
    println!("     Tilt range: {:?} degrees", caps.tilt_range_degrees);
    println!("     Preset count: {}", caps.preset_count);
    println!("   💡 These are based on the camera profile, not runtime queries");

    Ok(())
}

#[cfg(feature = "tokio")]
async fn demonstrate_camera_errors(camera_addr: &str) -> Result<(), Error> {
    use std::time::Duration;
    println!("   Attempting to connect to camera at {}...", camera_addr);

    // Try to create transport
    let transport = match Tcp::connect_timeout(camera_addr, Duration::from_secs(5)).await {
        Ok(t) => {
            println!("   ✓ Transport created successfully");
            t
        }
        Err(e) => {
            println!("   ✗ Failed to create transport: {}", e);
            println!("   💡 This is expected if the address is invalid");
            return Err(e);
        }
    };

    let camera = Camera::<PTZOpticsG2, _>::new(transport);

    // Demonstrate retry pattern
    println!("\n3. Retry Pattern Implementation:");
    println!("   Implementing exponential backoff for camera operations\n");

    // Example: Retry power on with exponential backoff
    let max_attempts = 3;
    let mut attempt = 0;
    let mut backoff = Duration::from_millis(100);

    let result = loop {
        attempt += 1;
        println!("   Attempt {}/{}: Power on", attempt, max_attempts);

        let start = Instant::now();
        match camera.power_on().await {
            Ok(_) => {
                let elapsed = start.elapsed();
                println!("   ✓ Power on succeeded in {:?}", elapsed);
                break Ok(());
            }
            Err(e) => {
                let elapsed = start.elapsed();
                println!("   ✗ Power on failed after {:?}: {}", elapsed, e);

                // Check if error is retryable
                let is_retryable = matches!(
                    e,
                    Error::CameraBusy
                        | Error::CommandTimeout { .. }
                        | Error::CommandBufferFull
                        | Error::Timeout
                );

                if !is_retryable || attempt >= max_attempts {
                    println!("   ✗ Error is not retryable or max attempts reached");
                    break Err(e);
                }

                println!("   ⏳ Waiting {:?} before retry...", backoff);
                tokio::time::sleep(backoff).await;
                backoff *= 2; // Exponential backoff
            }
        }
    };

    match result {
        Ok(_) => println!("   ✓ Power on successful after {} attempt(s)", attempt),
        Err(e) => println!("   ✗ Power on failed after {} attempts: {}", attempt, e),
    }

    // Scenario 1: Camera busy handling
    println!("\n4. Common Error Scenarios:");
    println!("   a) Camera busy while moving:");

    // Start a movement
    match camera
        .move_continuous(
            PanTiltDirection::Right,
            PanSpeed::new(10)?,
            TiltSpeed::new(0)?,
        )
        .await
    {
        Ok(_) => {
            println!("   ✓ Started continuous movement");

            // Immediately try another command
            match camera.zoom_in().await {
                Ok(_) => println!("   ✓ Zoom command accepted"),
                Err(Error::CameraBusy) => {
                    println!("   ⚠️  Camera busy (expected)");
                    println!("   💡 Solution: Stop movement first or wait");

                    // Stop movement and retry
                    camera.pan_tilt_stop().await?;
                    println!("   ✓ Movement stopped");

                    // Retry zoom
                    match camera.zoom_in().await {
                        Ok(_) => println!("   ✓ Zoom succeeded after stopping movement"),
                        Err(e) => println!("   ✗ Zoom still failed: {}", e),
                    }
                }
                Err(e) => println!("   ✗ Unexpected error: {}", e),
            }
        }
        Err(e) => println!("   ✗ Failed to start movement: {}", e),
    }

    // Scenario 2: Invalid preset
    println!("\n   b) Handling invalid preset:");



    // Try to recall a preset that might not exist
    match G2PresetId::new(99) {
        Ok(preset_id) => match camera.recall_preset(preset_id.into()).await {
            Ok(_) => println!("   ✓ Preset 99 recalled successfully"),
            Err(Error::PresetNotFound { id }) => {
                println!("   ⚠️  Preset {} not found (expected)", id);
                println!("   💡 Save preset first or use a different ID");
            }
            Err(e) => println!("   ✗ Unexpected error: {}", e),
        },
        Err(_) => {
            println!("   ⚠️  Preset ID 99 is out of range for this camera");
            println!("   💡 PTZOptics G2 supports presets 0-99");
        }
    }

    // Scenario 3: Feature not supported
    println!("\n   c) Handling unsupported features:");
    println!("   💡 Some cameras don't support all VISCA features");
    println!("   💡 Use capability queries to check support");

    // Get camera capabilities (profile-based, not from camera)
    let caps = camera.capabilities();
    println!("   ✓ Camera capabilities (from profile):");
    println!("     Model: {}", caps.model_name);
    println!("     Pan range: {:?} degrees", caps.pan_range_degrees);
    println!("     Tilt range: {:?} degrees", caps.tilt_range_degrees);
    println!("     Preset count: {}", caps.preset_count);
    println!("   💡 These are based on the camera profile, not runtime queries");

    Ok(())
}
