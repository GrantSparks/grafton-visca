//! Sony-specific protocol encapsulation for VISCA over IP.
//!
//! This module provides the Sony header format used for encapsulating
//! VISCA commands when communicating over Sony's IP protocol.

/// Sony encapsulated header payload types.
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

impl PayloadType {
    /// Convert payload type to its byte representation.
    #[inline]
    pub fn to_bytes(self) -> [u8; 2] {
        match self {
            PayloadType::ViscaCommand => [0x01, 0x00],
            PayloadType::ViscaInquiry => [0x01, 0x10],
            PayloadType::ViscaReply => [0x01, 0x11],
            PayloadType::ViscaDeviceSetting => [0x01, 0x02],
            PayloadType::ControlCommand => [0x01, 0x20],
            PayloadType::ControlReply => [0x01, 0x21],
        }
    }

    /// Parse payload type from bytes.
    #[inline]
    pub fn from_bytes(bytes: [u8; 2]) -> Option<Self> {
        match bytes {
            [0x01, 0x00] => Some(PayloadType::ViscaCommand),
            [0x01, 0x10] => Some(PayloadType::ViscaInquiry),
            [0x01, 0x11] => Some(PayloadType::ViscaReply),
            [0x01, 0x02] => Some(PayloadType::ViscaDeviceSetting),
            [0x01, 0x20] => Some(PayloadType::ControlCommand),
            [0x01, 0x21] => Some(PayloadType::ControlReply),
            _ => None,
        }
    }
}

/// Sony encapsulated header for VISCA over IP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SonyHeader {
    /// Payload type.
    pub payload_type: PayloadType,
    /// Payload length (excluding header).
    pub payload_length: u16,
    /// Sequence number for matching requests/responses.
    pub sequence_number: u32,
}

impl SonyHeader {
    /// Header size in bytes.
    pub const SIZE: usize = 8;

    /// Create a new command header with the given sequence.
    pub const fn new_command(payload_len: usize, sequence: u32) -> Self {
        Self {
            payload_type: PayloadType::ViscaCommand,
            payload_length: payload_len as u16,
            sequence_number: sequence,
        }
    }

    /// Create a new inquiry header with the given sequence.
    pub const fn new_inquiry(payload_len: usize, sequence: u32) -> Self {
        Self {
            payload_type: PayloadType::ViscaInquiry,
            payload_length: payload_len as u16,
            sequence_number: sequence,
        }
    }

    /// Encode header to bytes.
    #[inline(always)]
    pub fn encode(&self) -> [u8; 8] {
        let mut header = [0u8; 8];
        // Use the new to_bytes method for payload type
        let type_bytes = self.payload_type.to_bytes();
        header[0] = type_bytes[0];
        header[1] = type_bytes[1];
        header[2..4].copy_from_slice(&self.payload_length.to_be_bytes());
        header[4..8].copy_from_slice(&self.sequence_number.to_be_bytes());
        header
    }

    /// Decode header from bytes.
    #[inline]
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE {
            return None;
        }

        // Use the new from_bytes method for payload type
        let payload_type = PayloadType::from_bytes([bytes[0], bytes[1]])?;

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
        assert_eq!(encoded[1], 0x00); // Command subtype
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

    #[test]
    fn test_all_payload_types_round_trip() {
        let test_cases = vec![
            (PayloadType::ViscaCommand, [0x01, 0x00]),
            (PayloadType::ViscaInquiry, [0x01, 0x10]),
            (PayloadType::ViscaReply, [0x01, 0x11]),
            (PayloadType::ViscaDeviceSetting, [0x01, 0x02]),
            (PayloadType::ControlCommand, [0x01, 0x20]),
            (PayloadType::ControlReply, [0x01, 0x21]),
        ];

        for (payload_type, expected_bytes) in test_cases {
            // Test to_bytes
            let bytes = payload_type.to_bytes();
            assert_eq!(
                bytes, expected_bytes,
                "to_bytes failed for {:?}",
                payload_type
            );

            // Test from_bytes
            let parsed = PayloadType::from_bytes(bytes).unwrap();
            assert_eq!(
                parsed, payload_type,
                "from_bytes failed for {:?}",
                payload_type
            );

            // Test in header context
            let header = SonyHeader {
                payload_type,
                payload_length: 100,
                sequence_number: 999,
            };
            let encoded = header.encode();
            assert_eq!(&encoded[0..2], &expected_bytes);

            let decoded = SonyHeader::decode(&encoded).unwrap();
            assert_eq!(decoded.payload_type, payload_type);
            assert_eq!(decoded.payload_length, 100);
            assert_eq!(decoded.sequence_number, 999);
        }
    }

    #[test]
    fn test_invalid_payload_types() {
        let invalid_types = [
            [0x00, 0x00], // Zero payload type
            [0xFF, 0xFF], // All bits set
            [0x01, 0x01], // Invalid sub-type
            [0x01, 0x12], // Out of range sub-type
            [0x02, 0x00], // Wrong major type
            [0x01, 0xFF], // Invalid sub-type with correct major
            [0x80, 0x80], // High bit set
        ];

        for invalid_type in &invalid_types {
            assert_eq!(
                PayloadType::from_bytes(*invalid_type),
                None,
                "Should reject invalid payload type: {:02X?}",
                invalid_type
            );
        }
    }

    #[test]
    fn test_header_boundary_values() {
        // Test with minimum values
        let min_header = SonyHeader {
            payload_type: PayloadType::ViscaCommand,
            payload_length: 0,
            sequence_number: 0,
        };
        let encoded = min_header.encode();
        let decoded = SonyHeader::decode(&encoded).unwrap();
        assert_eq!(decoded.payload_length, 0);
        assert_eq!(decoded.sequence_number, 0);

        // Test with maximum values
        let max_header = SonyHeader {
            payload_type: PayloadType::ViscaReply,
            payload_length: u16::MAX,
            sequence_number: u32::MAX,
        };
        let encoded = max_header.encode();
        let decoded = SonyHeader::decode(&encoded).unwrap();
        assert_eq!(decoded.payload_length, u16::MAX);
        assert_eq!(decoded.sequence_number, u32::MAX);
    }

    #[test]
    fn test_decode_invalid_buffer_sizes() {
        // Too short buffer
        assert_eq!(SonyHeader::decode(&[]), None);
        assert_eq!(SonyHeader::decode(&[0x01]), None);
        assert_eq!(SonyHeader::decode(&[0x01, 0x00]), None);
        assert_eq!(SonyHeader::decode(&[0x01, 0x00, 0x00]), None);
        assert_eq!(SonyHeader::decode(&[0x01, 0x00, 0x00, 0x0A]), None);
        assert_eq!(SonyHeader::decode(&[0x01, 0x00, 0x00, 0x0A, 0x00]), None);
        assert_eq!(
            SonyHeader::decode(&[0x01, 0x00, 0x00, 0x0A, 0x00, 0x00]),
            None
        );
        assert_eq!(
            SonyHeader::decode(&[0x01, 0x00, 0x00, 0x0A, 0x00, 0x00, 0x00]),
            None
        );

        // Exactly 8 bytes should work
        let valid_8_bytes = [0x01, 0x00, 0x00, 0x0A, 0x00, 0x00, 0x00, 0x01];
        assert!(SonyHeader::decode(&valid_8_bytes).is_some());

        // More than 8 bytes should also work (only first 8 used)
        let valid_9_bytes = [0x01, 0x00, 0x00, 0x0A, 0x00, 0x00, 0x00, 0x01, 0x42];
        assert!(SonyHeader::decode(&valid_9_bytes).is_some());
    }

    #[test]
    fn test_const_constructors() {
        // Test that constructors can be used in const context
        const COMMAND_HEADER: SonyHeader = SonyHeader::new_command(42, 1337);
        assert_eq!(COMMAND_HEADER.payload_type, PayloadType::ViscaCommand);
        assert_eq!(COMMAND_HEADER.payload_length, 42);
        assert_eq!(COMMAND_HEADER.sequence_number, 1337);

        const INQUIRY_HEADER: SonyHeader = SonyHeader::new_inquiry(24, 7331);
        assert_eq!(INQUIRY_HEADER.payload_type, PayloadType::ViscaInquiry);
        assert_eq!(INQUIRY_HEADER.payload_length, 24);
        assert_eq!(INQUIRY_HEADER.sequence_number, 7331);
    }

    #[test]
    fn test_endianness() {
        let header = SonyHeader {
            payload_type: PayloadType::ViscaCommand,
            payload_length: 0x1234,      // Test big-endian encoding
            sequence_number: 0x12345678, // Test big-endian encoding
        };

        let encoded = header.encode();

        // Check length is big-endian
        assert_eq!(encoded[2], 0x12);
        assert_eq!(encoded[3], 0x34);

        // Check sequence is big-endian
        assert_eq!(encoded[4], 0x12);
        assert_eq!(encoded[5], 0x34);
        assert_eq!(encoded[6], 0x56);
        assert_eq!(encoded[7], 0x78);
    }
}
