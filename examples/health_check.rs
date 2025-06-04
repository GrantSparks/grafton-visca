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
    if udp_transport.is_healthy() {
        println!("✓ UDP connection is healthy");
    } else {
        println!("✗ UDP connection is not healthy");
    }

    // Send a command
    let power_on = PowerCommand { power: Power::On };
    udp_transport.send_command(&power_on)?;
    thread::sleep(Duration::from_millis(100));
    let _ = udp_transport.receive_response();

    // Check stats
    let stats = udp_transport.connection_stats();
    println!("\nUDP Connection Statistics:");
    println!("  Connected for: {:?}", stats.uptime());
    println!("  Commands sent: {}", stats.commands_sent);
    println!("  Responses received: {}", stats.responses_received);
    println!("  Bytes sent: {}", stats.bytes_sent);
    println!("  Bytes received: {}", stats.bytes_received);
    println!("  Errors: {}", stats.error_count);
    println!("  Idle time: {:?}", stats.idle_time());

    // Example using TCP transport
    println!("\n\nTesting TCP transport health check...");
    let mut tcp_transport = TcpTransport::new("192.168.1.100:5678")?;

    // Check health
    if tcp_transport.is_healthy() {
        println!("✓ TCP connection is healthy");
    } else {
        println!("✗ TCP connection is not healthy");
    }

    // Send some commands
    tcp_transport.send_command(&PanTiltCommand::Home)?;
    thread::sleep(Duration::from_millis(100));
    let _ = tcp_transport.receive_response();

    // Check stats again
    let stats = tcp_transport.connection_stats();
    println!("\nTCP Connection Statistics:");
    println!("  Connected for: {:?}", stats.uptime());
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
        if tcp_transport.is_healthy() {
            println!("✓ Healthy");
        } else {
            println!("✗ Not healthy");
        }
    }

    Ok(())
}
