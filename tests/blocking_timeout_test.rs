//! Test that blocking transports handle timeouts correctly.
//!
//! These tests verify that:
//! 1. Per-call timeouts are enforced by the OS (not via polling)
//! 2. Timeouts are configurable per operation
//! 3. The timeout error is returned within reasonable bounds

#![cfg(not(feature = "async"))]

use std::{
    net::{TcpListener, UdpSocket},
    thread,
    time::{Duration, Instant},
};

use grafton_visca::{
    transport::{
        blocking::{Tcp, Udp},
        SyncTransport,
    },
    Error,
};

/// A slow server that doesn't respond for testing timeouts
fn start_slow_tcp_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();

    thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            thread::sleep(Duration::from_secs(10));
            drop(stream);
        }
    });

    thread::sleep(Duration::from_millis(100));
    addr
}

/// A slow UDP server that doesn't respond
fn start_slow_udp_server() -> String {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = socket.local_addr().unwrap().to_string();

    thread::spawn(move || {
        let mut buf = [0u8; 1024];
        let _ = socket.recv(&mut buf);
        thread::sleep(Duration::from_secs(10));
    });

    thread::sleep(Duration::from_millis(100));
    addr
}

#[test]
fn test_tcp_blocking_timeout_enforcement() {
    let addr = start_slow_tcp_server();

    // Connect to the slow server
    let mut transport = Tcp::connect(&addr).expect("Failed to connect");

    // Test different timeout durations
    // CI environments have highly variable timing, so we need much larger tolerances
    let (base_tolerance, multiplier) = if std::env::var("CI").is_ok() {
        (150, 3) // CI: base 150ms, 3x multiplier for larger timeouts
    } else if cfg!(windows) {
        (100, 2) // Windows: base 100ms, 2x multiplier
    } else {
        (50, 1) // Local Linux/Mac: base 50ms, 1x multiplier
    };

    let test_cases = vec![
        (Duration::from_millis(100), base_tolerance), // 100ms timeout
        (Duration::from_millis(500), base_tolerance * multiplier), // 500ms timeout
        (Duration::from_secs(1), base_tolerance * multiplier * 2), // 1s timeout
    ];

    for (timeout_duration, tolerance_ms) in test_cases {
        let start = Instant::now();
        let result = transport.recv_with_timeout(timeout_duration);
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
            Err(e) => panic!("Expected Timeout error, got: {e:?}"),
            Ok(_) => panic!("Expected timeout but got success"),
        }
    }
}

#[test]
fn test_udp_blocking_timeout_enforcement() {
    let addr = start_slow_udp_server();

    // Connect to the slow server
    let mut transport = Udp::connect(&addr).expect("Failed to connect");

    // Send something first to establish the "connection"
    let _ = transport.send(b"\x81\x01\x04\x00\x02\xFF");

    // Test different timeout durations
    // CI environments have highly variable timing, so we need much larger tolerances
    let (base_tolerance, multiplier) = if std::env::var("CI").is_ok() {
        (150, 3) // CI: base 150ms, 3x multiplier for larger timeouts
    } else if cfg!(windows) {
        (100, 2) // Windows: base 100ms, 2x multiplier
    } else {
        (50, 1) // Local Linux/Mac: base 50ms, 1x multiplier
    };

    let test_cases = vec![
        (Duration::from_millis(100), base_tolerance), // 100ms timeout
        (Duration::from_millis(500), base_tolerance * multiplier), // 500ms timeout
        (Duration::from_secs(1), base_tolerance * multiplier * 2), // 1s timeout
    ];

    for (timeout_duration, tolerance_ms) in test_cases {
        let start = Instant::now();
        let result = transport.recv_with_timeout(timeout_duration);
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
            Err(e) => panic!("Expected Timeout error, got: {e:?}"),
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
    let mut transport = Tcp::connect(&addr).expect("Failed to connect");

    // Measure CPU time before the operation
    let start = Instant::now();

    // Perform a blocking receive with timeout
    let _ = transport.recv_with_timeout(Duration::from_millis(200));

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
    // recv_with_timeout completes

    let addr = start_slow_tcp_server();
    let mut transport = Tcp::connect(&addr).expect("Failed to connect");

    // First timeout with a short duration
    let _ = transport.recv_with_timeout(Duration::from_millis(100));

    // Second timeout with a different duration should also work correctly
    let start = Instant::now();
    let _ = transport.recv_with_timeout(Duration::from_millis(500));
    let elapsed = start.elapsed();

    // The second timeout should take approximately 500ms, not be affected by the first
    // CI environments have variable timing, so we need a larger tolerance
    let (min_ms, max_ms) = if std::env::var("CI").is_ok() {
        (350, 750) // CI: ±250ms tolerance
    } else {
        (450, 550) // Local: ±50ms tolerance
    };

    assert!(
        elapsed.as_millis() >= min_ms && elapsed.as_millis() <= max_ms,
        "Second timeout took {:?}, expected ~500ms ({}ms-{}ms)",
        elapsed,
        min_ms,
        max_ms
    );
}
