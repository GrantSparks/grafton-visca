//! Demonstration of new Camera<P> API features:
//! - CameraPool for multi-camera management
//! - ResilientTransport for automatic reconnection
//! - CommandBuilder for fluent command sequences
//! - Extension traits for custom functionality

use grafton_visca::{
    camera::{profiles::PTZOpticsG2, Camera, CameraExtension, CommandBuilderExt, DiagnosticsExt},
    camera_pool::{CameraInfo, CameraPool, PoolConfig},
    transport::{
        resilient::{ResilienceConfig, ResilienceEvent, ResilientTransport},
        UdpTransport,
    },
    Error,
};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Demonstrate all new features
    println!("=== VISCA Camera New Features Demo ===\n");

    // 1. Resilient Transport Demo
    println!("1. Resilient Transport with automatic reconnection:");
    resilient_transport_demo().await?;

    println!("\n2. Camera Pool for multi-camera management:");
    camera_pool_demo().await?;

    println!("\n3. Command Builder for fluent sequences:");
    command_builder_demo().await?;

    println!("\n4. Extension traits for custom functionality:");
    extension_traits_demo().await?;

    Ok(())
}

/// Demonstrates ResilientTransport with automatic retry and reconnection
async fn resilient_transport_demo() -> Result<(), Error> {
    println!("Creating resilient transport with automatic retry...");

    // Configure resilience
    let config = ResilienceConfig {
        max_retries: 3,
        initial_retry_delay: Duration::from_millis(100),
        max_retry_delay: Duration::from_secs(5),
        backoff_factor: 2.0,
        max_reconnect_attempts: 5,
        reconnect_delay: Duration::from_secs(1),
        health_check_interval: Some(Duration::from_secs(30)),
        operation_timeout: Duration::from_secs(5),
    };

    // Create base transport
    let base_transport = UdpTransport::new("192.168.1.100:52381")?;

    // Wrap with resilient transport
    let mut resilient = ResilientTransport::new(
        base_transport,
        || UdpTransport::new("192.168.1.100:52381"),
        config,
    );

    // Set up event monitoring
    resilient.set_event_callback(Arc::new(|event| match event {
        ResilienceEvent::OperationSucceeded { retries } => {
            println!("  ✓ Operation succeeded after {} retries", retries);
        }
        ResilienceEvent::OperationFailed { attempts, error } => {
            println!(
                "  ✗ Operation failed after {} attempts: {}",
                attempts, error
            );
        }
        ResilienceEvent::Reconnected { attempts } => {
            println!("  ↻ Reconnected after {} attempts", attempts);
        }
        ResilienceEvent::HealthCheckPassed => {
            println!("  ♥ Health check passed");
        }
        _ => {}
    }));

    // Create camera with resilient transport
    let mut camera = Camera::<PTZOpticsG2>::new(resilient);

    // Operations will automatically retry on failure
    println!("Executing commands with automatic retry...");
    camera.power_on().await?;
    camera.home().await?;

    // Get statistics (would need to keep reference to resilient transport)
    println!("Resilience stats: Available via ResilientTransport reference");

    Ok(())
}

/// Demonstrates CameraPool for managing multiple cameras
async fn camera_pool_demo() -> Result<(), Error> {
    println!("Creating camera pool for multi-camera management...");

    // Configure pool
    let config = PoolConfig {
        health_check_interval: Duration::from_secs(60),
        auto_remove_unhealthy: true,
        max_idle_time: Some(Duration::from_secs(300)),
        max_cameras: Some(10),
    };

    let pool = CameraPool::<PTZOpticsG2>::new(config);

    // Add cameras to pool
    for i in 1..=3 {
        let info = CameraInfo::new(format!("cam{}", i))
            .with_name(format!("Camera {}", i))
            .with_location(format!("Studio {}", i))
            .with_metadata("ip", format!("192.168.1.{}", 100 + i));

        // In real code, create actual transports
        let transport = Box::new(UdpTransport::new(&format!("192.168.1.{}:52381", 100 + i))?);

        match pool.add_camera_async(info, transport).await {
            Ok(camera) => println!("  ✓ Added camera {} to pool", i),
            Err(e) => println!("  ✗ Failed to add camera {}: {}", i, e),
        }
    }

    // Execute command on specific camera
    println!("\nExecuting command on camera 'cam1':");
    let result = pool
        .with_camera_async(
            "cam1",
            |camera| async move { camera.power_state_async().await },
        )
        .await;

    match result {
        Ok(power_on) => println!(
            "  Camera 1 power state: {}",
            if power_on { "ON" } else { "OFF" }
        ),
        Err(e) => println!("  Failed to query camera 1: {}", e),
    }

    // Execute command on all cameras concurrently
    println!("\nExecuting home command on all cameras concurrently:");
    let results = pool
        .with_all_cameras_async(|camera| async move { camera.home_async().await })
        .await;

    for (id, result) in results {
        match result {
            Ok(_) => println!("  ✓ Camera {} moved to home", id),
            Err(e) => println!("  ✗ Camera {} failed: {}", id, e),
        }
    }

    // Get pool statistics
    println!("\nPool statistics:");
    for stats in pool.get_all_stats() {
        println!(
            "  {} - Success: {}, Failed: {}, Healthy: {}",
            stats.info.id, stats.successful_ops, stats.failed_ops, stats.is_healthy
        );
    }

    // Perform maintenance
    println!("\nPerforming pool maintenance...");
    let report = pool.maintenance();
    println!(
        "  Removed {} unhealthy, {} stale cameras",
        report.unhealthy_removed.len(),
        report.stale_removed.len()
    );

    Ok(())
}

/// Demonstrates CommandBuilder for fluent command sequences
async fn command_builder_demo() -> Result<(), Error> {
    println!("Using CommandBuilder for complex sequences...");

    let transport = UdpTransport::new("192.168.1.100:52381")?;
    let camera = Camera::<PTZOpticsG2>::new(transport);

    // Build and execute a complex sequence
    println!("Building camera initialization sequence:");
    let results = camera
        .commands()
        .power_on()
        .pan_tilt_home()
        .zoom_stop()
        .focus_auto()
        .exposure_mode(grafton_visca::command::exposure::ExposureMode::Auto)
        .white_balance_mode(grafton_visca::command::white_balance::WhiteBalanceMode::Auto)
        .backlight(false)
        .execute_sequential_async()
        .await?;

    println!("  ✓ Executed {} commands successfully", results.len());

    // Build a movement sequence
    println!("\nBuilding movement sequence:");
    let movement_results = camera
        .commands()
        .pan_tilt_to_degrees(
            45.0,
            0.0,
            grafton_visca::command::pan_tilt::PanSpeed::new(15).unwrap(),
            grafton_visca::command::pan_tilt::TiltSpeed::new(15).unwrap(),
        )?
        .zoom_in(grafton_visca::command::zoom::ZoomSpeed::new(5).unwrap())
        .preset_set(PTZOpticsG2::PresetId::from(1))
        .pan_tilt_to_degrees(
            -45.0,
            0.0,
            grafton_visca::command::pan_tilt::PanSpeed::new(15).unwrap(),
            grafton_visca::command::pan_tilt::TiltSpeed::new(15).unwrap(),
        )?
        .zoom_out(grafton_visca::command::zoom::ZoomSpeed::new(5).unwrap())
        .preset_set(PTZOpticsG2::PresetId::from(2))
        .execute_sequential_async()
        .await?;

    println!(
        "  ✓ Movement sequence completed ({} commands)",
        movement_results.len()
    );

    // Demonstrate concurrent execution
    println!("\nExecuting concurrent commands:");
    let concurrent_results = camera
        .commands()
        .custom(
            grafton_visca::command::inquiry::PowerStateInquiry,
            "Query Power State",
        )
        .custom(
            grafton_visca::command::inquiry::ZoomPositionInquiry,
            "Query Zoom Position",
        )
        .custom(
            grafton_visca::command::inquiry::FocusModeInquiry,
            "Query Focus Mode",
        )
        .execute_concurrent_async()
        .await?;

    let successful = concurrent_results.iter().filter(|r| r.is_ok()).count();
    println!(
        "  ✓ {} of {} concurrent queries succeeded",
        successful,
        concurrent_results.len()
    );

    Ok(())
}

/// Demonstrates extension traits for custom functionality
async fn extension_traits_demo() -> Result<(), Error> {
    println!("Using extension traits for custom functionality...");

    let transport = UdpTransport::new("192.168.1.100:52381")?;
    let mut camera = Camera::<PTZOpticsG2>::new(transport);

    // Use DiagnosticsExt trait
    println!("Getting camera diagnostics:");
    let diagnostics = camera.get_diagnostics()?;
    println!("  Model: {}", diagnostics.model);
    println!("  Error count: {}", diagnostics.error_count);

    println!("\nRunning self-test:");
    let test_result = camera.run_self_test()?;
    println!(
        "  Pan/Tilt: {}",
        if test_result.pan_tilt_ok {
            "✓"
        } else {
            "✗"
        }
    );
    println!("  Zoom: {}", if test_result.zoom_ok { "✓" } else { "✗" });
    println!("  Focus: {}", if test_result.focus_ok { "✓" } else { "✗" });
    println!(
        "  Exposure: {}",
        if test_result.exposure_ok {
            "✓"
        } else {
            "✗"
        }
    );

    // Use ScriptingExt trait
    use grafton_visca::camera::extensions::{MovementAction, MovementStep, ScriptingExt};

    println!("\nExecuting movement script:");
    let script = vec![
        MovementStep {
            action: MovementAction::PanTilt {
                pan: 0.0,
                tilt: 0.0,
            },
            label: Some("Home".to_string()),
        },
        MovementStep {
            action: MovementAction::Wait {
                duration: Duration::from_secs(1),
            },
            label: Some("Pause".to_string()),
        },
        MovementStep {
            action: MovementAction::PanTilt {
                pan: 90.0,
                tilt: 15.0,
            },
            label: Some("Look right".to_string()),
        },
        MovementStep {
            action: MovementAction::Zoom { level: 5000 },
            label: Some("Zoom in".to_string()),
        },
    ];

    camera.execute_movement_script(&script)?;
    println!("  ✓ Script execution completed");

    Ok(())
}

// Define a custom extension trait
use grafton_visca::camera_extension_trait;

camera_extension_trait! {
    /// Custom broadcast-specific extensions.
    trait BroadcastExt {
        /// Set up camera for broadcast (custom preset).
        fn setup_for_broadcast(&mut self) -> Result<(), Error> {
            println!("  Setting up camera for broadcast...");
            // In real implementation, this would configure multiple settings
            Ok(())
        }

        /// Enable tally light (if supported).
        fn set_tally(&mut self, on: bool) -> Result<(), Error> {
            println!("  Tally light: {}", if on { "ON" } else { "OFF" });
            // Would send custom command for tally light
            Ok(())
        }
    }
}

// Usage of custom extension:
fn use_custom_extension() -> Result<(), Error> {
    let transport = UdpTransport::new("192.168.1.100:52381")?;
    let mut camera = Camera::<PTZOpticsG2>::new(transport);

    // Use our custom broadcast extension
    camera.setup_for_broadcast()?;
    camera.set_tally(true)?;

    Ok(())
}
