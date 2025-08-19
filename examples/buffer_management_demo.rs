//! Demonstrates the unified buffer management system across transports.

use grafton_visca::{
    transport::{
        buffer::{BufferConfig, BufferManager},
        builder::TransportBuilder,
    },
    Error,
};

fn main() -> Result<(), Error> {
    println!("=== Buffer Configuration via Builder ===");

    let _transport = TransportBuilder::udp()
        .address("192.168.0.110:5678")
        .udp_buffers()
        .recv_buffer_size(2048)
        .send_buffer_size(512)
        .max_buffer_size(4096)
        .build();

    println!("Created UDP transport with custom buffer configuration");

    println!("\n=== Direct Buffer Manager Usage ===");

    let default_manager = BufferManager::with_defaults();
    let recv_buf = default_manager.alloc_recv_buffer();
    println!(
        "Default receive buffer capacity: {capacity}",
        capacity = recv_buf.capacity()
    );

    let udp_config = BufferConfig::for_udp();
    let udp_manager = BufferManager::new(udp_config);
    let udp_buf = udp_manager.alloc_recv_buffer();
    println!(
        "UDP receive buffer capacity: {capacity}",
        capacity = udp_buf.capacity()
    );

    let sony_config = BufferConfig::for_sony_ip();
    let sony_manager = BufferManager::new(sony_config);
    let sony_buf = sony_manager.alloc_recv_buffer();
    println!(
        "Sony IP receive buffer capacity: {capacity}",
        capacity = sony_buf.capacity()
    );

    let raw_config = BufferConfig::for_raw_ip();
    let raw_manager = BufferManager::new(raw_config);
    let raw_buf = raw_manager.alloc_recv_buffer();
    println!(
        "Raw IP receive buffer capacity: {capacity}",
        capacity = raw_buf.capacity()
    );

    println!("\n=== Buffer Resizing ===");

    let manager = BufferManager::with_defaults();
    let mut buffer = manager.alloc_recv_buffer();
    println!(
        "Initial buffer capacity: {capacity}",
        capacity = buffer.capacity()
    );

    manager.resize_buffer(&mut buffer, 512);
    println!(
        "After resize to 512: {capacity}",
        capacity = buffer.capacity()
    );

    manager.resize_buffer(&mut buffer, 10000);
    println!(
        "After resize to 10000 (capped): {capacity}",
        capacity = buffer.capacity()
    );

    println!("\n=== Buffer Reset and Reuse ===");

    let mut buffer = manager.alloc_recv_buffer();

    buffer.extend_from_slice(b"VISCA command data");
    println!("Buffer contains {len} bytes", len = buffer.len());

    manager.reset_buffer(&mut buffer);
    println!("After reset: {len} bytes", len = buffer.len());

    println!("\n=== Transport-Specific Buffer Optimization ===");

    let _tcp = TransportBuilder::tcp()
        .address("192.168.0.110:5678")
        .buffer_size(128)
        .build();
    println!("TCP transport: optimized for small VISCA messages");

    let _udp = TransportBuilder::udp()
        .address("192.168.0.110:5678")
        .udp_buffers()
        .build();
    println!("UDP transport: optimized for datagram reception");

    let _sony_builder = TransportBuilder::tcp()
        .address("192.168.0.110:5678")
        .sony_ip_buffers()
        .build();
    println!("Sony IP transport: optimized for encapsulated VISCA");

    let _raw_builder = TransportBuilder::tcp()
        .address("192.168.0.110:5678")
        .raw_ip_buffers()
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
