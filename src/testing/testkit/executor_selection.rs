//! Test executor selection utilities.
//!
//! This module provides helpers for choosing the appropriate executor for different
//! test scenarios, addressing the limitations of DeterministicExecutor with timeout-based tests.
//!
//! ## Test Strategy
//!
//! Due to fundamental issues with virtual time and timeout handling in DeterministicExecutor,
//! we use different executors for different test scenarios:
//!
//! ### Use DeterministicExecutor for:
//! - Logic and sequencing tests
//! - Protocol correctness tests
//! - Tests that don't rely on actual timeouts
//! - Tests where deterministic execution order is important
//!
//! ### Use Real Runtime Executors for:
//! - Timeout behavior tests
//! - Tests that require actual time progression
//! - Tests involving concurrent timeouts
//! - Shutdown with pending timeouts
//!
//! ## Background
//!
//! The DeterministicExecutor uses virtual time for deterministic test execution,
//! but this creates complex interactions with runtime timeout handling:
//! - Runtime spawns background loops that manage timeouts
//! - Virtual time advancement doesn't properly coordinate with these background tasks
//! - This can create infinite loops where the executor waits for the runtime,
//!   and the runtime waits for time advancement
//!
//! Rather than trying to force deterministic execution on inherently time-dependent code,
//! we acknowledge this limitation and use appropriate tools for each testing scenario.

#![cfg(feature = "test-utils")]

/// Test executor type enumeration.
///
/// This enum represents the different executor types available for testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestExecutorType {
    /// DeterministicExecutor - for logic and sequencing tests
    Deterministic,
    /// TokioExecutor - for timeout and timing tests with Tokio
    #[cfg(feature = "runtime-tokio")]
    Tokio,
    /// SmolExecutor - for timeout and timing tests with smol
    #[cfg(feature = "runtime-smol")]
    Smol,
}

/// Trait for selecting and creating test executors.
pub trait TestExecutorSelector {
    /// Get the executor type suitable for logic and sequencing tests.
    fn for_logic_tests() -> TestExecutorType {
        TestExecutorType::Deterministic
    }

    /// Get the executor type suitable for timeout and timing tests.
    fn for_timeout_tests() -> TestExecutorType {
        // Use the first available real runtime
        #[cfg(feature = "runtime-tokio")]
        return TestExecutorType::Tokio;

        #[cfg(all(not(feature = "runtime-tokio"), feature = "runtime-smol"))]
        return TestExecutorType::Smol;

        #[cfg(all(not(feature = "runtime-tokio"), not(feature = "runtime-smol")))]
        panic!("No real runtime available for timeout tests. Enable at least one of: runtime-tokio, runtime-smol");
    }
}

/// Default test executor selector implementation.
#[derive(Debug, Clone, Copy)]
pub struct TestExecutors;

impl TestExecutorSelector for TestExecutors {}

/// Macro to create a test with the appropriate executor for logic tests.
///
/// This macro sets up a test that uses DeterministicExecutor for deterministic
/// logic and sequencing tests.
#[macro_export]
macro_rules! logic_test {
    ($name:ident, $body:expr) => {
        #[test]
        fn $name() {
            use $crate::testing::testkit::deterministic_executor::{
                DeterministicClock, DeterministicExecutor,
            };
            let (executor, clock): (std::sync::Arc<DeterministicExecutor>, DeterministicClock) =
                DeterministicExecutor::new();
            $body(executor, clock)
        }
    };
}

/// Macro to create a test with the appropriate executor for timeout tests.
///
/// This macro sets up a test that uses a real runtime executor for tests
/// that require actual timeout behavior.
#[macro_export]
macro_rules! timeout_test {
    ($name:ident, $body:expr) => {
        #[cfg(feature = "runtime-tokio")]
        #[tokio::test]
        async fn $name() {
            use $crate::executor::TokioExecutor;
            let executor = std::sync::Arc::new(TokioExecutor::from_current().unwrap());
            $body(executor).await
        }

        #[cfg(all(not(feature = "runtime-tokio"), feature = "runtime-smol"))]
        #[test]
        fn $name() {
            use $crate::executor::SmolExecutor;
            let executor = std::sync::Arc::new(SmolExecutor);
            smol::block_on(async { $body(executor).await })
        }
    };
}
