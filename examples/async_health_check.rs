// TODO: Update this example for v0.5.0 - async support is not yet available
fn main() {
    println!("This example needs to be updated for v0.5.0");
    println!("Async support is not yet available in the current version");
}

/*
#[cfg(feature = "async-client")]
use grafton_visca::command::power::Power;
#[cfg(feature = "async-client")]
use grafton_visca::command::{PanTiltCommand, PowerCommand};
#[cfg(feature = "async-client")]
use grafton_visca::{
    AsyncConnectionManagement, AsyncTcpTransport, AsyncUdpTransport, AsyncViscaTransport,
};
#[cfg(feature = "async-client")]
use std::net::SocketAddr;
#[cfg(feature = "async-client")]
use tokio::time::{sleep, Duration};

#[cfg(feature = "async-client")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Example using async UDP transport
    println!("Testing async UDP transport health check...");
    let addr: SocketAddr = "192.168.1.100:1259".parse()?;
    let mut udp_transport = AsyncUdpTransport::new(addr).await?;

    // Check initial health
    match udp_transport.is_healthy().await {
        Ok(true) => println!("✓ UDP connection is healthy"),
        Ok(false) => println!("✗ UDP connection is not healthy"),
        Err(e) => println!("✗ Error checking UDP health: {}", e),
    }

    // Send a command
    let power_on = PowerCommand { power: Power::On };
    udp_transport.send_command(&power_on).await?;
    sleep(Duration::from_millis(100)).await;
    let _ = udp_transport.receive_response().await;

    // Check stats
    let stats = udp_transport.connection_stats().snapshot();
    println!("\nAsync UDP Connection Statistics:");
    println!(
        "  Connected for: {:?}",
        stats.connected_since.map(|t| t.elapsed())
    );
    println!("  Commands sent: {}", stats.commands_sent);
    println!("  Responses received: {}", stats.responses_received);
    println!("  Bytes sent: {}", stats.bytes_sent);
    println!("  Bytes received: {}", stats.bytes_received);
    println!("  Errors: {}", stats.error_count);
    println!(
        "  Idle time: {:?}",
        stats.last_activity.map(|t| t.elapsed())
    );

    // Example using async TCP transport
    println!("\n\nTesting async TCP transport health check...");
    let tcp_addr: SocketAddr = "192.168.1.100:5678".parse()?;
    let mut tcp_transport = AsyncTcpTransport::new(tcp_addr).await?;

    // Check health
    match tcp_transport.is_healthy().await {
        Ok(true) => println!("✓ TCP connection is healthy"),
        Ok(false) => println!("✗ TCP connection is not healthy"),
        Err(e) => println!("✗ Error checking TCP health: {}", e),
    }

    // Send some commands
    tcp_transport.send_command(&PanTiltCommand::Home).await?;
    sleep(Duration::from_millis(100)).await;
    let _ = tcp_transport.receive_response().await;

    // Check stats again
    let stats = tcp_transport.connection_stats().snapshot();
    println!("\nAsync TCP Connection Statistics:");
    println!(
        "  Connected for: {:?}",
        stats.connected_since.map(|t| t.elapsed())
    );
    println!("  Commands sent: {}", stats.commands_sent);
    println!("  Responses received: {}", stats.responses_received);
    println!("  Bytes sent: {}", stats.bytes_sent);
    println!("  Bytes received: {}", stats.bytes_received);
    println!("  Errors: {}", stats.error_count);

    // Demonstrate concurrent health checks
    println!("\n\nPerforming concurrent health checks...");

    // Check both transports concurrently
    let (udp_health, tcp_health) =
        tokio::join!(udp_transport.is_healthy(), tcp_transport.is_healthy());

    println!(
        "UDP: {}",
        match udp_health {
            Ok(true) => "✓ Healthy",
            Ok(false) => "✗ Not healthy",
            Err(e) => {
                eprintln!("Error: {}", e);
                "✗ Error"
            }
        }
    );
    println!(
        "TCP: {}",
        match tcp_health {
            Ok(true) => "✓ Healthy",
            Ok(false) => "✗ Not healthy",
            Err(e) => {
                eprintln!("Error: {}", e);
                "✗ Error"
            }
        }
    );

    // Periodic health checks
    println!("\n\nPerforming periodic health checks...");
    for i in 1..=5 {
        sleep(Duration::from_secs(2)).await;
        print!("Health check {}: ", i);
        match tcp_transport.is_healthy().await {
            Ok(true) => println!("✓ Healthy"),
            Ok(false) => println!("✗ Not healthy"),
            Err(e) => println!("✗ Error: {}", e),
        }
    }

    Ok(())
}

#[cfg(not(feature = "async-client"))]
fn main() {
    eprintln!("This example requires the 'async' feature to be enabled.");
    eprintln!("Try running with: cargo run --example async_health_check --features async-client");
}
*/
