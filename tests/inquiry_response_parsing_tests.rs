//! Tests for inquiry response parsing with golden test data.
//!
//! This module tests the parsing of VISCA inquiry responses using
//! real-world response patterns from PTZ cameras.

use grafton_visca::command::{
    response::{Response, ResponseKind},
    InquiryResponse,
};

#[test]
fn test_parse_power_inquiry_responses() {
    // Power On response
    let data = vec![0x90, 0x50, 0x02, 0xFF];
    let result = Response::parse_with_type(&data, &ResponseKind::Power);
    assert!(
        result.is_ok(),
        "Failed to parse power on response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryResponse::Power { on }) => {
            assert!(on, "Power should be on");
        }
        _ => panic!("Unexpected response type"),
    }

    // Power Off response
    let data = vec![0x90, 0x50, 0x03, 0xFF];
    let result = Response::parse_with_type(&data, &ResponseKind::Power);
    assert!(
        result.is_ok(),
        "Failed to parse power off response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryResponse::Power { on }) => {
            assert!(!on, "Power should be off");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_pan_tilt_position_inquiry() {
    // Standard position
    let data = vec![
        0x90, 0x50, 0x00, 0x01, 0x02, 0x03, 0x00, 0x04, 0x05, 0x06, 0xFF,
    ];
    let result = Response::parse_with_type(&data, &ResponseKind::PanTiltPosition);
    assert!(
        result.is_ok(),
        "Failed to parse pan/tilt response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryResponse::PanTiltPosition { pan, tilt }) => {
            assert_eq!(pan, 0x0123, "Pan position mismatch");
            assert_eq!(tilt, 0x0456, "Tilt position mismatch");
        }
        _ => panic!("Unexpected response type"),
    }

    // Negative positions
    let data = vec![
        0x90, 0x50, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0E, 0x0D, 0xFF,
    ];
    let result = Response::parse_with_type(&data, &ResponseKind::PanTiltPosition);
    assert!(
        result.is_ok(),
        "Failed to parse negative pan/tilt: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryResponse::PanTiltPosition { pan, tilt }) => {
            assert_eq!(pan, -1, "Pan should be -1");
            assert_eq!(tilt, -19, "Tilt should be -19");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_zoom_position_inquiry() {
    let data = vec![0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0xFF];
    let result = Response::parse_with_type(&data, &ResponseKind::ZoomPosition);
    assert!(
        result.is_ok(),
        "Failed to parse zoom response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryResponse::ZoomPosition { position }) => {
            assert_eq!(position, 0x1234, "Zoom position mismatch");
        }
        _ => panic!("Unexpected response type"),
    }

    // Max zoom position
    let data = vec![0x90, 0x50, 0x0F, 0x0F, 0x0F, 0x0F, 0xFF];
    let result = Response::parse_with_type(&data, &ResponseKind::ZoomPosition);
    assert!(result.is_ok(), "Failed to parse max zoom: {:?}", result);

    match result.unwrap() {
        Response::Inquiry(InquiryResponse::ZoomPosition { position }) => {
            assert_eq!(position, 0xFFFF, "Should be max zoom");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_focus_position_inquiry() {
    let data = vec![0x90, 0x50, 0x05, 0x06, 0x07, 0x08, 0xFF];
    let result = Response::parse_with_type(&data, &ResponseKind::FocusPosition);
    assert!(
        result.is_ok(),
        "Failed to parse focus response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryResponse::FocusPosition { position }) => {
            assert_eq!(position, 0x5678, "Focus position mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_backlight_inquiry() {
    let data = vec![0x90, 0x50, 0x02, 0xFF];
    let result = Response::parse_with_type(&data, &ResponseKind::Backlight);
    assert!(
        result.is_ok(),
        "Failed to parse backlight response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryResponse::Backlight { status }) => {
            assert!(status, "Backlight should be on");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_image_flip_inquiry() {
    // Both flips on
    let data = vec![0x90, 0x50, 0x03, 0xFF];
    let result = Response::parse_with_type(&data, &ResponseKind::ImageFlip);
    assert!(
        result.is_ok(),
        "Failed to parse image flip response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryResponse::ImageFlip {
            vertical,
            horizontal,
        }) => {
            assert!(vertical, "Vertical flip should be on");
            assert!(horizontal, "Horizontal flip should be on");
        }
        _ => panic!("Unexpected response type"),
    }

    // Only vertical flip
    let data = vec![0x90, 0x50, 0x02, 0xFF];
    let result = Response::parse_with_type(&data, &ResponseKind::ImageFlip);
    assert!(
        result.is_ok(),
        "Failed to parse image flip response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryResponse::ImageFlip {
            vertical,
            horizontal,
        }) => {
            assert!(vertical, "Vertical flip should be on");
            assert!(!horizontal, "Horizontal flip should be off");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_brightness_inquiry() {
    let data = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x08, 0xFF];
    let result = Response::parse_with_type(&data, &ResponseKind::Bright);
    assert!(
        result.is_ok(),
        "Failed to parse brightness response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryResponse::Bright { position }) => {
            assert_eq!(position, 0x08, "Brightness position mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_gain_level_inquiry() {
    let data = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x05, 0xFF];
    let result = Response::parse_with_type(&data, &ResponseKind::Gain);
    assert!(
        result.is_ok(),
        "Failed to parse gain response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryResponse::GainLevel { gain }) => {
            assert_eq!(gain, 0x05, "Gain value mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_contrast_inquiry() {
    let data = vec![0x90, 0x50, 0x0C, 0xFF];
    let result = Response::parse_with_type(&data, &ResponseKind::Contrast);
    assert!(
        result.is_ok(),
        "Failed to parse contrast response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryResponse::Contrast(value)) => {
            assert_eq!(value, 0x0C, "Contrast value mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_luminance_inquiry() {
    let data = vec![0x90, 0x50, 0x07, 0xFF];
    let result = Response::parse_with_type(&data, &ResponseKind::Luminance);
    assert!(
        result.is_ok(),
        "Failed to parse luminance response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryResponse::Luminance(value)) => {
            assert_eq!(value, 0x07, "Luminance value mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_resolution_inquiry() {
    let data = vec![0x90, 0x50, 0x01, 0xFF];
    let result = Response::parse_with_type(&data, &ResponseKind::Resolution);
    assert!(
        result.is_ok(),
        "Failed to parse resolution response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryResponse::Resolution(code)) => {
            assert_eq!(code, 0x01, "Resolution code mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_sharpness_inquiry() {
    let data = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x08, 0xFF];
    let result = Response::parse_with_type(&data, &ResponseKind::Sharpness);
    assert!(
        result.is_ok(),
        "Failed to parse sharpness response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryResponse::Sharpness { value }) => {
            assert_eq!(value, 0x08, "Sharpness value mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_iris_inquiry() {
    let data = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0A, 0xFF];
    let result = Response::parse_with_type(&data, &ResponseKind::Iris);
    assert!(
        result.is_ok(),
        "Failed to parse iris response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryResponse::Iris { position }) => {
            assert_eq!(position, 0x0A, "Iris position mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_shutter_inquiry() {
    let data = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF];
    let result = Response::parse_with_type(&data, &ResponseKind::Shutter);
    assert!(
        result.is_ok(),
        "Failed to parse shutter response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryResponse::Shutter { position }) => {
            assert_eq!(position, 0x07, "Shutter position mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_noise_reduction_2d_inquiry() {
    let data = vec![0x90, 0x50, 0x03, 0xFF];
    let result = Response::parse_with_type(&data, &ResponseKind::NoiseReduction2D);
    assert!(
        result.is_ok(),
        "Failed to parse 2D NR response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryResponse::NoiseReduction2D { level }) => {
            assert_eq!(level, 0x03, "2D NR level mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_noise_reduction_3d_inquiry() {
    let data = vec![0x90, 0x50, 0x05, 0xFF];
    let result = Response::parse_with_type(&data, &ResponseKind::NoiseReduction3D);
    assert!(
        result.is_ok(),
        "Failed to parse 3D NR response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryResponse::NoiseReduction3D { level }) => {
            assert_eq!(level, 0x05, "3D NR level mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_saturation_inquiry() {
    let data = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0A, 0xFF];
    let result = Response::parse_with_type(&data, &ResponseKind::Saturation);
    assert!(
        result.is_ok(),
        "Failed to parse saturation response: {:?}",
        result
    );

    match result.unwrap() {
        Response::Inquiry(InquiryResponse::Saturation { level }) => {
            assert_eq!(level, 0x0A, "Saturation level mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_parse_hue_inquiry() {
    let data = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF];
    let result = Response::parse_with_type(&data, &ResponseKind::Hue);
    assert!(result.is_ok(), "Failed to parse hue response: {:?}", result);

    match result.unwrap() {
        Response::Inquiry(InquiryResponse::Hue { hue }) => {
            assert_eq!(hue, 0x07, "Hue value mismatch");
        }
        _ => panic!("Unexpected response type"),
    }
}

#[test]
fn test_error_handling() {
    // Test missing terminator
    let data = vec![0x90, 0x50, 0x02];
    let result = Response::parse_with_type(&data, &ResponseKind::Power);
    assert!(result.is_err(), "Should fail without terminator");

    // Test insufficient data for pan/tilt
    let data = vec![0x90, 0x50, 0x00, 0x01, 0xFF];
    let result = Response::parse_with_type(&data, &ResponseKind::PanTiltPosition);
    assert!(result.is_err(), "Should fail with insufficient data");

    // Test empty data
    let data = vec![];
    let result = Response::parse_with_type(&data, &ResponseKind::Power);
    assert!(result.is_err(), "Should fail with empty data");
}
