//! Protocol auto-detection for VISCA cameras.
//!
//! This module implements EPIC task B3: automatic detection of Sony encapsulated
//! vs raw VISCA protocol modes. It probes the camera with both formats to
//! determine which protocol the camera expects.

use std::time::Duration;
use tracing::{debug, info, warn};

use crate::capabilities::ProtocolStyle;
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
    pub async fn detect_protocol<T>(&self, transport: &mut T) -> Result<DetectionResult, Error>
    where
        T: AsyncTransport,
    {
        info!("Starting VISCA protocol detection");

        // Test command: Version Inquiry - should be supported by all VISCA cameras
        let test_command = &[0x81, 0x09, 0x00, 0x02, 0xFF];

        // Try Sony encapsulated format first (priority order from EPIC)
        debug!("Probing Sony encapsulated protocol (52381 style)");
        match self
            .try_protocol(
                transport,
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
                warn!("Error testing Sony format: {}", e);
            }
        }

        // Fallback to raw VISCA format
        debug!("Probing raw VISCA protocol (1259/5678 style)");
        match self
            .try_protocol(transport, test_command, ProtocolStyle::RawVisca)
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
                warn!("Error testing raw format: {}", e);
            }
        }

        warn!("No VISCA protocol response detected from camera");
        Ok(DetectionResult::NoResponse)
    }

    /// Test a specific protocol format by sending a command and waiting for response
    async fn try_protocol<T>(
        &self,
        transport: &mut T,
        command: &[u8],
        protocol_style: ProtocolStyle,
    ) -> Result<bool, Error>
    where
        T: AsyncTransport,
    {
        let envelope = TransportEnvelope::new(protocol_style);
        let buffer_manager = BufferManager::new(BufferConfig::default());

        // Frame the command according to the protocol style
        let framed_command = envelope.frame_command(command, true, &buffer_manager);

        debug!(
            "Sending {} bytes for protocol detection: {:02X?}",
            framed_command.len(),
            &framed_command[..std::cmp::min(framed_command.len(), 16)]
        );

        // Send command with retries
        for attempt in 0..=self.retry_config.max_retries {
            // Send the test command
            if let Err(e) = transport.send(&framed_command).await {
                warn!(
                    "Failed to send detection command (attempt {}): {}",
                    attempt + 1,
                    e
                );
                if attempt == self.retry_config.max_retries {
                    return Err(e);
                }
                continue;
            }

            // Wait for response with timeout
            let response_result = tokio::time::timeout(DETECTION_TIMEOUT, transport.recv()).await;

            match response_result {
                Ok(Ok(response_bytes)) => {
                    debug!(
                        "Received {} bytes response: {:02X?}",
                        response_bytes.len(),
                        &response_bytes[..std::cmp::min(response_bytes.len(), 16)]
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
                            debug!("Failed to extract VISCA payload: {}", e);
                        }
                    }
                }
                Ok(Err(e)) => {
                    debug!("Transport error during detection: {}", e);
                }
                Err(_timeout) => {
                    debug!("Timeout waiting for response (attempt {})", attempt + 1);
                }
            }

            // Wait before retry
            if attempt < self.retry_config.max_retries {
                let delay = self.retry_config.calculate_delay(attempt, None);
                tokio::time::sleep(delay).await;
            }
        }

        Ok(false)
    }

    /// Validate that received bytes look like a valid VISCA response
    fn is_valid_visca_response(&self, payload: &[u8]) -> bool {
        // VISCA responses should be at least 3 bytes and end with 0xFF
        if payload.len() < 3 {
            return false;
        }

        // Must end with VISCA terminator
        if payload[payload.len() - 1] != 0xFF {
            return false;
        }

        // Should start with 0x90 (response header) for most responses
        // Version inquiry responses start with 0x90 0x50
        if payload.len() >= 2 && payload[0] == 0x90 {
            // Check for common response types:
            // 0x50 = data reply (version inquiry)
            // 0x4X = ACK
            // 0x5X = completion
            // 0x6X = error
            let response_type = payload[1] & 0xF0;
            if response_type == 0x40 || response_type == 0x50 || response_type == 0x60 {
                return true;
            }
        }

        false
    }
}

impl Default for ProtocolDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, feature = "test-utils"))]
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
                    0x01, 0x11, 0x00, 0x06, 0x00, 0x00, 0x00, 0x01, // Sony header
                    0x90, 0x50, 0x01, 0x02, 0x03, 0xFF,
                ], // Version response
            ],
        }];
        let mut transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(steps);

        let detector = ProtocolDetector::new();
        let result = detector.detect_protocol(&mut transport).await.unwrap();

        assert_eq!(result, DetectionResult::SonyEncapsulated);
    }

    #[tokio::test]
    async fn test_detect_raw_visca() {
        // Create a script that responds with a valid VISCA version response
        // when receiving raw VISCA format (but not Sony encapsulated)
        let steps = vec![Step::OnSend {
            matches: Some(vec![0x81, 0x09]), // Raw VISCA inquiry
            responses: vec![
                vec![0x90, 0x50, 0x01, 0x02, 0x03, 0xFF], // Version response
            ],
        }];
        let mut transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(steps);

        let detector = ProtocolDetector::new();
        let result = detector.detect_protocol(&mut transport).await.unwrap();

        assert_eq!(result, DetectionResult::RawVisca);
    }

    #[tokio::test]
    async fn test_no_response_detected() {
        // Empty script - no responses configured
        let mut transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![]);

        let detector = ProtocolDetector::new();
        let result = detector.detect_protocol(&mut transport).await.unwrap();

        assert_eq!(result, DetectionResult::NoResponse);
    }

    #[test]
    fn test_is_valid_visca_response() {
        let detector = ProtocolDetector::new();

        // Valid version inquiry response
        assert!(detector.is_valid_visca_response(&[0x90, 0x50, 0x01, 0x02, 0x03, 0xFF]));

        // Valid ACK response
        assert!(detector.is_valid_visca_response(&[0x90, 0x41, 0xFF]));

        // Valid completion response
        assert!(detector.is_valid_visca_response(&[0x90, 0x51, 0xFF]));

        // Valid error response
        assert!(detector.is_valid_visca_response(&[0x90, 0x60, 0x02, 0xFF]));

        // Invalid - too short
        assert!(!detector.is_valid_visca_response(&[0x90, 0xFF]));

        // Invalid - doesn't end with 0xFF
        assert!(!detector.is_valid_visca_response(&[0x90, 0x50, 0x01, 0x00]));

        // Invalid - wrong header
        assert!(!detector.is_valid_visca_response(&[0x80, 0x50, 0x01, 0xFF]));

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
