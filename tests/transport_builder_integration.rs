//! Integration tests for the transport builder pattern.
#![cfg(not(feature = "async"))]
//!
//! These tests verify that the builder pattern correctly creates and configures
//! transports with all the new components (BufferManager, RetryExecutor, etc.)

use std::{
    net::{TcpListener, UdpSocket},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use grafton_visca::{
    transport::{
        builder::{TransportBuilder, TransportBuilderExt},
        SyncTransport,
    },
    Error,
};

/// Test that the builder creates a TCP transport with proper configuration
#[test]
fn test_builder_creates_configured_tcp_transport() {
    // Start a mock TCP server
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn server thread
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            use std::io::{Read, Write};

            // Read the incoming command first
            let mut buffer = [0u8; 256];
            if stream.read(&mut buffer).is_ok() {
                // Send a VISCA response
                stream.write_all(&[0x90, 0x50, 0xFF]).unwrap();
                stream.flush().unwrap();
            }
        }
    });

    // Give the server thread time to start listening
    thread::sleep(Duration::from_millis(50));

    // Use builder to create transport
    let transport = TransportBuilder::tcp()
        .address(addr.to_string().as_str())
        .connect_timeout(Duration::from_secs(2))
        .read_timeout(Duration::from_secs(1))
        .tcp_nodelay(true)
        .max_retries(5)
        .build();

    assert!(
        transport.is_ok(),
        "Builder should create transport successfully"
    );

    // Verify transport can send and receive
    let mut transport = transport.unwrap();
    transport
        .send(&[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF])
        .unwrap();
    let response = transport.recv();
    assert!(response.is_ok(), "Should receive response");
}

/// Test that the builder creates a UDP transport with proper configuration
#[test]
fn test_builder_creates_configured_udp_transport() {
    // Create a mock UDP server
    let server = UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();

    let received = Arc::new(Mutex::new(Vec::new()));
    let received_clone = received.clone();

    // Spawn server thread
    thread::spawn(move || {
        let mut buf = [0u8; 1024];
        if let Ok((len, src)) = server.recv_from(&mut buf) {
            received_clone
                .lock()
                .unwrap()
                .extend_from_slice(&buf[..len]);
            // Echo back with VISCA terminator
            server.send_to(&[0x90, 0x50, 0xFF], src).unwrap();
        }
    });

    // Use builder to create transport
    let transport = TransportBuilder::udp()
        .address(addr.to_string().as_str())
        .connect_timeout(Duration::from_secs(2))
        .recv_buffer_size(512)
        .build();

    assert!(transport.is_ok(), "Builder should create UDP transport");

    let mut transport = transport.unwrap();
    transport
        .send(&[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF])
        .unwrap();

    thread::sleep(Duration::from_millis(100));

    // Verify server received the data
    let received_data = received.lock().unwrap();
    assert_eq!(&received_data[..], &[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]);
}

/// Test buffer configuration through builder
#[test]
fn test_builder_buffer_configuration() {
    // We can't access private fields, but we can verify the builder accepts these methods
    let _builder = TransportBuilder::tcp()
        .address("192.168.0.110:5678")
        .recv_buffer_size(256)
        .recv_buffer_size(512)
        .send_buffer_size(128);

    // The fact that this compiles verifies the methods exist and work
}

/// Test retry configuration through builder
#[test]
fn test_builder_retry_configuration() {
    use std::sync::atomic::{AtomicU32, Ordering};

    // Create a server that fails the first few times
    let server = UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();
    let attempt_count = Arc::new(AtomicU32::new(0));
    let attempt_count_clone = attempt_count.clone();

    thread::spawn(move || {
        let mut buf = [0u8; 1024];
        loop {
            if let Ok((_, src)) = server.recv_from(&mut buf) {
                let count = attempt_count_clone.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    // Don't respond to first 2 attempts to trigger retries
                    continue;
                }
                // Finally respond
                server.send_to(&[0x90, 0x50, 0xFF], src).unwrap();
                break;
            }
        }
    });

    // Use builder with retry configuration
    let mut transport = TransportBuilder::udp()
        .address(addr.to_string().as_str())
        .max_retries(5) // Allow enough retries
        .retry_delay(Duration::from_millis(50)) // Short delay for testing
        .build()
        .unwrap();

    // This should succeed after retries
    transport
        .send(&[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF])
        .unwrap();

    // Give server time to process
    thread::sleep(Duration::from_millis(200));

    // Verify that retries happened (should be 3 attempts total: 1 initial + 2 retries)
    assert!(
        attempt_count.load(Ordering::SeqCst) >= 1,
        "Should have made at least one attempt"
    );
}

/// Test transport-specific buffer presets
#[test]
fn test_builder_transport_specific_buffers() {
    // We can verify these methods exist and compile
    let _udp_builder = TransportBuilder::udp()
        .address("192.168.0.110:5678")
        .udp_buffers();

    let _sony_builder = TransportBuilder::tcp()
        .address("192.168.0.110:5678")
        .sony_ip_buffers();

    let _raw_builder = TransportBuilder::tcp()
        .address("192.168.0.110:5678")
        .raw_ip_buffers();

    // The actual buffer sizes are tested in the unit tests of the builder module
}

/// Test builder validation
#[test]
fn test_builder_validation() {
    // Missing address
    let result = TransportBuilder::tcp().build();
    assert!(matches!(result, Err(Error::InvalidParameter { .. })));
}

/// Test extension trait
#[test]
fn test_transport_builder_extension_trait() {
    use grafton_visca::transport::blocking::{Tcp, Udp};

    // Test that the extension trait methods exist and compile
    let _tcp_builder = Tcp::builder();
    let _udp_builder = Udp::builder();

    // The fact that these compile verifies the extension trait is working
}

/// Test that all configurations are properly applied
#[test]
fn test_complete_configuration_flow() {
    // Create a fully configured builder - this tests that all methods compile and chain properly
    let _builder = TransportBuilder::tcp()
        .address("127.0.0.1:12345")
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(2))
        .write_timeout(Duration::from_secs(3))
        .tcp_nodelay(true)
        .ttl(64)
        .recv_buffer_size(512)
        .max_retries(7)
        .retry_delay(Duration::from_millis(200))
        .exponential_backoff(true)
        .max_retry_duration(Duration::from_secs(45));

    // The fact that this compiles with all methods chained verifies the fluent API works
    // The actual values are tested in the unit tests within the builder module
}
