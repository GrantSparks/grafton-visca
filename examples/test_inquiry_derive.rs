//! Test example for the InquiryCommand derive macro

use grafton_visca::{
    InquiryCommand,
    command::{Command, ResponseType},
    Error,
    timeout::CommandCategory,
};

#[derive(Debug, InquiryCommand)]
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
}

fn main() -> Result<(), Error> {
    println!("Testing InquiryCommand derive macro");
    
    // The macro should generate implementations for:
    // - Command trait with to_bytes() method
    // - response_type() method returning Option<ResponseType>
    // - command_category() returning CommandCategory::Inquiry
    
    let power_cmd = TestInquiry::Power;
    println!("Power command: {:?}", power_cmd);
    let power_bytes = power_cmd.to_bytes()?;
    println!("Power bytes: {:02x?}", power_bytes);
    assert_eq!(power_bytes, vec![0x81, 0x09, 0x04, 0x00, 0xFF]);
    assert_eq!(power_cmd.response_type(), Some(ResponseType::Power));
    assert_eq!(power_cmd.command_category(), CommandCategory::Quick);
    
    let zoom_cmd = TestInquiry::ZoomPos;
    println!("\nZoom command: {:?}", zoom_cmd);
    let zoom_bytes = zoom_cmd.to_bytes()?;
    println!("Zoom bytes: {:02x?}", zoom_bytes);
    assert_eq!(zoom_bytes, vec![0x81, 0x09, 0x04, 0x47, 0xFF]);
    assert_eq!(zoom_cmd.response_type(), Some(ResponseType::ZoomPosition));
    
    let pantilt_cmd = TestInquiry::PanTiltPos;
    println!("\nPanTilt command: {:?}", pantilt_cmd);
    let pantilt_bytes = pantilt_cmd.to_bytes()?;
    println!("PanTilt bytes: {:02x?}", pantilt_bytes);
    assert_eq!(pantilt_bytes, vec![0x81, 0x09, 0x06, 0x12, 0xFF]);
    assert_eq!(pantilt_cmd.response_type(), Some(ResponseType::PanTiltPosition));
    
    println!("\nAll tests passed!");
    Ok(())
}