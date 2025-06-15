//! Example program

//! Example demonstrating health check and connection monitoring.
//!
//! This example shows how to:
//! - Check camera connection health
//! - Monitor connection status
//! - Handle connection failures
//! - Use both UDP and TCP transports

use grafton_visca::{
    camera::{profiles::PTZOpticsG2, Camera},
    transport::{BlockingAdapter, TcpTransport, UdpTransport},
    Error,
};
use std::thread;
use std::time::Duration;

// Use a minimal tokio runtime for blocking execution
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(fut)
}

fn main() -> Result<(), Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Get camera address from command line or use default
    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    println!("=== VISCA Health Check Example ===\n");

    // Test UDP connection
    test_udp_health(&camera_addr)?;

    println!("\n{}\n", "=".repeat(50));

    // Test TCP connection
    test_tcp_health(&camera_addr)?;

    Ok(())
}

fn test_udp_health(camera_addr: &str) -> Result<(), Error> {
    println!("Testing UDP connection to {}...", camera_addr);

    // Try to connect
    let udp_transport = match UdpTransport::new(camera_addr) {
        Ok(t) => {
            println!("✓ UDP transport created");
            t
        }
        Err(e) => {
            println!("✗ Failed to create UDP transport: {}", e);
            return Ok(());
        }
    };

    let mut camera = Camera::<PTZOpticsG2>::new(BlockingAdapter(udp_transport));

    // Test basic commands as health check
    println!("\nSending test commands...");

    // Try to stop any ongoing movement (simple health check)
    match block_on(camera.stop()) {
        Ok(_) => println!("✓ Stop command succeeded - camera is responding"),
        Err(e) => println!("✗ Stop command failed: {}", e),
    }

    // Try to move home
    match block_on(camera.home()) {
        Ok(_) => println!("✓ Home command succeeded"),
        Err(e) => println!("✗ Home command failed: {}", e),
    }

    // Periodic health checks
    println!("\nPerforming periodic health checks...");
    for i in 1..=5 {
        thread::sleep(Duration::from_secs(2));
        print!("Health check #{}: ", i);

        // Use stop command as a simple ping
        match block_on(camera.stop()) {
            Ok(_) => println!("✓ Camera is responding"),
            Err(e) => println!("✗ Camera not responding: {}", e),
        }
    }

    println!("\nUDP health check complete!");
    Ok(())
}

fn test_tcp_health(camera_addr: &str) -> Result<(), Error> {
    println!("Testing TCP connection to {}...", camera_addr);

    // Parse address and adjust port for TCP (typically 5678 for TCP vs 1259 for UDP)
    let tcp_addr = if camera_addr.contains(":1259") {
        camera_addr.replace(":1259", ":5678")
    } else {
        camera_addr.to_string()
    };

    // Try to connect
    let tcp_transport = match TcpTransport::new(&tcp_addr) {
        Ok(t) => {
            println!("✓ TCP transport created");
            t
        }
        Err(e) => {
            println!("✗ Failed to create TCP transport: {}", e);
            return Ok(());
        }
    };

    let mut camera = Camera::<PTZOpticsG2>::new(BlockingAdapter(tcp_transport));

    // Test basic commands
    println!("\nSending test commands...");

    // Try to stop any ongoing movement
    match block_on(camera.stop()) {
        Ok(_) => println!("✓ Stop command succeeded - camera is responding"),
        Err(e) => println!("✗ Stop command failed: {}", e),
    }

    // Test connection resilience
    println!("\nTesting connection resilience...");

    for i in 1..=3 {
        println!("\nTest cycle #{}:", i);

        // Send multiple commands
        match block_on(camera.zoom_stop()) {
            Ok(_) => println!("  ✓ Zoom stop succeeded"),
            Err(e) => println!("  ✗ Zoom stop failed: {}", e),
        }

        thread::sleep(Duration::from_millis(500));

        match block_on(camera.focus_auto()) {
            Ok(_) => println!("  ✓ Focus auto succeeded"),
            Err(e) => println!("  ✗ Focus auto failed: {}", e),
        }

        thread::sleep(Duration::from_secs(1));
    }

    println!("\nTCP health check complete!");
    Ok(())
}
