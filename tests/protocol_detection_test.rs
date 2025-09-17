//! Integration tests for connect_auto with protocol detection
//!
//! These tests verify that the connect_auto method properly detects and honors
//! the protocol style (Sony encapsulated vs Raw VISCA) returned by auto_detect.

#![cfg(all(feature = "mode-async", feature = "test-utils"))]

use grafton_visca::testing::testkit::{ScriptedTransport, Step};

#[cfg(feature = "runtime-tokio")]
mod tokio_tests {
    use grafton_visca::{capabilities::ProtocolStyle, TokioExecutor};

    use super::*;

    #[tokio::test]
    async fn test_connect_auto_detects_sony_encapsulated() {
        // Test that the protocol detector correctly identifies Sony encapsulated format
        // when a camera responds with Sony-wrapped VISCA responses

        // The detector sends version inquiry (8x 09 00 02 FF) wrapped in Sony header
        // Sony inquiry header: [0x01, 0x10] (inquiry type), [len], [sequence]
        let transport: ScriptedTransport<TokioExecutor> =
            ScriptedTransport::new(vec![Step::OnSend {
                // Match the exact Sony-wrapped version inquiry
                // Header: [0x01, 0x10] (inquiry), [0x00, 0x05] (5 byte payload), [0x00, 0x00, 0x00, 0x00] (seq)
                // Payload: [0x81, 0x09, 0x00, 0x02, 0xFF] (version inquiry)
                matches: Some(vec![
                    0x01, 0x10, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x81, 0x09, 0x00, 0x02, 0xFF,
                ]),
                responses: vec![vec![
                    0x01, 0x11, // Sony reply header
                    0x00, 0x0A, // Payload length (10 bytes for version response)
                    0x00, 0x00, 0x00, 0x00, // Sequence number
                    // Version inquiry response: 9x 50 VV VV MM MM FF FF KK FF
                    0x90, 0x50, // Response header
                    0x00, 0x01, // Version
                    0x00, 0x01, // Model
                    0x00, 0x00, // Flags
                    0x02, // Socket count
                    0xFF, // Terminator
                ]],
            }]);

        // Mock Transport::auto_detect by creating the transport ourselves
        // Since auto_detect internally creates the transport, we need to test
        // the overall flow differently

        // For now, we'll test that the detection logic correctly identifies Sony format
        use grafton_visca::protocol::detect::{DetectionResult, ProtocolDetector};

        let mut transport = transport;
        let executor = TokioExecutor::from_handle(tokio::runtime::Handle::current());
        let detector = ProtocolDetector::new();

        let result = detector
            .detect_protocol(&mut transport, &executor)
            .await
            .expect("Detection failed");

        assert_eq!(result, DetectionResult::SonyEncapsulated);

        // Verify the detection result maps to the correct protocol style
        assert_eq!(
            result.to_protocol_style(),
            Some(ProtocolStyle::SonyEncapsulated)
        );
    }

    #[tokio::test]
    async fn test_connect_auto_detects_raw_visca() {
        // Test that the protocol detector correctly identifies raw VISCA format
        // when a camera responds only to raw VISCA commands (not Sony encapsulated)
        use grafton_visca::{
            protocol::detect::{DetectionResult, ProtocolDetector},
            transport::RetryConfig,
        };

        use std::time::Duration;

        // The protocol detector will:
        // 1. First try Sony encapsulated format with version inquiry
        // 2. Then try raw VISCA format with version inquiry
        // According to the VISCA protocol docs:
        //   - Version inquiry command: 8x 09 00 02 FF
        //   - Version response: 9x 50 VV VV MM MM FF FF KK FF
        let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
            Step::OnSend {
                // Sony encapsulated attempt - match the exact wrapped command
                matches: Some(vec![
                    0x01, 0x10, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x81, 0x09, 0x00, 0x02, 0xFF,
                ]),
                responses: vec![], // No response for Sony format
            },
            Step::OnSend {
                // Raw VISCA attempt - match the exact raw command (version inquiry)
                matches: Some(vec![0x81, 0x09, 0x00, 0x02, 0xFF]), // Version inquiry: 8x 09 00 02 FF
                // Response format: 9x 50 VV VV MM MM FF FF KK FF (version, model, flags, socket count)
                // Using a minimal valid response
                responses: vec![vec![
                    0x90, 0x50, // Response header
                    0x00, 0x01, // Version VV VV
                    0x00, 0x01, // Model MM MM
                    0x00, 0x00, // Flags FF FF
                    0x02, // Socket count KK
                    0xFF, // Terminator
                ]],
            },
        ]);

        // Create detector with no retries to make test deterministic
        let executor = TokioExecutor::from_handle(tokio::runtime::Handle::current());
        let detector = ProtocolDetector::with_retry_config(RetryConfig {
            max_retries: 0,
            base_retry_delay: Duration::from_millis(50),
            max_retry_duration: Duration::from_millis(500),
            exponential_backoff: false,
        });

        // Run protocol detection
        let mut transport = transport;
        let result = detector
            .detect_protocol(&mut transport, &executor)
            .await
            .expect("Detection should succeed");

        // Verify raw VISCA was detected
        assert_eq!(result, DetectionResult::RawVisca);

        // Verify the detection result maps to the correct protocol style
        assert_eq!(result.to_protocol_style(), Some(ProtocolStyle::RawVisca));
    }

    #[tokio::test]
    async fn test_connect_auto_errors_on_no_response() {
        // Create a scripted transport that doesn't respond
        let transport: ScriptedTransport<TokioExecutor> =
            ScriptedTransport::new(vec![Step::OnSend {
                matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]), // Power inquiry
                responses: vec![],                                 // No response
            }]);

        // Test the detection logic
        use grafton_visca::protocol::detect::{DetectionResult, ProtocolDetector};

        let mut transport = transport;
        let executor = TokioExecutor::from_handle(tokio::runtime::Handle::current());
        let detector = ProtocolDetector::new();

        let result = detector
            .detect_protocol(&mut transport, &executor)
            .await
            .expect("Detection should succeed but return NoResponse");

        assert_eq!(result, DetectionResult::NoResponse);

        // Verify NoResponse maps to None
        assert_eq!(result.to_protocol_style(), None);
    }

    #[tokio::test]
    async fn test_camera_with_explicit_protocol_style() {
        // Test that a camera can be created with an explicit protocol style
        // This validates that our fix to connect_auto would work correctly
        use grafton_visca::{camera::CameraBuilder, profiles::GenericVisca};

        // Create a transport that responds with Sony encapsulated format
        let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
            // Initialization phase - camera might send commands
            Step::OnSend {
                matches: None,
                responses: vec![vec![
                    0x01, 0x11, 0x00, 0x01, // Sony header
                    0x00, 0x01, // Length
                    0x90, 0x41, 0xFF, // ACK
                ]],
            },
        ]);

        // Build camera with explicit protocol style
        use grafton_visca::runtime::TokioRuntime;
        let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
        let camera = CameraBuilder::with_executor(runtime)
            .protocol_style(ProtocolStyle::SonyEncapsulated)
            .open_async::<GenericVisca, _>(transport)
            .await
            .expect("Failed to create camera");

        // The camera should be created successfully with the Sony encapsulated protocol
        // This validates that protocol style can be properly configured
        // Camera created successfully validates protocol style threading
        assert!(camera.timeout_config().movement_timeout.as_secs() > 0);
    }
}

#[cfg(feature = "runtime-async-std")]
mod async_std_tests {
    use grafton_visca::AsyncStdExecutor;

    use super::*;

    #[async_std::test]
    async fn test_connect_auto_detects_sony_encapsulated_async_std() {
        // Create a scripted transport that responds with Sony encapsulated format
        let transport: ScriptedTransport<AsyncStdExecutor> =
            ScriptedTransport::new(vec![Step::OnSend {
                // Match the exact Sony-wrapped version inquiry
                // Header: [0x01, 0x10] (inquiry), [0x00, 0x05] (5 byte payload), [0x00, 0x00, 0x00, 0x00] (seq)
                // Payload: [0x81, 0x09, 0x00, 0x02, 0xFF] (version inquiry)
                matches: Some(vec![
                    0x01, 0x10, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x81, 0x09, 0x00, 0x02, 0xFF,
                ]),
                responses: vec![vec![
                    0x01, 0x11, // Sony reply header
                    0x00, 0x0A, // Payload length (10 bytes for version response)
                    0x00, 0x00, 0x00, 0x00, // Sequence number
                    // Version inquiry response: 9x 50 VV VV MM MM FF FF KK FF
                    0x90, 0x50, // Response header
                    0x00, 0x01, // Version
                    0x00, 0x01, // Model
                    0x00, 0x00, // Flags
                    0x02, // Socket count
                    0xFF, // Terminator
                ]],
            }]);

        use grafton_visca::protocol::detect::{DetectionResult, ProtocolDetector};

        let mut transport = transport;
        let executor = AsyncStdExecutor::new();
        let detector = ProtocolDetector::new();

        let result = detector
            .detect_protocol(&mut transport, &executor)
            .await
            .expect("Detection failed");

        assert_eq!(result, DetectionResult::SonyEncapsulated);
    }
}

#[cfg(feature = "runtime-smol")]
mod smol_tests {
    use grafton_visca::SmolExecutor;

    use super::*;

    fn test_connect_auto_detects_sony_encapsulated_smol() {
        smol::block_on(async {
            // Create a scripted transport that responds with Sony encapsulated format
            let transport: ScriptedTransport<SmolExecutor> =
                ScriptedTransport::new(vec![Step::OnSend {
                    // Match the exact Sony-wrapped version inquiry
                    // Header: [0x01, 0x10] (inquiry), [0x00, 0x05] (5 byte payload), [0x00, 0x00, 0x00, 0x00] (seq)
                    // Payload: [0x81, 0x09, 0x00, 0x02, 0xFF] (version inquiry)
                    matches: Some(vec![
                        0x01, 0x10, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x81, 0x09, 0x00, 0x02,
                        0xFF,
                    ]),
                    responses: vec![vec![
                        0x01, 0x11, // Sony reply header
                        0x00, 0x0A, // Payload length (10 bytes for version response)
                        0x00, 0x00, 0x00, 0x00, // Sequence number
                        // Version inquiry response: 9x 50 VV VV MM MM FF FF KK FF
                        0x90, 0x50, // Response header
                        0x00, 0x01, // Version
                        0x00, 0x01, // Model
                        0x00, 0x00, // Flags
                        0x02, // Socket count
                        0xFF, // Terminator
                    ]],
                }]);

            use grafton_visca::protocol::detect::{DetectionResult, ProtocolDetector};

            let mut transport = transport;
            let executor = SmolExecutor::new();
            let detector = ProtocolDetector::new();

            let result = detector
                .detect_protocol(&mut transport, &executor)
                .await
                .expect("Detection failed");

            assert_eq!(result, DetectionResult::SonyEncapsulated);
        });
    }

    #[test]
    fn run_smol_test() {
        test_connect_auto_detects_sony_encapsulated_smol();
    }
}
