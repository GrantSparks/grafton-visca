//! Tests for the DeterministicExecutor integration with the runtime.

#![cfg(all(feature = "test-utils", feature = "async"))]

use grafton_visca::{
    runtime::{Priority, RuntimeHandle},
    testing::testkit::{
        deterministic_executor::{DeterministicExecutor, DeterministicExecutorExt},
        scripted_transport::{ScriptedTransport, Step},
    },
    Executor,
};
use std::time::Duration;

#[test]
fn test_deterministic_executor_with_simple_command() {
    let (executor, _clock) = DeterministicExecutor::new();

    // Create a scripted transport with a simple ACK/Completion response
    let steps = vec![Step::OnSend {
        matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]), // Power On command
        responses: vec![
            vec![0x90, 0x41, 0xFF], // ACK on socket 1
            vec![0x90, 0x51, 0xFF], // Completion on socket 1
        ],
    }];

    let transport = ScriptedTransport::new(steps).with_executor(executor.clone());
    let executor2 = executor.clone();

    // Use block_on_bg to drive the runtime
    let result = executor.block_on_bg(async move {
        eprintln!("Starting test - creating runtime");

        // Create runtime with the scripted transport
        let runtime = RuntimeHandle::new(transport, executor2).await?;

        eprintln!("Runtime created, sending command");

        // Send a simple power on command
        let response = runtime
            .send_command(
                &grafton_visca::command::power::PowerCommand::On,
                grafton_visca::camera_id::CameraId::default(),
                Some(Priority::Normal),
            )
            .await?;

        eprintln!("Command sent, got response: {:?}", response);

        // Check the response
        use grafton_visca::command::response::Response;
        assert!(matches!(response, Response::CmdAck | Response::Completion));

        Ok::<(), grafton_visca::Error>(())
    });

    assert!(result.is_ok(), "Command should succeed: {:?}", result.err());
}

#[test]
fn test_deterministic_executor_with_sleep() {
    let (executor, _clock) = DeterministicExecutor::new();

    // Create a scripted transport with delayed response
    let steps = vec![
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]),
            responses: vec![vec![0x90, 0x41, 0xFF]], // ACK immediately
        },
        Step::After {
            delay: Duration::from_millis(100),
            responses: vec![vec![0x90, 0x51, 0xFF]], // Completion after delay
        },
    ];

    let transport = ScriptedTransport::new(steps).with_executor(executor.clone());
    let executor2 = executor.clone();

    let result = executor.block_on_bg(async move {
        let runtime = RuntimeHandle::new(transport, executor2).await?;

        // Start the command (it will be waiting for the delayed response)
        let response = runtime
            .send_command(
                &grafton_visca::command::power::PowerCommand::On,
                grafton_visca::camera_id::CameraId::default(),
                Some(Priority::Normal),
            )
            .await?;

        // The virtual time should advance automatically in block_on_bg
        use grafton_visca::command::response::Response;
        assert!(matches!(response, Response::CmdAck | Response::Completion));

        Ok::<(), grafton_visca::Error>(())
    });

    assert!(
        result.is_ok(),
        "Command with delay should succeed: {:?}",
        result.err()
    );
}

#[test]
fn test_deterministic_executor_handles_busy_retry() {
    let (executor, _clock) = DeterministicExecutor::new();

    // Create a scripted transport that returns BUSY twice then success
    // PowerCommand uses Quick category which has 5 max retries, so 2 BUSYs followed by success should work
    let steps = vec![
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]),
            responses: vec![vec![0x90, 0x61, 0x41, 0xFF]], // BUSY error on socket 1 (0x41 = Camera Busy)
        },
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]), // First retry
            responses: vec![vec![0x90, 0x61, 0x41, 0xFF]],           // BUSY again on socket 1
        },
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]), // Second retry
            responses: vec![
                vec![0x90, 0x41, 0xFF], // ACK on second retry
                vec![0x90, 0x51, 0xFF], // Completion on second retry
            ],
        },
    ];

    let transport = ScriptedTransport::new(steps).with_executor(executor.clone());
    let executor2 = executor.clone();

    let result = executor.block_on_bg(async move {
        eprintln!("Creating runtime...");
        let runtime = RuntimeHandle::new(transport, executor2).await?;

        eprintln!("Runtime created, sending command...");
        // Send command that will get BUSY twice then succeed on retry
        let response = runtime
            .send_command(
                &grafton_visca::command::power::PowerCommand::On,
                grafton_visca::camera_id::CameraId::default(),
                Some(Priority::Normal),
            )
            .await;

        eprintln!("Got response: {:?}", response);

        // Should succeed after retry
        match response {
            Ok(resp) => {
                use grafton_visca::command::response::Response;
                assert!(matches!(resp, Response::CmdAck | Response::Completion));
                Ok::<(), grafton_visca::Error>(())
            }
            Err(e) => Err(e),
        }
    });

    assert!(
        result.is_ok(),
        "BUSY retry should succeed: {:?}",
        result.err()
    );
}

#[test]
fn test_deterministic_executor_handles_busy_exhaustion() {
    let (executor, _clock) = DeterministicExecutor::new();

    // Create a scripted transport that returns BUSY 6 times (exhaustion for max_retries=5)
    // PowerCommand uses Quick category which has 5 max retries
    // The runtime sends the initial command (attempt 1), then up to 5 retries (attempts 2-6)
    // On the 6th BUSY (attempt 6), it should exhaust and return MaxRetriesExceeded
    let steps = vec![
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]),
            responses: vec![vec![0x90, 0x61, 0x41, 0xFF]], // BUSY 1 on socket 1 - initial attempt
        },
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]),
            responses: vec![vec![0x90, 0x61, 0x41, 0xFF]], // BUSY 2 on socket 1 - retry 1
        },
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]),
            responses: vec![vec![0x90, 0x61, 0x41, 0xFF]], // BUSY 3 on socket 1 - retry 2
        },
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]),
            responses: vec![vec![0x90, 0x61, 0x41, 0xFF]], // BUSY 4 on socket 1 - retry 3
        },
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]),
            responses: vec![vec![0x90, 0x61, 0x41, 0xFF]], // BUSY 5 on socket 1 - retry 4
        },
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]),
            responses: vec![vec![0x90, 0x61, 0x41, 0xFF]], // BUSY 6 on socket 1 - retry 5, exhaustion
        },
    ];

    let transport = ScriptedTransport::new(steps).with_executor(executor.clone());
    let executor2 = executor.clone();

    let result = executor.block_on_bg(async move {
        let runtime = RuntimeHandle::new(transport, executor2).await?;

        // Send command that will get BUSY 6 times and exhaust retries
        let response = runtime
            .send_command(
                &grafton_visca::command::power::PowerCommand::On,
                grafton_visca::camera_id::CameraId::default(),
                Some(Priority::Normal),
            )
            .await;

        // Should fail with MaxRetriesExceeded
        match response {
            Err(grafton_visca::Error::MaxRetriesExceeded) => Ok::<(), grafton_visca::Error>(()),
            other => panic!("Expected MaxRetriesExceeded, got: {:?}", other),
        }
    });

    assert!(
        result.is_ok(),
        "BUSY exhaustion test should complete: {:?}",
        result.err()
    );
}

// Smokescreen tests as recommended in the issue comment

#[test]
fn test_det_drives_spawned_tasks_smokescreen() {
    use grafton_visca::testing::testkit::deterministic_executor::ExecutorExt;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    let (executor, _clock) = DeterministicExecutor::new();
    let flag = Arc::new(AtomicBool::new(false));
    let flag2 = flag.clone();

    executor.spawn_bg(async move {
        flag2.store(true, Ordering::SeqCst);
    });

    // Drive executor
    assert!(executor.run_until_idle(), "Should have made progress");
    assert!(flag.load(Ordering::SeqCst), "Task should have run");
}

#[test]
fn test_det_sleep_fires_only_when_time_advances_smokescreen() {
    use grafton_visca::testing::testkit::deterministic_executor::ExecutorExt;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    let (executor, _clock) = DeterministicExecutor::new();
    let flag = Arc::new(AtomicBool::new(false));
    let flag2 = flag.clone();
    let executor2 = executor.clone();

    executor.spawn_bg(async move {
        executor2.sleep(Duration::from_millis(50)).await;
        flag2.store(true, Ordering::SeqCst);
    });

    // Nothing should happen yet
    executor.run_until_idle();
    assert!(
        !flag.load(Ordering::SeqCst),
        "Sleep should not complete without time advancement"
    );

    // Jump clock to next deadline and drive
    assert!(
        executor.advance_to_next_deadline(),
        "Should have advanced to deadline"
    );
    executor.run_until_idle();
    assert!(
        flag.load(Ordering::SeqCst),
        "Sleep should complete after time advancement"
    );
}
