use grafton_visca::command::power::Power;
use grafton_visca::command::{PanTiltCommand, PowerCommand};
use grafton_visca::{ConnectionManagement, TcpTransport, UdpTransport, ViscaTransport};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Example using UDP transport
    println!("Testing UDP transport health check...");
    let mut udp_transport = UdpTransport::new("192.168.1.100:1259")?;

    // Check initial health
    match udp_transport.is_healthy() {
        Ok(true) => println!("✓ UDP connection is healthy"),
        Ok(false) => println!("✗ UDP connection is not healthy"),
        Err(e) => println!("✗ Error checking UDP health: {}", e),
    }

    // Send a command
    let power_on = PowerCommand { power: Power::On };
    udp_transport.send_command(&power_on)?;
    thread::sleep(Duration::from_millis(100));
    let _ = udp_transport.receive_response();

    // Check stats
    let stats = udp_transport.connection_stats().snapshot();
    println!("\nUDP Connection Statistics:");
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

    // Example using TCP transport
    println!("\n\nTesting TCP transport health check...");
    let mut tcp_transport = TcpTransport::new("192.168.1.100:5678")?;

    // Check health
    match tcp_transport.is_healthy() {
        Ok(true) => println!("✓ TCP connection is healthy"),
        Ok(false) => println!("✗ TCP connection is not healthy"),
        Err(e) => println!("✗ Error checking TCP health: {}", e),
    }

    // Send some commands
    tcp_transport.send_command(&PanTiltCommand::Home)?;
    thread::sleep(Duration::from_millis(100));
    let _ = tcp_transport.receive_response();

    // Check stats again
    let stats = tcp_transport.connection_stats().snapshot();
    println!("\nTCP Connection Statistics:");
    println!(
        "  Connected for: {:?}",
        stats.connected_since.map(|t| t.elapsed())
    );
    println!("  Commands sent: {}", stats.commands_sent);
    println!("  Responses received: {}", stats.responses_received);
    println!("  Bytes sent: {}", stats.bytes_sent);
    println!("  Bytes received: {}", stats.bytes_received);
    println!("  Errors: {}", stats.error_count);

    // Demonstrate periodic health checks
    println!("\n\nPerforming periodic health checks...");
    for i in 1..=5 {
        thread::sleep(Duration::from_secs(2));
        print!("Health check {}: ", i);
        match tcp_transport.is_healthy() {
            Ok(true) => println!("✓ Healthy"),
            Ok(false) => println!("✗ Not healthy"),
            Err(e) => println!("✗ Error: {}", e),
        }
    }

    Ok(())
}
