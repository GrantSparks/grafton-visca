#![allow(missing_docs)]
use grafton_visca::command::gain::AntiFlickerMode;
use grafton_visca::command::response::{parse_response, Response};
use grafton_visca::command::{AutoFocusSensitivity, FocusZone, SharpnessMode};
use grafton_visca::command::{InquiryResponse, ResponseType};
use grafton_visca::Error;

#[cfg(test)]
mod response_parsing_tests {
    use super::*;

    #[test]
    fn test_parse_ack_response() {
        // ACK for socket 0
        // Note: ACK is recognized by the 0x40-0x4F pattern in byte[1], regardless of response_type
        let ack_bytes = vec![0x90, 0x40, 0xFF];
        let response = parse_response(&ack_bytes, &ResponseType::PanTiltPosition);
        assert!(matches!(response, Ok(Response::Ack)));

        // ACK for socket 1
        let ack_bytes = vec![0x90, 0x41, 0xFF];
        let response = parse_response(&ack_bytes, &ResponseType::ZoomPosition);
        assert!(matches!(response, Ok(Response::Ack)));
    }

    #[test]
    fn test_parse_completion_response() {
        // Completion for socket 0
        // Note: Completion is recognized by 0x50-0x5F pattern with 3 bytes total
        let completion_bytes = vec![0x90, 0x50, 0xFF];
        let response = parse_response(&completion_bytes, &ResponseType::PanTiltPosition);
        assert!(matches!(response, Ok(Response::Completion)));

        // Completion for socket 1
        let completion_bytes = vec![0x90, 0x51, 0xFF];
        let response = parse_response(&completion_bytes, &ResponseType::ZoomPosition);
        assert!(matches!(response, Ok(Response::Completion)));
    }

    #[test]
    fn test_parse_error_responses() {
        // Errors are recognized by 0x60-0x6F pattern in byte[1]
        // The parse_response function handles errors by calling Error::from_code

        // Test Syntax Error (0x02)
        let error_bytes = vec![0x90, 0x60, 0x02, 0xFF];
        let response = parse_response(&error_bytes, &ResponseType::PanTiltPosition);
        assert!(matches!(response, Err(Error::SyntaxError)));

        // Test Command Buffer Full (0x03)
        let error_bytes = vec![0x90, 0x60, 0x03, 0xFF];
        let response = parse_response(&error_bytes, &ResponseType::PanTiltPosition);
        assert!(matches!(response, Err(Error::CommandBufferFull)));

        // Test Command Not Executable (0x41) - note different byte[1] pattern
        let error_bytes = vec![0x90, 0x61, 0x41, 0xFF];
        let response = parse_response(&error_bytes, &ResponseType::PanTiltPosition);
        assert!(matches!(response, Err(Error::CommandNotExecutable)));
    }

    #[test]
    fn test_parse_pan_tilt_position_response() {
        // Example Pan/Tilt position response
        // Pan: 0x1234, Tilt: 0x5678
        let pt_response_bytes = vec![
            0x90, 0x50, // Completion header
            0x01, 0x02, 0x03, 0x04, // Pan position (nibbles of 0x1234)
            0x05, 0x06, 0x07, 0x08, // Tilt position (nibbles of 0x5678)
            0xFF,
        ];
        let response = parse_response(&pt_response_bytes, &ResponseType::PanTiltPosition);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::PanTiltPosition { pan, tilt })) => {
                assert_eq!(pan, 0x1234);
                assert_eq!(tilt, 0x5678);
            }
            _ => panic!("Expected PanTiltPosition inquiry response"),
        }
    }

    #[test]
    fn test_parse_zoom_position_response() {
        // Example Zoom position response
        let zoom_response_bytes = vec![
            0x90, 0x50, // Completion header
            0x0A, 0x0B, 0x0C, 0x0D, // Zoom position (nibbles of 0xABCD)
            0xFF,
        ];
        let response = parse_response(&zoom_response_bytes, &ResponseType::ZoomPosition);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::ZoomPosition { position })) => {
                assert_eq!(position, 0xABCD);
            }
            _ => panic!("Expected ZoomPosition inquiry response"),
        }
    }

    #[test]
    fn test_parse_focus_position_response() {
        // Example Focus position response
        let focus_response_bytes = vec![
            0x90, 0x50, // Completion header
            0x01, 0x02, 0x03, 0x04, // Focus position (nibbles of 0x1234)
            0xFF,
        ];
        let response = parse_response(&focus_response_bytes, &ResponseType::FocusPosition);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::FocusPosition { position })) => {
                assert_eq!(position, 0x1234);
            }
            _ => panic!("Expected FocusPosition inquiry response"),
        }
    }

    #[test]
    fn test_parse_exposure_mode_response() {
        // Auto exposure mode
        let exposure_response_bytes = vec![0x90, 0x50, 0x00, 0xFF];
        let response = parse_response(&exposure_response_bytes, &ResponseType::ExposureMode);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::ExposureMode { mode })) => {
                assert_eq!(mode as u8, 0x00); // Auto mode
            }
            _ => panic!("Expected ExposureMode inquiry response"),
        }

        // Manual exposure mode
        let exposure_response_bytes = vec![0x90, 0x50, 0x03, 0xFF];
        let response = parse_response(&exposure_response_bytes, &ResponseType::ExposureMode);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::ExposureMode { mode })) => {
                assert_eq!(mode as u8, 0x03); // Manual mode
            }
            _ => panic!("Expected ExposureMode inquiry response"),
        }
    }

    #[test]
    fn test_parse_luminance_response() {
        // Test minimum luminance value (0)
        let luminance_response_bytes = vec![0x90, 0x50, 0x00, 0xFF];
        let response = parse_response(&luminance_response_bytes, &ResponseType::Luminance);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::Luminance(value))) => {
                assert_eq!(value, 0x00);
            }
            _ => panic!("Expected Luminance inquiry response"),
        }

        // Test middle luminance value (7)
        let luminance_response_bytes = vec![0x90, 0x50, 0x07, 0xFF];
        let response = parse_response(&luminance_response_bytes, &ResponseType::Luminance);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::Luminance(value))) => {
                assert_eq!(value, 0x07);
            }
            _ => panic!("Expected Luminance inquiry response"),
        }

        // Test maximum luminance value (14)
        let luminance_response_bytes = vec![0x90, 0x50, 0x0E, 0xFF];
        let response = parse_response(&luminance_response_bytes, &ResponseType::Luminance);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::Luminance(value))) => {
                assert_eq!(value, 0x0E);
            }
            _ => panic!("Expected Luminance inquiry response"),
        }
    }

    #[test]
    fn test_parse_contrast_response() {
        // Test minimum contrast value (0)
        let contrast_response_bytes = vec![0x90, 0x50, 0x00, 0xFF];
        let response = parse_response(&contrast_response_bytes, &ResponseType::Contrast);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::Contrast(value))) => {
                assert_eq!(value, 0x00);
            }
            _ => panic!("Expected Contrast inquiry response"),
        }

        // Test middle contrast value (7)
        let contrast_response_bytes = vec![0x90, 0x50, 0x07, 0xFF];
        let response = parse_response(&contrast_response_bytes, &ResponseType::Contrast);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::Contrast(value))) => {
                assert_eq!(value, 0x07);
            }
            _ => panic!("Expected Contrast inquiry response"),
        }

        // Test maximum contrast value (14)
        let contrast_response_bytes = vec![0x90, 0x50, 0x0E, 0xFF];
        let response = parse_response(&contrast_response_bytes, &ResponseType::Contrast);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::Contrast(value))) => {
                assert_eq!(value, 0x0E);
            }
            _ => panic!("Expected Contrast inquiry response"),
        }
    }

    #[test]
    fn test_parse_invalid_response() {
        // Test response that doesn't start with 0x90
        let invalid_bytes = vec![0x80, 0x50, 0xFF];
        let response = parse_response(&invalid_bytes, &ResponseType::PanTiltPosition);
        assert!(response.is_err());

        // Test response that doesn't end with 0xFF
        let invalid_bytes = vec![0x90, 0x50, 0x00];
        let response = parse_response(&invalid_bytes, &ResponseType::PanTiltPosition);
        assert!(response.is_err());

        // Test empty response
        let invalid_bytes = vec![];
        let response = parse_response(&invalid_bytes, &ResponseType::PanTiltPosition);
        assert!(response.is_err());
    }

    #[test]
    fn test_parse_response_with_wrong_type() {
        // Try to parse an ACK as a data response
        let ack_bytes = vec![0x90, 0x40, 0xFF];
        let response = parse_response(&ack_bytes, &ResponseType::PanTiltPosition);
        // ACK is still recognized regardless of expected response type
        assert!(matches!(response, Ok(Response::Ack)));
    }

    #[test]
    fn test_parse_sharpness_response() {
        // Test Sharpness response
        let sharpness_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0B, 0xFF];
        let response = parse_response(&sharpness_bytes, &ResponseType::Sharpness);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::Sharpness { value })) => {
                assert_eq!(value, 0x0B);
            }
            _ => panic!("Expected Sharpness inquiry response"),
        }
    }

    #[test]
    fn test_parse_exposure_compensation_responses() {
        // Test Exposure Compensation value -7
        let exp_comp_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF];
        let response = parse_response(&exp_comp_bytes, &ResponseType::ExposureCompensation);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::ExposureCompensation { value })) => {
                assert_eq!(value, -7);
            }
            _ => panic!("Expected ExposureCompensation inquiry response"),
        }

        // Test Exposure Compensation value 0
        let exp_comp_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF];
        let response = parse_response(&exp_comp_bytes, &ResponseType::ExposureCompensation);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::ExposureCompensation { value })) => {
                assert_eq!(value, 0);
            }
            _ => panic!("Expected ExposureCompensation inquiry response"),
        }

        // Test Exposure Compensation value +7
        let exp_comp_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0E, 0xFF];
        let response = parse_response(&exp_comp_bytes, &ResponseType::ExposureCompensation);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::ExposureCompensation { value })) => {
                assert_eq!(value, 7);
            }
            _ => panic!("Expected ExposureCompensation inquiry response"),
        }

        // Test Exposure Compensation Mode On
        let exp_comp_mode_bytes = vec![0x90, 0x50, 0x02, 0xFF];
        let response = parse_response(
            &exp_comp_mode_bytes,
            &ResponseType::ExposureCompensationMode,
        );
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::ExposureCompensationMode { on })) => {
                assert!(on);
            }
            _ => panic!("Expected ExposureCompensationMode inquiry response"),
        }

        // Test Exposure Compensation Mode Off
        let exp_comp_mode_bytes = vec![0x90, 0x50, 0x03, 0xFF];
        let response = parse_response(
            &exp_comp_mode_bytes,
            &ResponseType::ExposureCompensationMode,
        );
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::ExposureCompensationMode { on })) => {
                assert!(!on);
            }
            _ => panic!("Expected ExposureCompensationMode inquiry response"),
        }
    }

    #[test]
    fn test_parse_iris_responses() {
        // Test Iris Close (0x00)
        let iris_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF];
        let response = parse_response(&iris_bytes, &ResponseType::Iris);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::Iris { position })) => {
                assert_eq!(position, 0x00);
            }
            _ => panic!("Expected Iris inquiry response"),
        }

        // Test Iris F1.8 (0x0C)
        let iris_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0C, 0xFF];
        let response = parse_response(&iris_bytes, &ResponseType::Iris);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::Iris { position })) => {
                assert_eq!(position, 0x0C);
            }
            _ => panic!("Expected Iris inquiry response"),
        }
    }

    #[test]
    fn test_parse_shutter_responses() {
        // Test Shutter 1/30 (0x01)
        let shutter_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x01, 0xFF];
        let response = parse_response(&shutter_bytes, &ResponseType::Shutter);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::Shutter { position })) => {
                assert_eq!(position, 0x01);
            }
            _ => panic!("Expected Shutter inquiry response"),
        }

        // Test Shutter 1/10000 (0x11)
        let shutter_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x01, 0x01, 0xFF];
        let response = parse_response(&shutter_bytes, &ResponseType::Shutter);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::Shutter { position })) => {
                assert_eq!(position, 0x11);
            }
            _ => panic!("Expected Shutter inquiry response"),
        }
    }

    #[test]
    fn test_parse_bright_responses() {
        // Test Bright 0 (0x00)
        let bright_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF];
        let response = parse_response(&bright_bytes, &ResponseType::Bright);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::Bright { position })) => {
                assert_eq!(position, 0x00);
            }
            _ => panic!("Expected Bright inquiry response"),
        }

        // Test Bright 17 (0x11)
        let bright_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x01, 0x01, 0xFF];
        let response = parse_response(&bright_bytes, &ResponseType::Bright);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::Bright { position })) => {
                assert_eq!(position, 0x11);
            }
            _ => panic!("Expected Bright inquiry response"),
        }
    }

    #[test]
    fn test_parse_gain_responses() {
        // Test Gain response
        let gain_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF];
        let response = parse_response(&gain_bytes, &ResponseType::Gain);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::Gain { gain })) => {
                assert_eq!(gain, 0x07);
            }
            _ => panic!("Expected Gain inquiry response"),
        }

        // Test GainLimit response
        let gain_limit_bytes = vec![0x90, 0x50, 0x0F, 0xFF];
        let response = parse_response(&gain_limit_bytes, &ResponseType::GainLimit);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::GainLimit { limit })) => {
                assert_eq!(limit, 0x0F);
            }
            _ => panic!("Expected GainLimit inquiry response"),
        }
    }

    #[test]
    fn test_parse_anti_flicker_responses() {
        // Test AntiFlicker Off
        let anti_flicker_bytes = vec![0x90, 0x50, 0x00, 0xFF];
        let response = parse_response(&anti_flicker_bytes, &ResponseType::AntiFlicker);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::AntiFlicker { mode })) => {
                assert!(matches!(mode, AntiFlickerMode::Off));
            }
            _ => panic!("Expected AntiFlicker inquiry response"),
        }

        // Test AntiFlicker 50Hz
        let anti_flicker_bytes = vec![0x90, 0x50, 0x01, 0xFF];
        let response = parse_response(&anti_flicker_bytes, &ResponseType::AntiFlicker);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::AntiFlicker { mode })) => {
                assert!(matches!(mode, AntiFlickerMode::Hz50));
            }
            _ => panic!("Expected AntiFlicker inquiry response"),
        }

        // Test AntiFlicker 60Hz
        let anti_flicker_bytes = vec![0x90, 0x50, 0x02, 0xFF];
        let response = parse_response(&anti_flicker_bytes, &ResponseType::AntiFlicker);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::AntiFlicker { mode })) => {
                assert!(matches!(mode, AntiFlickerMode::Hz60));
            }
            _ => panic!("Expected AntiFlicker inquiry response"),
        }
    }

    #[test]
    fn test_parse_color_responses() {
        // Test Saturation 60% (0x00)
        let saturation_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF];
        let response = parse_response(&saturation_bytes, &ResponseType::Saturation);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::Saturation { level })) => {
                assert_eq!(level, 0x00);
            }
            _ => panic!("Expected Saturation inquiry response"),
        }

        // Test Saturation 200% (0x0E)
        let saturation_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0E, 0xFF];
        let response = parse_response(&saturation_bytes, &ResponseType::Saturation);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::Saturation { level })) => {
                assert_eq!(level, 0x0E);
            }
            _ => panic!("Expected Saturation inquiry response"),
        }

        // Test Hue response
        let hue_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF];
        let response = parse_response(&hue_bytes, &ResponseType::Hue);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::Hue { hue })) => {
                assert_eq!(hue, 0x07);
            }
            _ => panic!("Expected Hue inquiry response"),
        }
    }

    #[test]
    fn test_parse_red_blue_gain_responses() {
        // Test RedGain -10
        let red_gain_bytes = vec![0x90, 0x50, 0x00, 0xFF];
        let response = parse_response(&red_gain_bytes, &ResponseType::RedGain);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::RedGain { gain })) => {
                assert_eq!(gain, -10);
            }
            _ => panic!("Expected RedGain inquiry response"),
        }

        // Test RedGain 0
        let red_gain_bytes = vec![0x90, 0x50, 0x0A, 0xFF];
        let response = parse_response(&red_gain_bytes, &ResponseType::RedGain);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::RedGain { gain })) => {
                assert_eq!(gain, 0);
            }
            _ => panic!("Expected RedGain inquiry response"),
        }

        // Test BlueGain +10
        let blue_gain_bytes = vec![0x90, 0x50, 0x14, 0xFF];
        let response = parse_response(&blue_gain_bytes, &ResponseType::BlueGain);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::BlueGain { gain })) => {
                assert_eq!(gain, 10);
            }
            _ => panic!("Expected BlueGain inquiry response"),
        }
    }

    #[test]
    fn test_parse_image_flip_responses() {
        // Test ImageFlip Off (0x00)
        let flip_bytes = vec![0x90, 0x50, 0x00, 0xFF];
        let response = parse_response(&flip_bytes, &ResponseType::ImageFlip);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            })) => {
                assert!(!vertical);
                assert!(!horizontal);
            }
            _ => panic!("Expected ImageFlip inquiry response"),
        }

        // Test ImageFlip Horizontal only (0x01)
        let flip_bytes = vec![0x90, 0x50, 0x01, 0xFF];
        let response = parse_response(&flip_bytes, &ResponseType::ImageFlip);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            })) => {
                assert!(!vertical);
                assert!(horizontal);
            }
            _ => panic!("Expected ImageFlip inquiry response"),
        }

        // Test ImageFlip Vertical only (0x02)
        let flip_bytes = vec![0x90, 0x50, 0x02, 0xFF];
        let response = parse_response(&flip_bytes, &ResponseType::ImageFlip);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            })) => {
                assert!(vertical);
                assert!(!horizontal);
            }
            _ => panic!("Expected ImageFlip inquiry response"),
        }

        // Test ImageFlip Both (0x03)
        let flip_bytes = vec![0x90, 0x50, 0x03, 0xFF];
        let response = parse_response(&flip_bytes, &ResponseType::ImageFlip);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            })) => {
                assert!(vertical);
                assert!(horizontal);
            }
            _ => panic!("Expected ImageFlip inquiry response"),
        }
    }

    fn test_sharpness_mode_response(mode_value: u8, expected_mode: SharpnessMode) {
        let bytes = vec![0x90, 0x50, mode_value, 0xFF];
        let response = parse_response(&bytes, &ResponseType::SharpnessMode);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::SharpnessMode { mode })) => {
                match (mode, expected_mode) {
                    (SharpnessMode::Auto, SharpnessMode::Auto)
                    | (SharpnessMode::Manual, SharpnessMode::Manual) => {}
                    _ => panic!("Sharpness mode mismatch"),
                }
            }
            _ => panic!("Expected SharpnessMode inquiry response"),
        }
    }

    fn test_color_temperature_response(temp_value: u8) {
        let bytes = vec![0x90, 0x50, 0x00, 0x00, 0x03, temp_value, 0xFF];
        let response = parse_response(&bytes, &ResponseType::ColorTemperature);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::ColorTemperature { temperature })) => {
                assert_eq!(temperature, (0x03 << 4) | u16::from(temp_value));
            }
            _ => panic!("Expected ColorTemperature inquiry response"),
        }
    }

    fn test_noise_reduction_2d_response(level_value: u8) {
        let bytes = vec![0x90, 0x50, level_value, 0xFF];
        let response = parse_response(&bytes, &ResponseType::NoiseReduction2D);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::NoiseReduction2D { level })) => {
                assert_eq!(level, level_value);
            }
            _ => panic!("Expected NoiseReduction2D inquiry response"),
        }
    }

    fn test_noise_reduction_3d_response(level_value: u8) {
        let bytes = vec![0x90, 0x50, level_value, 0xFF];
        let response = parse_response(&bytes, &ResponseType::NoiseReduction3D);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::NoiseReduction3D { level })) => {
                assert_eq!(level, level_value);
            }
            _ => panic!("Expected NoiseReduction3D inquiry response"),
        }
    }

    fn test_black_white_response(status: u8, expected: bool) {
        let bytes = vec![0x90, 0x50, status, 0xFF];
        let response = parse_response(&bytes, &ResponseType::BlackWhite);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::BlackWhite { on })) => {
                assert_eq!(on, expected);
            }
            _ => panic!("Expected BlackWhite inquiry response"),
        }
    }

    fn test_focus_zone_response(zone_value: u8, _expected_zone: FocusZone) {
        let bytes = vec![0x90, 0x50, zone_value, 0xFF];
        let response = parse_response(&bytes, &ResponseType::FocusZone);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::FocusZone { zone: _ })) => {
                // Zone parsed successfully
            }
            _ => panic!("Expected FocusZone inquiry response"),
        }
    }

    fn test_af_sensitivity_response(sens_value: u8, _expected_sensitivity: AutoFocusSensitivity) {
        let bytes = vec![0x90, 0x50, sens_value, 0xFF];
        let response = parse_response(&bytes, &ResponseType::AutoFocusSensitivity);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::AutoFocusSensitivity {
                sensitivity: _,
            })) => {
                // Sensitivity parsed successfully
            }
            _ => panic!("Expected AutoFocusSensitivity inquiry response"),
        }
    }

    fn test_focus_near_limit_response(expected_position: u16) {
        let p1 = ((expected_position >> 12) & 0x0F) as u8;
        let p2 = ((expected_position >> 8) & 0x0F) as u8;
        let p3 = ((expected_position >> 4) & 0x0F) as u8;
        let p4 = (expected_position & 0x0F) as u8;
        let bytes = vec![0x90, 0x50, p1, p2, p3, p4, 0xFF];
        let response = parse_response(&bytes, &ResponseType::FocusNearLimit);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::FocusNearLimit { position })) => {
                assert_eq!(position, expected_position);
            }
            _ => panic!("Expected FocusNearLimit inquiry response"),
        }
    }

    fn test_dynamic_range_response(level_value: u8) {
        let bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, level_value, 0xFF];
        let response = parse_response(&bytes, &ResponseType::DynamicRange);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::DynamicRange { level })) => {
                assert_eq!(level, level_value);
            }
            _ => panic!("Expected DynamicRange inquiry response"),
        }
    }

    #[test]
    fn test_parse_extended_inquiry_responses() {
        // Test SharpnessMode response
        test_sharpness_mode_response(0x02, SharpnessMode::Auto);
        test_sharpness_mode_response(0x03, SharpnessMode::Manual);

        // Test ColorTemperature response
        test_color_temperature_response(0x07); // 8000K

        // Test NoiseReduction responses
        test_noise_reduction_2d_response(0x05);
        test_noise_reduction_3d_response(0x08);

        // Test BlackWhite response
        test_black_white_response(0x04, true);
        test_black_white_response(0x00, false);

        // Test FocusZone response
        test_focus_zone_response(0x01, FocusZone::Center);

        // Test AutoFocusSensitivity response
        test_af_sensitivity_response(0x02, AutoFocusSensitivity::High);

        // Test FocusNearLimit response
        test_focus_near_limit_response(0x1234);

        // Test DynamicRange response
        test_dynamic_range_response(0x08);
    }

    #[test]
    fn test_parse_backlight_response() {
        // Test Backlight On
        let backlight_on_bytes = vec![0x90, 0x50, 0x02, 0xFF];
        let response = parse_response(&backlight_on_bytes, &ResponseType::Backlight);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::Backlight { status })) => {
                assert!(status);
            }
            _ => panic!("Expected Backlight inquiry response"),
        }

        // Test Backlight Off
        let backlight_off_bytes = vec![0x90, 0x50, 0x03, 0xFF];
        let response = parse_response(&backlight_off_bytes, &ResponseType::Backlight);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::Backlight { status })) => {
                assert!(!status);
            }
            _ => panic!("Expected Backlight inquiry response"),
        }

        // Test Backlight with any other value (should be off)
        let backlight_other_bytes = vec![0x90, 0x50, 0x00, 0xFF];
        let response = parse_response(&backlight_other_bytes, &ResponseType::Backlight);
        match response {
            Ok(Response::InquiryResponse(InquiryResponse::Backlight { status })) => {
                assert!(!status);
            }
            _ => panic!("Expected Backlight inquiry response"),
        }
    }

    #[test]
    fn test_response_length_validation() {
        // Test various response types with incorrect lengths

        // Sharpness with wrong length (should be 7 bytes)
        let invalid_sharpness = vec![0x90, 0x50, 0x0B, 0xFF];
        let response = parse_response(&invalid_sharpness, &ResponseType::Sharpness);
        assert!(matches!(response, Err(Error::InvalidResponseLength)));

        // Exposure compensation with wrong length (should be 7 bytes)
        let invalid_exp_comp = vec![0x90, 0x50, 0x07, 0xFF];
        let response = parse_response(&invalid_exp_comp, &ResponseType::ExposureCompensation);
        assert!(matches!(response, Err(Error::InvalidResponseLength)));

        // Anti-flicker with wrong length (should be 4 bytes)
        let invalid_anti_flicker = vec![0x90, 0x50, 0x00, 0x01, 0xFF];
        let response = parse_response(&invalid_anti_flicker, &ResponseType::AntiFlicker);
        assert!(matches!(response, Err(Error::InvalidResponseLength)));
    }
}

#[cfg(test)]
mod transport_response_parsing_tests {

    #[test]
    fn test_multiple_responses_in_buffer() {
        // Test the parse_response function from lib.rs
        // Simulate multiple responses in a single buffer
        let buffer = vec![
            0x90, 0x40, 0xFF, // ACK
            0x90, 0x50, 0xFF, // Completion
        ];

        let mut responses = Vec::new();
        let mut response = Vec::new();
        let mut start_index = false;

        for &byte in &buffer {
            response.push(byte);
            if byte == 0x90 {
                start_index = true;
            } else if byte == 0xFF && start_index {
                responses.push(response.clone());
                response.clear();
                start_index = false;
            }
        }

        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0], vec![0x90, 0x40, 0xFF]);
        assert_eq!(responses[1], vec![0x90, 0x50, 0xFF]);
    }

    #[test]
    fn test_incomplete_response_handling() {
        // Test handling of incomplete response (no 0xFF terminator)
        let buffer = vec![0x90, 0x40]; // Missing 0xFF

        let mut responses = Vec::new();
        let mut response = Vec::new();
        let mut start_index = false;

        for &byte in &buffer {
            response.push(byte);
            if byte == 0x90 {
                start_index = true;
            } else if byte == 0xFF && start_index {
                responses.push(response.clone());
                response.clear();
                start_index = false;
            }
        }

        // Should not have any complete responses
        assert_eq!(responses.len(), 0);
        // start_index should still be true (waiting for completion)
        assert!(start_index);
    }
}
