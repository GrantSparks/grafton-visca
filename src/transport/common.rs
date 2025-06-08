//! Common utilities for transport implementations.
//!
//! This module contains shared code used by multiple transport implementations
//! to reduce duplication.

use crate::ViscaError;
use log::debug;

/// Buffer management utilities for VISCA transports.
#[derive(Debug)]
pub struct BufferManager {
    buffer: Vec<u8>,
}

impl BufferManager {
    /// Create a new buffer manager with the specified capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
        }
    }

    /// Get a mutable reference to the buffer.
    #[must_use]
    pub fn buffer_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buffer
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Extract complete VISCA frames from the buffer.
    ///
    /// VISCA frames start with 0x90 and end with 0xFF.
    pub fn extract_frames(&mut self) -> Vec<Vec<u8>> {
        let mut frames = Vec::new();
        let mut start = 0;

        while let Some(frame_start) = self.buffer[start..].iter().position(|&b| b == 0x90) {
            let frame_start = start + frame_start;

            if let Some(frame_end) = self.buffer[frame_start..].iter().position(|&b| b == 0xFF) {
                let frame_end = frame_start + frame_end + 1;
                frames.push(self.buffer[frame_start..frame_end].to_vec());
                start = frame_end;
            } else {
                // Incomplete frame, keep it in buffer
                let _ = self.buffer.drain(..frame_start);
                break;
            }
        }

        // Remove processed data
        if start > 0 {
            let _ = self.buffer.drain(..start);
        }

        frames
    }
}

/// Parse a VISCA response frame to determine its type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    /// ACK response (0x90 0x4Y 0xFF)
    Ack {
        /// Socket number (0 or 1)
        socket: u8,
    },
    /// Completion response (0x90 0x5Y 0xFF)
    Completion {
        /// Socket number (0 or 1)
        socket: u8,
    },
    /// Error response (0x90 0x6Y 0xZZ 0xFF)
    Error {
        /// Socket number (0 or 1)
        socket: u8,
        /// VISCA error code
        error_code: u8,
    },
    /// Inquiry response (0x90 0x50 ... 0xFF)
    Inquiry,
    /// Unknown response type
    Unknown,
}

/// Parse a VISCA response frame to determine its type.
#[must_use]
pub const fn parse_frame_type(frame: &[u8]) -> FrameType {
    if frame.len() < 3 || frame[0] != 0x90 || frame[frame.len() - 1] != 0xFF {
        return FrameType::Unknown;
    }

    match frame[1] >> 4 {
        0x4 => FrameType::Ack {
            socket: frame[1] & 0x0F,
        },
        0x5 => {
            if frame[1] == 0x50 && frame.len() > 3 {
                FrameType::Inquiry
            } else {
                FrameType::Completion {
                    socket: frame[1] & 0x0F,
                }
            }
        }
        0x6 if frame.len() >= 4 => FrameType::Error {
            socket: frame[1] & 0x0F,
            error_code: frame[2],
        },
        _ => FrameType::Unknown,
    }
}

/// Convert a VISCA error code to a `ViscaError`.
#[must_use]
pub const fn error_code_to_error(error_code: u8) -> ViscaError {
    ViscaError::from_code(error_code)
}

/// Log frame data in a readable format.
pub fn log_frame(prefix: &str, frame: &[u8]) {
    debug!(
        "{}: {:02X?} ({})",
        prefix,
        frame,
        format_frame_description(frame)
    );
}

/// Format a frame description for logging.
fn format_frame_description(frame: &[u8]) -> String {
    match parse_frame_type(frame) {
        FrameType::Ack { socket } => format!("ACK on socket {socket}"),
        FrameType::Completion { socket } => format!("Completion on socket {socket}"),
        FrameType::Error { socket, error_code } => {
            format!(
                "Error on socket {socket}: {:?}",
                error_code_to_error(error_code)
            )
        }
        FrameType::Inquiry => "Inquiry response".to_string(),
        FrameType::Unknown => "Unknown frame type".to_string(),
    }
}

/// Health check utilities for transports.
pub mod health_check {
    use crate::command::ViscaCommand;
    use crate::ViscaError;

    /// A simple health check command that queries camera power status.
    #[derive(Debug, Clone, Copy)]
    pub struct HealthCheckCommand;

    impl ViscaCommand for HealthCheckCommand {
        fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
            // Power inquiry command
            Ok(vec![0x81, 0x09, 0x04, 0x00, 0xFF])
        }

        fn response_type(&self) -> Option<crate::command::ViscaResponseType> {
            Some(crate::command::ViscaResponseType::Power)
        }

        fn command_category(&self) -> crate::timeout::CommandCategory {
            crate::timeout::CommandCategory::Quick
        }
    }

    /// Create a power inquiry command for health checks.
    #[must_use]
    pub fn create_health_check_command() -> impl ViscaCommand {
        HealthCheckCommand
    }

    /// Validate a health check response.
    ///
    /// # Errors
    /// Returns `ViscaError` if the frames cannot be parsed.
    pub fn validate_health_check_response(frames: &[Vec<u8>]) -> Result<bool, ViscaError> {
        // Look for a valid power inquiry response
        for frame in frames {
            if frame.len() >= 4 && frame[0] == 0x90 && frame[1] == 0x50 {
                // This is an inquiry response, camera is healthy
                return Ok(true);
            }
        }

        // No valid response found
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_manager() {
        let mut buffer = BufferManager::new(256);

        // Add some partial data
        buffer.buffer_mut().extend_from_slice(&[0x12, 0x34]);
        assert_eq!(buffer.extract_frames().len(), 0);

        // Add a complete frame
        buffer.buffer_mut().extend_from_slice(&[0x90, 0x41, 0xFF]);
        let frames = buffer.extract_frames();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], vec![0x90, 0x41, 0xFF]);

        // Buffer should be empty now
        assert!(buffer.buffer_mut().is_empty());

        // Add multiple frames
        buffer
            .buffer_mut()
            .extend_from_slice(&[0x90, 0x50, 0xFF, 0x90, 0x60, 0x01, 0xFF]);
        let frames = buffer.extract_frames();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0], vec![0x90, 0x50, 0xFF]);
        assert_eq!(frames[1], vec![0x90, 0x60, 0x01, 0xFF]);

        // Add incomplete frame
        buffer.buffer_mut().extend_from_slice(&[0x90, 0x41]);
        assert_eq!(buffer.extract_frames().len(), 0);
        assert_eq!(buffer.buffer_mut(), &[0x90, 0x41]);
    }

    #[test]
    fn test_parse_frame_type() {
        assert_eq!(
            parse_frame_type(&[0x90, 0x41, 0xFF]),
            FrameType::Ack { socket: 1 }
        );

        assert_eq!(
            parse_frame_type(&[0x90, 0x51, 0xFF]),
            FrameType::Completion { socket: 1 }
        );

        assert_eq!(
            parse_frame_type(&[0x90, 0x61, 0x02, 0xFF]),
            FrameType::Error {
                socket: 1,
                error_code: 0x02
            }
        );

        assert_eq!(
            parse_frame_type(&[0x90, 0x50, 0x02, 0xFF]),
            FrameType::Inquiry
        );

        assert_eq!(parse_frame_type(&[0x91, 0x41, 0xFF]), FrameType::Unknown);
    }

    #[test]
    fn test_error_code_conversion() {
        assert!(matches!(
            error_code_to_error(0x01),
            ViscaError::Unknown(0x01)
        ));
        assert!(matches!(error_code_to_error(0x02), ViscaError::SyntaxError));
        assert!(matches!(
            error_code_to_error(0x03),
            ViscaError::CommandBufferFull
        ));
        assert!(matches!(
            error_code_to_error(0x99),
            ViscaError::Unknown(0x99)
        ));
    }
}
