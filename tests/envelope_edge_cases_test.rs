//! Comprehensive edge case tests for TransportEnvelope and FrameParser.
//!
//! These tests ensure robust handling of malformed data, edge cases,
//! and protocol violations.

const VISCA_TERMINATOR: u8 = 0xFF;

#[test]
fn test_sony_response_too_short_for_header() {
    use grafton_visca::capabilities::ProtocolStyle;
    use grafton_visca::transport::envelope::TransportEnvelope;

    let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated {
        use_sequence: false,
    });

    let malformed = vec![0x01, 0x11, 0x00, 0x03, 0x00, 0x00, 0x00];
    assert!(
        envelope.extract_response(&malformed).is_err(),
        "Should reject response shorter than Sony header"
    );

    let empty = vec![];
    assert!(
        envelope.extract_response(&empty).is_err(),
        "Should reject empty response"
    );

    let single = vec![0x90];
    assert!(
        envelope.extract_response(&single).is_err(),
        "Should reject single byte response"
    );
}

#[test]
fn test_sony_invalid_payload_types() {
    use grafton_visca::capabilities::ProtocolStyle;
    use grafton_visca::transport::envelope::TransportEnvelope;

    let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated {
        use_sequence: false,
    });

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
        let mut response = Vec::new();
        response.extend_from_slice(invalid_type);
        response.extend_from_slice(&(3u16).to_be_bytes()); // Length
        response.extend_from_slice(&0u32.to_be_bytes()); // Sequence
        response.extend_from_slice(&[0x90, 0x41, VISCA_TERMINATOR]); // Valid VISCA

        let result = envelope.extract_response(&response);
        assert!(
            result.is_err(),
            "Should reject invalid Sony payload type: {:02X?}",
            invalid_type
        );
    }
}

#[test]
fn test_sony_length_field_mismatches() {
    use grafton_visca::capabilities::ProtocolStyle;
    use grafton_visca::transport::envelope::TransportEnvelope;

    let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated {
        use_sequence: false,
    });

    let mut response = Vec::new();
    response.extend_from_slice(&[0x01, 0x11]); // Reply type
    response.extend_from_slice(&(10u16).to_be_bytes()); // Wrong length
    response.extend_from_slice(&0u32.to_be_bytes()); // Sequence
    response.extend_from_slice(&[0x90, 0x41, VISCA_TERMINATOR]); // 3 bytes payload

    assert!(
        envelope.extract_response(&response).is_err(),
        "Should reject length field mismatch (claims 10, has 3)"
    );

    let mut response = Vec::new();
    response.extend_from_slice(&[0x01, 0x11]); // Reply type
    response.extend_from_slice(&(0u16).to_be_bytes()); // Zero length
    response.extend_from_slice(&0u32.to_be_bytes()); // Sequence
    response.extend_from_slice(&[0x90, 0x41, VISCA_TERMINATOR]); // But has payload

    assert!(
        envelope.extract_response(&response).is_err(),
        "Should reject zero length with payload"
    );
}

#[test]
fn test_sony_max_length_boundaries() {
    use grafton_visca::capabilities::ProtocolStyle;
    use grafton_visca::transport::envelope::TransportEnvelope;

    let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated {
        use_sequence: false,
    });

    let mut response = Vec::new();
    response.extend_from_slice(&[0x01, 0x11]); // Reply type
    response.extend_from_slice(&(u16::MAX).to_be_bytes()); // Max length
    response.extend_from_slice(&0u32.to_be_bytes()); // Sequence
    response.extend_from_slice(&[0x90, 0x41, VISCA_TERMINATOR]); // Small payload

    assert!(
        envelope.extract_response(&response).is_err(),
        "Should reject u16::MAX length mismatch"
    );
}

#[test]
fn test_sony_sequence_number_wraparound() {
    use grafton_visca::capabilities::ProtocolStyle;
    use grafton_visca::transport::envelope::TransportEnvelope;

    let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated { use_sequence: true });

    let dummy_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];

    for _ in 0..100 {
        let framed = envelope.frame_command(&dummy_cmd, false);
        assert!(framed.len() > 8, "Should produce framed output");
    }

    let frame1 = envelope.frame_command(&dummy_cmd, false);
    let frame2 = envelope.frame_command(&dummy_cmd, false);

    let seq1 = u32::from_be_bytes([frame1[4], frame1[5], frame1[6], frame1[7]]);
    let seq2 = u32::from_be_bytes([frame2[4], frame2[5], frame2[6], frame2[7]]);

    assert_eq!(seq2, seq1 + 1, "Sequence should increment by 1");
}

#[test]
fn test_sony_malformed_header_bytes() {
    use grafton_visca::capabilities::ProtocolStyle;
    use grafton_visca::transport::envelope::TransportEnvelope;

    let envelope = TransportEnvelope::new(ProtocolStyle::SonyEncapsulated {
        use_sequence: false,
    });

    let mut response = Vec::new();
    response.push(0x11); // Wrong byte order for type
    response.push(0x01);
    response.extend_from_slice(&(3u16).to_le_bytes()); // Wrong endianness
    response.extend_from_slice(&0u32.to_le_bytes()); // Wrong endianness
    response.extend_from_slice(&[0x90, 0x41, VISCA_TERMINATOR]);

    let result = envelope.extract_response(&response);
    assert!(
        result.is_err(),
        "Should reject malformed header with wrong byte order"
    );
}

#[test]
fn test_raw_visca_zero_length_handling() {
    use grafton_visca::capabilities::ProtocolStyle;
    use grafton_visca::transport::envelope::TransportEnvelope;

    let envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);

    let empty: &[u8] = &[];
    let framed = envelope.frame_command(empty, false);
    assert_eq!(framed.len(), 0, "Should handle empty command");

    let extracted = envelope.extract_response(empty);
    assert!(extracted.is_ok(), "Should handle empty response");
    assert_eq!(extracted.unwrap().len(), 0);
}

#[test]
fn test_raw_visca_maximum_size() {
    use grafton_visca::capabilities::ProtocolStyle;
    use grafton_visca::transport::envelope::TransportEnvelope;

    let envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);

    let large_cmd = vec![0x81; 1000];
    let framed = envelope.frame_command(&large_cmd, false);
    assert_eq!(framed.len(), 1000, "Should pass through large commands");

    let extracted = envelope.extract_response(&large_cmd);
    assert!(extracted.is_ok(), "Should extract large responses");
    assert_eq!(extracted.unwrap().len(), 1000);
}

#[test]
fn test_bytes_immutability_and_efficiency() {
    use grafton_visca::capabilities::ProtocolStyle;
    use grafton_visca::transport::envelope::TransportEnvelope;

    let envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);

    let original = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
    let framed = envelope.frame_command(&original, false);

    let cloned = framed.clone();
    assert_eq!(framed, cloned);

    assert_eq!(
        original,
        vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]
    );
}

#[test]
fn test_parser_multiple_terminators() {
    use grafton_visca::transport::frame_parser::FrameParser;

    let mut parser = FrameParser::new();

    parser.feed(&[0x81, 0xFF, 0xFF, 0x90, 0xFF]);

    let frame1 = parser.next_frame().expect("Should find first frame");
    assert_eq!(&frame1[..], &[0x81, 0xFF]);

    let frame2 = parser.next_frame().expect("Should find second frame");
    assert_eq!(&frame2[..], &[0xFF]);

    let frame3 = parser.next_frame().expect("Should find third frame");
    assert_eq!(&frame3[..], &[0x90, 0xFF]);

    assert!(parser.is_empty());
}

#[test]
fn test_parser_no_terminator_accumulation() {
    use grafton_visca::transport::frame_parser::FrameParser;

    let mut parser = FrameParser::new();

    for i in 0..100 {
        parser.feed(&[0x81, 0x01, i as u8]);
        assert!(
            parser.next_frame().is_none(),
            "Should not extract without terminator"
        );
    }

    assert_eq!(parser.len(), 300);

    parser.feed(&[0xFF]);

    let frame = parser
        .next_frame()
        .expect("Should extract after terminator");
    assert_eq!(frame.len(), 301);
    assert_eq!(frame[frame.len() - 1], 0xFF);
}

#[test]
fn test_parser_interleaved_data() {
    use grafton_visca::transport::frame_parser::FrameParser;

    let mut parser = FrameParser::new();

    parser.feed(&[0x81]);
    assert!(parser.next_frame().is_none());

    parser.feed(&[0x01, 0x04]);
    assert!(parser.next_frame().is_none());

    parser.feed(&[0x00, 0x02, 0xFF, 0x90]);

    let frame1 = parser.next_frame().expect("Should extract complete frame");
    assert_eq!(&frame1[..], &[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]);

    assert!(parser.next_frame().is_none());
    assert_eq!(parser.len(), 1); // Should have 0x90 remaining

    parser.feed(&[0x41, 0xFF]);
    let frame2 = parser.next_frame().expect("Should extract second frame");
    assert_eq!(&frame2[..], &[0x90, 0x41, 0xFF]);
}

#[test]
fn test_parser_clear_and_reuse() {
    use grafton_visca::transport::frame_parser::FrameParser;

    let mut parser = FrameParser::new();

    parser.feed(&[0x81, 0x01, 0x04]);
    assert_eq!(parser.len(), 3);

    parser.clear();
    assert!(parser.is_empty());

    parser.feed(&[0x90, 0x41, 0xFF]);
    let frame = parser.next_frame().expect("Should work after clear");
    assert_eq!(&frame[..], &[0x90, 0x41, 0xFF]);
}

#[test]
fn test_parser_capacity_management() {
    use grafton_visca::transport::frame_parser::FrameParser;

    let mut parser = FrameParser::with_capacity(10);

    let large_data = vec![0x81; 50];
    parser.feed(&large_data);

    assert!(parser.len() >= 50);

    parser.feed(&[0xFF]);
    let frame = parser.next_frame().expect("Should handle large frames");
    assert_eq!(frame.len(), 51);
}

#[test]
fn test_validate_frame_edge_cases() {
    use grafton_visca::transport::frame_parser::validate_frame;

    assert!(validate_frame(&[]).is_err(), "Should reject empty frame");

    assert!(
        validate_frame(&[0xFF]).is_ok(),
        "Single terminator should be valid"
    );

    assert!(
        validate_frame(&[0x81, 0x01, 0x04]).is_err(),
        "Should reject frame without terminator"
    );

    assert!(
        validate_frame(&[0x81, 0xFF, 0x01]).is_err(),
        "Should reject frame with terminator not at end"
    );

    let mut max_frame = vec![0x81; 31];
    max_frame.push(0xFF);
    assert!(
        validate_frame(&max_frame).is_ok(),
        "Should accept 32 byte frame"
    );

    let mut over_frame = vec![0x81; 32];
    over_frame.push(0xFF);
    assert!(
        validate_frame(&over_frame).is_err(),
        "Should reject 33 byte frame"
    );

    let all_ff = vec![0xFF; 10];
    assert!(
        validate_frame(&all_ff).is_ok(),
        "Should accept frame of all terminators"
    );
}

#[test]
fn test_validate_frame_null_bytes() {
    use grafton_visca::transport::frame_parser::validate_frame;

    let with_nulls = vec![0x00, 0x00, 0x00, 0xFF];
    assert!(
        validate_frame(&with_nulls).is_ok(),
        "Should accept frame with null bytes"
    );

    let null_term = vec![0x00, 0xFF];
    assert!(
        validate_frame(&null_term).is_ok(),
        "Should accept null byte with terminator"
    );
}

#[test]
fn test_bytesmut_split_behavior() {
    use grafton_visca::transport::frame_parser::FrameParser;

    let mut parser = FrameParser::new();

    parser.feed(&[0x81, 0x01, 0xFF, 0x90, 0x41]);

    let frame1 = parser.next_frame().expect("Should get first frame");
    assert_eq!(&frame1[..], &[0x81, 0x01, 0xFF]);

    assert_eq!(parser.len(), 2);
    assert!(!parser.is_empty());

    parser.feed(&[0xFF]);
    let frame2 = parser.next_frame().expect("Should get second frame");
    assert_eq!(&frame2[..], &[0x90, 0x41, 0xFF]);

    assert!(parser.is_empty());
}

#[test]
fn test_rapid_frame_parsing() {
    use grafton_visca::transport::frame_parser::FrameParser;

    let mut parser = FrameParser::new();

    for i in 0..1000 {
        let data_byte = if (i % 256) == 0xFF {
            0xFE
        } else {
            (i % 256) as u8
        };
        let cmd = vec![0x81, 0x01, data_byte, 0xFF];
        parser.feed(&cmd);

        let frame = parser.next_frame().expect("Should extract frame");
        assert_eq!(frame.len(), 4, "Frame length mismatch at iteration {}", i);
        assert_eq!(frame[2], data_byte, "Data byte mismatch at iteration {}", i);
    }

    assert!(parser.is_empty());
}

#[test]
fn test_alternating_protocol_styles() {
    use grafton_visca::capabilities::ProtocolStyle;
    use grafton_visca::transport::envelope::TransportEnvelope;

    let raw_envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);
    let sony_envelope =
        TransportEnvelope::new(ProtocolStyle::SonyEncapsulated { use_sequence: true });

    let cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];

    for _ in 0..100 {
        let raw_framed = raw_envelope.frame_command(&cmd, false);
        assert_eq!(raw_framed.len(), 6);

        let sony_framed = sony_envelope.frame_command(&cmd, false);
        assert_eq!(sony_framed.len(), 14);
    }
}

#[test]
fn test_concurrent_parsers() {
    use grafton_visca::transport::frame_parser::FrameParser;

    let mut parser1 = FrameParser::new();
    let mut parser2 = FrameParser::new();

    parser1.feed(&[0x81, 0x01]);
    parser2.feed(&[0x90, 0x41]);

    parser1.feed(&[0xFF]);
    parser2.feed(&[0xFF]);

    let frame1 = parser1.next_frame().expect("Parser1 should have frame");
    let frame2 = parser2.next_frame().expect("Parser2 should have frame");

    assert_eq!(&frame1[..], &[0x81, 0x01, 0xFF]);
    assert_eq!(&frame2[..], &[0x90, 0x41, 0xFF]);
}

#[test]
fn test_bytes_zero_copy_behavior() {
    use grafton_visca::capabilities::ProtocolStyle;
    use grafton_visca::transport::envelope::TransportEnvelope;

    let envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);

    let large_cmd = vec![0x81; 1000];

    let framed1 = envelope.frame_command(&large_cmd, false);
    let framed2 = framed1.clone(); // Should be cheap (reference counted)

    assert_eq!(framed1, framed2);

    drop(framed1);
    assert_eq!(framed2.len(), 1000);
}

#[test]
fn test_bytesmut_efficient_growth() {
    use grafton_visca::transport::frame_parser::FrameParser;

    let mut parser = FrameParser::with_capacity(16);

    for _ in 0..10 {
        parser.feed(&[0x81; 100]);
    }

    assert!(parser.len() >= 1000);

    // Add terminator and extract
    parser.feed(&[0xFF]);
    let frame = parser.next_frame().expect("Should extract large frame");
    assert_eq!(frame.len(), 1001);
}
