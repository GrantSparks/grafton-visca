//! Protocol auto-detection for VISCA cameras.
//!
//! This module implements EPIC task B3: automatic detection of Sony encapsulated
//! vs raw VISCA protocol modes. It probes the camera with both formats to
//! determine which protocol the camera expects.

use tracing::{debug, info, warn};

use std::time::Duration;

use crate::{
    capabilities::ProtocolStyle,
    command::bytes::VISCA_TERMINATOR,
    executor::Executor,
    protocol::{framer::ProtocolFramer, response::decode_basic},
    transport::{
        buffer::{BufferConfig, BufferManager},
        envelope::TransportEnvelope,
        AsyncTransport, RetryConfig,
    },
    Error,
};

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
            DetectionResult::SonyEncapsulated => Some(ProtocolStyle::SonyEncapsulated),
            DetectionResult::RawVisca => Some(ProtocolStyle::RawVisca),
            DetectionResult::NoResponse => None,
        }
    }
}

/// Detection candidate representing a transport/protocol combination to try
#[derive(Debug, Clone, Copy)]
pub struct DetectionCandidate {
    /// The protocol to use (TCP or UDP)
    pub protocol: TransportProtocol,
    /// The port number to connect to
    pub port: u16,
    /// The protocol style to test
    pub protocol_style: ProtocolStyle,
    /// Buffer configuration optimized for this candidate
    pub buffer_config: BufferConfig,
}

/// Transport protocol for detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportProtocol {
    /// TCP transport
    Tcp,
    /// UDP transport
    Udp,
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

    /// Generate detection candidates based on the provided address
    ///
    /// If a port is specified in the address, it will be used to filter candidates.
    /// Otherwise, all default ports will be tried.
    pub fn generate_candidates(address: &str) -> Vec<DetectionCandidate> {
        // Parse address to see if a port is specified
        let port_specified = if let Some(colon_pos) = address.rfind(':') {
            address[colon_pos + 1..].parse::<u16>().ok()
        } else {
            None
        };

        let mut candidates = Vec::new();

        if let Some(port) = port_specified {
            // User specified a port - try both protocol styles on that port
            if port == 52381 {
                // Sony default port - prioritize Sony encapsulated
                candidates.push(DetectionCandidate {
                    protocol: TransportProtocol::Udp,
                    port,
                    protocol_style: ProtocolStyle::SonyEncapsulated,
                    buffer_config: BufferConfig::for_sony_ip(),
                });
                candidates.push(DetectionCandidate {
                    protocol: TransportProtocol::Tcp,
                    port,
                    protocol_style: ProtocolStyle::SonyEncapsulated,
                    buffer_config: BufferConfig::for_sony_ip(),
                });
                // Also try raw VISCA as fallback
                candidates.push(DetectionCandidate {
                    protocol: TransportProtocol::Udp,
                    port,
                    protocol_style: ProtocolStyle::RawVisca,
                    buffer_config: BufferConfig::for_udp(),
                });
                candidates.push(DetectionCandidate {
                    protocol: TransportProtocol::Tcp,
                    port,
                    protocol_style: ProtocolStyle::RawVisca,
                    buffer_config: BufferConfig::for_raw_ip(),
                });
            } else if port == 1259 || port == 5678 {
                // PTZOptics default ports - prioritize raw VISCA
                let primary_protocol = if port == 1259 {
                    TransportProtocol::Udp
                } else {
                    TransportProtocol::Tcp
                };
                candidates.push(DetectionCandidate {
                    protocol: primary_protocol,
                    port,
                    protocol_style: ProtocolStyle::RawVisca,
                    buffer_config: if primary_protocol == TransportProtocol::Udp {
                        BufferConfig::for_udp()
                    } else {
                        BufferConfig::for_raw_ip()
                    },
                });
                // Also try Sony encapsulated as fallback
                candidates.push(DetectionCandidate {
                    protocol: primary_protocol,
                    port,
                    protocol_style: ProtocolStyle::SonyEncapsulated,
                    buffer_config: BufferConfig::for_sony_ip(),
                });
            } else {
                // Unknown port - try all combinations
                for &protocol in &[TransportProtocol::Udp, TransportProtocol::Tcp] {
                    candidates.push(DetectionCandidate {
                        protocol,
                        port,
                        protocol_style: ProtocolStyle::SonyEncapsulated,
                        buffer_config: BufferConfig::for_sony_ip(),
                    });
                    candidates.push(DetectionCandidate {
                        protocol,
                        port,
                        protocol_style: ProtocolStyle::RawVisca,
                        buffer_config: if protocol == TransportProtocol::Udp {
                            BufferConfig::for_udp()
                        } else {
                            BufferConfig::for_raw_ip()
                        },
                    });
                }
            }
        } else {
            // No port specified - try all known combinations in priority order
            // 1. Sony UDP on 52381 (primary Sony path)
            candidates.push(DetectionCandidate {
                protocol: TransportProtocol::Udp,
                port: 52381,
                protocol_style: ProtocolStyle::SonyEncapsulated,
                buffer_config: BufferConfig::for_sony_ip(),
            });
            // 2. Sony TCP on 52381 (some stacks support TCP)
            candidates.push(DetectionCandidate {
                protocol: TransportProtocol::Tcp,
                port: 52381,
                protocol_style: ProtocolStyle::SonyEncapsulated,
                buffer_config: BufferConfig::for_sony_ip(),
            });
            // 3. Raw UDP on 1259 (PTZOptics default)
            candidates.push(DetectionCandidate {
                protocol: TransportProtocol::Udp,
                port: 1259,
                protocol_style: ProtocolStyle::RawVisca,
                buffer_config: BufferConfig::for_udp(),
            });
            // 4. Raw TCP on 5678 (PTZOptics TCP)
            candidates.push(DetectionCandidate {
                protocol: TransportProtocol::Tcp,
                port: 5678,
                protocol_style: ProtocolStyle::RawVisca,
                buffer_config: BufferConfig::for_raw_ip(),
            });
        }

        candidates
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
                ProtocolStyle::SonyEncapsulated,
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

        // Send command with retries
        for attempt in 0..=self.retry_config.max_retries {
            // Frame the command inside the retry loop to ensure sequence number advances
            // This is critical for Sony encapsulated protocol which requires unique sequence
            // numbers for each retry attempt to avoid duplicate/abnormal sequence handling
            let framed_command = envelope.frame_bytes_with_kind(
                command,
                crate::command::CommandKind::Inquiry,
                &buffer_manager,
            );

            debug!(
                "Sending {len} bytes for protocol detection (attempt {attempt}): {bytes:02X?}",
                len = framed_command.len(),
                attempt = attempt + 1,
                bytes = &framed_command[..std::cmp::min(framed_command.len(), 16)]
            );

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

            // Create a local ProtocolFramer to handle chunked responses
            let mut framer = ProtocolFramer::new_with_config(BufferConfig::default());
            let start_time = std::time::Instant::now();

            // Loop to collect chunks until we get a frame or timeout
            loop {
                // Check if we've exceeded total detection timeout
                if start_time.elapsed() > DETECTION_TIMEOUT {
                    debug!(
                        "Detection timeout exceeded (attempt {attempt})",
                        attempt = attempt + 1
                    );
                    break;
                }

                // Calculate remaining timeout for this recv call
                let remaining = DETECTION_TIMEOUT.saturating_sub(start_time.elapsed());
                let recv_timeout = std::cmp::min(remaining, Duration::from_millis(20));

                // Try to receive a chunk with timeout using futures_lite::or
                use futures_lite::future;

                let outcome = future::or(async { Ok::<_, ()>(transport.recv().await) }, async {
                    executor.sleep(recv_timeout).await;
                    Err::<_, ()>(())
                })
                .await;

                match outcome {
                    Ok(Ok(chunk)) => {
                        debug!(
                            "Received {len} bytes chunk: {bytes:02X?}",
                            len = chunk.len(),
                            bytes = &chunk[..std::cmp::min(chunk.len(), 16)]
                        );

                        // Push chunk to framer
                        if let Err(e) = framer.push(chunk) {
                            debug!("Framer buffer exceeded limits: {e}");
                            return Ok(false);
                        }

                        // Try to extract a complete frame
                        if let Some(frame_result) = framer.drain_frames().next() {
                            let frame = match frame_result {
                                Ok(frame) => frame,
                                Err(e) => {
                                    debug!("Failed to extract frame: {e}");
                                    return Ok(false);
                                }
                            };
                            debug!(
                                "Extracted complete frame: {bytes:02X?}",
                                bytes = &frame[..std::cmp::min(frame.len(), 16)]
                            );

                            // Try to extract VISCA payload
                            match envelope.extract_response(&frame) {
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
                            // Got a frame but it wasn't valid, break to retry
                            break;
                        }
                    }
                    Ok(Err(e)) => {
                        debug!("Transport error during detection: {e}");
                        break;
                    }
                    Err(()) => {
                        // Short recv timeout, continue to check total timeout
                        continue;
                    }
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
    pub fn is_valid_visca_response(&self, payload: &[u8]) -> bool {
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
#[allow(clippy::panic, clippy::assertions_on_constants, clippy::expect_used)]
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
            Some(ProtocolStyle::SonyEncapsulated)
        );

        assert_eq!(
            DetectionResult::RawVisca.to_protocol_style(),
            Some(ProtocolStyle::RawVisca)
        );

        assert_eq!(DetectionResult::NoResponse.to_protocol_style(), None);
    }

    #[tokio::test]
    async fn test_sony_retry_sequencing() {
        use crate::testing::testkit::scripted_transport::{DynamicResponseFn, Step};
        use std::sync::{Arc, Mutex};

        // Track sequence numbers seen
        let sequences_seen = Arc::new(Mutex::new(Vec::new()));
        let sequences_clone = sequences_seen.clone();

        // Create a dynamic response that only responds if sequence number changes
        let dynamic_response: DynamicResponseFn = Box::new(move |sent_bytes| {
            // Sony encapsulated command should have at least 8 byte header
            if sent_bytes.len() >= 8 {
                // Extract sequence number from bytes 4-7 (big-endian u32)
                let sequence = u32::from_be_bytes([
                    sent_bytes[4],
                    sent_bytes[5],
                    sent_bytes[6],
                    sent_bytes[7],
                ]);

                let mut seqs = sequences_clone.lock().expect("Failed to lock sequences");

                // Only respond if this is a new sequence number
                if !seqs.contains(&sequence) {
                    seqs.push(sequence);
                    // Return valid Sony encapsulated response
                    vec![vec![
                        0x01,
                        0x11,
                        0x00,
                        0x06, // Sony header
                        0x00,
                        0x00,
                        0x00,
                        sequence as u8, // Echo back sequence
                        0x90,
                        0x50,
                        0x01,
                        0x02,
                        0x03,
                        VISCA_TERMINATOR, // Version response
                    ]]
                } else {
                    // Duplicate sequence - no response (simulating camera rejecting duplicate)
                    vec![]
                }
            } else {
                vec![]
            }
        });

        let steps = vec![Step::DynamicResponse(dynamic_response)];
        let mut transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(steps);

        let detector = ProtocolDetector::new();
        let executor = match TokioExecutor::from_current() {
            Ok(executor) => executor,
            Err(_) => panic!("Test requires Tokio runtime"),
        };

        // Run detection - should succeed because framing is now inside retry loop
        let result = detector.detect_protocol(&mut transport, &executor).await;
        match result {
            Ok(detection) => {
                assert_eq!(detection, DetectionResult::SonyEncapsulated);

                // Verify that multiple unique sequences were used
                let final_sequences = sequences_seen.lock().expect("Failed to lock sequences");
                assert!(
                    !final_sequences.is_empty(),
                    "Expected at least one unique sequence number, got 0"
                );

                // Verify sequences are incrementing
                for i in 1..final_sequences.len() {
                    assert!(
                        final_sequences[i] > final_sequences[i - 1],
                        "Sequences should increment: {:?}",
                        *final_sequences
                    );
                }
            }
            Err(e) => panic!("Detection should succeed with unique sequences but failed: {e}"),
        }
    }

    #[test]
    fn test_generate_candidates_no_port() {
        let candidates = ProtocolDetector::generate_candidates("192.168.0.110");

        // Should have 4 candidates when no port specified
        assert_eq!(candidates.len(), 4);

        // Check priority order
        assert_eq!(candidates[0].protocol, TransportProtocol::Udp);
        assert_eq!(candidates[0].port, 52381);
        assert!(matches!(
            candidates[0].protocol_style,
            ProtocolStyle::SonyEncapsulated
        ));

        assert_eq!(candidates[1].protocol, TransportProtocol::Tcp);
        assert_eq!(candidates[1].port, 52381);

        assert_eq!(candidates[2].protocol, TransportProtocol::Udp);
        assert_eq!(candidates[2].port, 1259);
        assert!(matches!(
            candidates[2].protocol_style,
            ProtocolStyle::RawVisca
        ));

        assert_eq!(candidates[3].protocol, TransportProtocol::Tcp);
        assert_eq!(candidates[3].port, 5678);
    }

    #[test]
    fn test_generate_candidates_with_sony_port() {
        let candidates = ProtocolDetector::generate_candidates("192.168.0.110:52381");

        // Should prioritize Sony encapsulated for port 52381
        assert!(candidates.len() >= 2);
        assert_eq!(candidates[0].port, 52381);
        assert!(matches!(
            candidates[0].protocol_style,
            ProtocolStyle::SonyEncapsulated
        ));
    }

    #[test]
    fn test_generate_candidates_with_ptz_port() {
        let candidates = ProtocolDetector::generate_candidates("192.168.0.110:1259");

        // Should prioritize raw VISCA for PTZOptics port
        assert!(candidates.len() >= 2);
        assert_eq!(candidates[0].port, 1259);
        assert_eq!(candidates[0].protocol, TransportProtocol::Udp);
        assert!(matches!(
            candidates[0].protocol_style,
            ProtocolStyle::RawVisca
        ));
    }
}
