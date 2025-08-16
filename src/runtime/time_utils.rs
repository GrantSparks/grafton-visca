//! Time utilities for unified time handling across different executor types.

#[cfg(feature = "async")]
use std::time::Instant;

/// Get the current time from the executor.
///
/// For DeterministicExecutor, this returns virtual time.
/// For other executors, this returns wall-clock time.
#[cfg(feature = "async")]
pub fn now_from_executor_arc<E: crate::executor_unified::Executor>(
    executor: &std::sync::Arc<E>,
) -> Instant {
    // Try to downcast to DeterministicExecutor
    #[cfg(any(feature = "rt-tokio", feature = "test-utils"))]
    {
        use std::any::Any;

        // Check if E is DeterministicExecutor
        let executor_any: &dyn Any = &**executor;
        if let Some(det_exec) =
            executor_any.downcast_ref::<crate::testing::testkit::DeterministicExecutor>()
        {
            return det_exec.now();
        }
    }

    #[cfg(not(any(feature = "rt-tokio", feature = "test-utils")))]
    let _ = executor;

    // Fall back to wall-clock time for all other executors
    Instant::now()
}

/// Get the current time from the executor reference.
///
/// For DeterministicExecutor, this returns virtual time.
/// For other executors, this returns wall-clock time.
#[cfg(feature = "async")]
pub fn now_from_executor<E: crate::executor_unified::Executor>(executor: &E) -> Instant {
    // Try to downcast to DeterministicExecutor
    #[cfg(any(feature = "rt-tokio", feature = "test-utils"))]
    {
        use std::any::Any;

        let executor_any: &dyn Any = executor;
        if let Some(det_exec) =
            executor_any.downcast_ref::<crate::testing::testkit::DeterministicExecutor>()
        {
            return det_exec.now();
        }

        // Also check if it's an Arc<DeterministicExecutor>
        if let Some(arc_det) = executor_any
            .downcast_ref::<std::sync::Arc<crate::testing::testkit::DeterministicExecutor>>()
        {
            return arc_det.now();
        }
    }

    #[cfg(not(any(feature = "rt-tokio", feature = "test-utils")))]
    let _ = executor;

    // Fall back to wall-clock time for all other executors
    Instant::now()
}
