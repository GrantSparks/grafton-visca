//! Unified sync I/O helpers for blocking transport implementations.
//!
//! This module provides shared functionality across different blocking transports
//! to reduce code duplication while maintaining zero-cost abstractions.

use bytes::Bytes;
use std::io::Read;

use crate::{
    command::bytes::VISCA_TERMINATOR, protocol::sony::SonyHeader, transport::buffer::BufferManager,
    Error,
};

/// Unified helper for reading VISCA frames with protocol-aware deframing (synchronous version).
///
/// This function automatically detects whether the incoming frame is:
/// - Raw VISCA: reads until 0xFF terminator
/// - Sony 52381: reads exactly header + payload_length bytes
///
/// This ensures that Sony frames with 0xFF in the header (e.g., in sequence number)
/// are not prematurely truncated.
pub fn read_visca_frame_sync<R: Read>(
    reader: &mut R,
    buffer_manager: &BufferManager,
) -> Result<Bytes, Error> {
    // Read at least 8 bytes to check for Sony header
    let mut initial_buf = buffer_manager.alloc_vec_buffer();
    initial_buf.clear(); // Ensure it's empty

    // First, try to read enough bytes to check for Sony header
    // We need to handle the case where 0xFF appears in the header
    while initial_buf.len() < SonyHeader::SIZE {
        let mut temp_byte = [0u8; 1];
        let n = reader.read(&mut temp_byte).map_err(Error::Io)?;

        if n == 0 {
            if initial_buf.is_empty() {
                return Err(Error::ConnectionClosed {
                    reason: Some("peer closed connection".into()),
                });
            }
            // Partial read, not enough for Sony header - treat as Raw VISCA
            // Look for terminator in what we have
            if let Some(term_pos) = initial_buf.iter().position(|&b| b == VISCA_TERMINATOR) {
                initial_buf.truncate(term_pos + 1);
            }
            let len = initial_buf.len();
            return Ok(buffer_manager.process_recv_data_borrowed(&mut initial_buf, len));
        }

        initial_buf.push(temp_byte[0]);

        // If we got a terminator before 8 bytes AND it's not a valid Sony header start,
        // it's definitely Raw VISCA
        if initial_buf.len() < SonyHeader::SIZE && temp_byte[0] == VISCA_TERMINATOR {
            // Check if what we have so far could be a Sony header
            if initial_buf.len() >= 2 {
                let could_be_sony = matches!(&initial_buf[0..2], [0x01, _]);
                if !could_be_sony {
                    let len = initial_buf.len();
                    return Ok(buffer_manager.process_recv_data_borrowed(&mut initial_buf, len));
                }
            } else {
                let len = initial_buf.len();
                return Ok(buffer_manager.process_recv_data_borrowed(&mut initial_buf, len));
            }
        }
    }

    // Check if this is a Sony header
    if let Some(header) = SonyHeader::decode(&initial_buf[..SonyHeader::SIZE]) {
        // It's a Sony frame - read the remaining payload based on header length
        let total_needed = SonyHeader::SIZE + header.payload_length as usize;

        // Continue reading until we have the full frame
        while initial_buf.len() < total_needed {
            let mut temp_byte = [0u8; 1];
            let n = reader.read(&mut temp_byte).map_err(Error::Io)?;
            if n == 0 {
                return Err(Error::ConnectionClosed {
                    reason: Some("connection closed during Sony frame read".into()),
                });
            }
            initial_buf.push(temp_byte[0]);
        }

        // Ensure we have exactly the right amount
        initial_buf.truncate(total_needed);
        Ok(buffer_manager.process_recv_data_borrowed(&mut initial_buf, total_needed))
    } else {
        // Not a Sony header - continue reading as Raw VISCA until terminator
        // We may have read past the terminator if checking for Sony header
        if let Some(term_pos) = initial_buf.iter().position(|&b| b == VISCA_TERMINATOR) {
            // Found terminator in what we already read
            initial_buf.truncate(term_pos + 1);
            let final_len = initial_buf.len();
            Ok(buffer_manager.process_recv_data_borrowed(&mut initial_buf, final_len))
        } else {
            // Continue reading until terminator
            loop {
                let mut temp_byte = [0u8; 1];
                let n = reader.read(&mut temp_byte).map_err(Error::Io)?;
                if n == 0 {
                    return Err(Error::ConnectionClosed {
                        reason: Some("peer closed connection".into()),
                    });
                }
                initial_buf.push(temp_byte[0]);
                if temp_byte[0] == VISCA_TERMINATOR {
                    break;
                }
            }
            let final_len = initial_buf.len();
            Ok(buffer_manager.process_recv_data_borrowed(&mut initial_buf, final_len))
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_read_raw_visca_frame_sync() {
        // Simple Raw VISCA command
        let data = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
        let mut reader = Cursor::new(data.clone());
        let buffer_manager = BufferManager::new(crate::transport::buffer::BufferConfig::default());

        let result = read_visca_frame_sync(&mut reader, &buffer_manager).expect("Failed in test");
        assert_eq!(result.as_ref(), &data[..]);
    }

    #[test]
    fn test_read_raw_visca_with_ff_in_data_sync() {
        // Raw VISCA stops at first 0xFF
        let data = vec![0x81, 0x01, VISCA_TERMINATOR, 0x02, VISCA_TERMINATOR];
        let mut reader = Cursor::new(data);
        let buffer_manager = BufferManager::new(crate::transport::buffer::BufferConfig::default());

        let result = read_visca_frame_sync(&mut reader, &buffer_manager).expect("Failed in test");
        assert_eq!(result.as_ref(), &[0x81, 0x01, VISCA_TERMINATOR]);
    }

    #[test]
    fn test_read_sony_frame_basic_sync() {
        // Sony header with 5-byte VISCA payload
        let data = vec![
            0x01, 0x00, // Payload type: ViscaCommand
            0x00, 0x05, // Payload length: 5 bytes
            0x00, 0x00, 0x00, 0x01, // Sequence number: 1
            0x81, 0x01, 0x04, 0x00, 0xFF, // VISCA payload
        ];

        let mut reader = Cursor::new(data.clone());
        let buffer_manager = BufferManager::new(crate::transport::buffer::BufferConfig::default());
        let result = read_visca_frame_sync(&mut reader, &buffer_manager).expect("Failed in test");
        assert_eq!(result.as_ref(), &data[..]);
    }

    #[test]
    fn test_read_sony_frame_with_ff_in_header_sync() {
        // Sony header where sequence number contains 0xFF
        let data = vec![
            0x01, 0x10, // Payload type: ViscaInquiry
            0x00, 0x05, // Payload length: 5 bytes
            0x00, 0xFF, 0x00, 0xFF, // Sequence number with 0xFF bytes
            0x81, 0x09, 0x04, 0x00, 0xFF, // VISCA payload
        ];

        let mut reader = Cursor::new(data.clone());
        let buffer_manager = BufferManager::new(crate::transport::buffer::BufferConfig::default());
        let result = read_visca_frame_sync(&mut reader, &buffer_manager).expect("Failed in test");

        // Should read exactly 8 header + 5 payload = 13 bytes
        assert_eq!(result.len(), 13);
        assert_eq!(result.as_ref(), &data[..]);
    }

    #[test]
    fn test_read_sony_frame_sequence_ff_sync() {
        // Test various sequence numbers with 0xFF
        let test_cases = vec![
            0x0000_00FF_u32, // 0xFF in last byte
            0x00FF_0000_u32, // 0xFF in third byte
            0xFF00_0000_u32, // 0xFF in first byte
            0x00FF_00FF_u32, // Multiple 0xFF
            0xFFFF_FFFF_u32, // All 0xFF
        ];

        let buffer_manager = BufferManager::new(crate::transport::buffer::BufferConfig::default());
        for seq_num in test_cases {
            let mut data = vec![
                0x01, 0x11, // Payload type: ViscaReply
                0x00, 0x03, // Payload length: 3 bytes
            ];
            data.extend_from_slice(&seq_num.to_be_bytes());
            data.extend_from_slice(&[0x90, 0x50, 0xFF]); // Short VISCA payload

            let mut reader = Cursor::new(data.clone());
            let result =
                read_visca_frame_sync(&mut reader, &buffer_manager).expect("Failed in test");

            assert_eq!(result.len(), 11, "Failed for sequence: 0x{:08X}", seq_num);
            assert_eq!(
                result.as_ref(),
                &data[..],
                "Failed for sequence: 0x{:08X}",
                seq_num
            );
        }
    }

    #[test]
    fn test_read_not_sony_header_sync() {
        // Data that looks like it could be Sony but isn't (wrong magic bytes)
        let data = vec![
            0x02, 0x00, // Not a valid Sony payload type
            0x00, 0x05, // Would be payload length
            0x00, 0x00, 0x00, 0x01, // Would be sequence
            0x81, 0x01,
            0xFF, // But actually just Raw VISCA that happens to start with these bytes
        ];

        let mut reader = Cursor::new(data.clone());
        let buffer_manager = BufferManager::new(crate::transport::buffer::BufferConfig::default());
        let result = read_visca_frame_sync(&mut reader, &buffer_manager).expect("Failed in test");

        // Should read as Raw VISCA up to first 0xFF (at position 10)
        assert_eq!(result.len(), 11);
        assert_eq!(result.as_ref(), &data[..11]);
    }

    #[test]
    fn test_connection_closed_sync() {
        let data = vec![];
        let mut reader = Cursor::new(data);
        let buffer_manager = BufferManager::new(crate::transport::buffer::BufferConfig::default());

        let result = read_visca_frame_sync(&mut reader, &buffer_manager);
        assert!(matches!(result, Err(Error::ConnectionClosed { .. })));
    }

    #[test]
    fn test_partial_sony_header_sync() {
        // Only 6 bytes when we need 8 for Sony header, followed by 0xFF
        let data = vec![0x01, 0x00, 0x00, 0x05, 0x00, VISCA_TERMINATOR];
        let mut reader = Cursor::new(data.clone());
        let buffer_manager = BufferManager::new(crate::transport::buffer::BufferConfig::default());

        let result = read_visca_frame_sync(&mut reader, &buffer_manager).expect("Failed in test");
        // Should read as Raw VISCA since we hit 0xFF before getting full header
        assert_eq!(result.as_ref(), &data[..]);
    }
}
