//! Example program

//! Example demonstrating async health check functionality.
//!
//! This example shows how to:
//! - Check camera connection health asynchronously
//! - Perform periodic health checks
//! - Monitor connection status over time
//! - Handle connection failures gracefully

use grafton_visca::{
    camera::{profiles::PTZOpticsG2, Camera},
    transport::{AsyncTcpTransport, AsyncUdpTransport},
    Error,
};
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Get camera address from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <camera_ip:port>", args[0]);
        eprintln!("Example: {} 192.168.1.100:1259", args[0]);
        std::process::exit(1);
    }

    // Connect to camera
    let camera_addr = &args[1];
    println!("Connecting to camera at {}...", camera_addr);

    // Try UDP connection
    println!("\n=== Testing UDP Connection ===");
    match AsyncUdpTransport::new(camera_addr).await {
        Ok(transport) => {
            println!("✓ UDP connection established");
            let camera = Arc::new(Mutex::new(Camera::<PTZOpticsG2>::new(transport)));

            // Check initial health
            match camera.lock().await.stop().await {
                Ok(_) => println!("✓ Camera is responding to commands"),
                Err(_) => println!("✗ Camera is not responding"),
            }

            // Perform health monitoring
            health_monitor_loop(camera.clone(), camera_addr, true).await?;
        }
        Err(e) => {
            println!("✗ UDP connection failed: {}", e);
        }
    }

    // Try TCP connection
    println!("\n=== Testing TCP Connection ===");
    let tcp_addr = if camera_addr.contains(":1259") {
        camera_addr.replace(":1259", ":5678")
    } else {
        camera_addr.to_string()
    };

    match AsyncTcpTransport::new(&tcp_addr).await {
        Ok(transport) => {
            println!("✓ TCP connection established");
            let camera = Arc::new(Mutex::new(Camera::<PTZOpticsG2>::new(transport)));

            // Check initial health
            if camera.lock().await.stop().await.is_ok() {
                println!("✓ Camera is responding to commands");
            } else {
                println!("✗ Camera is not responding");
            }

            // Perform health monitoring
            health_monitor_loop(camera.clone(), &tcp_addr, false).await?;
        }
        Err(e) => {
            println!("✗ TCP connection failed: {}", e);
        }
    }

    Ok(())
}

async fn health_monitor_loop(
    camera: Arc<Mutex<Camera<PTZOpticsG2>>>,
    camera_addr: &str,
    is_udp: bool,
) -> Result<(), Error> {
    println!("\n=== Starting Health Monitoring ===");
    println!("Performing health checks every 5 seconds...");
    println!("Press Ctrl+C to stop\n");

    let mut consecutive_failures = 0;
    let max_failures = 3;

    loop {
        sleep(Duration::from_secs(5)).await;

        // Perform health check using stop command
        let health_check = camera.lock().await.stop().await;

        match health_check {
            Ok(_) => {
                if consecutive_failures > 0 {
                    println!("\n✓ Connection restored!");
                    consecutive_failures = 0;
                }
                print!(".");
                use std::io::{stdout, Write};
                let _ = stdout().flush();
            }
            Err(e) => {
                consecutive_failures += 1;
                println!(
                    "\n⚠️  Health check failed ({}/{}) - {}",
                    consecutive_failures, max_failures, e
                );

                if consecutive_failures >= max_failures {
                    println!(
                        "\n✗ Camera appears to be offline after {} consecutive failures",
                        max_failures
                    );
                    println!("Attempting to reconnect...");

                    // Try to reconnect
                    let reconnect_result = if is_udp {
                        match AsyncUdpTransport::new(camera_addr).await {
                            Ok(transport) => {
                                *camera.lock().await = Camera::<PTZOpticsG2>::new(transport);
                                Ok(())
                            }
                            Err(e) => Err(e),
                        }
                    } else {
                        match AsyncTcpTransport::new(camera_addr).await {
                            Ok(transport) => {
                                *camera.lock().await = Camera::<PTZOpticsG2>::new(transport);
                                Ok(())
                            }
                            Err(e) => Err(e),
                        }
                    };

                    match reconnect_result {
                        Ok(_) => {
                            println!("✓ Reconnected successfully!");
                            consecutive_failures = 0;
                        }
                        Err(e) => {
                            println!("✗ Reconnection failed: {}", e);
                            println!("Waiting 10 seconds before next attempt...");
                            sleep(Duration::from_secs(10)).await;
                        }
                    }
                }
            }
        }
    }
}
