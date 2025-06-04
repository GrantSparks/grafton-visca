#[cfg(feature = "async")]
use grafton_visca::command::power::Power;
#[cfg(feature = "async")]
use grafton_visca::command::{PanTiltCommand, PowerCommand};
#[cfg(feature = "async")]
use grafton_visca::{
    AsyncConnectionManagement, AsyncTcpTransport, AsyncUdpTransport, AsyncViscaTransport,
};
#[cfg(feature = "async")]
use std::net::SocketAddr;
#[cfg(feature = "async")]
use tokio::time::{sleep, Duration};

#[cfg(feature = "async")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Example using async UDP transport
    println!("Testing async UDP transport health check...");
    let addr: SocketAddr = "192.168.1.100:1259".parse()?;
    let mut udp_transport = AsyncUdpTransport::new(addr).await?;

    // Check initial health
    if udp_transport.is_healthy().await? {
        println!("✓ UDP connection is healthy");
    } else {
        println!("✗ UDP connection is not healthy");
    }

    // Send a command
    let power_on = PowerCommand { power: Power::On };
    udp_transport.send_command(&power_on).await?;
    sleep(Duration::from_millis(100)).await;
    let _ = udp_transport.receive_response().await;

    // Check stats
    let stats = udp_transport.connection_stats();
    println!("\nAsync UDP Connection Statistics:");
    println!("  Connected for: {:?}", stats.uptime());
    println!("  Commands sent: {}", stats.commands_sent);
    println!("  Responses received: {}", stats.responses_received);
    println!("  Bytes sent: {}", stats.bytes_sent);
    println!("  Bytes received: {}", stats.bytes_received);
    println!("  Errors: {}", stats.error_count);
    println!("  Idle time: {:?}", stats.idle_time());

    // Example using async TCP transport
    println!("\n\nTesting async TCP transport health check...");
    let tcp_addr: SocketAddr = "192.168.1.100:5678".parse()?;
    let mut tcp_transport = AsyncTcpTransport::new(tcp_addr).await?;

    // Check health
    if tcp_transport.is_healthy().await? {
        println!("✓ TCP connection is healthy");
    } else {
        println!("✗ TCP connection is not healthy");
    }

    // Send some commands
    tcp_transport.send_command(&PanTiltCommand::Home).await?;
    sleep(Duration::from_millis(100)).await;
    let _ = tcp_transport.receive_response().await;

    // Check stats again
    let stats = tcp_transport.connection_stats();
    println!("\nAsync TCP Connection Statistics:");
    println!("  Connected for: {:?}", stats.uptime());
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
        if udp_health? {
            "✓ Healthy"
        } else {
            "✗ Not healthy"
        }
    );
    println!(
        "TCP: {}",
        if tcp_health? {
            "✓ Healthy"
        } else {
            "✗ Not healthy"
        }
    );

    // Periodic health checks
    println!("\n\nPerforming periodic health checks...");
    for i in 1..=5 {
        sleep(Duration::from_secs(2)).await;
        print!("Health check {}: ", i);
        if tcp_transport.is_healthy().await? {
            println!("✓ Healthy");
        } else {
            println!("✗ Not healthy");
        }
    }

    Ok(())
}

#[cfg(not(feature = "async"))]
fn main() {
    eprintln!("This example requires the 'async' feature to be enabled.");
    eprintln!("Try running with: cargo run --example async_health_check --features async");
}
