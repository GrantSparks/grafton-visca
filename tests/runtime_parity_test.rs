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
    #[allow(dead_code)]
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

    // Test error handling parity
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn test_tokio_error_handling() {
        use grafton_visca::TokioExecutor;
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
        let transport: ScriptedTransport<TokioExecutor> =
            ScriptedTransport::new(vec![Step::OnSend {
                matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]),
                responses: vec![vec![0x90, 0x60, 0x02, 0xFF]], // Error response
            }])
            .with_executor(executor.clone());

        let camera = CameraBuilder::<TokioExecutor>::with_executor(executor)
            .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        let result = camera.power().state().await;
        assert!(result.is_err(), "Expected error from error response");
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn test_smol_error_handling() {
        use grafton_visca::SmolExecutor;

        smol::block_on(async {
            let executor = Arc::new(SmolExecutor::new());
            let transport: ScriptedTransport<SmolExecutor> =
                ScriptedTransport::new(vec![Step::OnSend {
                    matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]),
                    responses: vec![vec![0x90, 0x60, 0x02, 0xFF]], // Error response
                }])
                .with_executor(executor.clone());

            let camera = CameraBuilder::<SmolExecutor>::with_executor(executor)
                .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
                .await
                .expect("Failed to create camera");

            let result = camera.power().state().await;
            assert!(result.is_err(), "Expected error from error response");
        });
    }
}
