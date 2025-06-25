//! Example demonstrating manual implementation of inquiry commands
//!
//! This example shows how to manually implement inquiry commands.
//! For reducing boilerplate, see the InquiryCommand derive macro
//! which can be used on structs within the grafton-visca crate.

use grafton_visca::{
    command::{Command, InquiryCommand, InquiryResponse},
    Error,
};

/// Example of using the existing InquiryCommand enum
fn main() -> Result<(), Error> {
    // Create inquiry commands
    let power_inquiry = InquiryCommand::Power;
    let zoom_inquiry = InquiryCommand::ZoomPosition;
    
    // Get byte sequences
    println!("Power inquiry bytes: {:?}", power_inquiry.to_bytes()?);
    println!("Zoom inquiry bytes: {:?}", zoom_inquiry.to_bytes()?);
    
    // Check response types
    println!("Power response type: {:?}", power_inquiry.response_type());
    println!("Zoom response type: {:?}", zoom_inquiry.response_type());
    
    // Example of parsing responses
    let power_on_response = &[0x02];
    let power_off_response = &[0x03];
    
    match power_inquiry.parse_response(power_on_response)? {
        InquiryResponse::Power { on } => println!("Power is {}", if on { "ON" } else { "OFF" }),
        _ => println!("Unexpected response type"),
    }
    
    match power_inquiry.parse_response(power_off_response)? {
        InquiryResponse::Power { on } => println!("Power is {}", if on { "ON" } else { "OFF" }),
        _ => println!("Unexpected response type"),
    }
    
    Ok(())
}