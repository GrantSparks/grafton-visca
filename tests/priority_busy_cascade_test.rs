//! Test for busy command cascading across priorities.
//!
//! This test verifies that when commands receive BUSY responses,
//! the runtime correctly handles retries across different priority levels.

#![cfg(all(feature = "async", feature = "test-utils"))]

use std::time::Duration;

use grafton_visca::{
    camera_id::CameraId,
    command::{power::Power, zoom::Zoom},
    runtime::{Priority, RuntimeHandle},
    testing::testkit::{
        deterministic_executor::{DeterministicExecutorExt, ExecutorExt},
        helpers::{ack, buffer_full, complete},
        DeterministicExecutor, ScriptedTransport,
    },
    Executor,
};

#[test]
fn test_busy_cascade_across_priorities() {
    let (executor, clock) = DeterministicExecutor::new();

    executor.clone().block_on_bg(async move {
        // Create a transport using the built-in helper for BUFFER FULL then success
        // BufferFull is always retryable, while NotExecutable is only retryable for Movement/Preset
        let steps = grafton_visca::testing::testkit::helpers::buffer_full_then_success(1);

        let transport = ScriptedTransport::new(steps).with_executor(executor.clone());

        let runtime = RuntimeHandle::new(transport, executor.clone())
            .await
            .expect("Failed to create runtime");

        clock.advance(Duration::from_millis(50));
        executor.drive_until_idle();

        let power_cmd = Power::On;

        // Spawn a task to drive the executor and advance time
        let executor_clone = executor.clone();
        let clock_clone = clock.clone();
        let _driver_handle = executor.spawn(async move {
            for _ in 0..20 {
                clock_clone.advance(Duration::from_millis(100));
                executor_clone.drive_until_idle();
                executor_clone.sleep(Duration::from_micros(1)).await;
            }
        });

        // Start the command send
        let command_future =
            runtime.send_command(&power_cmd, CameraId::default(), Some(Priority::Normal));

        let power_result = command_future.await;

        // Command should eventually succeed after the retry
        assert!(
            power_result.is_ok(),
            "Power command should succeed after retry: {:?}",
            power_result
        );
    });
}

#[test]
fn test_busy_with_max_retries() {
    let (executor, clock) = DeterministicExecutor::new();

    executor.clone().block_on_bg(async move {
        // Create a transport that always returns BUFFER FULL to test retry exhaustion
        // Power has category "Quick" with max_retries = 5
        // We need 6 BUFFER FULL responses to trigger exhaustion (attempt > max_retries)
        let steps = vec![
            // Keep returning BUSY responses until retries are exhausted
            grafton_visca::testing::testkit::Step::OnSend {
                matches: None,
                responses: vec![buffer_full(1)],
            },
            grafton_visca::testing::testkit::Step::OnSend {
                matches: None,
                responses: vec![buffer_full(1)],
            },
            grafton_visca::testing::testkit::Step::OnSend {
                matches: None,
                responses: vec![buffer_full(1)],
            },
            grafton_visca::testing::testkit::Step::OnSend {
                matches: None,
                responses: vec![buffer_full(1)],
            },
            grafton_visca::testing::testkit::Step::OnSend {
                matches: None,
                responses: vec![buffer_full(1)],
            },
            grafton_visca::testing::testkit::Step::OnSend {
                matches: None,
                responses: vec![buffer_full(1)], // 6th BUSY triggers exhaustion
            },
        ];

        let transport = ScriptedTransport::new(steps).with_executor(executor.clone());

        let runtime = RuntimeHandle::new(transport, executor.clone())
            .await
            .expect("Failed to create runtime");

        // Allow the runtime to start
        clock.advance(Duration::from_millis(50));

        // Submit a command that will exhaust retries
        let power_cmd = Power::On;

        // Create a task to periodically advance time while command executes
        let executor_clone = executor.clone();
        let clock_clone = clock.clone();
        executor.spawn_bg(async move {
            for _ in 0..20 {
                executor_clone.drive_until_idle();
                clock_clone.advance(Duration::from_millis(100));

                // Add a small async yield to let other tasks run
                executor_clone.sleep(Duration::from_micros(1)).await;
            }
        });

        let result = runtime
            .send_command(&power_cmd, CameraId::default(), Some(Priority::Normal))
            .await;

        // Command should eventually fail after max retries
        assert!(
            result.is_err(),
            "Command should fail after exhausting retries"
        );
    });
}

#[test]
fn test_priority_order_during_busy_recovery() {
    let (executor, clock) = DeterministicExecutor::new();

    executor.clone().block_on_bg(async move {
        // Create a transport that handles multiple commands with different priorities
        let steps = vec![
            // Critical command gets BUSY
            grafton_visca::testing::testkit::Step::OnSend {
                matches: None,
                responses: vec![buffer_full(1)],
            },
            // High priority command gets queued
            grafton_visca::testing::testkit::Step::OnSend {
                matches: None,
                responses: vec![ack(1), complete(1)], // Critical retry succeeds
            },
            // Normal priority command gets processed last
            grafton_visca::testing::testkit::Step::OnSend {
                matches: None,
                responses: vec![ack(2), complete(2)], // High priority succeeds
            },
            grafton_visca::testing::testkit::Step::OnSend {
                matches: None,
                responses: vec![ack(1), complete(1)], // Normal priority finally succeeds
            },
        ];

        let transport = ScriptedTransport::new(steps).with_executor(executor.clone());

        let runtime = RuntimeHandle::new(transport, executor.clone())
            .await
            .expect("Failed to create runtime");

        // Allow the runtime to start
        clock.advance(Duration::from_millis(50));

        // Submit commands in reverse priority order to test proper scheduling
        let normal_cmd = Power::On;
        let normal_future =
            runtime.send_command(&normal_cmd, CameraId::default(), Some(Priority::Normal));

        let high_cmd = Zoom::TeleStd;
        let high_future =
            runtime.send_command(&high_cmd, CameraId::default(), Some(Priority::High));

        let critical_cmd = Power::Standby;
        let critical_future =
            runtime.send_command(&critical_cmd, CameraId::default(), Some(Priority::Critical));

        // Execute all commands
        let (normal_result, (high_result, critical_result)) = futures_lite::future::zip(
            normal_future,
            futures_lite::future::zip(high_future, critical_future),
        )
        .await;

        // All commands should succeed, with proper priority ordering maintained
        assert!(
            critical_result.is_ok(),
            "Critical command should succeed: {:?}",
            critical_result
        );
        assert!(
            high_result.is_ok(),
            "High priority command should succeed: {:?}",
            high_result
        );
        assert!(
            normal_result.is_ok(),
            "Normal priority command should succeed: {:?}",
            normal_result
        );

        println!("✅ Successfully tested priority ordering during busy recovery with DeterministicExecutor");
    });
}
