//! Sony-specific protocol encapsulation for VISCA over IP.
//!
//! This module provides the Sony header format used for encapsulating
//! VISCA commands when communicating over Sony's IP protocol.

/// Sony encapsulated header payload types.
///
/// Only compiled when blocking Sony IP transport is available.
#[cfg(any(not(feature = "async"), test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PayloadType {
    /// VISCA command payload (0x01 0x00).
    ViscaCommand,
    /// VISCA inquiry payload (0x01 0x10).
    ViscaInquiry,
    /// VISCA reply payload (0x01 0x11).
    ViscaReply,
    /// VISCA device setting command (0x01 0x02).
    ViscaDeviceSetting,
    /// Control command (0x01 0x20).
    ControlCommand,
    /// Control reply (0x01 0x21).
    ControlReply,
}

/// Sony encapsulated header for VISCA over IP.
///
/// Only compiled when blocking Sony IP transport is available.
#[cfg(any(not(feature = "async"), test))]
#[derive(Debug, Clone, Copy)]
pub(crate) struct SonyHeader {
    /// Payload type.
    pub payload_type: PayloadType,
    /// Payload length (excluding header).
    pub payload_length: u16,
    /// Sequence number for matching requests/responses.
    pub sequence_number: u32,
}

#[cfg(any(not(feature = "async"), test))]
impl SonyHeader {
    /// Header size in bytes.
    pub const SIZE: usize = 8;

    /// Create a new command header with the given sequence.
    pub fn new_command(payload_len: usize, sequence: u32) -> Self {
        Self {
            payload_type: PayloadType::ViscaCommand,
            payload_length: payload_len as u16,
            sequence_number: sequence,
        }
    }

    /// Create a new inquiry header with the given sequence.
    #[cfg(not(feature = "async"))] // Only used by blocking Sony IP transport
    pub fn new_inquiry(payload_len: usize, sequence: u32) -> Self {
        Self {
            payload_type: PayloadType::ViscaInquiry,
            payload_length: payload_len as u16,
            sequence_number: sequence,
        }
    }

    /// Encode header to bytes.
    pub fn encode(&self) -> [u8; 8] {
        let mut header = [0u8; 8];
        // Encode payload type as two bytes to match Sony spec
        match self.payload_type {
            PayloadType::ViscaCommand => {
                header[0] = 0x01;
                header[1] = 0x00;
            }
            PayloadType::ViscaInquiry => {
                header[0] = 0x01;
                header[1] = 0x10;
            }
            PayloadType::ViscaReply => {
                header[0] = 0x01;
                header[1] = 0x11;
            }
            PayloadType::ViscaDeviceSetting => {
                header[0] = 0x01;
                header[1] = 0x02;
            }
            PayloadType::ControlCommand => {
                header[0] = 0x01;
                header[1] = 0x20;
            }
            PayloadType::ControlReply => {
                header[0] = 0x01;
                header[1] = 0x21;
            }
        }
        header[2..4].copy_from_slice(&self.payload_length.to_be_bytes());
        header[4..8].copy_from_slice(&self.sequence_number.to_be_bytes());
        header
    }

    /// Decode header from bytes.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE {
            return None;
        }

        // Decode two-byte payload type
        let payload_type = match (bytes[0], bytes[1]) {
            (0x01, 0x00) => PayloadType::ViscaCommand,
            (0x01, 0x10) => PayloadType::ViscaInquiry,
            (0x01, 0x11) => PayloadType::ViscaReply,
            (0x01, 0x02) => PayloadType::ViscaDeviceSetting,
            (0x01, 0x20) => PayloadType::ControlCommand,
            (0x01, 0x21) => PayloadType::ControlReply,
            _ => return None,
        };

        let payload_length = u16::from_be_bytes([bytes[2], bytes[3]]);
        let sequence_number = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);

        Some(Self {
            payload_type,
            payload_length,
            sequence_number,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_sony_header_encode_decode() {
        let header = SonyHeader::new_command(10, 12345);
        let encoded = header.encode();

        assert_eq!(encoded[0], 0x01); // PayloadType::ViscaCommand
        assert_eq!(encoded[1], 0x00); // Reserved
        assert_eq!(u16::from_be_bytes([encoded[2], encoded[3]]), 10);
        assert_eq!(
            u32::from_be_bytes([encoded[4], encoded[5], encoded[6], encoded[7]]),
            12345
        );

        let decoded = SonyHeader::decode(&encoded).unwrap();
        assert_eq!(decoded.payload_type, PayloadType::ViscaCommand);
        assert_eq!(decoded.payload_length, 10);
        assert_eq!(decoded.sequence_number, 12345);
    }
}
