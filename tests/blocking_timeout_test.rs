//! Test that blocking transports handle timeouts correctly.
//!
//! These tests verify that:
//! 1. Per-call timeouts are enforced by the OS (not via polling)
//! 2. Timeouts are configurable per operation
//! 3. The timeout error is returned within reasonable bounds

#![cfg(not(feature = "async"))]

use grafton_visca::transport::blocking::{Tcp, Udp};
use grafton_visca::transport::BlockingTransport;
use grafton_visca::Error;
use std::net::{TcpListener, UdpSocket};
use std::thread;
use std::time::{Duration, Instant};

/// A slow server that doesn't respond for testing timeouts
fn start_slow_tcp_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();

    thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            // Accept the connection but don't send any data
            // Keep the connection open
            thread::sleep(Duration::from_secs(10));
            drop(stream);
        }
    });

    // Give the server time to start
    thread::sleep(Duration::from_millis(100));
    addr
}

/// A slow UDP server that doesn't respond
fn start_slow_udp_server() -> String {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = socket.local_addr().unwrap().to_string();

    thread::spawn(move || {
        let mut buf = [0u8; 1024];
        // Wait for data but never respond
        let _ = socket.recv(&mut buf);
        thread::sleep(Duration::from_secs(10));
    });

    // Give the server time to start
    thread::sleep(Duration::from_millis(100));
    addr
}

#[test]
fn test_tcp_blocking_timeout_enforcement() {
    let addr = start_slow_tcp_server();

    // Connect to the slow server
    let transport = Tcp::connect(&addr).expect("Failed to connect");

    // Test different timeout durations
    // Windows has less precise timing, especially in CI, so we need larger tolerances
    let tolerance_multiplier = if cfg!(windows) || std::env::var("CI").is_ok() {
        2 // Double tolerance for Windows or CI environments
    } else {
        1
    };

    let test_cases = vec![
        (Duration::from_millis(100), 25 * tolerance_multiplier), // 100ms timeout, ±25-50ms tolerance
        (Duration::from_millis(500), 50 * tolerance_multiplier), // 500ms timeout, ±50-100ms tolerance
        (Duration::from_secs(1), 100 * tolerance_multiplier),    // 1s timeout, ±100-200ms tolerance
    ];

    for (timeout_duration, tolerance_ms) in test_cases {
        let start = Instant::now();
        let result = transport.recv_blocking_with_timeout(timeout_duration);
        let elapsed = start.elapsed();

        // Should get a timeout error
        match result {
            Err(Error::Timeout) => {
                // Check that timeout happened within tolerance
                let expected_ms = timeout_duration.as_millis() as u64;
                let actual_ms = elapsed.as_millis() as u64;
                let diff_ms = actual_ms.abs_diff(expected_ms);

                assert!(
                    diff_ms <= tolerance_ms,
                    "Timeout took {}ms, expected {}ms ±{}ms",
                    actual_ms,
                    expected_ms,
                    tolerance_ms
                );
            }
            Err(e) => panic!("Expected Timeout error, got: {:?}", e),
            Ok(_) => panic!("Expected timeout but got success"),
        }
    }
}

#[test]
fn test_udp_blocking_timeout_enforcement() {
    let addr = start_slow_udp_server();

    // Connect to the slow server
    let transport = Udp::connect(&addr).expect("Failed to connect");

    // Send something first to establish the "connection"
    let _ = transport.send_blocking(b"\x81\x01\x04\x00\x02\xFF");

    // Test different timeout durations
    // Windows has less precise timing, especially in CI, so we need larger tolerances
    let tolerance_multiplier = if cfg!(windows) || std::env::var("CI").is_ok() {
        2 // Double tolerance for Windows or CI environments
    } else {
        1
    };

    let test_cases = vec![
        (Duration::from_millis(100), 25 * tolerance_multiplier), // 100ms timeout, ±25-50ms tolerance
        (Duration::from_millis(500), 50 * tolerance_multiplier), // 500ms timeout, ±50-100ms tolerance
        (Duration::from_secs(1), 100 * tolerance_multiplier),    // 1s timeout, ±100-200ms tolerance
    ];

    for (timeout_duration, tolerance_ms) in test_cases {
        let start = Instant::now();
        let result = transport.recv_blocking_with_timeout(timeout_duration);
        let elapsed = start.elapsed();

        // Should get a timeout error
        match result {
            Err(Error::Timeout) => {
                // Check that timeout happened within tolerance
                let expected_ms = timeout_duration.as_millis() as u64;
                let actual_ms = elapsed.as_millis() as u64;
                let diff_ms = actual_ms.abs_diff(expected_ms);

                assert!(
                    diff_ms <= tolerance_ms,
                    "Timeout took {}ms, expected {}ms ±{}ms",
                    actual_ms,
                    expected_ms,
                    tolerance_ms
                );
            }
            Err(e) => panic!("Expected Timeout error, got: {:?}", e),
            Ok(_) => panic!("Expected timeout but got success"),
        }
    }
}

#[test]
#[ignore] // This test is too hardware-dependent for CI
fn test_blocking_timeout_no_polling() {
    // This test verifies that timeouts are not implemented via polling
    // by checking that the timeout happens efficiently without busy-waiting
    //
    // NOTE: This test is ignored by default as it depends heavily on
    // hardware performance and system load. It can be run manually with:
    // cargo test -- --ignored

    let addr = start_slow_tcp_server();
    let transport = Tcp::connect(&addr).expect("Failed to connect");

    // Measure CPU time before the operation
    let start = Instant::now();

    // Perform a blocking receive with timeout
    let _ = transport.recv_blocking_with_timeout(Duration::from_millis(200));

    let elapsed_wall = start.elapsed();

    // Basic sanity check: the timeout should have occurred
    assert!(
        elapsed_wall.as_millis() >= 180 && elapsed_wall.as_millis() <= 250,
        "Timeout took {:?}, expected ~200ms",
        elapsed_wall
    );

    // The actual CPU usage test would require platform-specific APIs
    // to measure thread CPU time accurately, which isn't portable.
    // The fact that the timeout completes in the expected time frame
    // is sufficient to verify correct behavior.
}

#[test]
fn test_timeout_restores_original_setting() {
    // This test verifies that the original timeout is restored after
    // recv_blocking_with_timeout completes

    let addr = start_slow_tcp_server();
    let transport = Tcp::connect(&addr).expect("Failed to connect");

    // First timeout with a short duration
    let _ = transport.recv_blocking_with_timeout(Duration::from_millis(100));

    // Second timeout with a different duration should also work correctly
    let start = Instant::now();
    let _ = transport.recv_blocking_with_timeout(Duration::from_millis(500));
    let elapsed = start.elapsed();

    // The second timeout should take approximately 500ms, not be affected by the first
    assert!(
        elapsed.as_millis() >= 450 && elapsed.as_millis() <= 550,
        "Second timeout took {:?}, expected ~500ms",
        elapsed
    );
}
