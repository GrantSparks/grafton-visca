//! Tests for issue #339: Async runtime timeout behavior
//!
//! These tests verify that:
//! 1. Recv idle (timeout) does NOT trigger network error
//! 2. Genuine IO errors DO trigger network error
//! 3. Normal traffic flow is not affected by the fix

#![cfg(all(feature = "test-utils", feature = "async"))]

#[cfg(feature = "rt-tokio")]
mod timeout_behavior_tests {
    use grafton_visca::{
        camera::CameraBuilder,
        testing::testkit::{ScriptedTransport, Step},
        Error, Executor, PowerControl, TokioExecutor,
    };
    use std::{sync::Arc, time::Duration};

    /// Test that Error::Timeout injected by transport does not cause issues
    /// This simulates what happens when the nested race generates Operation::RecvErr(Error::Timeout)
    #[tokio::test]
    async fn timeout_error_handled_as_idle() {
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));

        // Create a transport that injects timeout errors after initial handshake
        let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
            // Power inquiry gets a response
            Step::OnSend {
                matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]),
                responses: vec![vec![0x90, 0x50, 0x02, 0xFF]], // Power on
            },
            // Then inject multiple timeout errors
            Step::InjectError(Error::Timeout),
            Step::InjectError(Error::Timeout),
            Step::InjectError(Error::Timeout),
            // Eventually allow a power off command to succeed
            Step::OnSend {
                matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]),
                responses: vec![
                    vec![0x90, 0x41, 0xFF], // ACK
                    vec![0x90, 0x51, 0xFF], // Completion
                ],
            },
        ])
        .with_executor(executor.clone());

        // Create camera - this should succeed despite timeout errors
        let camera = CameraBuilder::<TokioExecutor>::with_executor(executor.clone())
            .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera despite timeout errors");

        // Verify camera still works after timeouts
        camera
            .power_off()
            .await
            .expect("Power off should succeed after timeouts");
    }

    /// Test that genuine IO errors still trigger proper error handling
    #[tokio::test]
    async fn genuine_io_error_causes_failure() {
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));

        // Create a transport that injects a real transport error
        let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
            // Power inquiry gets a response first
            Step::OnSend {
                matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]),
                responses: vec![vec![0x90, 0x50, 0x02, 0xFF]], // Power on
            },
            // Then inject a transport error on next command
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
    #[tokio::test]
    async fn normal_traffic_flow_unaffected() {
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));

        // Create a transport with normal command/response flow, including some idle periods
        let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
            // Power inquiry
            Step::OnSend {
                matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]),
                responses: vec![vec![0x90, 0x50, 0x02, 0xFF]], // Power on
            },
            // Inject a timeout to simulate idle period
            Step::InjectError(Error::Timeout),
            // Power off command should still work after timeout
            Step::OnSend {
                matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]),
                responses: vec![
                    vec![0x90, 0x41, 0xFF], // ACK
                    vec![0x90, 0x51, 0xFF], // Completion
                ],
            },
        ])
        .with_executor(executor.clone());

        // Create camera
        let camera = CameraBuilder::<TokioExecutor>::with_executor(executor.clone())
            .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Test power operations work normally even with timeouts interspersed
        let power_status = camera.power().state().await.expect("Power inquiry failed");
        assert!(power_status, "Expected power to be on");

        // Wait a bit to let the timeout occur
        executor.sleep(Duration::from_millis(50)).await;

        // Power off should still work after idle timeout
        camera
            .power_off()
            .await
            .expect("Power off should succeed after idle period");
    }

    /// Test that multiple consecutive timeouts don't break the runtime
    #[tokio::test]
    async fn multiple_timeouts_handled_gracefully() {
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));

        // Create a transport with many timeout errors
        let mut steps = vec![
            // Initial power inquiry succeeds
            Step::OnSend {
                matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]),
                responses: vec![vec![0x90, 0x50, 0x02, 0xFF]], // Power on
            },
        ];

        // Add many timeout errors
        for _ in 0..10 {
            steps.push(Step::InjectError(Error::Timeout));
        }

        // Finally allow a command to succeed
        steps.push(Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]),
            responses: vec![
                vec![0x90, 0x41, 0xFF], // ACK
                vec![0x90, 0x51, 0xFF], // Completion
            ],
        });

        let transport: ScriptedTransport<TokioExecutor> =
            ScriptedTransport::new(steps).with_executor(executor.clone());

        // Create camera - should tolerate all the timeouts
        let camera = CameraBuilder::<TokioExecutor>::with_executor(executor.clone())
            .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Command should eventually succeed despite many timeouts
        camera
            .power_off()
            .await
            .expect("Power off should succeed after many timeouts");
    }
}

#[cfg(feature = "test-utils")]
mod deterministic_tests {
    use grafton_visca::testing::testkit::deterministic_executor::DeterministicExecutor;
    use grafton_visca::{
        camera::CameraBuilder,
        testing::testkit::{ScriptedTransport, Step},
        Error, Executor, PowerControl,
    };
    use std::time::Duration;

    /// Test with deterministic executor for precise timeout testing
    #[test]
    fn deterministic_timeout_handling() {
        let (executor, _clock) = DeterministicExecutor::new();

        executor.block_on(async {
            // Create a transport that times out after initial response
            let transport = ScriptedTransport::new(vec![
                // Power inquiry gets response
                Step::OnSend {
                    matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]),
                    responses: vec![vec![0x90, 0x50, 0x02, 0xFF]],
                },
                // Multiple timeouts
                Step::InjectError(Error::Timeout),
                Step::InjectError(Error::Timeout),
                // Then allow power off
                Step::OnSend {
                    matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]),
                    responses: vec![
                        vec![0x90, 0x41, 0xFF], // ACK
                        vec![0x90, 0x51, 0xFF], // Completion
                    ],
                },
            ])
            .with_executor(executor.clone());

            // Create camera
            let camera = CameraBuilder::<DeterministicExecutor>::with_executor(executor.clone())
                .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
                .await
                .expect("Failed to create camera");

            // Power off should work despite timeouts
            camera.power_off().await.expect("Power off should succeed");
        });
    }

    /// Test that the runtime continues to function after errors
    #[test]
    fn runtime_resilient_to_timeout_errors() {
        let (executor, _clock) = DeterministicExecutor::new();

        executor.block_on(async {
            // Mix of timeouts and real responses
            let transport = ScriptedTransport::new(vec![
                // Power inquiry
                Step::OnSend {
                    matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]),
                    responses: vec![vec![0x90, 0x50, 0x02, 0xFF]],
                },
                Step::InjectError(Error::Timeout),
                // Power off
                Step::OnSend {
                    matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]),
                    responses: vec![vec![0x90, 0x41, 0xFF], vec![0x90, 0x51, 0xFF]],
                },
                Step::InjectError(Error::Timeout),
                // Power on
                Step::OnSend {
                    matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]),
                    responses: vec![vec![0x90, 0x41, 0xFF], vec![0x90, 0x51, 0xFF]],
                },
            ])
            .with_executor(executor.clone());

            let camera = CameraBuilder::<DeterministicExecutor>::with_executor(executor.clone())
                .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
                .await
                .expect("Failed to create camera");

            // Multiple operations should work with timeouts interspersed
            camera.power_off().await.expect("First command should work");

            // Advance time to let timeout occur
            executor.sleep(Duration::from_millis(10)).await;

            camera.power_on().await.expect("Second command should work");
        });
    }
}
