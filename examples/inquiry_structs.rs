//! Example demonstrating the new InquiryCommand derive macro pattern
//!
//! This shows how to define inquiry commands as individual structs
//! that automatically generate Command implementations and conversions
//! to the InquiryCommand enum.

use grafton_visca::command::{Command, ExposureMode, InquiryCommand, ResponseType};
use grafton_visca_macros::InquiryCommand;

// Example 1: Simple inquiry command without subcategory
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(command = 0x00, response = "Power", inquiry_variant = "Power")]
struct PowerInquiry;

// Example 2: Inquiry with subcategory
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x12,
    sub_command = 0x06,
    response = "PanTiltPosition",
    inquiry_variant = "PanTiltPosition"
)]
struct PanTiltPositionInquiry;

// Example 3: Inquiry with parser
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x47,
    response = "ZoomPosition",
    inquiry_variant = "ZoomPosition",
    parser = "position"
)]
struct ZoomPositionInquiry;

// Example 4: Inquiry with custom parser
#[derive(InquiryCommand, Debug, Copy, Clone)]
#[visca(
    command = 0x39,
    response = "ExposureMode",
    inquiry_variant = "ExposureMode",
    parser = "mode",
    type = "ExposureMode"
)]
struct ExposureModeInquiry;

fn main() {
    // Create inquiry instances
    let power_inquiry = PowerInquiry;
    let pan_tilt_inquiry = PanTiltPositionInquiry;
    let zoom_inquiry = ZoomPositionInquiry;
    let exposure_inquiry = ExposureModeInquiry;

    // Test Command trait implementations
    println!(
        "Power inquiry bytes: {:02X?}",
        power_inquiry.to_bytes().unwrap()
    );
    println!(
        "Pan/Tilt inquiry bytes: {:02X?}",
        pan_tilt_inquiry.to_bytes().unwrap()
    );
    println!(
        "Zoom inquiry bytes: {:02X?}",
        zoom_inquiry.to_bytes().unwrap()
    );
    println!(
        "Exposure inquiry bytes: {:02X?}",
        exposure_inquiry.to_bytes().unwrap()
    );

    // Test response type
    assert_eq!(power_inquiry.response_type(), Some(ResponseType::Power));
    assert_eq!(
        pan_tilt_inquiry.response_type(),
        Some(ResponseType::PanTiltPosition)
    );
    assert_eq!(
        zoom_inquiry.response_type(),
        Some(ResponseType::ZoomPosition)
    );
    assert_eq!(
        exposure_inquiry.response_type(),
        Some(ResponseType::ExposureMode)
    );

    // Test From conversions
    let cmd: InquiryCommand = power_inquiry.into();
    println!("Converted to enum: {:?}", cmd);

    let cmd: InquiryCommand = pan_tilt_inquiry.into();
    println!("Converted to enum: {:?}", cmd);

    // The benefit: All the boilerplate is generated, but the enum variants
    // are explicitly defined in the InquiryCommand enum for discoverability
    println!("\nThis pattern provides:");
    println!("- Type-safe command definitions");
    println!("- Automatic trait implementations");
    println!("- Seamless conversion to the central enum");
    println!("- Clear, discoverable API");
}
