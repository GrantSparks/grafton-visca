//! VISCA protocol encoding utilities.
//!
//! This module provides functions for encoding VISCA commands and handling
//! different transport encapsulation formats (raw, Sony header).

// External crates
use bytes::{BufMut, BytesMut};

// Standard library
use std::sync::atomic::{AtomicU32, Ordering};

/// VISCA frame terminator byte.
pub const VISCA_TERMINATOR: u8 = 0xFF;

/// Sony encapsulated header payload types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadType {
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
#[derive(Debug, Clone, Copy)]
pub struct SonyHeader {
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
    pub fn new_command(payload_len: usize, sequence: u32) -> Self {
        Self {
            payload_type: PayloadType::ViscaCommand,
            payload_length: payload_len as u16,
            sequence_number: sequence,
        }
    }

    /// Create a new inquiry header with the given sequence.
    pub fn new_inquiry(payload_len: usize, sequence: u32) -> Self {
        Self {
            payload_type: PayloadType::ViscaInquiry,
            payload_length: payload_len as u16,
            sequence_number: sequence,
        }
    }

    /// Create a new reply header with the given sequence.
    pub fn new_reply(payload_len: usize, sequence: u32) -> Self {
        Self {
            payload_type: PayloadType::ViscaReply,
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

/// Sequence number generator for Sony encapsulated mode.
#[derive(Debug)]
pub struct SequenceGenerator {
    next: AtomicU32,
}

impl SequenceGenerator {
    /// Create a new sequence generator starting at 1.
    pub fn new() -> Self {
        Self {
            next: AtomicU32::new(1),
        }
    }

    /// Get the next sequence number.
    pub fn next(&self) -> u32 {
        self.next.fetch_add(1, Ordering::SeqCst)
    }
}

impl Default for SequenceGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Encode a VISCA command frame.
///
/// Ensures the frame is properly terminated with 0xFF.
pub fn encode_frame(command: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(command.len() + 1);
    frame.extend_from_slice(command);

    // Add terminator if not present
    if !command.ends_with(&[VISCA_TERMINATOR]) {
        frame.push(VISCA_TERMINATOR);
    }

    frame
}

/// Encode a VISCA command with Sony header.
pub fn encode_sony_frame(command: &[u8], sequence: u32) -> Vec<u8> {
    let visca_frame = encode_frame(command);
    let header = SonyHeader::new_command(visca_frame.len(), sequence);

    let mut frame = Vec::with_capacity(SonyHeader::SIZE + visca_frame.len());
    frame.extend_from_slice(&header.encode());
    frame.extend_from_slice(&visca_frame);

    frame
}

/// Builder for VISCA commands.
#[derive(Debug)]
pub struct CommandBuilder {
    buffer: BytesMut,
}

impl CommandBuilder {
    /// Create a new command builder.
    pub fn new() -> Self {
        Self {
            buffer: BytesMut::with_capacity(16),
        }
    }

    /// Add the device address byte (usually 0x81 for device 1).
    pub fn device(mut self, device_id: u8) -> Self {
        self.buffer.put_u8(0x80 | (device_id & 0x0F));
        self
    }

    /// Add a command byte.
    pub fn byte(mut self, byte: u8) -> Self {
        self.buffer.put_u8(byte);
        self
    }

    /// Add multiple bytes.
    pub fn bytes(mut self, bytes: &[u8]) -> Self {
        self.buffer.extend_from_slice(bytes);
        self
    }

    /// Add a nibble-encoded value (0x0p 0x0q for value pq).
    pub fn nibbles(mut self, value: u8) -> Self {
        self.buffer.put_u8(value >> 4);
        self.buffer.put_u8(value & 0x0F);
        self
    }

    /// Add a 4-nibble encoded value (0x0p 0x0q 0x0r 0x0s for value pqrs).
    pub fn nibbles_u16(mut self, value: u16) -> Self {
        self.buffer.put_u8((value >> 12) as u8);
        self.buffer.put_u8((value >> 8) as u8 & 0x0F);
        self.buffer.put_u8((value >> 4) as u8 & 0x0F);
        self.buffer.put_u8(value as u8 & 0x0F);
        self
    }

    /// Build the final command with terminator.
    pub fn build(mut self) -> Vec<u8> {
        if !self.buffer.ends_with(&[VISCA_TERMINATOR]) {
            self.buffer.put_u8(VISCA_TERMINATOR);
        }
        self.buffer.to_vec()
    }
}

impl Default for CommandBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Cancel command bytes for each socket.
pub fn encode_cancel(socket: u8) -> Vec<u8> {
    vec![0x81, socket | 0x20, VISCA_TERMINATOR]
}

/// Interface clear command (serial only).
pub fn encode_if_clear() -> Vec<u8> {
    vec![0x88, 0x01, 0x00, 0x01, VISCA_TERMINATOR]
}

/// Address set command (serial only).
pub fn encode_address_set() -> Vec<u8> {
    vec![0x88, 0x30, 0x01, VISCA_TERMINATOR]
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

    #[test]
    fn test_sequence_generator() {
        let gen = SequenceGenerator::new();
        assert_eq!(gen.next(), 1);
        assert_eq!(gen.next(), 2);
        assert_eq!(gen.next(), 3);
    }

    #[test]
    fn test_encode_frame() {
        // Without terminator
        let cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02];
        let frame = encode_frame(&cmd);
        assert_eq!(frame, vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]);

        // With terminator already
        let cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        let frame = encode_frame(&cmd);
        assert_eq!(frame, vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]);
    }

    #[test]
    fn test_command_builder() {
        let cmd = CommandBuilder::new()
            .device(1)
            .byte(0x01)
            .byte(0x04)
            .byte(0x00)
            .byte(0x02)
            .build();

        assert_eq!(cmd, vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]);

        // Test nibbles
        let cmd = CommandBuilder::new()
            .device(1)
            .byte(0x01)
            .nibbles(0x23)
            .build();

        assert_eq!(cmd, vec![0x81, 0x01, 0x02, 0x03, VISCA_TERMINATOR]);

        // Test nibbles_u16
        let cmd = CommandBuilder::new().device(1).nibbles_u16(0x1234).build();

        assert_eq!(cmd, vec![0x81, 0x01, 0x02, 0x03, 0x04, VISCA_TERMINATOR]);
    }

    #[test]
    fn test_special_commands() {
        assert_eq!(encode_cancel(1), vec![0x81, 0x21, VISCA_TERMINATOR]);
        assert_eq!(encode_cancel(2), vec![0x81, 0x22, VISCA_TERMINATOR]);
        assert_eq!(
            encode_if_clear(),
            vec![0x88, 0x01, 0x00, 0x01, VISCA_TERMINATOR]
        );
        assert_eq!(
            encode_address_set(),
            vec![0x88, 0x30, 0x01, VISCA_TERMINATOR]
        );
    }
}
