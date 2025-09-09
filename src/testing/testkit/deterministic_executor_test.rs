//! Test the sleep cleanup fix for deterministic executor

#[cfg(test)]
mod tests {
    use crate::testing::testkit::deterministic_executor::{
        DeterministicClock, DeterministicExecutor, DeterministicExecutorExt,
    };
    use crate::Executor;
    use futures_lite::future;
    use std::time::Duration;

    #[test]
    fn test_sleep_cleanup_in_race() {
        let (executor, clock) = DeterministicExecutor::new();

        // Test that when a sleep loses a race, it's cleaned up
        let exec = executor.clone();
        executor.block_on_bg(async move {
            // Create a race between a sleep and an immediate value
            let winner = future::race(
                async {
                    exec.sleep(Duration::from_millis(100)).await;
                    "sleep"
                },
                async { "immediate" },
            )
            .await;

            assert_eq!(winner, "immediate", "Immediate value should win");
        });

        // After the future completes, there should be no pending deadlines
        // because the losing sleep should have been dropped and cleaned up
        assert!(
            !clock.has_pending_deadlines(),
            "Sleep future should have been cleaned up when it lost the race"
        );
    }

    #[test]
    fn test_multiple_sleep_cleanup() {
        let (executor, clock) = DeterministicExecutor::new();

        let exec = executor.clone();
        executor.block_on_bg(async move {
            // Start multiple sleeps in a race
            let winner = future::race(
                future::race(
                    exec.sleep(Duration::from_millis(50)),
                    exec.sleep(Duration::from_millis(100)),
                ),
                async {
                    // Immediate completion
                },
            )
            .await;
        });

        // All sleeps should be cleaned up
        assert!(
            !clock.has_pending_deadlines(),
            "All sleep futures should have been cleaned up"
        );
    }
}
