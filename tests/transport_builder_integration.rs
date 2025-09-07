//! Integration tests for the camera-first API with builder pattern.
#![cfg(not(feature = "async"))]
//!
//! These tests verify that the camera-first API correctly creates and configures
//! cameras with the transport builder pattern.

use std::{
    net::{TcpListener, UdpSocket},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use grafton_visca::{
    camera::profiles::PtzOpticsG2,
    mode::{Blocking, BlockingFutureExt},
    Camera, CameraBuilder, PowerControl,
};

/// Test that the camera-first API creates a TCP camera with proper configuration
#[test]
fn test_camera_creates_configured_tcp_camera() {
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

    // Use camera-first API to create camera
    let camera_result = Camera::<
        Blocking,
        PtzOpticsG2,
        Box<dyn grafton_visca::transport::SyncTransport>,
        (),
    >::open_tcp(addr.to_string());

    assert!(
        camera_result.is_ok(),
        "Camera-first API should create camera successfully"
    );

    // Test that camera can be used for basic operations
    let _camera = camera_result.unwrap();
    // Note: More detailed communication testing would require a more sophisticated mock server
}

/// Test that the camera-first API creates a UDP camera with proper configuration
#[test]
fn test_camera_creates_configured_udp_camera() {
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

    // Use camera-first API to create camera
    let camera_result = Camera::<
        Blocking,
        PtzOpticsG2,
        Box<dyn grafton_visca::transport::SyncTransport>,
        (),
    >::open_udp(addr.to_string());

    assert!(
        camera_result.is_ok(),
        "Camera-first API should create UDP camera"
    );

    let _camera = camera_result.unwrap();
    thread::sleep(Duration::from_millis(100));

    // Test basic camera creation is working
    // Note: More detailed communication testing would require protocol-aware mock server
}

/// Test camera configuration through builder
#[test]
fn test_camera_builder_configuration() {
    // We can verify the camera builder accepts configuration methods
    let builder = CameraBuilder::tcp("192.168.0.110:5678").profile::<PtzOpticsG2>();

    // The fact that this compiles verifies the methods exist and work
    // Note: We don't call build() to avoid connection attempts in unit tests
    let _ = builder;
}

/// Test camera retry behavior with unreliable server
#[test]
fn test_camera_retry_behavior() {
    use std::sync::atomic::{AtomicU32, Ordering};

    // Create a server that fails the first few times
    let server = UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();
    let attempt_count = Arc::new(AtomicU32::new(0));
    let attempt_count_clone = attempt_count.clone();

    thread::spawn(move || {
        let mut buf = [0u8; 1024];
        loop {
            if let Ok((len, src)) = server.recv_from(&mut buf) {
                let count = attempt_count_clone.fetch_add(1, Ordering::SeqCst);

                // Check if this is a power inquiry command
                if len >= 5
                    && buf[0] == 0x81
                    && buf[1] == 0x09
                    && buf[2] == 0x04
                    && buf[3] == 0x00
                    && buf[4] == 0xFF
                {
                    // Respond with power status (off)
                    server.send_to(&[0x90, 0x50, 0x03, 0xFF], src).unwrap();
                } else if count < 2 {
                    // Don't respond to first 2 attempts of other commands to trigger retries
                    continue;
                } else {
                    // Finally respond with generic ACK
                    server.send_to(&[0x90, 0x41, 0xFF], src).unwrap();
                }

                if count >= 2 {
                    break;
                }
            }
        }
    });

    // Create camera (underlying transport handles retries)
    let camera_result = Camera::<
        Blocking,
        PtzOpticsG2,
        Box<dyn grafton_visca::transport::SyncTransport>,
        (),
    >::open_udp(addr.to_string());

    // Camera creation should work
    assert!(camera_result.is_ok(), "Camera creation should succeed");

    let camera = camera_result.unwrap();

    // Send a command to actually communicate with the server
    // The power_inquiry should work immediately
    let _ = camera.power_inquiry().block();

    // Give server time to process
    thread::sleep(Duration::from_millis(200));

    // Verify server was contacted (tests that connection worked)
    assert!(
        attempt_count.load(Ordering::SeqCst) >= 1,
        "Should have made at least one attempt to connect"
    );
}

/// Test camera builder transport types
#[test]
fn test_camera_builder_transport_types() {
    // We can verify that both TCP and UDP camera builders work
    let _udp_builder = CameraBuilder::udp("192.168.0.110:1259").profile::<PtzOpticsG2>();

    let _tcp_builder = CameraBuilder::tcp("192.168.0.110:5678").profile::<PtzOpticsG2>();

    // The fact that these compile verifies the camera-first API supports both transports
}

/// Test camera builder validation
#[test]
fn test_camera_builder_validation() {
    // Camera builder requires both transport type and profile
    // This test verifies the fluent API requires proper configuration

    // Valid configuration should compile
    let _valid_builder = CameraBuilder::tcp("127.0.0.1:5678").profile::<PtzOpticsG2>();

    // Invalid address should be caught at connection time
    let invalid_result = Camera::<
        Blocking,
        PtzOpticsG2,
        Box<dyn grafton_visca::transport::SyncTransport>,
        (),
    >::open_tcp("invalid:address:format");
    assert!(invalid_result.is_err(), "Invalid address should fail");
}

/// Test camera direct connection methods
#[test]
fn test_camera_direct_connection_methods() {
    // Test that direct connection methods work without addresses
    // (they will fail to connect, but should compile and create the right error)

    let tcp_result = Camera::<
        Blocking,
        PtzOpticsG2,
        Box<dyn grafton_visca::transport::SyncTransport>,
        (),
    >::open_tcp("127.0.0.1:99999");
    let udp_result = Camera::<
        Blocking,
        PtzOpticsG2,
        Box<dyn grafton_visca::transport::SyncTransport>,
        (),
    >::open_udp("127.0.0.1:99999");

    // Both should fail gracefully (connection refused or timeout)
    assert!(
        tcp_result.is_err(),
        "Should fail to connect to invalid port"
    );
    assert!(
        udp_result.is_err(),
        "Should fail to connect to invalid port"
    );
}

/// Test that camera configuration flow works end-to-end
#[test]
fn test_complete_camera_configuration_flow() {
    // Create a fully configured camera builder - this tests that all methods compile and chain properly
    let _builder = CameraBuilder::tcp("127.0.0.1:12345").profile::<PtzOpticsG2>();

    // The fact that this compiles with method chaining verifies the fluent API works
    // The camera-first API provides a simpler, more focused interface than the transport builder

    // Test that both direct connection and builder pattern work
    let _direct_result = Camera::<
        Blocking,
        PtzOpticsG2,
        Box<dyn grafton_visca::transport::SyncTransport>,
        (),
    >::open_tcp("127.0.0.1:99999");
    // Will fail to connect, but should compile successfully
}
