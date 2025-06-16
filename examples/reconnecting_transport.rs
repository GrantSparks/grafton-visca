//! Example demonstrating connection resilience and recovery strategies using Camera<P> API.
//!
//! This example shows how to:
//! - Handle connection failures gracefully
//! - Implement reconnection logic  
//! - Monitor connection state
//! - Build resilient camera control applications
//!
//! NOTE: This example demonstrates manual reconnection patterns. For production use,
//! consider using the ReconnectingTransport wrapper in the library.
//! See async_reconnecting.rs for an example using automatic reconnection.

#[cfg(feature = "blocking-client")]
use grafton_visca::{
    camera::{
        profiles::PTZOpticsG2,
        Camera,
    },
    command::pan_tilt::PanTiltDirection,
    transport::{BlockingAdapter, UdpTransport},
    Error,
};
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
// Use a minimal tokio runtime for blocking execution
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(fut)
}

#[cfg(feature = "blocking-client")]
fn main() -> Result<(), Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=== Connection Resilience Example with Camera<P> API ===\n");

    // Get camera address
    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:52381".to_string());

    // Demonstrate different resilience patterns
    demo_basic_reconnection(&camera_addr)?;
    demo_resilient_camera(&camera_addr)?;
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
    fn connect_with_retry(addr: &str, config: &RetryConfig) -> Result<Camera<PTZOpticsG2>, Error> {
        for attempt in 1..=config.max_attempts {
            println!(
                "   Connection attempt {}/{}...",
                attempt, config.max_attempts
            );

            match UdpTransport::new(addr) {
                Ok(udp_transport) => {
                    let transport = BlockingAdapter(udp_transport);
                    let camera = Camera::<PTZOpticsG2>::new(transport);
                    println!("   ✓ Connected successfully!");
                    return Ok(camera);
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
    let mut camera = connect_with_retry(camera_addr, &config)?;

    // Test the connection with power inquiry
    match block_on(camera.get_power_state()) {
        Ok(is_on) => println!("   ✓ Connection verified, power state: {}", if is_on { "ON" } else { "OFF" }),
        Err(e) => println!("   ✗ Health check failed: {}", e),
    }

    println!();
    Ok(())
}

#[cfg(feature = "blocking-client")]
fn demo_resilient_camera(camera_addr: &str) -> Result<(), Error> {
    println!("2. Resilient Camera Pattern:");
    println!("   Creating a wrapper that handles reconnection automatically\n");

    // Simple resilient camera wrapper
    struct ResilientCamera {
        addr: String,
        camera: Arc<Mutex<Option<Camera<PTZOpticsG2>>>>,
    }

    impl ResilientCamera {
        fn new(addr: &str) -> Self {
            Self {
                addr: addr.to_string(),
                camera: Arc::new(Mutex::new(None)),
            }
        }

        fn ensure_connected(&self) -> Result<(), Error> {
            let mut camera_guard = self.camera.lock().unwrap();

            // Check if we have a working camera
            if let Some(ref mut camera) = *camera_guard {
                // Try a simple inquiry to test connection
                if block_on(camera.get_power_state()).is_ok() {
                    return Ok(());
                }
            }

            // Need to (re)connect
            println!("   Establishing connection to {}...", self.addr);
            match UdpTransport::new(&self.addr) {
                Ok(udp_transport) => {
                    let transport = BlockingAdapter(udp_transport);
                    let camera = Camera::<PTZOpticsG2>::new(transport);
                    *camera_guard = Some(camera);
                    println!("   ✓ Connected successfully");
                    Ok(())
                }
                Err(e) => {
                    *camera_guard = None;
                    Err(Error::Io(e))
                }
            }
        }

        fn execute<F, T>(&self, operation: F) -> Result<T, Error>
        where
            F: Fn(&mut Camera<PTZOpticsG2>) -> Result<T, Error>,
        {
            // Ensure we're connected
            self.ensure_connected()?;

            // Try to execute operation
            let mut camera_guard = self.camera.lock().unwrap();
            if let Some(ref mut camera) = *camera_guard {
                match operation(camera) {
                    Ok(result) => Ok(result),
                    Err(e) => {
                        println!(
                            "   ⚠️  Operation failed: {}, will retry after reconnection",
                            e
                        );
                        drop(camera_guard); // Release lock before reconnecting

                        // Clear the failed connection
                        self.camera.lock().unwrap().take();

                        // Try once more after reconnection
                        self.ensure_connected()?;

                        let mut camera_guard = self.camera.lock().unwrap();
                        if let Some(ref mut camera) = *camera_guard {
                            operation(camera)
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

    // Create resilient camera
    let resilient = ResilientCamera::new(camera_addr);

    // Test with various operations
    println!("   Testing resilient command execution...\n");

    // Power inquiry
    match resilient.execute(|camera| block_on(camera.get_power_state())) {
        Ok(is_on) => {
            println!("   ✓ Power state: {}", if is_on { "ON" } else { "OFF" });
        }
        Err(e) => println!("   ✗ Power inquiry failed: {}", e),
    }

    // Get current position
    match resilient.execute(|camera| block_on(camera.get_position())) {
        Ok((pan, tilt)) => {
            println!("   ✓ Current position: pan={:.1}°, tilt={:.1}°", pan.0, tilt.0);
        }
        Err(e) => println!("   ✗ Position inquiry failed: {}", e),
    }

    // Movement command
    match resilient.execute(|camera| {
        block_on(camera.move_continuous(PanTiltDirection::Right, 5, 0))
    }) {
        Ok(_) => {
            println!("   ✓ Movement started");
            thread::sleep(Duration::from_secs(1));

            // Stop movement
            let _ = resilient.execute(|camera| {
                block_on(camera.stop())
            });
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

    let udp_transport = UdpTransport::new(camera_addr)?;
    let transport = BlockingAdapter(udp_transport);
    let camera = Arc::new(Mutex::new(Camera::<PTZOpticsG2>::new(transport)));
    let is_healthy = Arc::new(Mutex::new(true));
    let should_stop = Arc::new(Mutex::new(false));

    // Start monitoring thread
    let camera_clone = camera.clone();
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

            let mut camera_guard = camera_clone.lock().unwrap();
            match block_on(camera_guard.get_power_state()) {
                Ok(_) => {
                    println!("✓ Healthy");
                    *is_healthy_clone.lock().unwrap() = true;
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
        let mut camera_guard = camera.lock().unwrap();
        match block_on(camera_guard.get_zoom_position()) {
            Ok(zoom) => {
                println!("   ✓ Operation {} succeeded: zoom position = {:.1}x", i, zoom);
            }
            Err(e) => println!("   ✗ Operation {} failed: {}", i, e),
        }
        drop(camera_guard);

        thread::sleep(Duration::from_secs(1));
    }

    // Stop monitoring
    *should_stop.lock().unwrap() = true;
    monitor_thread.join().unwrap();

    println!("\n   Monitoring completed!");
    println!();
    Ok(())
}