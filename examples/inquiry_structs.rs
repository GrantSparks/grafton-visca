//! Example demonstrating the new InquiryCommand derive macro pattern
//!
//! This shows how inquiry commands are defined as individual structs
//! that automatically generate Command implementations.

use grafton_visca::command::{Command, PowerInquiry, PanTiltPositionInquiry, ZoomPositionInquiry};

fn main() {
    // Create inquiry instances directly
    let power_inquiry = PowerInquiry;
    let pan_tilt_inquiry = PanTiltPositionInquiry;
    let zoom_inquiry = ZoomPositionInquiry;
    
    // Use the Command trait to get command bytes
    println!("Power inquiry bytes: {:?}", power_inquiry.to_bytes().unwrap());
    println!("Pan/Tilt inquiry bytes: {:?}", pan_tilt_inquiry.to_bytes().unwrap());
    println!("Zoom inquiry bytes: {:?}", zoom_inquiry.to_bytes().unwrap());
    
    // Each inquiry has its response type
    println!("Power response type: {:?}", power_inquiry.response_type());
    println!("Pan/Tilt response type: {:?}", pan_tilt_inquiry.response_type());
    println!("Zoom response type: {:?}", zoom_inquiry.response_type());
}

// Example showing how to define new inquiry commands using the derive macro:
//
// use grafton_visca_macros::InquiryCommand;
//
// #[derive(InquiryCommand, Debug, Copy, Clone)]
// #[visca(command = 0x00, response = "Power")]
// struct MyPowerInquiry;
//
// The derive macro automatically generates:
// - Command trait implementation
// - to_bytes() method returning the VISCA command bytes
// - response_type() method returning the expected ResponseType
// - command_category() method (returns CommandCategory::Quick for inquiries)