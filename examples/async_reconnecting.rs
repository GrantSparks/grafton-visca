//! Example demonstrating connection resilience and recovery with async operations.
//!
//! This example shows how to:
//! - Handle connection failures gracefully
//! - Implement retry logic for failed commands
//! - Monitor connection health
//! - Recover from network interruptions

use grafton_visca::command::{
    pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
    power::{Power, PowerCommand},
    InquiryCommand, Response, ZoomCommand,
};
use grafton_visca::{Client, Error};
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
    
    let mut client = None;
    
    for attempt in 1..=max_retries {
        println!("   Connection attempt {}/{}...", attempt, max_retries);
        
        match Client::connect_udp_async(camera_addr).await {
            Ok(c) => {
                println!("   ✓ Connected successfully!");
                client = Some(c);
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
    
    let client = match client {
        Some(c) => c,
        None => {
            println!("   ✗ All connection attempts failed");
            return Ok(());
        }
    };

    // Test the connection
    match client.is_healthy().await {
        Ok(true) => println!("   ✓ Connection is healthy"),
        Ok(false) => println!("   ✗ Connection established but camera not responding"),
        Err(e) => println!("   ✗ Health check failed: {}", e),
    }

    println!();
    Ok(())
}

#[cfg(feature = "async-client")]
async fn demo_health_monitoring(camera_addr: &str) -> Result<(), Error> {
    println!("2. Connection Health Monitoring:");
    println!("   Setting up periodic health checks...\n");

    let client = Client::connect_udp_async(camera_addr).await?;
    let is_healthy = Arc::new(AtomicBool::new(true));
    let health_check_count = Arc::new(AtomicU32::new(0));
    
    // Spawn health monitoring task
    let client_clone = client.clone();
    let is_healthy_clone = is_healthy.clone();
    let health_check_count_clone = health_check_count.clone();
    
    let health_monitor = tokio::spawn(async move {
        let check_interval = Duration::from_secs(2);
        
        loop {
            sleep(check_interval).await;
            
            let count = health_check_count_clone.fetch_add(1, Ordering::SeqCst) + 1;
            print!("   Health check #{}: ", count);
            
            match client_clone.is_healthy().await {
                Ok(true) => {
                    println!("✓ Healthy");
                    is_healthy_clone.store(true, Ordering::SeqCst);
                }
                Ok(false) => {
                    println!("✗ Not responding");
                    is_healthy_clone.store(false, Ordering::SeqCst);
                }
                Err(e) => {
                    println!("✗ Error: {}", e);
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
        match timeout(Duration::from_secs(1), client.send_async(&InquiryCommand::Power)).await {
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

    let client = Client::connect_udp_async(camera_addr).await?;
    
    // Define retry configuration
    #[derive(Clone)]
    struct RetryConfig {
        max_attempts: u32,
        initial_delay: Duration,
        max_delay: Duration,
        backoff_factor: f64,
    }
    
    let retry_config = RetryConfig {
        max_attempts: 3,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(5),
        backoff_factor: 2.0,
    };
    
    // Helper function to send command with retry
    async fn send_with_retry<C: grafton_visca::Command>(
        client: &Client,
        command: &C,
        config: &RetryConfig,
        operation_name: &str,
    ) -> Result<Response, Error> {
        let mut delay = config.initial_delay;
        
        for attempt in 1..=config.max_attempts {
            println!("   {} - Attempt {}/{}", operation_name, attempt, config.max_attempts);
            
            match timeout(Duration::from_secs(2), client.send_async(command)).await {
                Ok(Ok(response)) => {
                    println!("   ✓ {} succeeded", operation_name);
                    return Ok(response);
                }
                Ok(Err(e)) => {
                    println!("   ✗ {} failed: {}", operation_name, e);
                }
                Err(_) => {
                    println!("   ✗ {} timed out", operation_name);
                }
            }
            
            if attempt < config.max_attempts {
                println!("   Waiting {:?} before retry...", delay);
                sleep(delay).await;
                
                // Exponential backoff
                delay = Duration::from_secs_f64(
                    (delay.as_secs_f64() * config.backoff_factor).min(config.max_delay.as_secs_f64())
                );
            }
        }
        
        Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("{} failed after {} attempts", operation_name, config.max_attempts),
        )))
    }
    
    // Execute a sequence of operations with retry
    println!("   Executing camera control sequence with automatic retry...\n");
    
    // Power on
    let _ = send_with_retry(
        &client,
        &PowerCommand { power: Power::On },
        &retry_config,
        "Power On",
    ).await;
    
    sleep(Duration::from_secs(1)).await;
    
    // Move to home
    let _ = send_with_retry(
        &client,
        &PanTiltCommand::Home,
        &retry_config,
        "Home Position",
    ).await;
    
    sleep(Duration::from_secs(2)).await;
    
    // Pan right
    let _ = send_with_retry(
        &client,
        &PanTiltCommand::Move {
            direction: PanTiltDirection::Right,
            pan_speed: PanSpeed::new(5)?,
            tilt_speed: TiltSpeed::new(0)?,
        },
        &retry_config,
        "Pan Right",
    ).await;
    
    sleep(Duration::from_secs(2)).await;
    
    // Stop movement
    let _ = send_with_retry(
        &client,
        &PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0)?,
            tilt_speed: TiltSpeed::new(0)?,
        },
        &retry_config,
        "Stop Movement",
    ).await;
    
    // Zoom in
    let _ = send_with_retry(
        &client,
        &ZoomCommand::ZoomInStandard,
        &retry_config,
        "Zoom In",
    ).await;
    
    sleep(Duration::from_secs(1)).await;
    
    // Stop zoom
    let _ = send_with_retry(
        &client,
        &ZoomCommand::Stop,
        &retry_config,
        "Stop Zoom",
    ).await;
    
    println!("\n   Control sequence completed!");
    println!();
    Ok(())
}