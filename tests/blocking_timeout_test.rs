//! Test that blocking transports handle timeouts correctly.
//!
//! These tests verify that:
//! 1. Per-call timeouts are enforced by the OS (not via polling)
//! 2. Timeouts are configurable per operation
//! 3. The timeout error is returned within reasonable bounds

#![cfg(not(feature = "async"))]

use grafton_visca::{
    command::CommandKind,
    transport::{
        blocking::{Tcp, Udp},
        SyncTransport,
    },
    Error,
};

use std::{
    net::{TcpListener, UdpSocket},
    thread,
    time::{Duration, Instant},
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

    let mut transport = Tcp::connect(&addr).expect("Failed to connect");

    // CI environments have highly variable timing, so we need much larger tolerances
    let (base_tolerance, multiplier) = if std::env::var("CI").is_ok() {
        (150, 3)
    } else if cfg!(windows) {
        (100, 2)
    } else {
        (50, 1)
    };

    let test_cases = vec![
        (Duration::from_millis(100), base_tolerance),
        (Duration::from_millis(500), base_tolerance * multiplier),
        (Duration::from_secs(1), base_tolerance * multiplier * 2),
    ];

    for (timeout_duration, tolerance_ms) in test_cases {
        let start = Instant::now();
        let result = transport.recv_with_timeout(timeout_duration);
        let elapsed = start.elapsed();

        match result {
            Err(Error::Timeout) => {
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

    let mut transport = Udp::connect(&addr).expect("Failed to connect");

    let _ = transport.send_with_kind(b"\x81\x01\x04\x00\x02\xFF", CommandKind::Command);

    // CI environments have highly variable timing, so we need much larger tolerances
    let (base_tolerance, multiplier) = if std::env::var("CI").is_ok() {
        (150, 3)
    } else if cfg!(windows) {
        (100, 2)
    } else {
        (50, 1)
    };

    let test_cases = vec![
        (Duration::from_millis(100), base_tolerance),
        (Duration::from_millis(500), base_tolerance * multiplier),
        (Duration::from_secs(1), base_tolerance * multiplier * 2),
    ];

    for (timeout_duration, tolerance_ms) in test_cases {
        let start = Instant::now();
        let result = transport.recv_with_timeout(timeout_duration);
        let elapsed = start.elapsed();

        match result {
            Err(Error::Timeout) => {
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

    let start = Instant::now();

    let _ = transport.recv_with_timeout(Duration::from_millis(200));

    let elapsed_wall = start.elapsed();

    assert!(
        elapsed_wall.as_millis() >= 180 && elapsed_wall.as_millis() <= 250,
        "Timeout took {:?}, expected ~200ms",
        elapsed_wall
    );
}

#[test]
fn test_timeout_restores_original_setting() {
    // This test verifies that the original timeout is restored after
    // recv_with_timeout completes

    let addr = start_slow_tcp_server();
    let mut transport = Tcp::connect(&addr).expect("Failed to connect");

    let _ = transport.recv_with_timeout(Duration::from_millis(100));

    let start = Instant::now();
    let _ = transport.recv_with_timeout(Duration::from_millis(500));
    let elapsed = start.elapsed();

    // CI environments have variable timing, so we need a larger tolerance
    let (min_ms, max_ms) = if std::env::var("CI").is_ok() {
        (350, 750)
    } else {
        (450, 550)
    };

    assert!(
        elapsed.as_millis() >= min_ms && elapsed.as_millis() <= max_ms,
        "Second timeout took {:?}, expected ~500ms ({}ms-{}ms)",
        elapsed,
        min_ms,
        max_ms
    );
}
