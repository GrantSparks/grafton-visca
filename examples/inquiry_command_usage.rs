//! Example demonstrating proper usage of the InquiryCommand derive macro
//!
//! This example shows how to use the InquiryCommand derive macro to reduce
//! boilerplate when implementing inquiry commands. The macro generates the
//! Command trait implementation and optional parser methods.

use grafton_visca::{
    command::{Command, InquiryResponse},
    Error, InquiryCommand,
};

/// Custom inquiry commands using the derive macro
///
/// Note: All response types referenced here must already exist in the
/// ResponseType and InquiryResponse enums.
#[derive(Debug, InquiryCommand, Clone, PartialEq)]
enum CustomInquiry {
    /// Power inquiry - uses boolean parser
    #[visca(0x00, response = Power, parser = "bool")]
    Power,

    /// Zoom position inquiry - uses position parser (4 nibbles -> u16)
    #[visca(0x47, response = ZoomPosition, parser = "position")]
    ZoomPos,

    /// Luminance inquiry - uses direct byte parser
    #[visca(0xA1, response = Luminance, parser = "byte")]
    Luminance,

    /// Pan/Tilt position inquiry - special subcategory and pan_tilt parser
    #[visca(0x12, subcategory = 0x06, response = PanTiltPosition, parser = "pan_tilt")]
    PanTiltPos,

    /// Sharpness inquiry - uses nibble parser for extended values
    #[visca(0x42, response = Sharpness, parser = "nibble", field = "value")]
    #[allow(dead_code)]
    Sharpness,

    /// Red gain inquiry - uses offset parser (subtracts 10 from byte value)
    #[visca(0x44, response = RedGain, parser = "offset", field = "gain", offset = 10)]
    #[allow(dead_code)]
    RedGain,

    /// Image flip inquiry - uses bit flags parser
    #[visca(0x66, response = ImageFlip, parser = "flags")]
    #[allow(dead_code)]
    ImageFlip,
}

fn main() -> Result<(), Error> {
    println!("InquiryCommand Derive Macro Usage Example\n");

    // The derive macro generates the to_bytes() method
    let power_cmd = CustomInquiry::Power;
    println!("Power inquiry bytes: {:02X?}", power_cmd.to_bytes()?);

    // It also generates the response_type() method
    println!(
        "Power response type: {:?}",
        power_cmd.response_type().unwrap()
    );

    // With parser support, it generates parse_response() method
    println!("\nTesting parser functionality:");

    // Test boolean parser
    let power_on_response = &[0x02]; // 0x02 = on
    let result = power_cmd.parse_response(power_on_response)?;
    println!("Power on response: {:?}", result);

    let power_off_response = &[0x03]; // 0x03 = off
    let result = power_cmd.parse_response(power_off_response)?;
    println!("Power off response: {:?}", result);

    // Test position parser (4 nibbles)
    let zoom_cmd = CustomInquiry::ZoomPos;
    let zoom_response = &[0x00, 0x01, 0x02, 0x03]; // Position 0x0123
    let result = zoom_cmd.parse_response(zoom_response)?;
    match result {
        InquiryResponse::ZoomPosition { position } => {
            println!("Zoom position: 0x{:04X}", position);
        }
        _ => panic!("Unexpected response type"),
    }

    // Test direct byte parser
    let luminance_cmd = CustomInquiry::Luminance;
    let luminance_response = &[0x0A]; // Luminance level 10
    let result = luminance_cmd.parse_response(luminance_response)?;
    match result {
        InquiryResponse::Luminance(level) => {
            println!("Luminance level: {}", level);
        }
        _ => panic!("Unexpected response type"),
    }

    // Test pan/tilt parser (two 4-nibble values)
    let pan_tilt_cmd = CustomInquiry::PanTiltPos;
    let pan_tilt_response = &[0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x05, 0x00];
    let result = pan_tilt_cmd.parse_response(pan_tilt_response)?;
    match result {
        InquiryResponse::PanTiltPosition { pan, tilt } => {
            println!("Pan: {}, Tilt: {}", pan, tilt);
        }
        _ => panic!("Unexpected response type"),
    }

    println!("\nThe macro generates:");
    println!("1. Command trait implementation with to_bytes() and response_type()");
    println!("2. parse_response() method when parser attribute is specified");
    println!("\nTo add new inquiry commands:");
    println!("1. Add variants to ResponseType enum in src/command/response.rs");
    println!("2. Add variants to InquiryResponse enum in src/command/mod.rs");
    println!("3. Use the InquiryCommand derive with matching response names");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macro_generates_correct_bytes() {
        assert_eq!(
            CustomInquiry::Power.to_bytes().unwrap(),
            vec![0x81, 0x09, 0x04, 0x00, 0xFF]
        );
        assert_eq!(
            CustomInquiry::PanTiltPos.to_bytes().unwrap(),
            vec![0x81, 0x09, 0x06, 0x12, 0xFF]
        );
    }

    #[test]
    fn test_parser_generation() {
        // Test that parse_response is generated
        let cmd = CustomInquiry::Power;
        assert!(cmd.parse_response(&[0x02]).is_ok());
    }
}
