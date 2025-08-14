//! Compile-fail tests for executor type safety.
//!
//! These tests verify that the new executor-based API prevents runtime/spawner
//! mismatches at compile time.

#[test]
fn executor_compile_tests() {
    let t = trybuild::TestCases::new();

    // Test that mismatched executor types are rejected
    t.compile_fail("tests/ui/executor_mismatch.rs");

    // Test that cameras require executor for async operations
    t.compile_fail("tests/ui/async_without_executor.rs");

    // Test that blocking cameras cannot use async methods
    t.compile_fail("tests/ui/blocking_with_async.rs");

    // Test that executor types are properly constrained
    t.compile_fail("tests/ui/executor_type_mismatch.rs");
}
