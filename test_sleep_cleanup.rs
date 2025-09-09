// Test that dropping SleepFuture cleans up sleepers
use std::time::Duration;

fn main() {
    // This will be a quick Rust test to verify sleep cleanup
    println!("Testing sleep future cleanup...");

    // We'll use the deterministic executor in a unit test instead
    #[cfg(test)]
    mod tests {
        use super::*;
        use grafton_visca::testing::testkit::deterministic_executor::{
            DeterministicExecutor, DeterministicExecutorExt,
        };
        use grafton_visca::Executor;

        #[test]
        fn test_sleep_cleanup_on_drop() {
            let (executor, clock) = DeterministicExecutor::new();

            // Start a sleep that we'll cancel
            let exec = executor.clone();
            let handle = executor.spawn(async move {
                // Start a 100ms sleep
                let sleep_fut = exec.sleep(Duration::from_millis(100));

                // Drop it immediately without awaiting
                drop(sleep_fut);

                // Return so we can verify
                42
            });

            // Drive the executor
            executor.drive_until_idle();

            // Check that there are no pending deadlines
            assert!(
                !clock.has_pending_deadlines(),
                "Sleep should have been cleaned up on drop"
            );

            println!("✓ Sleep cleanup works!");
        }
    }
}
