//! Tests for the InquiryCommand derive macro

use grafton_visca::{
    InquiryCommand,
    command::{Command, ResponseType},
    Error,
    timeout::CommandCategory,
};

#[derive(Debug, InquiryCommand, PartialEq)]
enum TestInquiry {
    #[visca(0x00, response = Power)]
    Power,
    
    #[visca(0x47, response = ZoomPosition)]
    ZoomPos,
    
    #[visca(0x48, response = FocusPosition)]
    FocusPos,
    
    #[visca(0x12, subcategory = 0x06, response = PanTiltPosition)]
    PanTiltPos,
    
    #[visca(0xA1, response = Luminance)]
    Luminance,
    
    #[visca(0xA2, response = Contrast)]
    Contrast,
    
    #[visca(0x42, response = Sharpness)]
    Sharpness,
    
    #[visca(0x4E, response = ExposureCompensation)]
    ExposureCompensation,
    
    #[visca(0x3E, response = ExposureCompensationMode)]
    ExposureCompensationMode,
}

#[test]
fn test_derive_to_bytes() -> Result<(), Error> {
    // Test standard inquiry commands
    assert_eq!(TestInquiry::Power.to_bytes()?, vec![0x81, 0x09, 0x04, 0x00, 0xFF]);
    assert_eq!(TestInquiry::ZoomPos.to_bytes()?, vec![0x81, 0x09, 0x04, 0x47, 0xFF]);
    assert_eq!(TestInquiry::FocusPos.to_bytes()?, vec![0x81, 0x09, 0x04, 0x48, 0xFF]);
    assert_eq!(TestInquiry::Luminance.to_bytes()?, vec![0x81, 0x09, 0x04, 0xA1, 0xFF]);
    assert_eq!(TestInquiry::Contrast.to_bytes()?, vec![0x81, 0x09, 0x04, 0xA2, 0xFF]);
    assert_eq!(TestInquiry::Sharpness.to_bytes()?, vec![0x81, 0x09, 0x04, 0x42, 0xFF]);
    
    // Test subcategory command
    assert_eq!(TestInquiry::PanTiltPos.to_bytes()?, vec![0x81, 0x09, 0x06, 0x12, 0xFF]);
    
    Ok(())
}

#[test]
fn test_derive_response_type() {
    assert_eq!(TestInquiry::Power.response_type(), Some(ResponseType::Power));
    assert_eq!(TestInquiry::ZoomPos.response_type(), Some(ResponseType::ZoomPosition));
    assert_eq!(TestInquiry::FocusPos.response_type(), Some(ResponseType::FocusPosition));
    assert_eq!(TestInquiry::PanTiltPos.response_type(), Some(ResponseType::PanTiltPosition));
    assert_eq!(TestInquiry::Luminance.response_type(), Some(ResponseType::Luminance));
    assert_eq!(TestInquiry::Contrast.response_type(), Some(ResponseType::Contrast));
    assert_eq!(TestInquiry::Sharpness.response_type(), Some(ResponseType::Sharpness));
    assert_eq!(TestInquiry::ExposureCompensation.response_type(), Some(ResponseType::ExposureCompensation));
    assert_eq!(TestInquiry::ExposureCompensationMode.response_type(), Some(ResponseType::ExposureCompensationMode));
}

#[test]
fn test_derive_command_category() {
    // All inquiry commands should have Quick category
    assert_eq!(TestInquiry::Power.command_category(), CommandCategory::Quick);
    assert_eq!(TestInquiry::ZoomPos.command_category(), CommandCategory::Quick);
    assert_eq!(TestInquiry::PanTiltPos.command_category(), CommandCategory::Quick);
}