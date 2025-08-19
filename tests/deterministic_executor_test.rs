//! Tests for the DeterministicExecutor integration with the runtime.

#![cfg(all(feature = "test-utils", feature = "async"))]

use std::time::Duration;

use grafton_visca::{
    runtime::{Priority, RuntimeHandle},
    testing::testkit::{
        deterministic_executor::{DeterministicExecutor, DeterministicExecutorExt},
        scripted_transport::{ScriptedTransport, Step},
    },
    Executor,
};

#[test]
fn test_deterministic_executor_with_simple_command() {
    let (executor, _clock) = DeterministicExecutor::new();

    let steps = vec![Step::OnSend {
        matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]), // Power On command
        responses: vec![
            vec![0x90, 0x41, 0xFF], // ACK on socket 1
            vec![0x90, 0x51, 0xFF], // Completion on socket 1
        ],
    }];

    let transport = ScriptedTransport::new(steps).with_executor(executor.clone());
    let executor2 = executor.clone();

    let result = executor.block_on_bg(async move {
        eprintln!("Starting test - creating runtime");

        let runtime = RuntimeHandle::new(transport, executor2).await?;

        eprintln!("Runtime created, sending command");

        let response = runtime
            .send_command(
                &grafton_visca::command::power::Power::On,
                grafton_visca::camera_id::CameraId::default(),
                Some(Priority::Normal),
            )
            .await?;

        eprintln!("Command sent, got response: {:?}", response);

        use grafton_visca::command::response::ViscaResponse;

        assert!(matches!(
            response,
            ViscaResponse::CmdAck | ViscaResponse::Completion
        ));

        Ok::<(), grafton_visca::Error>(())
    });

    assert!(result.is_ok(), "Command should succeed: {:?}", result.err());
}

#[test]
fn test_deterministic_executor_with_sleep() {
    let (executor, _clock) = DeterministicExecutor::new();

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

        let response = runtime
            .send_command(
                &grafton_visca::command::power::Power::On,
                grafton_visca::camera_id::CameraId::default(),
                Some(Priority::Normal),
            )
            .await?;

        use grafton_visca::command::response::ViscaResponse;

        assert!(matches!(
            response,
            ViscaResponse::CmdAck | ViscaResponse::Completion
        ));

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

    let steps = vec![
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]),
            responses: vec![vec![0x90, 0x60, 0x03, 0xFF]], // Buffer Full error (0x03 = Command Buffer Full - the actual BUSY)
        },
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]), // First retry
            responses: vec![vec![0x90, 0x60, 0x03, 0xFF]],           // Buffer Full again
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
        let response = runtime
            .send_command(
                &grafton_visca::command::power::Power::On,
                grafton_visca::camera_id::CameraId::default(),
                Some(Priority::Normal),
            )
            .await;

        eprintln!("Got response: {:?}", response);

        match response {
            Ok(resp) => {
                use grafton_visca::command::response::ViscaResponse;

                assert!(matches!(
                    resp,
                    ViscaResponse::CmdAck | ViscaResponse::Completion
                ));
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

    let steps = vec![
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]),
            responses: vec![vec![0x90, 0x60, 0x03, 0xFF]], // Buffer Full 1 - initial attempt
        },
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]),
            responses: vec![vec![0x90, 0x60, 0x03, 0xFF]], // Buffer Full 2 - retry 1
        },
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]),
            responses: vec![vec![0x90, 0x60, 0x03, 0xFF]], // Buffer Full 3 - retry 2
        },
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]),
            responses: vec![vec![0x90, 0x60, 0x03, 0xFF]], // Buffer Full 4 - retry 3
        },
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]),
            responses: vec![vec![0x90, 0x60, 0x03, 0xFF]], // Buffer Full 5 - retry 4
        },
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]),
            responses: vec![vec![0x90, 0x60, 0x03, 0xFF]], // Buffer Full 6 - retry 5, exhaustion
        },
    ];

    let transport = ScriptedTransport::new(steps).with_executor(executor.clone());
    let executor2 = executor.clone();

    let result = executor.block_on_bg(async move {
        let runtime = RuntimeHandle::new(transport, executor2).await?;

        let response = runtime
            .send_command(
                &grafton_visca::command::power::Power::On,
                grafton_visca::camera_id::CameraId::default(),
                Some(Priority::Normal),
            )
            .await;

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

    executor.run_until_idle();
    assert!(
        !flag.load(Ordering::SeqCst),
        "Sleep should not complete without time advancement"
    );

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
