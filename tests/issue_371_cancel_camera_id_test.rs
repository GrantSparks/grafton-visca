//! Tests for issue #371: Using per-command CameraId for CANCEL commands.
//!
//! This test verifies that CANCEL commands use the correct camera ID from the command
//! being cancelled, not a hard-coded CAMERA_1.
//!
//! Cancellation is exercised on a real runtime only, per the decision in issue
//! #394. The `DeterministicExecutor` variants of these tests were removed in
//! issue #600: they never ran in CI and could not run, because virtual time
//! cannot be advanced from a plain `block_on`.

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
