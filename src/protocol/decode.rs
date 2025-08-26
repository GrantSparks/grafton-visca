//! VISCA protocol decoding utilities.
//!
//! This module provides functions for parsing VISCA responses including
//! ACK, Completion, Data Reply, and Error messages.

#[cfg(feature = "async")]
use crate::command::const_encoding::VISCA_TERMINATOR;

#[cfg(feature = "async")]
use tracing::{debug, trace, warn};

#[cfg(feature = "async")]
use crate::runtime::scheduler::{SocketId, ViscaError};

/// Protocol-level response types from VISCA frame parsing.
///
/// This is the internal representation used by the runtime for tracking
/// socket states and protocol-level details. It differs from the public
/// `ViscaResponse` type which provides a simpler API for users.
#[cfg(feature = "async")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProtocolResponse {
    /// Acknowledgment - command accepted (90 4y FF).
    Ack {
        /// Socket that acknowledged (y = 1 or 2).
        socket: SocketId,
    },
    /// Command completion (90 5y FF).
    Completion {
        /// Socket that completed (y = 1 or 2).
        socket: SocketId,
    },
    /// Data reply from inquiry (90 50 ... FF).
    DataReply {
        /// Response data (excluding header and terminator).
        data: Vec<u8>,
    },
    /// Error response (90 6y zz FF).
    Error {
        /// Socket if error is socket-specific.
        socket: Option<SocketId>,
        /// Error code.
        error: ViscaError,
    },
    /// Network change message (90 38 FF).
    NetworkChange,
    /// Unknown response type.
    Unknown {
        /// Raw response bytes.
        data: Vec<u8>,
    },
}

/// Parse a VISCA response frame.
///
/// Takes a complete frame (including terminator) and returns the parsed response.
#[cfg(feature = "async")]
pub(crate) fn parse_response(frame: &[u8]) -> ProtocolResponse {
    trace!("Parsing VISCA response: {:02X?}", frame);

    // Minimum valid response is 3 bytes (e.g., 90 38 FF)
    if frame.len() < 3 {
        warn!("Response too short: {:02X?}", frame);
        return ProtocolResponse::Unknown {
            data: frame.to_vec(),
        };
    }

    // Check for terminator
    if frame[frame.len() - 1] != VISCA_TERMINATOR {
        warn!("Response missing terminator: {:02X?}", frame);
        return ProtocolResponse::Unknown {
            data: frame.to_vec(),
        };
    }

    // Check first byte (should be 9x for replies)
    if (frame[0] & 0xF0) != 0x90 {
        warn!("Invalid response header byte: {:02X}", frame[0]);
        return ProtocolResponse::Unknown {
            data: frame.to_vec(),
        };
    }

    let source_device = frame[0] & 0x0F;
    debug!("Response from device {}", source_device);

    // Parse based on second byte
    match frame[1] {
        // ACK (90 4y FF)
        byte if (byte & 0xF0) == 0x40 => {
            let socket_num = byte & 0x0F;
            match SocketId::from_byte(socket_num) {
                Some(socket) => {
                    debug!("ACK on {:?}", socket);
                    ProtocolResponse::Ack { socket }
                }
                None => {
                    warn!("Invalid socket number in ACK: {}", socket_num);
                    ProtocolResponse::Unknown {
                        data: frame.to_vec(),
                    }
                }
            }
        }

        // Completion (90 5y FF)
        byte if (byte & 0xF0) == 0x50 => {
            let socket_num = byte & 0x0F;

            // Special case: 90 50 is data reply
            if socket_num == 0 {
                if frame.len() > 3 {
                    let data = frame[2..frame.len() - 1].to_vec();
                    debug!("Data reply: {:02X?}", data);
                    ProtocolResponse::DataReply { data }
                } else {
                    debug!("Empty data reply");
                    ProtocolResponse::DataReply { data: vec![] }
                }
            } else {
                match SocketId::from_byte(socket_num) {
                    Some(socket) => {
                        debug!("Completion on {:?}", socket);
                        ProtocolResponse::Completion { socket }
                    }
                    None => {
                        warn!("Invalid socket number in completion: {}", socket_num);
                        ProtocolResponse::Unknown {
                            data: frame.to_vec(),
                        }
                    }
                }
            }
        }

        // Error (90 6y zz FF)
        byte if (byte & 0xF0) == 0x60 => {
            let socket_num = byte & 0x0F;
            let socket = SocketId::from_byte(socket_num);

            if frame.len() >= 4 {
                let error_code = frame[2];
                let error = ViscaError::from_byte(error_code);
                debug!("Error {:?} on socket {:?}", error, socket);
                ProtocolResponse::Error { socket, error }
            } else {
                warn!("Error response too short: {:02X?}", frame);
                ProtocolResponse::Unknown {
                    data: frame.to_vec(),
                }
            }
        }

        // Network change (90 38 FF)
        0x38 => {
            debug!("Network change notification");
            ProtocolResponse::NetworkChange
        }

        // Unknown
        _ => {
            warn!("Unknown response type: {:02X?}", frame);
            ProtocolResponse::Unknown {
                data: frame.to_vec(),
            }
        }
    }
}

/// Extract data value from an inquiry response.
///
/// Many VISCA inquiry responses follow patterns like:
/// - Single byte: 90 50 0p FF (value = p)
/// - Two nibbles: 90 50 0p 0q FF (value = pq)
/// - Four nibbles: 90 50 0p 0q 0r 0s FF (value = pqrs)
#[cfg(all(test, feature = "async"))]
fn extract_inquiry_value(data: &[u8]) -> Option<u32> {
    match data.len() {
        1 => {
            // Single nibble value
            Some((data[0] & 0x0F) as u32)
        }
        2 => {
            // Two nibbles form a byte
            let high = (data[0] & 0x0F) as u32;
            let low = (data[1] & 0x0F) as u32;
            Some((high << 4) | low)
        }
        4 => {
            // Four nibbles form a 16-bit value
            let b3 = (data[0] & 0x0F) as u32;
            let b2 = (data[1] & 0x0F) as u32;
            let b1 = (data[2] & 0x0F) as u32;
            let b0 = (data[3] & 0x0F) as u32;
            Some((b3 << 12) | (b2 << 8) | (b1 << 4) | b0)
        }
        _ => None,
    }
}

/// Find the next complete frame in a buffer.
///
/// Returns the frame and remaining data.
#[cfg(feature = "async")]
pub(crate) fn find_next_frame(buffer: &[u8]) -> Option<(Vec<u8>, &[u8])> {
    if let Some(pos) = buffer.iter().position(|&b| b == VISCA_TERMINATOR) {
        let frame = buffer[..=pos].to_vec();
        let remaining = &buffer[pos + 1..];
        Some((frame, remaining))
    } else {
        None
    }
}

/// Parse multiple frames from a buffer.
///
/// Returns all complete frames found and any remaining incomplete data.
#[cfg(feature = "async")]
pub(crate) fn parse_frames(buffer: &[u8]) -> (Vec<Vec<u8>>, Vec<u8>) {
    let mut frames = Vec::new();
    let mut remaining = buffer;

    while let Some((frame, rest)) = find_next_frame(remaining) {
        frames.push(frame);
        remaining = rest;
    }

    (frames, remaining.to_vec())
}

#[cfg(all(test, feature = "async"))]
mod tests {
    use super::*;
    use crate::protocol::encode::VISCA_TERMINATOR;

    #[test]
    fn test_parse_ack() {
        let frame = vec![0x90, 0x41, VISCA_TERMINATOR];
        let response = parse_response(&frame);
        assert_eq!(
            response,
            ProtocolResponse::Ack {
                socket: SocketId::Socket1
            }
        );

        let frame = vec![0x90, 0x42, VISCA_TERMINATOR];
        let response = parse_response(&frame);
        assert_eq!(
            response,
            ProtocolResponse::Ack {
                socket: SocketId::Socket2
            }
        );
    }

    #[test]
    fn test_parse_completion() {
        let frame = vec![0x90, 0x51, VISCA_TERMINATOR];
        let response = parse_response(&frame);
        assert_eq!(
            response,
            ProtocolResponse::Completion {
                socket: SocketId::Socket1
            }
        );

        let frame = vec![0x90, 0x52, VISCA_TERMINATOR];
        let response = parse_response(&frame);
        assert_eq!(
            response,
            ProtocolResponse::Completion {
                socket: SocketId::Socket2
            }
        );
    }

    #[test]
    fn test_parse_data_reply() {
        let frame = vec![0x90, 0x50, 0x02, VISCA_TERMINATOR];
        let response = parse_response(&frame);
        assert_eq!(response, ProtocolResponse::DataReply { data: vec![0x02] });

        let frame = vec![0x90, 0x50, 0x00, 0x01, 0x02, 0x03, VISCA_TERMINATOR];
        let response = parse_response(&frame);
        assert_eq!(
            response,
            ProtocolResponse::DataReply {
                data: vec![0x00, 0x01, 0x02, 0x03]
            }
        );
    }

    #[test]
    fn test_parse_error() {
        let frame = vec![0x90, 0x60, 0x02, VISCA_TERMINATOR];
        let response = parse_response(&frame);
        assert_eq!(
            response,
            ProtocolResponse::Error {
                socket: None,
                error: ViscaError::from_byte(0x02), // SyntaxError
            }
        );

        let frame = vec![0x90, 0x61, 0x03, VISCA_TERMINATOR];
        let response = parse_response(&frame);
        assert_eq!(
            response,
            ProtocolResponse::Error {
                socket: Some(SocketId::Socket1),
                error: ViscaError::from_byte(0x03), // BufferFull
            }
        );
    }

    #[test]
    fn test_extract_inquiry_value() {
        // Single nibble
        assert_eq!(extract_inquiry_value(&[0x03]), Some(3));

        // Two nibbles (byte)
        assert_eq!(extract_inquiry_value(&[0x01, 0x05]), Some(0x15));

        // Four nibbles (16-bit)
        assert_eq!(
            extract_inquiry_value(&[0x01, 0x02, 0x03, 0x04]),
            Some(0x1234)
        );

        // Invalid length
        assert_eq!(extract_inquiry_value(&[0x01, 0x02, 0x03]), None);
    }

    #[test]
    fn test_find_frames() {
        let buffer = vec![
            0x90,
            0x41,
            VISCA_TERMINATOR,
            0x90,
            0x51,
            VISCA_TERMINATOR,
            0x90,
            0x50,
        ];
        let (frames, remaining) = parse_frames(&buffer);

        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0], vec![0x90, 0x41, VISCA_TERMINATOR]);
        assert_eq!(frames[1], vec![0x90, 0x51, VISCA_TERMINATOR]);
        assert_eq!(remaining, vec![0x90, 0x50]);
    }
}
