//! Tests for issue #371: Using per-command CameraId for CANCEL commands.
//!
//! This test verifies that CANCEL commands use the correct camera ID from the command
//! being cancelled, not a hard-coded CAMERA_1.

#![cfg(all(feature = "mode-async", feature = "test-utils"))]

#[cfg(feature = "runtime-tokio")]
mod tokio_regression {
    use std::sync::Arc;
    use std::time::Duration;

    use grafton_visca::{
        camera::profiles::GenericVisca,
        command::Zoom,
        runtime::testing::RuntimeHandle,
        testing::testkit::{helpers, ScriptedTransport, Step},
        transport::{AddressingMode, TransportConfig},
        CameraId, Error, TokioExecutor, ViscaSocket,
    };

    fn serial_config() -> TransportConfig {
        TransportConfig {
            addressing: AddressingMode::Serial,
            ..TransportConfig::default()
        }
    }

    async fn wait_for_sent_len(
        transport: &ScriptedTransport<TokioExecutor>,
        expected_len: usize,
        context: &str,
    ) {
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if transport.sent().len() >= expected_len {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("{context}: expected at least {expected_len} sent frames"));
    }

    fn assert_command_canceled<T: std::fmt::Debug>(result: Result<T, Error>, context: &str) {
        assert!(
            matches!(result, Err(Error::CommandCanceled)),
            "{context} should resolve as CommandCanceled, got {result:?}"
        );
    }

    /// Test that cancel commands use the camera ID from the command being cancelled.
    #[tokio::test]
    async fn test_cancel_uses_correct_camera_id() {
        let executor = Arc::new(TokioExecutor::from_current().unwrap());

        let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
            Step::OnSend {
                matches: Some(vec![0x84, 0x01, 0x04, 0x07, 0x02, 0xFF]),
                responses: vec![helpers::ack(1)],
            },
            Step::OnSend {
                matches: Some(vec![0x84, 0x21, 0xFF]),
                responses: vec![vec![0x90, 0x61, 0x04, 0xFF]],
            },
        ])
        .with_executor(executor.clone())
        .with_config(serial_config());
        let sent_transport = transport.clone();

        let runtime: RuntimeHandle<GenericVisca, TokioExecutor> =
            RuntimeHandle::new(transport, executor)
                .await
                .expect("runtime should start");

        let camera_id = CameraId::new(4).unwrap();
        let (cmd_id, response) = runtime
            .send_command_with_id(&Zoom::TeleStd, camera_id, None)
            .await
            .expect("command should be admitted");

        runtime
            .cancel(camera_id, cmd_id)
            .await
            .expect("cancel should be processed");

        let result = tokio::time::timeout(Duration::from_secs(1), response)
            .await
            .expect("command response should not hang");
        assert_command_canceled(result, "camera 4 command");

        assert!(
            sent_transport
                .sent()
                .iter()
                .any(|frame| frame.starts_with(&[0x84, 0x21])),
            "cancel frame should use camera ID 4"
        );

        runtime.shutdown().await.expect("runtime shutdown");
    }

    /// Test multiple camera IDs on the same runtime to ensure each cancel uses
    /// scheduler-owned command state rather than a hard-coded camera address.
    #[tokio::test]
    async fn test_multiple_camera_ids_cancel_with_own_ids() {
        let executor = Arc::new(TokioExecutor::from_current().unwrap());

        let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
            Step::OnSend {
                matches: Some(vec![0x82, 0x01, 0x04, 0x07, 0x02, 0xFF]),
                responses: vec![helpers::ack(1)],
            },
            Step::OnSend {
                matches: Some(vec![0x82, 0x21, 0xFF]),
                responses: vec![vec![0x90, 0x61, 0x04, 0xFF]],
            },
            Step::OnSend {
                matches: Some(vec![0x87, 0x01, 0x04, 0x07, 0x03, 0xFF]),
                responses: vec![helpers::ack(1)],
            },
            Step::OnSend {
                matches: Some(vec![0x87, 0x21, 0xFF]),
                responses: vec![vec![0x90, 0x61, 0x04, 0xFF]],
            },
        ])
        .with_executor(executor.clone())
        .with_config(serial_config());
        let sent_transport = transport.clone();

        let runtime: RuntimeHandle<GenericVisca, TokioExecutor> =
            RuntimeHandle::new(transport, executor)
                .await
                .expect("runtime should start");

        let camera_2 = CameraId::new(2).unwrap();
        let (cmd_id_2, response_2) = runtime
            .send_command_with_id(&Zoom::TeleStd, camera_2, None)
            .await
            .expect("camera 2 command should be admitted");
        runtime
            .cancel(camera_2, cmd_id_2)
            .await
            .expect("camera 2 cancel should be processed");
        let result_2 = tokio::time::timeout(Duration::from_secs(1), response_2)
            .await
            .expect("camera 2 response should not hang");
        assert_command_canceled(result_2, "camera 2 command");

        let camera_7 = CameraId::new(7).unwrap();
        let (cmd_id_7, response_7) = runtime
            .send_command_with_id(&Zoom::WideStd, camera_7, None)
            .await
            .expect("camera 7 command should be admitted");
        wait_for_sent_len(
            &sent_transport,
            3,
            "camera 7 command should reach the transport before cancellation",
        )
        .await;
        runtime
            .cancel(camera_7, cmd_id_7)
            .await
            .expect("camera 7 cancel should be processed");
        let result_7 = tokio::time::timeout(Duration::from_secs(1), response_7)
            .await
            .expect("camera 7 response should not hang");
        assert_command_canceled(result_7, "camera 7 command");

        let sent = sent_transport.sent();
        assert!(
            sent.iter().any(|frame| frame.starts_with(&[0x82, 0x21])),
            "camera 2 cancel frame should use camera ID 2: {sent:?}"
        );
        assert!(
            sent.iter().any(|frame| frame.starts_with(&[0x87, 0x21])),
            "camera 7 cancel frame should use camera ID 7: {sent:?}"
        );

        runtime.shutdown().await.expect("runtime shutdown");
    }

    /// Test direct socket cancellation uses the caller-provided camera ID.
    #[tokio::test]
    async fn test_cancel_socket_uses_requested_camera_id() {
        let executor = Arc::new(TokioExecutor::from_current().unwrap());

        let transport: ScriptedTransport<TokioExecutor> =
            ScriptedTransport::new(vec![Step::OnSend {
                matches: Some(vec![0x85, 0x21, 0xFF]),
                responses: vec![vec![0x90, 0x61, 0x04, 0xFF]],
            }])
            .with_executor(executor.clone())
            .with_config(serial_config());
        let sent_transport = transport.clone();

        let runtime: RuntimeHandle<GenericVisca, TokioExecutor> =
            RuntimeHandle::new(transport, executor)
                .await
                .expect("runtime should start");

        let camera_5 = CameraId::new(5).unwrap();
        runtime
            .cancel_socket(camera_5, ViscaSocket::S1)
            .await
            .expect("socket cancel should be processed");

        let sent = sent_transport.sent();
        assert!(
            sent.iter().any(|frame| frame.starts_with(&[0x85, 0x21])),
            "socket cancel frame should use camera ID 5: {sent:?}"
        );

        runtime.shutdown().await.expect("runtime shutdown");
    }
}

#[cfg(not(any(feature = "runtime-tokio", feature = "runtime-smol")))]
mod deterministic_regression {
    use grafton_visca::{
        camera::{profiles::GenericVisca, CameraBuilder},
        command::Zoom,
        testing::testkit::{
            scripted_transport::{ScriptedTransport, Step},
            DeterministicExecutor,
        },
        CameraId, Error, Executor, ViscaSocket,
    };

    /// Test that cancel commands use the correct camera ID for the command being cancelled
    #[test]
    fn test_cancel_uses_correct_camera_id() {
        // Create executor and transport
        let (executor, _clock) = DeterministicExecutor::new();

        // Camera ID 4 test - verify that cancel uses 0x84, not 0x81
        let steps = vec![
            // Zoom command with camera ID 4
            Step::OnSend {
                matches: Some(vec![0x84, 0x01, 0x04, 0x07, 0x02, 0xFF]), // Zoom Tele with camera 4
                responses: vec![
                    vec![0x90, 0x41, 0xFF], // ACK on socket 1
                ],
            },
            // Cancel should use camera ID 4 (0x84), not CAMERA_1 (0x81)
            Step::OnSend {
                matches: Some(vec![0x84, 0x21, 0xFF]), // Cancel socket 1 with camera 4
                responses: vec![
                    vec![0x90, 0x61, 0x04, 0xFF], // Command cancelled
                ],
            },
        ];

        let transport = ScriptedTransport::new(steps).with_executor(executor.clone());
        let executor_clone = executor.clone();

        executor.clone().block_on(async move {
            // Build camera with custom camera ID 4
            let camera =
                CameraBuilder::<DeterministicExecutor>::with_executor(executor_clone.clone())
                    .camera_id(CameraId::new(4).unwrap())
                    .open_async::<GenericVisca, _>(transport)
                    .await
                    .expect("Failed to create camera");

            // Start a zoom command
            let (cmd_id, future) = camera
                .start_command_with_id(&Zoom::TeleStd)
                .await
                .expect("Failed to send command");

            // Cancel the command - this should use camera ID 4
            camera.cancel(cmd_id).await.expect("Failed to cancel");

            // The command should be cancelled
            let result = future.await;
            assert!(
                matches!(result, Err(Error::CommandCanceled)),
                "Command should be canceled, got: {:?}",
                result
            );
        });
    }

    /// Test multiple cameras with different IDs to ensure each uses its own ID for cancel
    #[test]
    fn test_multiple_cameras_cancel_with_own_ids() {
        // Create executor and transport
        let (executor, _clock) = DeterministicExecutor::new();

        let steps = vec![
            // First camera (ID 2) zoom command
            Step::OnSend {
                matches: Some(vec![0x82, 0x01, 0x04, 0x07, 0x02, 0xFF]), // Zoom Tele with camera 2
                responses: vec![
                    vec![0x90, 0x41, 0xFF], // ACK on socket 1
                ],
            },
            // Cancel for camera 2 should use 0x82
            Step::OnSend {
                matches: Some(vec![0x82, 0x21, 0xFF]), // Cancel socket 1 with camera 2
                responses: vec![
                    vec![0x90, 0x61, 0x04, 0xFF], // Command cancelled
                ],
            },
            // Second camera (ID 7) zoom command
            Step::OnSend {
                matches: Some(vec![0x87, 0x01, 0x04, 0x07, 0x03, 0xFF]), // Zoom Wide with camera 7
                responses: vec![
                    vec![0x90, 0x42, 0xFF], // ACK on socket 2
                ],
            },
            // Cancel for camera 7 should use 0x87
            Step::OnSend {
                matches: Some(vec![0x87, 0x22, 0xFF]), // Cancel socket 2 with camera 7
                responses: vec![
                    vec![0x90, 0x62, 0x04, 0xFF], // Command cancelled
                ],
            },
        ];

        let transport = ScriptedTransport::new(steps).with_executor(executor.clone());
        let executor_clone = executor.clone();

        executor.clone().block_on(async move {
            // Create first camera with ID 2
            let camera1 =
                CameraBuilder::<DeterministicExecutor>::with_executor(executor_clone.clone())
                    .camera_id(CameraId::new(2).unwrap())
                    .open_async::<GenericVisca, _>(transport.clone())
                    .await
                    .expect("Failed to create camera 1");

            // Create second camera with ID 7
            let camera2 =
                CameraBuilder::<DeterministicExecutor>::with_executor(executor_clone.clone())
                    .camera_id(CameraId::new(7).unwrap())
                    .open_async::<GenericVisca, _>(transport)
                    .await
                    .expect("Failed to create camera 2");

            // Start commands on both cameras
            let (cmd_id1, future1) = camera1
                .start_command_with_id(&Zoom::TeleStd)
                .await
                .expect("Failed to send command on camera 1");

            let (cmd_id2, future2) = camera2
                .start_command_with_id(&Zoom::WideStd)
                .await
                .expect("Failed to send command on camera 2");

            // Cancel both commands - each should use its own camera ID
            camera1
                .cancel(cmd_id1)
                .await
                .expect("Failed to cancel camera 1");
            camera2
                .cancel(cmd_id2)
                .await
                .expect("Failed to cancel camera 2");

            // Both commands should be cancelled
            let result1 = future1.await;
            let result2 = future2.await;

            assert!(
                matches!(result1, Err(Error::CommandCanceled)),
                "Camera 1 command should be canceled, got: {:?}",
                result1
            );
            assert!(
                matches!(result2, Err(Error::CommandCanceled)),
                "Camera 2 command should be canceled, got: {:?}",
                result2
            );
        });
    }

    /// Test cancel by socket uses correct camera ID when command is on socket
    ///
    /// Note: cancel_socket is only meaningful for sockets that have active command context.
    /// Use cancel() with the command ID when canceling a specific operation.
    #[test]
    fn test_cancel_socket_uses_correct_camera_id() {
        // Create executor and transport
        let (executor, _clock) = DeterministicExecutor::new();

        let steps = vec![
            // Zoom command with camera ID 5
            Step::OnSend {
                matches: Some(vec![0x85, 0x01, 0x04, 0x07, 0x02, 0xFF]), // Zoom with camera 5
                responses: vec![
                    vec![0x90, 0x41, 0xFF], // ACK on socket 1
                ],
            },
            // Process the ACK to assign command to socket, then cancel
            // The cancel will use camera ID 5 from the command on the socket
            Step::OnSend {
                matches: Some(vec![0x85, 0x21, 0xFF]), // Cancel socket 1 with camera 5
                responses: vec![
                    vec![0x90, 0x61, 0x04, 0xFF], // Command cancelled
                ],
            },
        ];

        let transport = ScriptedTransport::new(steps).with_executor(executor.clone());
        let executor_clone = executor.clone();

        executor.clone().block_on(async move {
            // Build camera with camera ID 5
            let camera =
                CameraBuilder::<DeterministicExecutor>::with_executor(executor_clone.clone())
                    .camera_id(CameraId::new(5).unwrap())
                    .open_async::<GenericVisca, _>(transport)
                    .await
                    .expect("Failed to create camera");

            // Start a command
            let (_cmd_id, future) = camera
                .start_command_with_id(&Zoom::TeleStd)
                .await
                .expect("Failed to send command");

            // Wait briefly for the ACK to be processed and command assigned to socket
            // This simulates real-world timing where you wouldn't immediately cancel
            executor_clone
                .sleep(std::time::Duration::from_millis(1))
                .await;

            // Cancel by socket - should use camera ID 5 from the command on the socket
            camera
                .cancel_socket(ViscaSocket::S1)
                .await
                .expect("Failed to cancel socket");

            // Command should be cancelled
            let result = future.await;
            assert!(
                matches!(result, Err(Error::CommandCanceled)),
                "Command should be canceled, got: {:?}",
                result
            );
        });
    }
}
