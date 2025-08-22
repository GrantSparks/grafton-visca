//! Time utilities for unified time handling across different executor types.

#[cfg(feature = "async")]
use std::time::Instant;

/// Get the current time from the executor.
///
/// For DeterministicExecutor, this returns virtual time.
/// For other executors, this returns wall-clock time.
#[cfg(feature = "async")]
pub fn now_from_executor_arc<E: crate::executor::Executor>(
    executor: &std::sync::Arc<E>,
) -> Instant {
    // Try to downcast to DeterministicExecutor
    #[cfg(feature = "test-utils")]
    {
        use std::any::Any;

        // When E is Arc<DeterministicExecutor>, &**executor gives us &DeterministicExecutor
        let executor_any: &dyn Any = &**executor;
        if let Some(det_exec) =
            executor_any.downcast_ref::<crate::testing::testkit::DeterministicExecutor>()
        {
            return det_exec.now();
        }
    }

    #[cfg(not(feature = "test-utils"))]
    let _ = executor;

    // Default: use standard library time
    // Runtime-specific executors should override this if they need special behavior
    Instant::now()
}

/// Get the current time from the executor reference.
///
/// For DeterministicExecutor, this returns virtual time.
/// For other executors, this returns wall-clock time.
#[cfg(feature = "async")]
pub fn now_from_executor<E: crate::executor::Executor>(executor: &E) -> Instant {
    // Try to downcast to DeterministicExecutor
    #[cfg(feature = "test-utils")]
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

    // Default: use standard library time
    // Runtime-specific executors should override this if they need special behavior
    let _ = executor;
    Instant::now()
}
