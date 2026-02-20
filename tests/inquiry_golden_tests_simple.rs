//! Simplified golden tests for VISCA inquiry commands and responses.
//!
//! This module provides comprehensive testing for inquiry command types
//! using real-world response patterns (golden replies) from PTZ cameras.

use grafton_visca::{
    command::{
        response::{InquiryKind, Response},
        InquiryData,
    },
    ResolutionMode,
};

#[test]
fn test_power_inquiry_on() {
    // Test power inquiry response parsing - Power On
    let payload = vec![0x90, 0x50, 0x02, 0xFF];

    let result = Response::parse_with_type(&payload, &InquiryKind::Power);
    assert!(
        result.is_ok(),
        "Failed to parse power response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::Power { on }) => {
            assert!(on, "Power should be on");
        }
        _ => panic!("ViscaResponse type mismatch"),
    }
}

#[test]
fn test_power_inquiry_off() {
    // Test power inquiry response parsing - Power Off
    let payload = vec![0x90, 0x50, 0x03, 0xFF];

    let result = Response::parse_with_type(&payload, &InquiryKind::Power);
    assert!(
        result.is_ok(),
        "Failed to parse power response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::Power { on }) => {
            assert!(!on, "Power should be off");
        }
        _ => panic!("ViscaResponse type mismatch"),
    }
}

#[test]
fn test_pan_tilt_position_inquiry() {
    // Test pan/tilt position inquiry response parsing
    let payload = vec![
        0x90, 0x50, 0x00, 0x01, 0x02, 0x03, 0x00, 0x04, 0x05, 0x06, 0xFF,
    ];

    let result = Response::parse_with_type(&payload, &InquiryKind::PanTiltPosition);
    assert!(
        result.is_ok(),
        "Failed to parse pan/tilt response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::PanTiltPosition { pan, tilt }) => {
            assert_eq!(pan, 0x0123, "Pan position mismatch");
            assert_eq!(tilt, 0x0456, "Tilt position mismatch");
        }
        _ => panic!("ViscaResponse type mismatch"),
    }
}

#[test]
fn test_zoom_position_inquiry() {
    // Test zoom position inquiry response parsing
    let payload = vec![0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0xFF];

    let result = Response::parse_with_type(&payload, &InquiryKind::ZoomPosition);
    assert!(
        result.is_ok(),
        "Failed to parse zoom response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::ZoomPosition { position }) => {
            assert_eq!(position, 0x1234, "Zoom position mismatch");
        }
        _ => panic!("ViscaResponse type mismatch"),
    }
}

#[test]
fn test_focus_position_inquiry() {
    // Test focus position inquiry response parsing
    let payload = vec![0x90, 0x50, 0x05, 0x06, 0x07, 0x08, 0xFF];

    let result = Response::parse_with_type(&payload, &InquiryKind::FocusPosition);
    assert!(
        result.is_ok(),
        "Failed to parse focus response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::FocusPosition { position }) => {
            assert_eq!(position, 0x5678, "Focus position mismatch");
        }
        _ => panic!("ViscaResponse type mismatch"),
    }
}

#[test]
fn test_backlight_inquiry() {
    // Test backlight compensation inquiry response parsing
    let payload = vec![0x90, 0x50, 0x02, 0xFF]; // On

    let result = Response::parse_with_type(&payload, &InquiryKind::Backlight);
    assert!(
        result.is_ok(),
        "Failed to parse backlight response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::Backlight { status }) => {
            assert!(status, "Backlight should be on");
        }
        _ => panic!("ViscaResponse type mismatch"),
    }
}

#[test]
fn test_image_flip_inquiry() {
    // Test image flip inquiry response parsing
    let payload = vec![0x90, 0x50, 0x03, 0xFF]; // Both vertical and horizontal

    let result = Response::parse_with_type(&payload, &InquiryKind::FlipState);
    assert!(
        result.is_ok(),
        "Failed to parse image flip response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::FlipState {
            vertical,
            horizontal,
        }) => {
            assert!(vertical, "Vertical flip should be on");
            assert!(horizontal, "Horizontal flip should be on");
        }
        _ => panic!("ViscaResponse type mismatch"),
    }
}

#[test]
fn test_brightness_inquiry() {
    // Test brightness inquiry response parsing
    // Brightness uses 4 nibbles to encode position (0x0008 = 8)
    let payload = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x08, 0xFF];

    let result = Response::parse_with_type(&payload, &InquiryKind::Brightness);
    assert!(
        result.is_ok(),
        "Failed to parse brightness response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::Brightness { position }) => {
            assert_eq!(position, 0x0008, "Brightness position mismatch");
        }
        _ => panic!("ViscaResponse type mismatch"),
    }
}

#[test]
fn test_gain_inquiry() {
    // Test gain inquiry response parsing
    // Gain uses 4 bytes with the gain value in the last byte
    let payload = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x05, 0xFF];

    let result = Response::parse_with_type(&payload, &InquiryKind::Gain);
    assert!(
        result.is_ok(),
        "Failed to parse gain response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::Gain { gain }) => {
            assert_eq!(gain, 0x05, "Gain value mismatch");
        }
        _ => panic!("ViscaResponse type mismatch"),
    }
}

#[test]
fn test_version_inquiry() {
    // Test version inquiry response parsing
    // Version format: [0x90, 0x50, VV VV MM MM FF FF KK, 0xFF]
    // Where: VV=Vendor ID, MM=Model ID, FF=ROM version, KK=Max socket
    let payload = vec![
        0x90, 0x50, 0x00, 0x14, // Vendor ID (0x0014)
        0x00, 0x10, // Model ID (0x0010)
        0x02, 0x05, // ROM version (0x0205)
        0x00, // Max socket number
        0xFF,
    ];

    let result = Response::parse_with_type(&payload, &InquiryKind::Version);
    assert!(
        result.is_ok(),
        "Failed to parse version response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::Version {
            vendor,
            model,
            rom_version,
            max_socket,
        }) => {
            assert_eq!(vendor, 0x0014, "Vendor ID mismatch");
            assert_eq!(model, 0x0010, "Model code mismatch");
            assert_eq!(rom_version, 0x0205, "ROM version mismatch");
            assert_eq!(max_socket, 0x00, "Socket number mismatch");
        }
        _ => panic!("ViscaResponse type mismatch"),
    }
}

#[test]
fn test_contrast_inquiry() {
    // Test contrast inquiry response parsing
    // Response format: y0 50 00 00 0p 0q FF where pq = position
    let payload = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0C, 0xFF];

    let result = Response::parse_with_type(&payload, &InquiryKind::Contrast);
    assert!(
        result.is_ok(),
        "Failed to parse contrast response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::Contrast { level }) => {
            assert_eq!(level, 0x0C, "Contrast value mismatch");
        }
        _ => panic!("ViscaResponse type mismatch"),
    }
}

#[test]
fn test_luminance_inquiry() {
    // Test luminance inquiry response parsing
    // Response format: y0 50 00 00 0p 0q FF where pq = position
    let payload = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF];

    let result = Response::parse_with_type(&payload, &InquiryKind::Luminance);
    assert!(
        result.is_ok(),
        "Failed to parse luminance response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::Luminance { level }) => {
            assert_eq!(level, 0x07, "Luminance value mismatch");
        }
        _ => panic!("ViscaResponse type mismatch"),
    }
}

#[test]
fn test_resolution_inquiry() {
    // Test resolution inquiry response parsing
    let payload = vec![0x90, 0x50, 0x00, 0xFF]; // 1080p60 (0x00 = FullHD60)

    let result = Response::parse_with_type(&payload, &InquiryKind::Resolution);
    assert!(
        result.is_ok(),
        "Failed to parse resolution response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::Resolution(code)) => {
            assert_eq!(code, ResolutionMode::FullHD60, "Resolution code mismatch");
        }
        _ => panic!("ViscaResponse type mismatch"),
    }
}

#[test]
fn test_negative_pan_tilt_values() {
    // Test parsing negative values in pan/tilt response
    let payload = vec![
        0x90, 0x50, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0E, 0x0D, 0xFF,
    ];

    let result = Response::parse_with_type(&payload, &InquiryKind::PanTiltPosition);
    assert!(
        result.is_ok(),
        "Failed to parse negative pan/tilt: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryData::PanTiltPosition { pan, tilt }) => {
            assert_eq!(pan, -1, "Pan should be -1");
            assert_eq!(tilt, -19, "Tilt should be -19");
        }
        _ => panic!("ViscaResponse type mismatch"),
    }
}

#[test]
fn test_unknown_inquiry_response() {
    // Test handling of an unknown response format - this will likely error
    // since parse_response requires a specific InquiryKind
    let payload = vec![0x90, 0x50, 0xFF, 0xFF, 0xFF, 0xFF];

    // Try parsing with a known type to see if it handles invalid data gracefully
    let result = Response::parse_with_type(&payload, &InquiryKind::Power);
    // We expect this to error since 0xFF is not a valid power state
    assert!(result.is_err(), "Should error on invalid power state");
}

#[test]
fn test_inquiry_response_length_validation() {
    // Test various response lengths for different inquiry types

    // Power response should be 4 bytes total
    let payload = vec![0x90, 0x50, 0x02]; // Missing terminator

    let result = Response::parse_with_type(&payload, &InquiryKind::Power);
    assert!(result.is_err(), "Should fail without terminator");

    // Pan/Tilt response should be 11 bytes total
    let payload = vec![0x90, 0x50, 0x00, 0x01, 0xFF]; // Too short for pan/tilt

    let result = Response::parse_with_type(&payload, &InquiryKind::PanTiltPosition);
    assert!(result.is_err(), "Should fail with insufficient data");
}

#[test]
fn test_inquiry_nibble_parsing() {
    // Test parsing of nibble-encoded values

    // Zoom position with max nibbles
    let payload = vec![0x90, 0x50, 0x0F, 0x0F, 0x0F, 0x0F, 0xFF];

    let result = Response::parse_with_type(&payload, &InquiryKind::ZoomPosition);
    assert!(result.is_ok(), "Should parse nibbles");

    match result.unwrap() {
        Response::Inquiry(InquiryData::ZoomPosition { position }) => {
            assert_eq!(position, 0xFFFF, "Should be max value");
        }
        _ => panic!("Wrong response type"),
    }
}
