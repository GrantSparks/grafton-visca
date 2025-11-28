//! Tests for Issue #435: Make `Mode::from_future` sound for `Blocking`.
//!
//! This test verifies that `Blocking::from_future` works correctly by:
//! 1. Testing that `from_future` executes async blocks synchronously
//! 2. Testing various scenarios where async blocks are used in blocking mode
//! 3. Ensuring no panics occur when using `from_future` with complex async operations

#![cfg(not(feature = "mode-async"))]

use grafton_visca::mode::{Blocking, BlockingFutureExt, Mode};

/// Test that Blocking::from_future executes async blocks synchronously
#[test]
fn test_blocking_from_future_executes_synchronously() {
    // Create an async block that performs computation
    let result = Blocking::from_future(async {
        let a = 10;
        let b = 20;
        a + b
    })
    .block();

    assert_eq!(
        result, 30,
        "Blocking::from_future should execute async blocks"
    );
}

/// Test that Blocking::from_future works with Result types
#[test]
fn test_blocking_from_future_with_result() {
    let ok_result: Result<i32, &str> = Blocking::from_future(async { Ok(42) }).block();

    assert_eq!(ok_result, Ok(42));

    let err_result: Result<i32, &str> = Blocking::from_future(async { Err("error") }).block();

    assert_eq!(err_result, Err("error"));
}

/// Test that Blocking::from_future can capture and use external state
#[test]
fn test_blocking_from_future_captures_state() {
    let external_value = 100;

    let result = Blocking::from_future(async move {
        // Use captured value
        external_value * 2
    })
    .block();

    assert_eq!(result, 200);
}

/// Test that Blocking::from_future handles nested async operations
#[test]
fn test_blocking_from_future_nested() {
    let result = Blocking::from_future(async {
        // Simulate nested async computation
        let inner = async { 21 };
        inner.await * 2
    })
    .block();

    assert_eq!(result, 42);
}

/// Test that Blocking::from_future works with Option types
#[test]
fn test_blocking_from_future_with_option() {
    let some_result: Option<i32> = Blocking::from_future(async { Some(42) }).block();
    assert_eq!(some_result, Some(42));

    let none_result: Option<i32> = Blocking::from_future(async { None }).block();
    assert_eq!(none_result, None);
}

/// Test that Blocking::from_future works with string types (to verify Send bound)
#[test]
fn test_blocking_from_future_with_string() {
    let result = Blocking::from_future(async {
        let s = String::from("hello");
        format!("{} world", s)
    })
    .block();

    assert_eq!(result, "hello world");
}

/// Test that Blocking::from_future works with vectors
#[test]
fn test_blocking_from_future_with_vec() {
    let result = Blocking::from_future(async {
        let mut v = vec![1, 2, 3];
        v.push(4);
        v
    })
    .block();

    assert_eq!(result, vec![1, 2, 3, 4]);
}

/// Test that Blocking::from_future can be used in a loop pattern
/// (simulating what impl_diagnostics! measure_latency does)
#[test]
fn test_blocking_from_future_in_loop_pattern() {
    let samples = 5;
    let result = Blocking::from_future(async move {
        let mut total = 0;
        for i in 0..samples {
            // Simulate some async work
            let inner = async move { i };
            total += inner.await;
        }
        total
    })
    .block();

    // 0 + 1 + 2 + 3 + 4 = 10
    assert_eq!(result, 10);
}

/// Test that Blocking::from_future works with complex error handling
/// (simulating what impl_diagnostics! probe/ping does)
#[test]
fn test_blocking_from_future_with_error_handling() {
    use std::time::{Duration, Instant};

    let result: Result<(Duration, bool), &str> = Blocking::from_future(async {
        let start = Instant::now();

        // Simulate an operation that might fail
        let operation_result: Result<bool, &str> = Ok(true);

        match operation_result {
            Ok(_) => {
                let elapsed = start.elapsed();
                Ok((elapsed, true))
            }
            Err(_) => {
                let elapsed = start.elapsed();
                Ok((elapsed, false))
            }
        }
    })
    .block();

    assert!(result.is_ok());
    let (_duration, success) = result.unwrap();
    assert!(success);
}

/// Test that the fix doesn't break Blocking::ready
#[test]
fn test_blocking_ready_still_works() {
    let result = Blocking::ready(42).block();
    assert_eq!(result, 42);
}

/// Test that into_inner alias works correctly
#[test]
fn test_blocking_into_inner_alias() {
    let result = Blocking::from_future(async { "test" }).into_inner();
    assert_eq!(result, "test");
}
