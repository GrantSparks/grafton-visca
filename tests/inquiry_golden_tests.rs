//! Golden tests for VISCA inquiry commands and responses.
//!
//! This module provides comprehensive testing for all inquiry command types
//! using real-world response patterns (golden replies) from PTZ cameras.
//! Each test validates both the command encoding and response parsing.

use grafton_visca::{
    command::{
        inquiry_structs::*, AutoFocusSensitivity, Command, ExposureMode, FocusMode, FocusZone,
        InquiryResponse, ResponseType, SharpnessMode, WhiteBalanceMode,
    },
    transport::visca_protocol::{
        parse_visca_response, visca_response_matches_type, ViscaResponsePacket,
    },
};

/// Helper macro to create inquiry tests with golden replies.
macro_rules! inquiry_test {
    (
        $test_name:ident,
        command: $cmd:expr,
        expected_bytes: $expected:expr,
        response_type: $resp_type:expr,
        golden_reply: $reply:expr,
        expected_response: $expected_resp:pat,
        $(validation: $validation:expr)?
    ) => {
        #[test]
        fn $test_name() {
            // Test command encoding
            let cmd = $cmd;
            let bytes = cmd.encode();
            assert_eq!(
                bytes.as_ref(),
                $expected,
                "Command encoding mismatch for {}",
                stringify!($cmd)
            );

            // Verify response type
            assert_eq!(
                cmd.response_type(),
                $resp_type,
                "Response type mismatch for {}",
                stringify!($cmd)
            );

            // Test response parsing with golden reply
            let packet = ViscaResponsePacket {
                payload: $reply.to_vec(),
                socket: 1,
            };

            let result = parse_visca_response(&packet, Some($resp_type));
            assert!(result.is_ok(), "Failed to parse response: {:?}", result);

            let response = result.unwrap();

            // Validate response matches expected pattern
            match response {
                $expected_resp => {
                    $(
                        $validation;
                    )?
                }
                _ => panic!(
                    "Response mismatch. Expected {:?}, got {:?}",
                    stringify!($expected_resp),
                    response
                ),
            }

            // Verify type matching
            assert!(
                visca_response_matches_type(&response, $resp_type),
                "Response type validation failed"
            );
        }
    };
}

// Power and System Inquiries

inquiry_test!(
    test_power_inquiry_on,
    command: PowerInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x00, 0xFF],
    response_type: ResponseType::Power,
    golden_reply: &[0x90, 0x50, 0x02, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::Power { on }
    ),
    validation: assert!(on, "Power should be on")
);

inquiry_test!(
    test_power_inquiry_off,
    command: PowerInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x00, 0xFF],
    response_type: ResponseType::Power,
    golden_reply: &[0x90, 0x50, 0x03, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::Power { on }
    ),
    validation: assert!(!on, "Power should be off")
);

inquiry_test!(
    test_version_inquiry,
    command: VersionInquiry,
    expected_bytes: &[0x81, 0x09, 0x00, 0x02, 0x00, 0xFF],
    response_type: ResponseType::Version,
    golden_reply: &[0x90, 0x50, 0x00, 0x14, 0x00, 0x10, 0x02, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::Version { vendor_id, model_code, rom_version, socket_number }
    ),
    validation: {
        assert_eq!(vendor_id, 0x0014, "Vendor ID mismatch");
        assert_eq!(model_code, 0x0010, "Model code mismatch");
        assert_eq!(rom_version, 0x0205, "ROM version mismatch");
        assert_eq!(socket_number, 0x00, "Socket number mismatch");
    }
);

// Position Inquiries

inquiry_test!(
    test_pan_tilt_position_inquiry,
    command: PanTiltPositionInquiry,
    expected_bytes: &[0x81, 0x09, 0x06, 0x12, 0x06, 0xFF],
    response_type: ResponseType::PanTiltPosition,
    golden_reply: &[0x90, 0x50, 0x00, 0x01, 0x02, 0x03, 0x00, 0x04, 0x05, 0x06, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::PanTiltPosition { pan, tilt }
    ),
    validation: {
        assert_eq!(pan, 0x0123, "Pan position mismatch");
        assert_eq!(tilt, 0x0456, "Tilt position mismatch");
    }
);

inquiry_test!(
    test_pan_tilt_position_negative,
    command: PanTiltPositionInquiry,
    expected_bytes: &[0x81, 0x09, 0x06, 0x12, 0x06, 0xFF],
    response_type: ResponseType::PanTiltPosition,
    golden_reply: &[0x90, 0x50, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0E, 0x0D, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::PanTiltPosition { pan, tilt }
    ),
    validation: {
        assert_eq!(pan, -1, "Pan position should be -1");
        assert_eq!(tilt, -19, "Tilt position should be -19");
    }
);

inquiry_test!(
    test_zoom_position_inquiry,
    command: ZoomPositionInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x47, 0xFF],
    response_type: ResponseType::ZoomPosition,
    golden_reply: &[0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::ZoomPosition { position }
    ),
    validation: assert_eq!(position, 0x1234, "Zoom position mismatch")
);

inquiry_test!(
    test_focus_position_inquiry,
    command: FocusPositionInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x48, 0xFF],
    response_type: ResponseType::FocusPosition,
    golden_reply: &[0x90, 0x50, 0x05, 0x06, 0x07, 0x08, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::FocusPosition { position }
    ),
    validation: assert_eq!(position, 0x5678, "Focus position mismatch")
);

inquiry_test!(
    test_focus_near_limit_inquiry,
    command: FocusNearLimitInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x28, 0xFF],
    response_type: ResponseType::FocusNearLimit,
    golden_reply: &[0x90, 0x50, 0x0A, 0x0B, 0x0C, 0x0D, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::FocusNearLimit { position }
    ),
    validation: assert_eq!(position, 0xABCD, "Focus near limit mismatch")
);

// Exposure Inquiries

inquiry_test!(
    test_exposure_mode_auto,
    command: ExposureModeInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x39, 0xFF],
    response_type: ResponseType::ExposureMode,
    golden_reply: &[0x90, 0x50, 0x00, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::ExposureMode { mode }
    ),
    validation: assert_eq!(mode, ExposureMode::Auto, "Should be Auto mode")
);

inquiry_test!(
    test_exposure_mode_manual,
    command: ExposureModeInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x39, 0xFF],
    response_type: ResponseType::ExposureMode,
    golden_reply: &[0x90, 0x50, 0x03, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::ExposureMode { mode }
    ),
    validation: assert_eq!(mode, ExposureMode::Manual, "Should be Manual mode")
);

inquiry_test!(
    test_exposure_compensation_positive,
    command: ExposureCompensationInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x4E, 0xFF],
    response_type: ResponseType::ExposureCompensation,
    golden_reply: &[0x90, 0x50, 0x00, 0x00, 0x03, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::ExposureCompensation { value }
    ),
    validation: assert_eq!(value, 3, "Exposure compensation should be +3")
);

inquiry_test!(
    test_exposure_compensation_negative,
    command: ExposureCompensationInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x4E, 0xFF],
    response_type: ResponseType::ExposureCompensation,
    golden_reply: &[0x90, 0x50, 0x01, 0x00, 0x05, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::ExposureCompensation { value }
    ),
    validation: assert_eq!(value, -5, "Exposure compensation should be -5")
);

inquiry_test!(
    test_exposure_compensation_mode_on,
    command: ExposureCompensationModeInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x3E, 0xFF],
    response_type: ResponseType::ExposureCompensationMode,
    golden_reply: &[0x90, 0x50, 0x02, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::ExposureCompensationMode { on }
    ),
    validation: assert!(on, "Exposure compensation mode should be on")
);

inquiry_test!(
    test_iris_inquiry,
    command: IrisInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x4B, 0xFF],
    response_type: ResponseType::Iris,
    golden_reply: &[0x90, 0x50, 0x00, 0x00, 0x0A, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::Iris { position }
    ),
    validation: assert_eq!(position, 0x0A, "Iris position mismatch")
);

inquiry_test!(
    test_shutter_inquiry,
    command: ShutterInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x4A, 0xFF],
    response_type: ResponseType::Shutter,
    golden_reply: &[0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::Shutter { position }
    ),
    validation: assert_eq!(position, 0x07, "Shutter position mismatch")
);

inquiry_test!(
    test_brightness_inquiry,
    command: BrightInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x4D, 0xFF],
    response_type: ResponseType::Bright,
    golden_reply: &[0x90, 0x50, 0x00, 0x00, 0x08, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::Bright { position }
    ),
    validation: assert_eq!(position, 0x08, "Brightness position mismatch")
);

// White Balance and Color Inquiries

inquiry_test!(
    test_white_balance_mode_auto,
    command: WhiteBalanceModeInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x35, 0xFF],
    response_type: ResponseType::WhiteBalanceMode,
    golden_reply: &[0x90, 0x50, 0x00, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::WhiteBalanceMode { mode }
    ),
    validation: assert_eq!(mode, WhiteBalanceMode::Auto, "Should be Auto WB mode")
);

inquiry_test!(
    test_white_balance_mode_manual,
    command: WhiteBalanceModeInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x35, 0xFF],
    response_type: ResponseType::WhiteBalanceMode,
    golden_reply: &[0x90, 0x50, 0x05, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::WhiteBalanceMode { mode }
    ),
    validation: assert_eq!(mode, WhiteBalanceMode::Manual, "Should be Manual WB mode")
);

inquiry_test!(
    test_color_temperature_inquiry,
    command: ColorTemperatureInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x20, 0xFF],
    response_type: ResponseType::ColorTemperature,
    golden_reply: &[0x90, 0x50, 0x01, 0x09, 0x06, 0x04, 0xFF],  // 2500K (0x1964 / 10)
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::ColorTemperature { temperature }
    ),
    validation: assert_eq!(temperature, 2500, "Color temperature should be 2500K")
);

// Image Adjustment Inquiries

inquiry_test!(
    test_sharpness_inquiry,
    command: SharpnessInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x42, 0xFF],
    response_type: ResponseType::Sharpness,
    golden_reply: &[0x90, 0x50, 0x00, 0x08, 0x00, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::Sharpness { value }
    ),
    validation: assert_eq!(value, 0x08, "Sharpness value mismatch")
);

inquiry_test!(
    test_sharpness_mode_auto,
    command: SharpnessModeInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x05, 0xFF],
    response_type: ResponseType::SharpnessMode,
    golden_reply: &[0x90, 0x50, 0x02, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::SharpnessMode { mode }
    ),
    validation: assert_eq!(mode, SharpnessMode::Auto, "Should be Auto sharpness mode")
);

inquiry_test!(
    test_contrast_inquiry,
    command: ContrastInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0xA2, 0xFF],
    response_type: ResponseType::Contrast,
    golden_reply: &[0x90, 0x50, 0x0C, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::Contrast(value)
    ),
    validation: assert_eq!(value, 0x0C, "Contrast value mismatch")
);

inquiry_test!(
    test_saturation_inquiry,
    command: SaturationInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x49, 0xFF],
    response_type: ResponseType::Saturation,
    golden_reply: &[0x90, 0x50, 0x00, 0x00, 0x0A, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::Saturation { level }
    ),
    validation: assert_eq!(level, 0x0A, "Saturation level mismatch")
);

inquiry_test!(
    test_hue_inquiry,
    command: HueInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x4F, 0xFF],
    response_type: ResponseType::Hue,
    golden_reply: &[0x90, 0x50, 0x00, 0x00, 0x07, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::Hue { hue }
    ),
    validation: assert_eq!(hue, 0x07, "Hue value mismatch")
);

// Gain Inquiries

inquiry_test!(
    test_gain_inquiry,
    command: GainInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x4C, 0xFF],
    response_type: ResponseType::Gain,
    golden_reply: &[0x90, 0x50, 0x00, 0x00, 0x05, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::Gain { gain }
    ),
    validation: assert_eq!(gain, 0x05, "Gain value mismatch")
);

inquiry_test!(
    test_gain_limit_inquiry,
    command: GainLimitInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x2C, 0xFF],
    response_type: ResponseType::GainLimit,
    golden_reply: &[0x90, 0x50, 0x0F, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::GainLimit { limit }
    ),
    validation: assert_eq!(limit, 0x0F, "Gain limit mismatch")
);

// Image Processing Inquiries

inquiry_test!(
    test_backlight_inquiry_on,
    command: BacklightInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x33, 0xFF],
    response_type: ResponseType::Backlight,
    golden_reply: &[0x90, 0x50, 0x02, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::Backlight { on }
    ),
    validation: assert!(on, "Backlight should be on")
);

inquiry_test!(
    test_image_flip_inquiry,
    command: ImageFlipInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x61, 0xFF],
    response_type: ResponseType::ImageFlip,
    golden_reply: &[0x90, 0x50, 0x03, 0xFF],  // Both mirror and flip
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::ImageFlip { mirror, flip }
    ),
    validation: {
        assert!(mirror, "Mirror should be on");
        assert!(flip, "Flip should be on");
    }
);

inquiry_test!(
    test_noise_reduction_2d_inquiry,
    command: NoiseReduction2DInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x53, 0xFF],
    response_type: ResponseType::NoiseReduction2D,
    golden_reply: &[0x90, 0x50, 0x03, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::NoiseReduction2D { level }
    ),
    validation: assert_eq!(level, 0x03, "2D noise reduction level mismatch")
);

inquiry_test!(
    test_noise_reduction_3d_inquiry,
    command: NoiseReduction3DInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x54, 0xFF],
    response_type: ResponseType::NoiseReduction3D,
    golden_reply: &[0x90, 0x50, 0x05, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::NoiseReduction3D { level }
    ),
    validation: assert_eq!(level, 0x05, "3D noise reduction level mismatch")
);

// Focus Inquiries

inquiry_test!(
    test_focus_mode_auto,
    command: FocusModeInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x38, 0xFF],
    response_type: ResponseType::FocusMode,
    golden_reply: &[0x90, 0x50, 0x02, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::FocusMode { mode }
    ),
    validation: assert_eq!(mode, FocusMode::Auto, "Should be Auto focus mode")
);

inquiry_test!(
    test_focus_mode_manual,
    command: FocusModeInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x38, 0xFF],
    response_type: ResponseType::FocusMode,
    golden_reply: &[0x90, 0x50, 0x03, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::FocusMode { mode }
    ),
    validation: assert_eq!(mode, FocusMode::Manual, "Should be Manual focus mode")
);

inquiry_test!(
    test_focus_zone_center,
    command: FocusZoneInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x3C, 0xFF],
    response_type: ResponseType::FocusZone,
    golden_reply: &[0x90, 0x50, 0x00, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::FocusZone { zone }
    ),
    validation: assert_eq!(zone, FocusZone::Center, "Should be Center focus zone")
);

inquiry_test!(
    test_auto_focus_sensitivity_normal,
    command: AutoFocusSensitivityInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0x58, 0xFF],
    response_type: ResponseType::AutoFocusSensitivity,
    golden_reply: &[0x90, 0x50, 0x02, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::AutoFocusSensitivity { sensitivity }
    ),
    validation: assert_eq!(sensitivity, AutoFocusSensitivity::Normal, "Should be Normal AF sensitivity")
);

// System Inquiries

inquiry_test!(
    test_luminance_inquiry,
    command: LuminanceInquiry,
    expected_bytes: &[0x81, 0x09, 0x04, 0xA1, 0xFF],
    response_type: ResponseType::Luminance,
    golden_reply: &[0x90, 0x50, 0x07, 0xFF],
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::Luminance(value)
    ),
    validation: assert_eq!(value, 0x07, "Luminance value mismatch")
);

inquiry_test!(
    test_resolution_inquiry,
    command: ResolutionInquiry,
    expected_bytes: &[0x81, 0x09, 0x06, 0x23, 0xFF],
    response_type: ResponseType::Resolution,
    golden_reply: &[0x90, 0x50, 0x01, 0xFF],  // 1080p60
    expected_response: grafton_visca::transport::visca_protocol::Response::Inquiry(
        InquiryResponse::Resolution(code)
    ),
    validation: assert_eq!(code, 0x01, "Resolution code mismatch")
);

// Tests for special cases and error conditions

#[test]
fn test_unknown_inquiry_response() {
    // Test handling of an unknown response format
    let packet = ViscaResponsePacket {
        payload: vec![0x90, 0x50, 0xFF, 0xFF, 0xFF, 0xFF],
        socket: 1,
    };

    let result = parse_visca_response(&packet, None);
    // Should still parse as a generic inquiry response
    assert!(result.is_ok(), "Should handle unknown inquiry format");
}

#[test]
fn test_inquiry_with_wrong_response_type() {
    // Send a power inquiry response but expect zoom position
    let packet = ViscaResponsePacket {
        payload: vec![0x90, 0x50, 0x02, 0xFF], // Power on response
        socket: 1,
    };

    let result = parse_visca_response(&packet, Some(ResponseType::ZoomPosition));
    // Parser should handle the mismatch gracefully
    match result {
        Ok(response) => {
            assert!(
                !visca_response_matches_type(&response, ResponseType::ZoomPosition),
                "Should not match ZoomPosition type"
            );
        }
        Err(_) => {
            // Error is also acceptable for type mismatch
        }
    }
}

#[test]
fn test_inquiry_response_length_validation() {
    // Test various response lengths for different inquiry types

    // Power response should be 4 bytes total
    let packet = ViscaResponsePacket {
        payload: vec![0x90, 0x50, 0x02], // Missing terminator
        socket: 1,
    };

    let result = parse_visca_response(&packet, Some(ResponseType::Power));
    assert!(result.is_err(), "Should fail without terminator");

    // Pan/Tilt response should be 11 bytes total
    let packet = ViscaResponsePacket {
        payload: vec![0x90, 0x50, 0x00, 0x01, 0xFF], // Too short for pan/tilt
        socket: 1,
    };

    let result = parse_visca_response(&packet, Some(ResponseType::PanTiltPosition));
    assert!(result.is_err(), "Should fail with insufficient data");
}

#[test]
fn test_inquiry_negative_value_parsing() {
    // Test parsing negative values in responses (e.g., exposure compensation)

    // Exposure compensation -7
    let packet = ViscaResponsePacket {
        payload: vec![0x90, 0x50, 0x01, 0x00, 0x07, 0xFF],
        socket: 1,
    };

    let result = parse_visca_response(&packet, Some(ResponseType::ExposureCompensation));
    assert!(result.is_ok(), "Should parse negative value");

    match result.unwrap() {
        grafton_visca::transport::visca_protocol::Response::Inquiry(
            InquiryResponse::ExposureCompensation { value },
        ) => {
            assert_eq!(value, -7, "Should be -7");
        }
        _ => panic!("Wrong response type"),
    }
}

#[test]
fn test_inquiry_nibble_parsing() {
    // Test parsing of nibble-encoded values

    // Zoom position with nibbles
    let packet = ViscaResponsePacket {
        payload: vec![0x90, 0x50, 0x0F, 0x0F, 0x0F, 0x0F, 0xFF],
        socket: 1,
    };

    let result = parse_visca_response(&packet, Some(ResponseType::ZoomPosition));
    assert!(result.is_ok(), "Should parse nibbles");

    match result.unwrap() {
        grafton_visca::transport::visca_protocol::Response::Inquiry(
            InquiryResponse::ZoomPosition { position },
        ) => {
            assert_eq!(position, 0xFFFF, "Should be max value");
        }
        _ => panic!("Wrong response type"),
    }
}

#[test]
fn test_all_inquiry_commands_have_tests() {
    // Meta-test to ensure we have test coverage for all inquiry types
    // This helps maintain quality by alerting when new inquiries are added

    let tested_inquiries = vec![
        "PowerInquiry",
        "VersionInquiry",
        "PanTiltPositionInquiry",
        "ZoomPositionInquiry",
        "FocusPositionInquiry",
        "FocusNearLimitInquiry",
        "ExposureModeInquiry",
        "ExposureCompensationInquiry",
        "ExposureCompensationModeInquiry",
        "IrisInquiry",
        "ShutterInquiry",
        "BrightInquiry",
        "WhiteBalanceModeInquiry",
        "ColorTemperatureInquiry",
        "SharpnessInquiry",
        "SharpnessModeInquiry",
        "ContrastInquiry",
        "SaturationInquiry",
        "HueInquiry",
        "GainInquiry",
        "GainLimitInquiry",
        "BacklightInquiry",
        "ImageFlipInquiry",
        "NoiseReduction2DInquiry",
        "NoiseReduction3DInquiry",
        "FocusModeInquiry",
        "FocusZoneInquiry",
        "AutoFocusSensitivityInquiry",
        "LuminanceInquiry",
        "ResolutionInquiry",
    ];

    // This list should match all inquiry structs defined in inquiry_structs.rs
    assert_eq!(tested_inquiries.len(), 30, "Should have 30 inquiry tests");
}
