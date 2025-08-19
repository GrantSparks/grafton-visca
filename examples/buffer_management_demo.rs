//! Demonstrates the unified buffer management system across transports.

use grafton_visca::transport::buffer::{BufferConfig, BufferManager};
use grafton_visca::transport::builder::TransportBuilder;
use grafton_visca::Error;

fn main() -> Result<(), Error> {
    // Example 1: Using the builder with custom buffer sizes
    println!("=== Buffer Configuration via Builder ===");

    // Create a transport with custom buffer configuration
    let _transport = TransportBuilder::udp()
        .address("192.168.0.110:5678")
        .udp_buffers() // Use optimized UDP buffer sizes
        .recv_buffer_size(2048) // Override receive buffer size
        .send_buffer_size(512) // Override send buffer size
        .max_buffer_size(4096) // Set maximum buffer growth limit
        .build();

    println!("Created UDP transport with custom buffer configuration");

    // Example 2: Direct buffer manager usage
    println!("\n=== Direct Buffer Manager Usage ===");

    // Create a buffer manager with default configuration
    let default_manager = BufferManager::with_defaults();
    let recv_buf = default_manager.alloc_recv_buffer();
    println!("Default receive buffer capacity: {}", recv_buf.capacity());

    // Create a buffer manager for UDP
    let udp_config = BufferConfig::for_udp();
    let udp_manager = BufferManager::new(udp_config);
    let udp_buf = udp_manager.alloc_recv_buffer();
    println!("UDP receive buffer capacity: {}", udp_buf.capacity());

    // Create a buffer manager for Sony IP
    let sony_config = BufferConfig::for_sony_ip();
    let sony_manager = BufferManager::new(sony_config);
    let sony_buf = sony_manager.alloc_recv_buffer();
    println!("Sony IP receive buffer capacity: {}", sony_buf.capacity());

    // Create a buffer manager for raw IP
    let raw_config = BufferConfig::for_raw_ip();
    let raw_manager = BufferManager::new(raw_config);
    let raw_buf = raw_manager.alloc_recv_buffer();
    println!("Raw IP receive buffer capacity: {}", raw_buf.capacity());

    // Example 3: Buffer resizing
    println!("\n=== Buffer Resizing ===");

    let manager = BufferManager::with_defaults();
    let mut buffer = manager.alloc_recv_buffer();
    println!("Initial buffer capacity: {}", buffer.capacity());

    // Resize buffer for larger data
    manager.resize_buffer(&mut buffer, 512);
    println!("After resize to 512: {}", buffer.capacity());

    // Try to resize beyond max limit
    manager.resize_buffer(&mut buffer, 10000);
    println!("After resize to 10000 (capped): {}", buffer.capacity());

    // Example 4: Buffer reset and reuse
    println!("\n=== Buffer Reset and Reuse ===");

    let mut buffer = manager.alloc_recv_buffer();

    // Simulate receiving data
    buffer.extend_from_slice(b"VISCA command data");
    println!("Buffer contains {} bytes", buffer.len());

    // Reset for reuse
    manager.reset_buffer(&mut buffer);
    println!("After reset: {} bytes", buffer.len());

    // Example 5: Different transport types with appropriate buffers
    println!("\n=== Transport-Specific Buffer Optimization ===");

    // TCP uses small buffers as it reads until terminator
    let _tcp = TransportBuilder::tcp()
        .address("192.168.0.110:5678")
        .buffer_size(128) // Small buffer for TCP
        .build();
    println!("TCP transport: optimized for small VISCA messages");

    // UDP uses larger buffers for full datagrams
    let _udp = TransportBuilder::udp()
        .address("192.168.0.110:5678")
        .udp_buffers() // 1024 byte buffers
        .build();
    println!("UDP transport: optimized for datagram reception");

    // Sony IP needs medium buffers for encapsulated messages
    let _sony_builder = TransportBuilder::tcp()
        .address("192.168.0.110:5678")
        .sony_ip_buffers() // 512 byte buffers
        .build();
    println!("Sony IP transport: optimized for encapsulated VISCA");

    // Raw IP can batch multiple commands
    let _raw_builder = TransportBuilder::tcp()
        .address("192.168.0.110:5678")
        .raw_ip_buffers() // 256 byte buffers
        .build();
    println!("Raw IP transport: optimized for command batching");

    println!("\n=== Buffer Management Complete ===");
    println!("The unified buffer management system ensures:");
    println!("- Consistent buffer allocation across all transports");
    println!("- Optimal default sizes for each transport type");
    println!("- Protection against unbounded buffer growth");
    println!("- Efficient buffer reuse with automatic shrinking");

    Ok(())
}
