//! Integration tests for the camera-first connection APIs.
#![cfg(not(feature = "mode-async"))]
//!
//! These tests verify that `Connect` and `CameraConfig` are the standard camera
//! construction path, while `CameraBuilder` remains scoped to advanced
//! BYO-transport flows.

use std::{
    net::{TcpListener, UdpSocket},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, CameraConfig, Connect},
    transport::{Transport, TransportConfig},
    CameraBuilder, Error,
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
    let camera_result = Connect::open_tcp_blocking::<PtzOpticsG2>(addr.to_string());

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
    let camera_result = Connect::open_udp_blocking::<PtzOpticsG2>(addr.to_string());

    assert!(
        camera_result.is_ok(),
        "Camera-first API should create UDP camera"
    );

    let _camera = camera_result.unwrap();
    thread::sleep(Duration::from_millis(100));

    // Test basic camera creation is working
    // Note: More detailed communication testing would require protocol-aware mock server
}

/// Test that CameraConfig uses the same explicit endpoint grammar at open time.
#[test]
fn test_camera_config_opens_configured_tcp_camera() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        let _ = listener.accept();
    });

    thread::sleep(Duration::from_millis(50));

    let camera_result = CameraConfig::<PtzOpticsG2>::tcp(addr.to_string()).open_blocking();

    assert!(
        camera_result.is_ok(),
        "CameraConfig should open TCP cameras with explicit endpoints"
    );
}

/// Test that low-level public transport builders reject missing ports before I/O.
#[test]
fn test_low_level_transport_builders_require_explicit_ports() {
    fn is_invalid_address<T>(result: &Result<T, Error>) -> bool {
        matches!(result, Err(Error::InvalidAddress { .. }))
    }

    let tcp_result = Transport::tcp().address("127.0.0.1").build_blocking();
    assert!(is_invalid_address(&tcp_result));

    let udp_result = Transport::udp().address("127.0.0.1").build_blocking();
    assert!(is_invalid_address(&udp_result));

    let bare_ipv6_result = Transport::udp().address("::1").build_blocking();
    assert!(is_invalid_address(&bare_ipv6_result));
}

/// Test advanced camera construction through CameraBuilder.
#[test]
fn test_camera_builder_configuration() {
    let transport = Transport::udp()
        .address("127.0.0.1:65535")
        .build_blocking()
        .expect("UDP transport should be constructible");

    let camera = CameraBuilder::from_transport_handle(transport)
        .profile::<PtzOpticsG2>()
        .open()
        .expect("CameraBuilder should attach an existing transport");

    camera.close().expect("closing camera should succeed");
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
    let camera_result = Connect::open_udp_blocking::<PtzOpticsG2>(addr.to_string());

    // Camera creation should work
    assert!(camera_result.is_ok(), "Camera creation should succeed");

    let camera = camera_result.unwrap();

    // Send a command to actually communicate with the server
    // The power state inquiry should work immediately
    let _ = camera.power().state();

    // Give server time to process
    thread::sleep(Duration::from_millis(200));

    // Verify server was contacted (tests that connection worked)
    assert!(
        attempt_count.load(Ordering::SeqCst) >= 1,
        "Should have made at least one attempt to connect"
    );
}

/// Test standard camera configuration transport types.
#[test]
fn test_camera_config_transport_types() {
    let _udp_config = CameraConfig::<PtzOpticsG2>::udp("192.168.0.110");

    let _tcp_config = CameraConfig::<PtzOpticsG2>::tcp("192.168.0.110")
        .transport_config(TransportConfig::default());
}

/// Test camera connection validation.
#[test]
fn test_camera_connection_validation() {
    let _valid_builder = Connect::builder().tcp("127.0.0.1").with_default_port();

    // Invalid address should be caught at connection time
    let invalid_result = Connect::open_tcp_blocking::<PtzOpticsG2>("invalid:address:format");
    assert!(invalid_result.is_err(), "Invalid address should fail");
}

/// Test primary Connect methods.
#[test]
fn test_connect_connection_methods() {
    // Test that Connect methods work with explicit addresses
    // (they will fail to connect, but should compile and create the right error)

    let tcp_result = Connect::open_tcp_blocking::<PtzOpticsG2>("127.0.0.1:65535");
    let udp_result = Connect::open_udp_blocking::<PtzOpticsG2>("127.0.0.1:65535");

    // TCP should fail gracefully (connection refused or timeout)
    assert!(
        tcp_result.is_err(),
        "TCP should fail to connect to invalid port"
    );

    // UDP might succeed initially since it's connectionless, but will fail on first operation
    // So we just verify it creates a camera instance (succeeds or fails gracefully)
    match udp_result {
        Ok(_camera) => {
            // UDP "connection" succeeded (expected for connectionless protocol)
            // Would fail on first actual command
        }
        Err(_) => {
            // UDP failed immediately (also acceptable)
        }
    }
}

/// Test that camera configuration flow works end-to-end
#[test]
fn test_complete_camera_configuration_flow() {
    let _config =
        CameraConfig::<PtzOpticsG2>::tcp("127.0.0.1").transport_config(TransportConfig::default());

    let _connect_result = Connect::open_tcp_blocking::<PtzOpticsG2>("127.0.0.1:65535");
    // Will fail to connect, but should compile successfully
}
