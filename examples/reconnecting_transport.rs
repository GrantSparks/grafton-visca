//! Example demonstrating connection resilience and recovery strategies.
//!
//! This example shows how to:
//! - Handle connection failures gracefully
//! - Implement reconnection logic
//! - Monitor connection state
//! - Build resilient camera control applications

#[cfg(feature = "blocking-client")]
use grafton_visca::command::{
    pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
    InquiryCommand, Response,
};
#[cfg(feature = "blocking-client")]
use grafton_visca::{Client, Error};
#[cfg(feature = "blocking-client")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "blocking-client")]
use std::thread;
#[cfg(feature = "blocking-client")]
use std::time::Duration;

#[cfg(not(feature = "blocking-client"))]
fn main() {
    eprintln!("This example requires the 'blocking-client' feature.");
    eprintln!("Run with: cargo run --example reconnecting_transport --features blocking-client");
}

#[cfg(feature = "blocking-client")]
fn main() -> Result<(), Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=== Connection Resilience Example ===\n");

    // Get camera address
    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    // Demonstrate different resilience patterns
    demo_basic_reconnection(&camera_addr)?;
    demo_resilient_client(&camera_addr)?;
    demo_connection_monitoring(&camera_addr)?;

    Ok(())
}

#[cfg(feature = "blocking-client")]
fn demo_basic_reconnection(camera_addr: &str) -> Result<(), Error> {
    println!("1. Basic Reconnection Pattern:");
    println!("   Implementing simple retry logic\n");

    #[derive(Clone)]
    struct RetryConfig {
        max_attempts: u32,
        delay: Duration,
    }

    let config = RetryConfig {
        max_attempts: 3,
        delay: Duration::from_secs(1),
    };

    // Helper function to connect with retry
    fn connect_with_retry(addr: &str, config: &RetryConfig) -> Result<Client, Error> {
        for attempt in 1..=config.max_attempts {
            println!(
                "   Connection attempt {}/{}...",
                attempt, config.max_attempts
            );

            match Client::connect_udp(addr) {
                Ok(client) => {
                    println!("   ✓ Connected successfully!");
                    return Ok(client);
                }
                Err(e) => {
                    println!("   ✗ Connection failed: {}", e);
                    if attempt < config.max_attempts {
                        println!("   Waiting {:?} before retry...", config.delay);
                        thread::sleep(config.delay);
                    }
                }
            }
        }

        Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "Failed to connect after all retries",
        )))
    }

    // Try to connect
    let client = connect_with_retry(camera_addr, &config)?;

    // Test the connection
    match client.is_healthy_blocking() {
        Ok(true) => println!("   ✓ Connection verified as healthy"),
        Ok(false) => println!("   ⚠️  Connection established but camera not responding"),
        Err(e) => println!("   ✗ Health check failed: {}", e),
    }

    println!();
    Ok(())
}

#[cfg(feature = "blocking-client")]
fn demo_resilient_client(camera_addr: &str) -> Result<(), Error> {
    println!("2. Resilient Client Pattern:");
    println!("   Creating a wrapper that handles reconnection automatically\n");

    // Simple resilient client wrapper
    struct ResilientClient {
        addr: String,
        client: Arc<Mutex<Option<Client>>>,
    }

    impl ResilientClient {
        fn new(addr: &str) -> Self {
            Self {
                addr: addr.to_string(),
                client: Arc::new(Mutex::new(None)),
            }
        }

        fn ensure_connected(&self) -> Result<(), Error> {
            let mut client_guard = self.client.lock().unwrap();

            // Check if we have a healthy connection
            if let Some(ref client) = *client_guard {
                if let Ok(true) = client.is_healthy_blocking() {
                    return Ok(());
                }
            }

            // Need to (re)connect
            println!("   Establishing connection to {}...", self.addr);
            match Client::connect_udp(&self.addr) {
                Ok(new_client) => {
                    *client_guard = Some(new_client);
                    println!("   ✓ Connected successfully");
                    Ok(())
                }
                Err(e) => {
                    *client_guard = None;
                    Err(e)
                }
            }
        }

        fn send_command<C: grafton_visca::Command>(&self, command: &C) -> Result<Response, Error> {
            // Ensure we're connected
            self.ensure_connected()?;

            // Try to send command
            let client_guard = self.client.lock().unwrap();
            if let Some(ref client) = *client_guard {
                match client.send(command) {
                    Ok(response) => Ok(response),
                    Err(e) => {
                        println!(
                            "   ⚠️  Command failed: {}, will retry after reconnection",
                            e
                        );
                        drop(client_guard); // Release lock before reconnecting

                        // Clear the failed connection
                        self.client.lock().unwrap().take();

                        // Try once more after reconnection
                        self.ensure_connected()?;

                        let client_guard = self.client.lock().unwrap();
                        if let Some(ref client) = *client_guard {
                            client.send(command)
                        } else {
                            Err(Error::Io(std::io::Error::new(
                                std::io::ErrorKind::NotConnected,
                                "Failed to reconnect",
                            )))
                        }
                    }
                }
            } else {
                Err(Error::Io(std::io::Error::new(
                    std::io::ErrorKind::NotConnected,
                    "No active connection",
                )))
            }
        }
    }

    // Create resilient client
    let resilient = ResilientClient::new(camera_addr);

    // Test with various commands
    println!("   Testing resilient command execution...\n");

    // Power inquiry
    match resilient.send_command(&InquiryCommand::Power) {
        Ok(Response::InquiryResponse(resp)) => {
            println!("   ✓ Power inquiry: {:?}", resp);
        }
        _ => println!("   ✗ Power inquiry failed"),
    }

    // Movement command
    let move_cmd = PanTiltCommand::Move {
        direction: PanTiltDirection::Right,
        pan_speed: PanSpeed::new(5)?,
        tilt_speed: TiltSpeed::new(0)?,
    };

    match resilient.send_command(&move_cmd) {
        Ok(_) => {
            println!("   ✓ Movement started");
            thread::sleep(Duration::from_secs(1));

            // Stop movement
            let stop_cmd = PanTiltCommand::Move {
                direction: PanTiltDirection::Stop,
                pan_speed: PanSpeed::new(0)?,
                tilt_speed: TiltSpeed::new(0)?,
            };
            let _ = resilient.send_command(&stop_cmd);
            println!("   ✓ Movement stopped");
        }
        Err(e) => println!("   ✗ Movement command failed: {}", e),
    }

    println!();
    Ok(())
}

#[cfg(feature = "blocking-client")]
fn demo_connection_monitoring(camera_addr: &str) -> Result<(), Error> {
    println!("3. Connection Monitoring:");
    println!("   Background thread monitoring connection health\n");

    let client = Arc::new(Client::connect_udp(camera_addr)?);
    let is_healthy = Arc::new(Mutex::new(true));
    let should_stop = Arc::new(Mutex::new(false));

    // Start monitoring thread
    let client_clone = client.clone();
    let is_healthy_clone = is_healthy.clone();
    let should_stop_clone = should_stop.clone();

    let monitor_thread = thread::spawn(move || {
        let check_interval = Duration::from_secs(2);
        let mut check_count = 0;

        loop {
            thread::sleep(check_interval);

            // Check if we should stop
            if *should_stop_clone.lock().unwrap() {
                break;
            }

            check_count += 1;
            print!("   Health check #{}: ", check_count);

            match client_clone.is_healthy_blocking() {
                Ok(true) => {
                    println!("✓ Healthy");
                    *is_healthy_clone.lock().unwrap() = true;
                }
                Ok(false) => {
                    println!("✗ Not responding");
                    *is_healthy_clone.lock().unwrap() = false;
                }
                Err(e) => {
                    println!("✗ Error: {}", e);
                    *is_healthy_clone.lock().unwrap() = false;
                }
            }

            if check_count >= 5 {
                break;
            }
        }
    });

    // Perform operations while monitoring
    println!("   Performing operations with background monitoring...\n");

    for i in 1..=5 {
        // Check health status
        let healthy = *is_healthy.lock().unwrap();
        if !healthy {
            println!("   ⚠️  Connection unhealthy, operation {} may fail", i);
        }

        // Try a command
        match client.send(&InquiryCommand::ZoomPosition) {
            Ok(Response::InquiryResponse(resp)) => {
                println!("   ✓ Operation {} succeeded: {:?}", i, resp);
            }
            _ => println!("   ✗ Operation {} failed", i),
        }

        thread::sleep(Duration::from_secs(1));
    }

    // Stop monitoring
    *should_stop.lock().unwrap() = true;
    monitor_thread.join().unwrap();

    println!("\n   Monitoring completed!");
    println!();
    Ok(())
}
