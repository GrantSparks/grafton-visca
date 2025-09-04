//! Protocol auto-detection for VISCA cameras.
//!
//! This module implements EPIC task B3: automatic detection of Sony encapsulated
//! vs raw VISCA protocol modes. It probes the camera with both formats to
//! determine which protocol the camera expects.

use std::time::Duration;
use tracing::{debug, info, warn};

use crate::capabilities::ProtocolStyle;
use crate::command::bytes::VISCA_TERMINATOR;
use crate::executor::Executor;
use crate::protocol::response::decode_basic;
use crate::transport::buffer::{BufferConfig, BufferManager};
use crate::transport::envelope::TransportEnvelope;
use crate::transport::{AsyncTransport, RetryConfig};
use crate::Error;

/// Protocol detection timeout - how long to wait for camera response
const DETECTION_TIMEOUT: Duration = Duration::from_millis(100);

/// Maximum retry attempts during detection
const DETECTION_MAX_RETRIES: u32 = 2;

/// Result of protocol detection attempt
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionResult {
    /// Sony encapsulated protocol detected (8-byte header format)
    SonyEncapsulated,
    /// Raw VISCA protocol detected (no header)
    RawVisca,
    /// No response from either protocol
    NoResponse,
}

impl DetectionResult {
    /// Convert detection result to protocol style
    pub fn to_protocol_style(self) -> Option<ProtocolStyle> {
        match self {
            DetectionResult::SonyEncapsulated => {
                Some(ProtocolStyle::SonyEncapsulated { use_sequence: true })
            }
            DetectionResult::RawVisca => Some(ProtocolStyle::RawVisca),
            DetectionResult::NoResponse => None,
        }
    }
}

/// Protocol detector for automatic VISCA protocol detection
#[derive(Debug, Clone, Copy)]
pub struct ProtocolDetector {
    retry_config: RetryConfig,
}

impl ProtocolDetector {
    /// Create a new protocol detector with default configuration
    pub fn new() -> Self {
        Self {
            retry_config: RetryConfig {
                max_retries: DETECTION_MAX_RETRIES,
                base_retry_delay: Duration::from_millis(50),
                max_retry_duration: Duration::from_millis(500),
                exponential_backoff: false,
            },
        }
    }

    /// Create a protocol detector with custom retry configuration
    pub fn with_retry_config(retry_config: RetryConfig) -> Self {
        Self { retry_config }
    }

    /// Detect the protocol used by the camera by probing with test commands.
    ///
    /// This implements the EPIC B3 detection algorithm:
    /// 1. Try Sony encapsulated format first (most cameras support this)
    /// 2. If no response, fallback to raw VISCA format
    /// 3. If neither works, return NoResponse
    ///
    /// The test command used is a simple Version Inquiry (81 09 00 02 FF)
    /// which should be supported by all VISCA cameras.
    pub async fn detect_protocol<T, E>(
        &self,
        transport: &mut T,
        executor: &E,
    ) -> Result<DetectionResult, Error>
    where
        T: AsyncTransport,
        E: Executor,
    {
        info!("Starting VISCA protocol detection");

        // Test command: Version Inquiry - should be supported by all VISCA cameras
        let test_command = &[0x81, 0x09, 0x00, 0x02, VISCA_TERMINATOR];

        // Try Sony encapsulated format first (priority order from EPIC)
        debug!("Probing Sony encapsulated protocol (52381 style)");
        match self
            .try_protocol(
                transport,
                executor,
                test_command,
                ProtocolStyle::SonyEncapsulated { use_sequence: true },
            )
            .await
        {
            Ok(true) => {
                info!("✓ Sony encapsulated protocol detected");
                return Ok(DetectionResult::SonyEncapsulated);
            }
            Ok(false) => {
                debug!("✗ No response from Sony encapsulated format");
            }
            Err(e) => {
                warn!("Error testing Sony format: {e}");
            }
        }

        // Fallback to raw VISCA format
        debug!("Probing raw VISCA protocol (1259/5678 style)");
        match self
            .try_protocol(transport, executor, test_command, ProtocolStyle::RawVisca)
            .await
        {
            Ok(true) => {
                info!("✓ Raw VISCA protocol detected");
                return Ok(DetectionResult::RawVisca);
            }
            Ok(false) => {
                debug!("✗ No response from raw VISCA format");
            }
            Err(e) => {
                warn!("Error testing raw format: {e}");
            }
        }

        warn!("No VISCA protocol response detected from camera");
        Ok(DetectionResult::NoResponse)
    }

    /// Test a specific protocol format by sending a command and waiting for response
    async fn try_protocol<T, E>(
        &self,
        transport: &mut T,
        executor: &E,
        command: &[u8],
        protocol_style: ProtocolStyle,
    ) -> Result<bool, Error>
    where
        T: AsyncTransport,
        E: Executor,
    {
        let envelope = TransportEnvelope::new(protocol_style);
        let buffer_manager = BufferManager::new(BufferConfig::default());

        // Frame the command according to the protocol style (inquiry detection done internally)
        let framed_command = envelope.frame_command(command, &buffer_manager);

        debug!(
            "Sending {len} bytes for protocol detection: {bytes:02X?}",
            len = framed_command.len(),
            bytes = &framed_command[..std::cmp::min(framed_command.len(), 16)]
        );

        // Send command with retries
        for attempt in 0..=self.retry_config.max_retries {
            // Send the test command
            if let Err(e) = transport.send(&framed_command).await {
                warn!(
                    "Failed to send detection command (attempt {attempt}): {e}",
                    attempt = attempt + 1
                );
                if attempt == self.retry_config.max_retries {
                    return Err(e);
                }
                continue;
            }

            // Wait for response with timeout
            let response_result = executor.timeout(DETECTION_TIMEOUT, transport.recv()).await;

            match response_result {
                Ok(Ok(response_bytes)) => {
                    debug!(
                        "Received {len} bytes response: {bytes:02X?}",
                        len = response_bytes.len(),
                        bytes = &response_bytes[..std::cmp::min(response_bytes.len(), 16)]
                    );

                    // Try to extract VISCA payload
                    match envelope.extract_response(&response_bytes) {
                        Ok(visca_payload) => {
                            // Validate this looks like a VISCA response
                            if self.is_valid_visca_response(&visca_payload) {
                                debug!(
                                    "Valid VISCA response detected for protocol style: {:?}",
                                    protocol_style
                                );
                                return Ok(true);
                            } else {
                                debug!("Received data but not a valid VISCA response");
                            }
                        }
                        Err(e) => {
                            debug!("Failed to extract VISCA payload: {e}");
                        }
                    }
                }
                Ok(Err(e)) => {
                    debug!("Transport error during detection: {e}");
                }
                Err(_timeout) => {
                    debug!(
                        "Timeout waiting for response (attempt {attempt})",
                        attempt = attempt + 1
                    );
                }
            }

            // Wait before retry
            if attempt < self.retry_config.max_retries {
                let delay = self.retry_config.calculate_delay(attempt, None);
                executor.sleep(delay).await;
            }
        }

        Ok(false)
    }

    /// Validate that received bytes look like a valid VISCA response
    fn is_valid_visca_response(&self, payload: &[u8]) -> bool {
        // Use the unified decode_basic function to validate
        decode_basic(payload).is_some()
    }
}

impl Default for ProtocolDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, feature = "rt-tokio", feature = "test-utils"))]
#[allow(clippy::panic, clippy::assertions_on_constants)]
mod tests {
    use super::*;
    use crate::executor::TokioExecutor;
    use crate::testing::testkit::scripted_transport::{ScriptedTransport, Step};

    #[tokio::test]
    async fn test_detect_sony_encapsulated() {
        // Create a script that responds with a valid VISCA version response
        // when receiving Sony encapsulated format
        let steps = vec![Step::OnSend {
            matches: Some(vec![0x01, 0x10]), // Sony inquiry payload type
            responses: vec![
                vec![
                    0x01,
                    0x11,
                    0x00,
                    0x06,
                    0x00,
                    0x00,
                    0x00,
                    0x01, // Sony header
                    0x90,
                    0x50,
                    0x01,
                    0x02,
                    0x03,
                    VISCA_TERMINATOR,
                ], // Version response
            ],
        }];
        let mut transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(steps);

        let detector = ProtocolDetector::new();
        let executor = match TokioExecutor::from_current() {
            Ok(executor) => executor,
            Err(_) => panic!("Test requires Tokio runtime"),
        };
        let result = detector.detect_protocol(&mut transport, &executor).await;
        match result {
            Ok(detection) => assert_eq!(detection, DetectionResult::SonyEncapsulated),
            Err(e) => panic!("Detection should succeed but failed: {e}"),
        }
    }

    #[tokio::test]
    async fn test_detect_raw_visca() {
        // Create a script that responds with a valid VISCA version response
        // when receiving raw VISCA format (but not Sony encapsulated)
        let steps = vec![Step::OnSend {
            matches: Some(vec![0x81, 0x09]), // Raw VISCA inquiry
            responses: vec![
                vec![0x90, 0x50, 0x01, 0x02, 0x03, VISCA_TERMINATOR], // Version response
            ],
        }];
        let mut transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(steps);

        let detector = ProtocolDetector::new();
        let executor = match TokioExecutor::from_current() {
            Ok(executor) => executor,
            Err(_) => panic!("Test requires Tokio runtime"),
        };
        let result = detector.detect_protocol(&mut transport, &executor).await;
        match result {
            Ok(detection) => assert_eq!(detection, DetectionResult::RawVisca),
            Err(e) => panic!("Detection should succeed but failed: {e}"),
        }
    }

    #[tokio::test]
    async fn test_no_response_detected() {
        // Empty script - no responses configured
        let mut transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![]);

        let detector = ProtocolDetector::new();
        let executor = match TokioExecutor::from_current() {
            Ok(executor) => executor,
            Err(_) => panic!("Test requires Tokio runtime"),
        };
        let result = detector.detect_protocol(&mut transport, &executor).await;
        match result {
            Ok(detection) => assert_eq!(detection, DetectionResult::NoResponse),
            Err(e) => panic!("Detection should succeed but failed: {e}"),
        }
    }

    #[test]
    fn test_is_valid_visca_response() {
        let detector = ProtocolDetector::new();

        // Valid version inquiry response
        assert!(detector.is_valid_visca_response(&[
            0x90,
            0x50,
            0x01,
            0x02,
            0x03,
            VISCA_TERMINATOR
        ]));

        // Valid ACK response
        assert!(detector.is_valid_visca_response(&[0x90, 0x41, VISCA_TERMINATOR]));

        // Valid completion response
        assert!(detector.is_valid_visca_response(&[0x90, 0x51, VISCA_TERMINATOR]));

        // Valid error response
        assert!(detector.is_valid_visca_response(&[0x90, 0x60, 0x02, VISCA_TERMINATOR]));

        // Invalid - too short
        assert!(!detector.is_valid_visca_response(&[0x90, VISCA_TERMINATOR]));

        // Invalid - doesn't end with VISCA_TERMINATOR
        assert!(!detector.is_valid_visca_response(&[0x90, 0x50, 0x01, 0x00]));

        // Invalid - wrong header
        assert!(!detector.is_valid_visca_response(&[0x80, 0x50, 0x01, VISCA_TERMINATOR]));

        // Empty payload
        assert!(!detector.is_valid_visca_response(&[]));
    }

    #[test]
    fn test_detection_result_to_protocol_style() {
        assert_eq!(
            DetectionResult::SonyEncapsulated.to_protocol_style(),
            Some(ProtocolStyle::SonyEncapsulated { use_sequence: true })
        );

        assert_eq!(
            DetectionResult::RawVisca.to_protocol_style(),
            Some(ProtocolStyle::RawVisca)
        );

        assert_eq!(DetectionResult::NoResponse.to_protocol_style(), None);
    }
}
