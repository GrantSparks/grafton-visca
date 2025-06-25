//! Tests for the command module

use crate::{
    command::{Command, InquiryResponse, ResponseType},
    timeout::CommandCategory,
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

    // Test that all commands are Quick category
    assert_eq!(
        TestInquiryWithParser::Power.command_category(),
        CommandCategory::Quick
    );

    Ok(())
}

#[test]
fn test_bool_parser_generation() -> Result<(), Error> {
    let cmd = TestInquiryWithParser::Power;

    // Test parsing "on" response
    let response = cmd.parse_response(&[0x02])?;
    assert_eq!(response, InquiryResponse::Power { on: true });

    // Test parsing "off" response
    let response = cmd.parse_response(&[0x03])?;
    assert_eq!(response, InquiryResponse::Power { on: false });

    // Test invalid response
    assert!(cmd.parse_response(&[0x00]).is_err());
    assert!(cmd.parse_response(&[0x01]).is_err());
    assert!(cmd.parse_response(&[0x04]).is_err());

    Ok(())
}