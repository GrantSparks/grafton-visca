//! Integration tests verifying parity across async runtime implementations.
//!
//! These tests ensure that Tokio and smol runtime adapters provide
//! identical behavior for all transport operations.

#![cfg(feature = "mode-async")]

#[cfg(feature = "test-utils")]
mod parity_tests {
    use std::sync::Arc;

    use grafton_visca::{
        camera::CameraBuilder,
        testing::testkit::{ScriptedTransport, Step},
    };

    // Common test scenario: Power on, zoom in, recall preset
    fn create_test_script() -> Vec<Step> {
        vec![
            // Power inquiry
            Step::OnSend {
                matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]),
                responses: vec![vec![0x90, 0x50, 0x02, 0xFF]], // Power on
            },
            // Power off command
            Step::OnSend {
                matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]),
                responses: vec![
                    vec![0x90, 0x41, 0xFF], // ACK
                    vec![0x90, 0x51, 0xFF], // Completion
                ],
            },
            // Zoom tele command
            Step::OnSend {
                matches: Some(vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]),
                responses: vec![
                    vec![0x90, 0x41, 0xFF], // ACK
                    vec![0x90, 0x51, 0xFF], // Completion
                ],
            },
            // Preset recall
            Step::OnSend {
                matches: Some(vec![0x81, 0x01, 0x04, 0x3F, 0x02, 0x01, 0xFF]),
                responses: vec![
                    vec![0x90, 0x41, 0xFF], // ACK
                    vec![0x90, 0x51, 0xFF], // Completion
                ],
            },
        ]
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn test_tokio_runtime_operations() {
        use grafton_visca::TokioExecutor;
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
        let transport: ScriptedTransport<TokioExecutor> =
            ScriptedTransport::new(create_test_script()).with_executor(executor.clone());

        let camera = CameraBuilder::<TokioExecutor>::with_executor(executor)
            .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Test power operations
        let power_status = camera.power().state().await.expect("Power inquiry failed");
        assert!(power_status, "Expected power to be on");

        camera.power().off().await.expect("Power off failed");

        // Test zoom operations
        camera.zoom().tele().await.expect("Zoom tele failed");

        // Test preset operations
        camera
            .presets()
            .recall(1)
            .await
            .expect("Preset recall failed");
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn test_smol_runtime_operations() {
        use grafton_visca::SmolExecutor;

        smol::block_on(async {
            let executor = Arc::new(SmolExecutor::new());
            let transport: ScriptedTransport<SmolExecutor> =
                ScriptedTransport::new(create_test_script()).with_executor(executor.clone());

            let camera = CameraBuilder::<SmolExecutor>::with_executor(executor)
                .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
                .await
                .expect("Failed to create camera");

            // Test power operations
            let power_status = camera.power().state().await.expect("Power inquiry failed");
            assert!(power_status, "Expected power to be on");

            camera.power().off().await.expect("Power off failed");

            // Test zoom operations
            camera.zoom().tele().await.expect("Zoom tele failed");

            // Test preset operations
            camera
                .presets()
                .recall(1)
                .await
                .expect("Preset recall failed");
        });
    }

    // Test builder method parity
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn test_tokio_builder_transport_creation() {
        use grafton_visca::TokioExecutor;

        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
        let transport: ScriptedTransport<TokioExecutor> =
            ScriptedTransport::new(vec![Step::OnSend {
                matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]),
                responses: vec![vec![0x90, 0x50, 0x03, 0xFF]], // Power off
            }])
            .with_executor(executor.clone());

        let camera = CameraBuilder::<TokioExecutor>::with_executor(executor)
            .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to build camera");

        let power_status = camera.power().state().await.expect("Power inquiry failed");
        assert!(!power_status, "Expected power to be off");
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn test_smol_builder_transport_creation() {
        use grafton_visca::SmolExecutor;

        smol::block_on(async {
            let executor = Arc::new(SmolExecutor::new());
            let transport: ScriptedTransport<SmolExecutor> =
                ScriptedTransport::new(vec![Step::OnSend {
                    matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]),
                    responses: vec![vec![0x90, 0x50, 0x03, 0xFF]], // Power off
                }])
                .with_executor(executor.clone());

            let camera = CameraBuilder::<SmolExecutor>::with_executor(executor)
                .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
                .await
                .expect("Failed to build camera");

            let power_status = camera.power().state().await.expect("Power inquiry failed");
            assert!(!power_status, "Expected power to be off");
        });
    }

    // Command-side syntax error (0x02) must surface to the caller as a terminal
    // error, on both runtimes. (Inquiry-side 0x02 is transient; see the retry
    // tests below.)
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn test_tokio_command_syntax_error_is_terminal() {
        use grafton_visca::TokioExecutor;
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
        let transport: ScriptedTransport<TokioExecutor> =
            ScriptedTransport::new(vec![Step::OnSend {
                matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]), // Power off command
                responses: vec![vec![0x90, 0x60, 0x02, 0xFF]],           // Syntax error
            }])
            .with_executor(executor.clone());

        let camera = CameraBuilder::<TokioExecutor>::with_executor(executor)
            .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport.clone())
            .await
            .expect("Failed to create camera");

        let result = camera.power().off().await;
        assert!(result.is_err(), "Command-side 0x02 must fail terminally");
        assert_eq!(
            transport.sent().len(),
            1,
            "Command-side 0x02 must not be retried"
        );
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn test_smol_command_syntax_error_is_terminal() {
        use grafton_visca::SmolExecutor;

        smol::block_on(async {
            let executor = Arc::new(SmolExecutor::new());
            let transport: ScriptedTransport<SmolExecutor> =
                ScriptedTransport::new(vec![Step::OnSend {
                    matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]),
                    responses: vec![vec![0x90, 0x60, 0x02, 0xFF]],
                }])
                .with_executor(executor.clone());

            let camera = CameraBuilder::<SmolExecutor>::with_executor(executor)
                .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport.clone())
                .await
                .expect("Failed to create camera");

            let result = camera.power().off().await;
            assert!(result.is_err(), "Command-side 0x02 must fail terminally");
            assert_eq!(transport.sent().len(), 1);
        });
    }

    // Issue #536: a transient inquiry-side 0x02 is retried by the scheduler and
    // the original operation still succeeds once the camera answers, on both
    // runtimes. Exactly one resend occurs, and it is delayed by the profile's
    // inquiry pacing (PtzOpticsG2 MIN_INQUIRY_SPACING = 150ms).
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn test_tokio_inquiry_syntax_error_retries_then_succeeds() {
        use grafton_visca::testing::testkit::scripted_transport::helpers;
        use grafton_visca::TokioExecutor;
        use std::time::Instant;

        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
        let transport: ScriptedTransport<TokioExecutor> =
            ScriptedTransport::new(helpers::syntax_error_then_inquiry_success(
                vec![0x81, 0x09, 0x04, 0x00, 0xFF], // Power inquiry
                vec![0x90, 0x50, 0x02, 0xFF],       // Power on
            ))
            .with_executor(executor.clone());

        let camera = CameraBuilder::<TokioExecutor>::with_executor(executor)
            .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport.clone())
            .await
            .expect("Failed to create camera");

        let started = Instant::now();
        let power_status = camera
            .power()
            .state()
            .await
            .expect("inquiry should succeed after a transient 0x02 retry");
        let elapsed = started.elapsed();

        assert!(power_status, "expected power on after retry");
        assert_eq!(
            transport.sent().len(),
            2,
            "expected exactly one resend after the transient 0x02"
        );
        assert!(
            elapsed >= std::time::Duration::from_millis(140),
            "resend must honor profile inquiry pacing (elapsed = {elapsed:?})"
        );
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn test_smol_inquiry_syntax_error_retries_then_succeeds() {
        use grafton_visca::testing::testkit::scripted_transport::helpers;
        use grafton_visca::SmolExecutor;

        smol::block_on(async {
            let executor = Arc::new(SmolExecutor::new());
            let transport: ScriptedTransport<SmolExecutor> =
                ScriptedTransport::new(helpers::syntax_error_then_inquiry_success(
                    vec![0x81, 0x09, 0x04, 0x00, 0xFF],
                    vec![0x90, 0x50, 0x02, 0xFF],
                ))
                .with_executor(executor.clone());

            let camera = CameraBuilder::<SmolExecutor>::with_executor(executor)
                .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport.clone())
                .await
                .expect("Failed to create camera");

            let power_status = camera
                .power()
                .state()
                .await
                .expect("inquiry should succeed after a transient 0x02 retry");

            assert!(power_status, "expected power on after retry");
            assert_eq!(
                transport.sent().len(),
                2,
                "expected exactly one resend after the transient 0x02"
            );
        });
    }
}
