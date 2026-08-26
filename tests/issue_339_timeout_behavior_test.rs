//! Tests for issue #339: Async runtime timeout behavior
//!
//! These tests verify that:
//! 1. Recv idle (timeout) does NOT trigger network error
//! 2. Genuine IO errors DO trigger network error
//! 3. Normal traffic flow is not affected by the fix
//!
//! Timeout behaviour is exercised on a real runtime only, per the decision in
//! issue #394. The `DeterministicExecutor` variants of these tests were removed
//! in issue #600: they never ran in CI and could not run, because virtual time
//! cannot be advanced from a plain `block_on`.

#![cfg(all(feature = "test-utils", feature = "mode-async"))]

#[cfg(feature = "runtime-tokio")]
mod timeout_behavior_tests {
    use std::sync::Arc;

    use grafton_visca::{
        camera::CameraBuilder,
        testing::testkit::{ScriptedTransport, Step},
        Error, PowerControl, TokioExecutor,
    };

    /// Test that Error::Timeout injected by transport does not cause issues
    /// This simulates what happens when the nested race generates Operation::RecvErr(Error::Timeout)
    ///
    /// The test verifies that:
    /// 1. Timeout errors from recv are treated as idle ticks (not transport failures)
    /// 2. Commands can still succeed after multiple timeout errors
    ///
    /// Note: InjectError steps are consumed on recv, not send. The OnSend step
    /// is placed first so it gets processed when the command is sent.
    #[tokio::test]
    async fn timeout_error_handled_as_idle() {
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));

        // Create a transport that:
        // 1. First responds to the power off command with ACK + Completion
        // 2. Then injects timeout errors (simulating idle recv after command completes)
        let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
            // OnSend is processed when the command is sent
            Step::OnSend {
                matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]),
                responses: vec![
                    vec![0x90, 0x41, 0xFF], // ACK
                    vec![0x90, 0x51, 0xFF], // Completion
                ],
            },
            // After the command succeeds, inject timeout errors to verify they don't break anything
            Step::InjectError(Error::Timeout),
            Step::InjectError(Error::Timeout),
            Step::InjectError(Error::Timeout),
        ])
        .with_executor(executor.clone());

        // Create camera
        let camera = CameraBuilder::<TokioExecutor>::with_executor(executor.clone())
            .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Verify command succeeds and subsequent timeouts don't break the runtime
        camera.power_off().await.expect("Power off should succeed");
    }

    /// Test that genuine IO errors still trigger proper error handling
    #[tokio::test]
    async fn genuine_io_error_causes_failure() {
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));

        // Create a transport that injects a real transport error
        // Power commands are Quick category which get base_retries + 2 = 3 + 2 = 5 retries
        // So we need 6 attempts total (1 initial + 5 retries) to exhaust retries
        let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
            // Initial send - no response to trigger timeout
            Step::OnSend {
                matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]),
                responses: vec![], // No responses queued
            },
            // Inject transport error on first recv (triggers retry 1)
            Step::InjectError(Error::TransportError("Network failure".into())),
            // Retry 1
            Step::OnSend {
                matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]),
                responses: vec![], // No responses queued
            },
            // Inject transport error on second recv (triggers retry 2)
            Step::InjectError(Error::TransportError("Network failure".into())),
            // Retry 2
            Step::OnSend {
                matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]),
                responses: vec![], // No responses queued
            },
            // Inject transport error on third recv (triggers retry 3)
            Step::InjectError(Error::TransportError("Network failure".into())),
            // Retry 3
            Step::OnSend {
                matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]),
                responses: vec![], // No responses queued
            },
            // Inject transport error on fourth recv (triggers retry 4)
            Step::InjectError(Error::TransportError("Network failure".into())),
            // Retry 4
            Step::OnSend {
                matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]),
                responses: vec![], // No responses queued
            },
            // Inject transport error on fifth recv (triggers retry 5)
            Step::InjectError(Error::TransportError("Network failure".into())),
            // Retry 5 (final)
            Step::OnSend {
                matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]),
                responses: vec![], // No responses queued
            },
            // Inject transport error on sixth recv (exhausts retries)
            Step::InjectError(Error::TransportError("Network failure".into())),
        ])
        .with_executor(executor.clone());

        // Create camera
        let camera = CameraBuilder::<TokioExecutor>::with_executor(executor.clone())
            .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
            .await
            .expect("Camera creation should succeed");

        // Try to send a command - should fail due to transport error
        let result = camera.power_off().await;
        assert!(
            result.is_err(),
            "Command should fail due to transport error"
        );

        match result {
            Err(Error::TransportError(_)) => {
                // Expected - transport error propagated correctly
            }
            Err(e) => panic!("Expected TransportError, got: {:?}", e),
            Ok(_) => panic!("Command should have failed"),
        }
    }

    /// Test normal command flow is unaffected by the fix
    ///
    /// Note: OnSend must come before InjectError steps because InjectError
    /// at the front of the queue blocks OnSend from being processed on send.
    #[tokio::test]
    async fn normal_traffic_flow_unaffected() {
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));

        // Create a transport with normal command/response flow
        let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
            // Power off command responds with ACK + Completion
            Step::OnSend {
                matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]),
                responses: vec![
                    vec![0x90, 0x41, 0xFF], // ACK
                    vec![0x90, 0x51, 0xFF], // Completion
                ],
            },
            // After command completes, inject a timeout to simulate idle period
            Step::InjectError(Error::Timeout),
        ])
        .with_executor(executor.clone());

        // Create camera
        let camera = CameraBuilder::<TokioExecutor>::with_executor(executor.clone())
            .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Power off should work
        camera.power_off().await.expect("Power off should succeed");
    }

    /// Test that multiple consecutive timeouts don't break the runtime
    ///
    /// Note: OnSend must come before InjectError steps. We put the command
    /// response first, then add timeout errors after to verify they don't
    /// corrupt the runtime state.
    #[tokio::test]
    async fn multiple_timeouts_handled_gracefully() {
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));

        // Create a transport that:
        // 1. First responds to the power off command
        // 2. Then injects many timeout errors (simulating idle recv)
        let mut steps = vec![
            // Command response first (OnSend is processed when send happens)
            Step::OnSend {
                matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]),
                responses: vec![
                    vec![0x90, 0x41, 0xFF], // ACK
                    vec![0x90, 0x51, 0xFF], // Completion
                ],
            },
        ];

        // Add many timeout errors after the command response
        for _ in 0..10 {
            steps.push(Step::InjectError(Error::Timeout));
        }

        let transport: ScriptedTransport<TokioExecutor> =
            ScriptedTransport::new(steps).with_executor(executor.clone());

        // Create camera
        let camera = CameraBuilder::<TokioExecutor>::with_executor(executor.clone())
            .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Command should succeed and subsequent timeouts should not break the runtime
        camera.power_off().await.expect("Power off should succeed");
    }

    /// Test that a second command still succeeds after an idle timeout tick.
    ///
    /// Ported from the deleted `runtime_resilient_to_timeout_errors` deterministic
    /// variant (issue #600): the guarantee is that a recv timeout between two
    /// commands is absorbed as an idle tick and leaves the runtime usable, not
    /// that virtual time can be single-stepped.
    #[tokio::test]
    async fn runtime_resilient_to_timeout_errors_between_commands() {
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));

        // Steps are consumed strictly front-to-back: each OnSend is popped when the
        // matching command is sent, and the InjectError behind it is then served to
        // the next recv as an idle tick.
        let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
            // Power off
            Step::OnSend {
                matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]),
                responses: vec![
                    vec![0x90, 0x41, 0xFF], // ACK
                    vec![0x90, 0x51, 0xFF], // Completion
                ],
            },
            Step::InjectError(Error::Timeout),
            // Power on
            Step::OnSend {
                matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]),
                responses: vec![
                    vec![0x90, 0x41, 0xFF], // ACK
                    vec![0x90, 0x51, 0xFF], // Completion
                ],
            },
            Step::InjectError(Error::Timeout),
        ])
        .with_executor(executor.clone());

        let camera = CameraBuilder::<TokioExecutor>::with_executor(executor.clone())
            .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        camera.power_off().await.expect("First command should work");
        camera.power_on().await.expect("Second command should work");
    }
}
