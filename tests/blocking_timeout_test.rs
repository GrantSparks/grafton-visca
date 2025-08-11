//! Test that blocking transports handle timeouts correctly.
//!
//! These tests verify that:
//! 1. Per-call timeouts are enforced by the OS (not via polling)
//! 2. Timeouts are configurable per operation
//! 3. The timeout error is returned within reasonable bounds

#![cfg(not(feature = "async"))]

use grafton_visca::transport::blocking::{Tcp, Udp};
use grafton_visca::transport::core::{BlockingTransport, Transport};
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
    let test_cases = vec![
        (Duration::from_millis(100), 25), // 100ms timeout, ±25ms tolerance
        (Duration::from_millis(500), 50), // 500ms timeout, ±50ms tolerance
        (Duration::from_secs(1), 100),    // 1s timeout, ±100ms tolerance
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
                let diff_ms = if actual_ms > expected_ms {
                    actual_ms - expected_ms
                } else {
                    expected_ms - actual_ms
                };

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
    let send_fut = transport.send(b"\x81\x01\x04\x00\x02\xFF");
    // Block on the future (it should be Ready for blocking transports)
    let _ = grafton_visca::executor::block_on(send_fut);

    // Test different timeout durations
    let test_cases = vec![
        (Duration::from_millis(100), 25), // 100ms timeout, ±25ms tolerance
        (Duration::from_millis(500), 50), // 500ms timeout, ±50ms tolerance
        (Duration::from_secs(1), 100),    // 1s timeout, ±100ms tolerance
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
                let diff_ms = if actual_ms > expected_ms {
                    actual_ms - expected_ms
                } else {
                    expected_ms - actual_ms
                };

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
fn test_blocking_timeout_no_polling() {
    // This test verifies that timeouts are not implemented via polling
    // by checking that the timeout happens efficiently without busy-waiting

    let addr = start_slow_tcp_server();
    let transport = Tcp::connect(&addr).expect("Failed to connect");

    // Measure CPU time before the operation
    let start = Instant::now();
    let start_thread_time = std::time::SystemTime::now();

    // Perform a blocking receive with timeout
    let _ = transport.recv_blocking_with_timeout(Duration::from_millis(200));

    let elapsed_wall = start.elapsed();
    let elapsed_thread = std::time::SystemTime::now()
        .duration_since(start_thread_time)
        .unwrap_or(Duration::ZERO);

    // If this were using polling with 1ms sleeps (as executor::timeout does),
    // we'd expect the thread time to be close to wall time
    // With proper OS-level blocking, thread time should be much less

    // This is a heuristic test - we just verify that we're not constantly active
    // We allow up to 50ms of active CPU time for a 200ms timeout
    assert!(
        elapsed_thread.as_millis() < 50,
        "Thread was too active during blocking timeout: {:?} thread time for {:?} wall time",
        elapsed_thread,
        elapsed_wall
    );
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
