//! Tests for InquiryCommand derive macro with parser generation

use grafton_visca::{
    command::{Command, InquiryResponse, ResponseType},
    Error, InquiryCommand,
};

#[derive(Debug, InquiryCommand, PartialEq)]
enum TestInquiryWithParser {
    #[visca(0x00, response = Power, parser = "bool")]
    Power,

    #[visca(0x47, response = ZoomPosition, parser = "position")]
    ZoomPos,

    #[visca(0xA1, response = Luminance, parser = "byte")]
    Luminance,

    #[visca(0x12, subcategory = 0x06, response = PanTiltPosition, parser = "pan_tilt")]
    PanTiltPos,
}

#[test]
fn test_derive_with_parser_generates_methods() -> Result<(), Error> {
    // Test that the macro generates correct byte sequences
    assert_eq!(
        TestInquiryWithParser::Power.to_bytes()?,
        vec![0x81, 0x09, 0x04, 0x00, 0xFF]
    );
    assert_eq!(
        TestInquiryWithParser::ZoomPos.to_bytes()?,
        vec![0x81, 0x09, 0x04, 0x47, 0xFF]
    );
    assert_eq!(
        TestInquiryWithParser::Luminance.to_bytes()?,
        vec![0x81, 0x09, 0x04, 0xA1, 0xFF]
    );
    assert_eq!(
        TestInquiryWithParser::PanTiltPos.to_bytes()?,
        vec![0x81, 0x09, 0x06, 0x12, 0xFF]
    );

    // Test that response types are correct
    assert_eq!(
        TestInquiryWithParser::Power.response_type(),
        Some(ResponseType::Power)
    );
    assert_eq!(
        TestInquiryWithParser::ZoomPos.response_type(),
        Some(ResponseType::ZoomPosition)
    );
    assert_eq!(
        TestInquiryWithParser::Luminance.response_type(),
        Some(ResponseType::Luminance)
    );
    assert_eq!(
        TestInquiryWithParser::PanTiltPos.response_type(),
        Some(ResponseType::PanTiltPosition)
    );

    Ok(())
}

#[test]
fn test_parse_response_bool_parser() -> Result<(), Error> {
    let cmd = TestInquiryWithParser::Power;

    // Test power on response
    let response = cmd.parse_response(&[0x02])?;
    match response {
        InquiryResponse::Power { on } => assert!(on, "Power should be on"),
        _ => panic!("Expected Power response, got {:?}", response),
    }

    // Test power off response
    let response = cmd.parse_response(&[0x03])?;
    match response {
        InquiryResponse::Power { on } => assert!(!on, "Power should be off"),
        _ => panic!("Expected Power response, got {:?}", response),
    }

    // Test invalid response
    assert!(cmd.parse_response(&[0x01]).is_err());

    Ok(())
}

#[test]
fn test_parse_response_position_parser() -> Result<(), Error> {
    let cmd = TestInquiryWithParser::ZoomPos;

    // Test zoom position parsing - 0x1234 should become 0x1234
    let response = cmd.parse_response(&[0x01, 0x02, 0x03, 0x04])?;
    match response {
        InquiryResponse::ZoomPosition { position } => {
            assert_eq!(position, 0x1234, "Zoom position should be 0x1234");
        }
        _ => panic!("Expected ZoomPosition response, got {:?}", response),
    }

    // Test insufficient data
    assert!(cmd.parse_response(&[0x01, 0x02]).is_err());

    Ok(())
}

#[test]
fn test_parse_response_byte_parser() -> Result<(), Error> {
    let cmd = TestInquiryWithParser::Luminance;

    // Test luminance value parsing
    let response = cmd.parse_response(&[0x7F])?;
    match response {
        InquiryResponse::Luminance(value) => {
            assert_eq!(value, 0x7F, "Luminance value should be 0x7F");
        }
        _ => panic!("Expected Luminance response, got {:?}", response),
    }

    // Test empty data
    assert!(cmd.parse_response(&[]).is_err());

    Ok(())
}

#[test]
fn test_parse_response_pan_tilt_parser() -> Result<(), Error> {
    let cmd = TestInquiryWithParser::PanTiltPos;

    // Test pan/tilt position parsing
    // Pan: 0x1234, Tilt: 0x5678
    let response = cmd.parse_response(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08])?;
    match response {
        InquiryResponse::PanTiltPosition { pan, tilt } => {
            assert_eq!(pan, 0x1234, "Pan position should be 0x1234");
            assert_eq!(tilt, 0x5678, "Tilt position should be 0x5678");
        }
        _ => panic!("Expected PanTiltPosition response, got {:?}", response),
    }

    // Test insufficient data
    assert!(cmd.parse_response(&[0x01, 0x02, 0x03, 0x04]).is_err());

    Ok(())
}

// Test with more parser types
#[derive(Debug, InquiryCommand, PartialEq)]
enum AdvancedInquiry {
    #[visca(0x42, response = Sharpness, parser = "nibble", field = "value")]
    Sharpness,

    #[visca(0x44, response = RedGain, parser = "offset", field = "gain", offset = 10)]
    RedGain,

    #[visca(0x66, response = ImageFlip, parser = "flags")]
    ImageFlip,
}

#[test]
fn test_parse_response_nibble_parser() -> Result<(), Error> {
    let cmd = AdvancedInquiry::Sharpness;

    // Test nibble parsing - 0x3F should become 0x3F
    let response = cmd.parse_response(&[0x03, 0x0F])?;
    match response {
        InquiryResponse::Sharpness { value } => {
            assert_eq!(value, 0x3F, "Sharpness value should be 0x3F");
        }
        _ => panic!("Expected Sharpness response, got {:?}", response),
    }

    Ok(())
}

#[test]
fn test_parse_response_offset_parser() -> Result<(), Error> {
    let cmd = AdvancedInquiry::RedGain;

    // Test offset parsing - 0x14 (20) - 10 = 10
    let response = cmd.parse_response(&[0x14])?;
    match response {
        InquiryResponse::RedGain { gain } => {
            assert_eq!(gain, 10, "Red gain should be 10 (0x14 - 10)");
        }
        _ => panic!("Expected RedGain response, got {:?}", response),
    }

    Ok(())
}

#[test]
fn test_parse_response_flags_parser() -> Result<(), Error> {
    let cmd = AdvancedInquiry::ImageFlip;

    // Test bit flags parsing
    // 0x00 = both off
    let response = cmd.parse_response(&[0x00])?;
    match response {
        InquiryResponse::ImageFlip {
            horizontal,
            vertical,
        } => {
            assert!(!horizontal, "Horizontal flip should be off");
            assert!(!vertical, "Vertical flip should be off");
        }
        _ => panic!("Expected ImageFlip response, got {:?}", response),
    }

    // 0x01 = horizontal on
    let response = cmd.parse_response(&[0x01])?;
    match response {
        InquiryResponse::ImageFlip {
            horizontal,
            vertical,
        } => {
            assert!(horizontal, "Horizontal flip should be on");
            assert!(!vertical, "Vertical flip should be off");
        }
        _ => panic!("Expected ImageFlip response, got {:?}", response),
    }

    // 0x02 = vertical on
    let response = cmd.parse_response(&[0x02])?;
    match response {
        InquiryResponse::ImageFlip {
            horizontal,
            vertical,
        } => {
            assert!(!horizontal, "Horizontal flip should be off");
            assert!(vertical, "Vertical flip should be on");
        }
        _ => panic!("Expected ImageFlip response, got {:?}", response),
    }

    // 0x03 = both on
    let response = cmd.parse_response(&[0x03])?;
    match response {
        InquiryResponse::ImageFlip {
            horizontal,
            vertical,
        } => {
            assert!(horizontal, "Horizontal flip should be on");
            assert!(vertical, "Vertical flip should be on");
        }
        _ => panic!("Expected ImageFlip response, got {:?}", response),
    }

    Ok(())
}
