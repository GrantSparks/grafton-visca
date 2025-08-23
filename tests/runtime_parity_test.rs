//! Integration tests verifying parity across all three async runtime implementations.
//!
//! These tests ensure that Tokio, async-std, and smol runtime adapters provide
//! identical behavior for all transport operations.

#![cfg(feature = "async")]

#[cfg(feature = "test-utils")]
mod parity_tests {
    use grafton_visca::{
        camera::CameraBuilder,
        testing::testkit::{ScriptedTransport, Step},
        PowerControl, PresetsControl, ZoomControl,
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

    #[cfg(feature = "rt-tokio")]
    #[tokio::test]
    async fn test_tokio_runtime_operations() {
        use grafton_visca::TokioExecutor;

        use std::sync::Arc;
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
        let transport: ScriptedTransport<TokioExecutor> =
            ScriptedTransport::new(create_test_script()).with_executor(executor.clone());

        let camera = CameraBuilder::with_executor(executor)
            .build_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Test power operations
        let power_status = camera.power_inquiry().await.expect("Power inquiry failed");
        assert!(power_status, "Expected power to be on");

        camera.power_off().await.expect("Power off failed");

        // Test zoom operations
        camera.zoom_tele_std().await.expect("Zoom tele failed");

        // Test preset operations
        use grafton_visca::command::preset::PresetNumber;
        camera
            .preset_recall(PresetNumber::new(1).unwrap())
            .await
            .expect("Preset recall failed");
    }

    #[cfg(feature = "rt-async-std")]
    #[async_std::test]
    async fn test_async_std_runtime_operations() {
        use grafton_visca::AsyncStdExecutor;

        use std::sync::Arc;
        let executor = Arc::new(AsyncStdExecutor::new());
        let transport: ScriptedTransport<AsyncStdExecutor> =
            ScriptedTransport::new(create_test_script()).with_executor(executor.clone());

        let camera = CameraBuilder::with_executor(executor)
            .build_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Test power operations
        let power_status = camera.power_inquiry().await.expect("Power inquiry failed");
        assert!(power_status, "Expected power to be on");

        camera.power_off().await.expect("Power off failed");

        // Test zoom operations
        camera.zoom_tele_std().await.expect("Zoom tele failed");

        // Test preset operations
        use grafton_visca::command::preset::PresetNumber;
        camera
            .preset_recall(PresetNumber::new(1).unwrap())
            .await
            .expect("Preset recall failed");
    }

    #[cfg(feature = "rt-smol")]
    #[test]
    #[ignore = "TODO: Fix smol executor initialization - currently stalls"]
    fn test_smol_runtime_operations() {
        use grafton_visca::SmolExecutor;

        smol::block_on(async {
            use std::sync::Arc;
            let executor = Arc::new(SmolExecutor::new());
            let transport: ScriptedTransport<SmolExecutor> =
                ScriptedTransport::new(create_test_script()).with_executor(executor.clone());

            let camera = CameraBuilder::with_executor(executor)
                .build_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
                .await
                .expect("Failed to create camera");

            // Test power operations
            let power_status = camera.power_inquiry().await.expect("Power inquiry failed");
            assert!(power_status, "Expected power to be on");

            camera.power_off().await.expect("Power off failed");

            // Test zoom operations
            camera.zoom_tele_std().await.expect("Zoom tele failed");

            // Test preset operations
            use grafton_visca::command::preset::PresetNumber;
            camera
                .preset_recall(PresetNumber::new(1).unwrap())
                .await
                .expect("Preset recall failed");
        });
    }

    // Test builder method parity
    #[cfg(feature = "rt-tokio")]
    #[tokio::test]
    async fn test_tokio_builder_transport_creation() {
        use grafton_visca::TokioExecutor;
        use std::sync::Arc;

        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
        let transport: ScriptedTransport<TokioExecutor> =
            ScriptedTransport::new(vec![Step::OnSend {
                matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]),
                responses: vec![vec![0x90, 0x50, 0x03, 0xFF]], // Power off
            }])
            .with_executor(executor.clone());

        let camera = CameraBuilder::with_executor(executor)
            .build_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to build camera");

        let power_status = camera.power_inquiry().await.expect("Power inquiry failed");
        assert!(!power_status, "Expected power to be off");
    }

    #[cfg(feature = "rt-async-std")]
    #[async_std::test]
    async fn test_async_std_builder_transport_creation() {
        use grafton_visca::AsyncStdExecutor;
        use std::sync::Arc;

        let executor = Arc::new(AsyncStdExecutor::new());
        let transport: ScriptedTransport<AsyncStdExecutor> =
            ScriptedTransport::new(vec![Step::OnSend {
                matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]),
                responses: vec![vec![0x90, 0x50, 0x03, 0xFF]], // Power off
            }])
            .with_executor(executor.clone());

        let camera = CameraBuilder::with_executor(executor)
            .build_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to build camera");

        let power_status = camera.power_inquiry().await.expect("Power inquiry failed");
        assert!(!power_status, "Expected power to be off");
    }

    #[cfg(feature = "rt-smol")]
    #[test]
    #[ignore = "TODO: Fix smol executor initialization - currently stalls"]
    fn test_smol_builder_transport_creation() {
        use grafton_visca::SmolExecutor;
        use std::sync::Arc;

        smol::block_on(async {
            let executor = Arc::new(SmolExecutor::new());
            let transport: ScriptedTransport<SmolExecutor> =
                ScriptedTransport::new(vec![Step::OnSend {
                    matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]),
                    responses: vec![vec![0x90, 0x50, 0x03, 0xFF]], // Power off
                }])
                .with_executor(executor.clone());

            let camera = CameraBuilder::with_executor(executor)
                .build_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
                .await
                .expect("Failed to build camera");

            let power_status = camera.power_inquiry().await.expect("Power inquiry failed");
            assert!(!power_status, "Expected power to be off");
        });
    }

    // Test error handling parity
    #[cfg(feature = "rt-tokio")]
    #[tokio::test]
    #[ignore = "TODO: Fix runtime initialization with ScriptedTransport executor"]
    async fn test_tokio_error_handling() {
        use grafton_visca::TokioExecutor;

        use std::sync::Arc;
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
        let transport: ScriptedTransport<TokioExecutor> =
            ScriptedTransport::new(vec![Step::OnSend {
                matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]),
                responses: vec![vec![0x90, 0x60, 0x02, 0xFF]], // Error response
            }])
            .with_executor(executor.clone());

        let camera = CameraBuilder::with_executor(executor)
            .build_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        let result = camera.power_inquiry().await;
        assert!(result.is_err(), "Expected error from error response");
    }

    #[cfg(feature = "rt-async-std")]
    #[async_std::test]
    #[ignore = "TODO: Fix runtime initialization with ScriptedTransport executor"]
    async fn test_async_std_error_handling() {
        use grafton_visca::AsyncStdExecutor;

        use std::sync::Arc;
        let executor = Arc::new(AsyncStdExecutor::new());
        let transport: ScriptedTransport<AsyncStdExecutor> =
            ScriptedTransport::new(vec![Step::OnSend {
                matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]),
                responses: vec![vec![0x90, 0x60, 0x02, 0xFF]], // Error response
            }])
            .with_executor(executor.clone());

        let camera = CameraBuilder::with_executor(executor)
            .build_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        let result = camera.power_inquiry().await;
        assert!(result.is_err(), "Expected error from error response");
    }

    #[cfg(feature = "rt-smol")]
    #[test]
    #[ignore = "TODO: Fix smol executor initialization - currently stalls"]
    fn test_smol_error_handling() {
        use grafton_visca::SmolExecutor;

        smol::block_on(async {
            use std::sync::Arc;
            let executor = Arc::new(SmolExecutor::new());
            let transport: ScriptedTransport<SmolExecutor> =
                ScriptedTransport::new(vec![Step::OnSend {
                    matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]),
                    responses: vec![vec![0x90, 0x60, 0x02, 0xFF]], // Error response
                }])
                .with_executor(executor.clone());

            let camera = CameraBuilder::with_executor(executor)
                .build_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
                .await
                .expect("Failed to create camera");

            let result = camera.power_inquiry().await;
            assert!(result.is_err(), "Expected error from error response");
        });
    }
}

#[cfg(all(
    feature = "test-utils",
    feature = "rt-tokio",
    feature = "rt-async-std",
    feature = "rt-smol"
))]
mod all_runtimes_test {
    use grafton_visca::{
        camera::CameraBuilder,
        testing::testkit::{ScriptedTransport, Step},
        PowerControl,
    };

    /// Verify that all three runtimes produce identical results for the same operations
    #[tokio::test]
    #[ignore = "TODO: Fix smol executor initialization - currently stalls in smol portion"]
    async fn test_all_runtimes_produce_same_results() {
        let script = vec![Step::OnSend {
            matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]),
            responses: vec![vec![0x90, 0x50, 0x02, 0xFF]], // Power on
        }];

        // Test with Tokio
        let tokio_result = {
            use grafton_visca::TokioExecutor;
            use std::sync::Arc;
            let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
            let transport: ScriptedTransport<TokioExecutor> =
                ScriptedTransport::new(script.clone()).with_executor(executor.clone());
            let camera = CameraBuilder::with_executor(executor)
                .build_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
                .await
                .unwrap();
            camera.power_inquiry().await.unwrap()
        };

        // Test with async-std
        let async_std_result = {
            use grafton_visca::AsyncStdExecutor;
            use std::sync::Arc;
            let executor = Arc::new(AsyncStdExecutor::new());
            let transport: ScriptedTransport<AsyncStdExecutor> =
                ScriptedTransport::new(script.clone()).with_executor(executor.clone());
            let camera = CameraBuilder::with_executor(executor)
                .build_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
                .await
                .unwrap();
            camera.power_inquiry().await.unwrap()
        };

        // Test with smol (in a blocking context since we're already in tokio)
        let smol_result = {
            use grafton_visca::SmolExecutor;
            use std::sync::Arc;
            let executor = Arc::new(SmolExecutor::new());
            let transport: ScriptedTransport<SmolExecutor> =
                ScriptedTransport::new(script.clone()).with_executor(executor.clone());

            // Run smol in a separate thread to avoid runtime conflicts
            std::thread::spawn(move || {
                smol::block_on(async {
                    let camera = CameraBuilder::with_executor(executor)
                        .build_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
                        .await
                        .unwrap();
                    camera.power_inquiry().await.unwrap()
                })
            })
            .join()
            .unwrap()
        };

        // All runtimes should produce the same result
        assert_eq!(
            tokio_result, async_std_result,
            "Tokio and async-std results differ"
        );
        assert_eq!(
            async_std_result, smol_result,
            "async-std and smol results differ"
        );
        assert!(tokio_result, "All runtimes should report power as on");
    }
}
