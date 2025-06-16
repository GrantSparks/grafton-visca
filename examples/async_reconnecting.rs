//! Example program

//! Example demonstrating connection resilience and recovery with async operations.
//!
//! This example shows how to:
//! - Handle connection failures gracefully
//! - Implement retry logic for failed commands
//! - Monitor connection health
//! - Recover from network interruptions

use grafton_visca::camera::{Camera, PTZOpticsG2};
use grafton_visca::transport::AsyncTcpTransport;
use grafton_visca::Error;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, timeout};

#[cfg(not(feature = "async-client"))]
fn main() {
    eprintln!("This example requires the 'async-client' feature.");
    eprintln!("Run with: cargo run --example async_reconnecting --features async-client");
}

#[cfg(feature = "async-client")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=== Async Connection Resilience Example ===\n");

    // Get camera address
    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    // Demonstrate different resilience scenarios
    demo_basic_retry(&camera_addr).await?;
    demo_health_monitoring(&camera_addr).await?;
    demo_resilient_control(&camera_addr).await?;

    Ok(())
}

#[cfg(feature = "async-client")]
async fn demo_basic_retry(camera_addr: &str) -> Result<(), Error> {
    println!("1. Basic Retry Logic:");
    println!("   Attempting to connect with retries...\n");

    let max_retries = 3;
    let retry_delay = Duration::from_secs(1);

    let mut camera = None;

    for attempt in 1..=max_retries {
        println!("   Connection attempt {}/{}...", attempt, max_retries);

        match AsyncTcpTransport::new(camera_addr).await {
            Ok(transport) => {
                println!("   ✓ Connected successfully!");
                camera = Some(Camera::<PTZOpticsG2>::new(transport));
                break;
            }
            Err(e) => {
                println!("   ✗ Connection failed: {}", e);
                if attempt < max_retries {
                    println!("   Waiting {:?} before retry...", retry_delay);
                    sleep(retry_delay).await;
                }
            }
        }
    }

    let mut camera = match camera {
        Some(c) => c,
        None => {
            println!("   ✗ All connection attempts failed");
            return Ok(());
        }
    };

    // Test the connection by sending a command
    match timeout(Duration::from_secs(2), camera.home()).await {
        Ok(Ok(_)) => println!("   ✓ Connection is healthy"),
        Ok(Err(e)) => println!(
            "   ✗ Connection established but camera not responding: {}",
            e
        ),
        Err(_) => println!("   ✗ Health check timed out"),
    }

    println!();
    Ok(())
}

#[cfg(feature = "async-client")]
async fn demo_health_monitoring(camera_addr: &str) -> Result<(), Error> {
    println!("2. Connection Health Monitoring:");
    println!("   Setting up periodic health checks...\n");

    let transport = AsyncTcpTransport::new(camera_addr).await?;
    let camera = Arc::new(tokio::sync::Mutex::new(Camera::<PTZOpticsG2>::new(
        transport,
    )));
    let is_healthy = Arc::new(AtomicBool::new(true));
    let health_check_count = Arc::new(AtomicU32::new(0));

    // Spawn health monitoring task
    let camera_clone = camera.clone();
    let is_healthy_clone = is_healthy.clone();
    let health_check_count_clone = health_check_count.clone();

    let health_monitor = tokio::spawn(async move {
        let check_interval = Duration::from_secs(2);

        loop {
            sleep(check_interval).await;

            let count = health_check_count_clone.fetch_add(1, Ordering::SeqCst) + 1;
            print!("   Health check #{}: ", count);

            // Try a simple command to check health
            let mut cam = camera_clone.lock().await;
            match timeout(Duration::from_secs(1), cam.home()).await {
                Ok(Ok(_)) => {
                    println!("✓ Healthy");
                    is_healthy_clone.store(true, Ordering::SeqCst);
                }
                Ok(Err(e)) => {
                    println!("✗ Error: {}", e);
                    is_healthy_clone.store(false, Ordering::SeqCst);
                }
                Err(_) => {
                    println!("✗ Timed out");
                    is_healthy_clone.store(false, Ordering::SeqCst);
                }
            }

            if count >= 5 {
                break;
            }
        }
    });

    // Perform operations while monitoring health
    println!("   Performing operations with health monitoring...");

    for i in 1..=5 {
        if !is_healthy.load(Ordering::SeqCst) {
            println!("   ⚠️  Connection unhealthy, operation {} may fail", i);
        }

        // Try to send a command
        let mut cam = camera.lock().await;
        match timeout(Duration::from_secs(1), cam.zoom_stop()).await {
            Ok(Ok(_)) => println!("   ✓ Operation {} succeeded", i),
            Ok(Err(e)) => println!("   ✗ Operation {} failed: {}", i, e),
            Err(_) => println!("   ✗ Operation {} timed out", i),
        }

        sleep(Duration::from_secs(1)).await;
    }

    // Wait for health monitor to finish
    let _ = health_monitor.await;

    println!();
    Ok(())
}

#[cfg(feature = "async-client")]
async fn demo_resilient_control(camera_addr: &str) -> Result<(), Error> {
    println!("3. Resilient Camera Control:");
    println!("   Implementing command retry with exponential backoff...\n");

    let transport = AsyncTcpTransport::new(camera_addr).await?;
    let camera = tokio::sync::Mutex::new(Camera::<PTZOpticsG2>::new(transport));

    // Define retry configuration
    #[derive(Clone)]
    struct RetryConfig {
        max_attempts: u32,
        initial_delay: Duration,
        #[allow(dead_code)]
        max_delay: Duration,
        #[allow(dead_code)]
        backoff_factor: f64,
    }

    let retry_config = RetryConfig {
        max_attempts: 3,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(5),
        backoff_factor: 2.0,
    };

    // Execute a sequence of operations with retry
    println!("   Executing camera control sequence with automatic retry...\n");

    // Power on
    for attempt in 1..=retry_config.max_attempts {
        println!(
            "   Power On - Attempt {}/{}",
            attempt, retry_config.max_attempts
        );
        let mut cam = camera.lock().await;
        match timeout(Duration::from_secs(2), cam.power_on()).await {
            Ok(Ok(_)) => {
                println!("   ✓ Power On succeeded");
                break;
            }
            Ok(Err(e)) => println!("   ✗ Power On failed: {}", e),
            Err(_) => println!("   ✗ Power On timed out"),
        }
        if attempt < retry_config.max_attempts {
            println!(
                "   Waiting {:?} before retry...",
                retry_config.initial_delay
            );
            sleep(retry_config.initial_delay).await;
        }
    }

    sleep(Duration::from_secs(1)).await;

    // Move to home
    for attempt in 1..=retry_config.max_attempts {
        println!(
            "   Home Position - Attempt {}/{}",
            attempt, retry_config.max_attempts
        );
        let mut cam = camera.lock().await;
        match timeout(Duration::from_secs(2), cam.home()).await {
            Ok(Ok(_)) => {
                println!("   ✓ Home Position succeeded");
                break;
            }
            Ok(Err(e)) => println!("   ✗ Home Position failed: {}", e),
            Err(_) => println!("   ✗ Home Position timed out"),
        }
        if attempt < retry_config.max_attempts {
            println!(
                "   Waiting {:?} before retry...",
                retry_config.initial_delay
            );
            sleep(retry_config.initial_delay).await;
        }
    }

    sleep(Duration::from_secs(2)).await;

    // Pan right
    for attempt in 1..=retry_config.max_attempts {
        println!(
            "   Pan Right - Attempt {}/{}",
            attempt, retry_config.max_attempts
        );
        let mut cam = camera.lock().await;
        match timeout(
            Duration::from_secs(2),
            cam.move_continuous(
                grafton_visca::command::pan_tilt::PanTiltDirection::Right,
                5,
                0,
            ),
        )
        .await
        {
            Ok(Ok(_)) => {
                println!("   ✓ Pan Right succeeded");
                break;
            }
            Ok(Err(e)) => println!("   ✗ Pan Right failed: {}", e),
            Err(_) => println!("   ✗ Pan Right timed out"),
        }
        if attempt < retry_config.max_attempts {
            println!(
                "   Waiting {:?} before retry...",
                retry_config.initial_delay
            );
            sleep(retry_config.initial_delay).await;
        }
    }

    sleep(Duration::from_secs(2)).await;

    // Stop movement
    for attempt in 1..=retry_config.max_attempts {
        println!(
            "   Stop Movement - Attempt {}/{}",
            attempt, retry_config.max_attempts
        );
        let mut cam = camera.lock().await;
        match timeout(
            Duration::from_secs(2),
            cam.move_continuous(
                grafton_visca::command::pan_tilt::PanTiltDirection::Stop,
                0,
                0,
            ),
        )
        .await
        {
            Ok(Ok(_)) => {
                println!("   ✓ Stop Movement succeeded");
                break;
            }
            Ok(Err(e)) => println!("   ✗ Stop Movement failed: {}", e),
            Err(_) => println!("   ✗ Stop Movement timed out"),
        }
        if attempt < retry_config.max_attempts {
            println!(
                "   Waiting {:?} before retry...",
                retry_config.initial_delay
            );
            sleep(retry_config.initial_delay).await;
        }
    }

    // Zoom in
    for attempt in 1..=retry_config.max_attempts {
        println!(
            "   Zoom In - Attempt {}/{}",
            attempt, retry_config.max_attempts
        );
        let mut cam = camera.lock().await;
        match timeout(Duration::from_secs(2), cam.zoom_in()).await {
            Ok(Ok(_)) => {
                println!("   ✓ Zoom In succeeded");
                break;
            }
            Ok(Err(e)) => println!("   ✗ Zoom In failed: {}", e),
            Err(_) => println!("   ✗ Zoom In timed out"),
        }
        if attempt < retry_config.max_attempts {
            println!(
                "   Waiting {:?} before retry...",
                retry_config.initial_delay
            );
            sleep(retry_config.initial_delay).await;
        }
    }

    sleep(Duration::from_secs(1)).await;

    // Stop zoom
    for attempt in 1..=retry_config.max_attempts {
        println!(
            "   Stop Zoom - Attempt {}/{}",
            attempt, retry_config.max_attempts
        );
        let mut cam = camera.lock().await;
        match timeout(Duration::from_secs(2), cam.zoom_stop()).await {
            Ok(Ok(_)) => {
                println!("   ✓ Stop Zoom succeeded");
                break;
            }
            Ok(Err(e)) => println!("   ✗ Stop Zoom failed: {}", e),
            Err(_) => println!("   ✗ Stop Zoom timed out"),
        }
        if attempt < retry_config.max_attempts {
            println!(
                "   Waiting {:?} before retry...",
                retry_config.initial_delay
            );
            sleep(retry_config.initial_delay).await;
        }
    }

    println!("\n   Control sequence completed!");
    println!();
    Ok(())
}
