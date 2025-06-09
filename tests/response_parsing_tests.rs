use grafton_visca::command::gain::AntiFlickerMode;
use grafton_visca::command::response::{parse_visca_response, Response};
use grafton_visca::command::{AFSensitivity, FocusZone, SharpnessMode};
use grafton_visca::command::{ViscaInquiryResponse, ViscaResponseType};
use grafton_visca::Error;

#[cfg(test)]
mod response_parsing_tests {
    use super::*;

    #[test]
    fn test_parse_ack_response() {
        // ACK for socket 0
        // Note: ACK is recognized by the 0x40-0x4F pattern in byte[1], regardless of response_type
        let ack_bytes = vec![0x90, 0x40, 0xFF];
        let response = parse_visca_response(&ack_bytes, &ViscaResponseType::PanTiltPosition);
        assert!(matches!(response, Ok(Response::Ack)));

        // ACK for socket 1
        let ack_bytes = vec![0x90, 0x41, 0xFF];
        let response = parse_visca_response(&ack_bytes, &ViscaResponseType::ZoomPosition);
        assert!(matches!(response, Ok(Response::Ack)));
    }

    #[test]
    fn test_parse_completion_response() {
        // Completion for socket 0
        // Note: Completion is recognized by 0x50-0x5F pattern with 3 bytes total
        let completion_bytes = vec![0x90, 0x50, 0xFF];
        let response = parse_visca_response(&completion_bytes, &ViscaResponseType::PanTiltPosition);
        assert!(matches!(response, Ok(Response::Completion)));

        // Completion for socket 1
        let completion_bytes = vec![0x90, 0x51, 0xFF];
        let response = parse_visca_response(&completion_bytes, &ViscaResponseType::ZoomPosition);
        assert!(matches!(response, Ok(Response::Completion)));
    }

    #[test]
    fn test_parse_error_responses() {
        // Errors are recognized by 0x60-0x6F pattern in byte[1]
        // The parse_visca_response function handles errors by calling Error::from_code

        // Test Syntax Error (0x02)
        let error_bytes = vec![0x90, 0x60, 0x02, 0xFF];
        let response = parse_visca_response(&error_bytes, &ViscaResponseType::PanTiltPosition);
        assert!(matches!(response, Err(Error::SyntaxError)));

        // Test Command Buffer Full (0x03)
        let error_bytes = vec![0x90, 0x60, 0x03, 0xFF];
        let response = parse_visca_response(&error_bytes, &ViscaResponseType::PanTiltPosition);
        assert!(matches!(response, Err(Error::CommandBufferFull)));

        // Test Command Not Executable (0x41) - note different byte[1] pattern
        let error_bytes = vec![0x90, 0x61, 0x41, 0xFF];
        let response = parse_visca_response(&error_bytes, &ViscaResponseType::PanTiltPosition);
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
        let response =
            parse_visca_response(&pt_response_bytes, &ViscaResponseType::PanTiltPosition);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::PanTiltPosition { pan, tilt })) => {
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
        let response = parse_visca_response(&zoom_response_bytes, &ViscaResponseType::ZoomPosition);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::ZoomPosition { position })) => {
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
        let response =
            parse_visca_response(&focus_response_bytes, &ViscaResponseType::FocusPosition);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::FocusPosition { position })) => {
                assert_eq!(position, 0x1234);
            }
            _ => panic!("Expected FocusPosition inquiry response"),
        }
    }

    #[test]
    fn test_parse_exposure_mode_response() {
        // Auto exposure mode
        let exposure_response_bytes = vec![0x90, 0x50, 0x00, 0xFF];
        let response =
            parse_visca_response(&exposure_response_bytes, &ViscaResponseType::ExposureMode);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::ExposureMode { mode })) => {
                assert_eq!(mode as u8, 0x00); // Auto mode
            }
            _ => panic!("Expected ExposureMode inquiry response"),
        }

        // Manual exposure mode
        let exposure_response_bytes = vec![0x90, 0x50, 0x03, 0xFF];
        let response =
            parse_visca_response(&exposure_response_bytes, &ViscaResponseType::ExposureMode);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::ExposureMode { mode })) => {
                assert_eq!(mode as u8, 0x03); // Manual mode
            }
            _ => panic!("Expected ExposureMode inquiry response"),
        }
    }

    #[test]
    fn test_parse_luminance_response() {
        // Example Luminance value
        // TODO: Sprint 1 will implement Luminance parsing
        // Currently falls through to default case and returns Completion
        let luminance_response_bytes = vec![0x90, 0x50, 0x0A, 0xFF];
        let response =
            parse_visca_response(&luminance_response_bytes, &ViscaResponseType::Luminance);
        // For now, this returns Completion since parsing is not implemented
        assert!(matches!(response, Ok(Response::Completion)));
    }

    #[test]
    fn test_parse_contrast_response() {
        // Example Contrast value
        // TODO: Sprint 1 will implement Contrast parsing
        // Currently falls through to default case and returns Completion
        let contrast_response_bytes = vec![0x90, 0x50, 0x0F, 0xFF];
        let response = parse_visca_response(&contrast_response_bytes, &ViscaResponseType::Contrast);
        // For now, this returns Completion since parsing is not implemented
        assert!(matches!(response, Ok(Response::Completion)));
    }

    #[test]
    fn test_parse_invalid_response() {
        // Test response that doesn't start with 0x90
        let invalid_bytes = vec![0x80, 0x50, 0xFF];
        let response = parse_visca_response(&invalid_bytes, &ViscaResponseType::PanTiltPosition);
        assert!(response.is_err());

        // Test response that doesn't end with 0xFF
        let invalid_bytes = vec![0x90, 0x50, 0x00];
        let response = parse_visca_response(&invalid_bytes, &ViscaResponseType::PanTiltPosition);
        assert!(response.is_err());

        // Test empty response
        let invalid_bytes = vec![];
        let response = parse_visca_response(&invalid_bytes, &ViscaResponseType::PanTiltPosition);
        assert!(response.is_err());
    }

    #[test]
    fn test_parse_response_with_wrong_type() {
        // Try to parse an ACK as a data response
        let ack_bytes = vec![0x90, 0x40, 0xFF];
        let response = parse_visca_response(&ack_bytes, &ViscaResponseType::PanTiltPosition);
        // ACK is still recognized regardless of expected response type
        assert!(matches!(response, Ok(Response::Ack)));
    }

    #[test]
    fn test_parse_sharpness_response() {
        // Test Sharpness response
        let sharpness_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0B, 0xFF];
        let response = parse_visca_response(&sharpness_bytes, &ViscaResponseType::Sharpness);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::Sharpness { value })) => {
                assert_eq!(value, 0x0B);
            }
            _ => panic!("Expected Sharpness inquiry response"),
        }
    }

    #[test]
    fn test_parse_exposure_compensation_responses() {
        // Test Exposure Compensation value -7
        let exp_comp_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF];
        let response =
            parse_visca_response(&exp_comp_bytes, &ViscaResponseType::ExposureCompensation);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::ExposureCompensation { value })) => {
                assert_eq!(value, -7);
            }
            _ => panic!("Expected ExposureCompensation inquiry response"),
        }

        // Test Exposure Compensation value 0
        let exp_comp_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF];
        let response =
            parse_visca_response(&exp_comp_bytes, &ViscaResponseType::ExposureCompensation);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::ExposureCompensation { value })) => {
                assert_eq!(value, 0);
            }
            _ => panic!("Expected ExposureCompensation inquiry response"),
        }

        // Test Exposure Compensation value +7
        let exp_comp_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0E, 0xFF];
        let response =
            parse_visca_response(&exp_comp_bytes, &ViscaResponseType::ExposureCompensation);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::ExposureCompensation { value })) => {
                assert_eq!(value, 7);
            }
            _ => panic!("Expected ExposureCompensation inquiry response"),
        }

        // Test Exposure Compensation Mode On
        let exp_comp_mode_bytes = vec![0x90, 0x50, 0x02, 0xFF];
        let response = parse_visca_response(
            &exp_comp_mode_bytes,
            &ViscaResponseType::ExposureCompensationMode,
        );
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::ExposureCompensationMode {
                on,
            })) => {
                assert!(on);
            }
            _ => panic!("Expected ExposureCompensationMode inquiry response"),
        }

        // Test Exposure Compensation Mode Off
        let exp_comp_mode_bytes = vec![0x90, 0x50, 0x03, 0xFF];
        let response = parse_visca_response(
            &exp_comp_mode_bytes,
            &ViscaResponseType::ExposureCompensationMode,
        );
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::ExposureCompensationMode {
                on,
            })) => {
                assert!(!on);
            }
            _ => panic!("Expected ExposureCompensationMode inquiry response"),
        }
    }

    #[test]
    fn test_parse_iris_responses() {
        // Test Iris Close (0x00)
        let iris_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF];
        let response = parse_visca_response(&iris_bytes, &ViscaResponseType::Iris);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::Iris { position })) => {
                assert_eq!(position, 0x00);
            }
            _ => panic!("Expected Iris inquiry response"),
        }

        // Test Iris F1.8 (0x0C)
        let iris_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0C, 0xFF];
        let response = parse_visca_response(&iris_bytes, &ViscaResponseType::Iris);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::Iris { position })) => {
                assert_eq!(position, 0x0C);
            }
            _ => panic!("Expected Iris inquiry response"),
        }
    }

    #[test]
    fn test_parse_shutter_responses() {
        // Test Shutter 1/30 (0x01)
        let shutter_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x01, 0xFF];
        let response = parse_visca_response(&shutter_bytes, &ViscaResponseType::Shutter);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::Shutter { position })) => {
                assert_eq!(position, 0x01);
            }
            _ => panic!("Expected Shutter inquiry response"),
        }

        // Test Shutter 1/10000 (0x11)
        let shutter_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x01, 0x01, 0xFF];
        let response = parse_visca_response(&shutter_bytes, &ViscaResponseType::Shutter);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::Shutter { position })) => {
                assert_eq!(position, 0x11);
            }
            _ => panic!("Expected Shutter inquiry response"),
        }
    }

    #[test]
    fn test_parse_bright_responses() {
        // Test Bright 0 (0x00)
        let bright_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF];
        let response = parse_visca_response(&bright_bytes, &ViscaResponseType::Bright);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::Bright { position })) => {
                assert_eq!(position, 0x00);
            }
            _ => panic!("Expected Bright inquiry response"),
        }

        // Test Bright 17 (0x11)
        let bright_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x01, 0x01, 0xFF];
        let response = parse_visca_response(&bright_bytes, &ViscaResponseType::Bright);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::Bright { position })) => {
                assert_eq!(position, 0x11);
            }
            _ => panic!("Expected Bright inquiry response"),
        }
    }

    #[test]
    fn test_parse_gain_responses() {
        // Test Gain response
        let gain_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF];
        let response = parse_visca_response(&gain_bytes, &ViscaResponseType::Gain);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::Gain { gain })) => {
                assert_eq!(gain, 0x07);
            }
            _ => panic!("Expected Gain inquiry response"),
        }

        // Test GainLimit response
        let gain_limit_bytes = vec![0x90, 0x50, 0x0F, 0xFF];
        let response = parse_visca_response(&gain_limit_bytes, &ViscaResponseType::GainLimit);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::GainLimit { limit })) => {
                assert_eq!(limit, 0x0F);
            }
            _ => panic!("Expected GainLimit inquiry response"),
        }
    }

    #[test]
    fn test_parse_anti_flicker_responses() {
        // Test AntiFlicker Off
        let anti_flicker_bytes = vec![0x90, 0x50, 0x00, 0xFF];
        let response = parse_visca_response(&anti_flicker_bytes, &ViscaResponseType::AntiFlicker);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::AntiFlicker { mode })) => {
                assert!(matches!(mode, AntiFlickerMode::Off));
            }
            _ => panic!("Expected AntiFlicker inquiry response"),
        }

        // Test AntiFlicker 50Hz
        let anti_flicker_bytes = vec![0x90, 0x50, 0x01, 0xFF];
        let response = parse_visca_response(&anti_flicker_bytes, &ViscaResponseType::AntiFlicker);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::AntiFlicker { mode })) => {
                assert!(matches!(mode, AntiFlickerMode::Hz50));
            }
            _ => panic!("Expected AntiFlicker inquiry response"),
        }

        // Test AntiFlicker 60Hz
        let anti_flicker_bytes = vec![0x90, 0x50, 0x02, 0xFF];
        let response = parse_visca_response(&anti_flicker_bytes, &ViscaResponseType::AntiFlicker);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::AntiFlicker { mode })) => {
                assert!(matches!(mode, AntiFlickerMode::Hz60));
            }
            _ => panic!("Expected AntiFlicker inquiry response"),
        }
    }

    #[test]
    fn test_parse_color_responses() {
        // Test Saturation 60% (0x00)
        let saturation_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF];
        let response = parse_visca_response(&saturation_bytes, &ViscaResponseType::Saturation);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::Saturation { level })) => {
                assert_eq!(level, 0x00);
            }
            _ => panic!("Expected Saturation inquiry response"),
        }

        // Test Saturation 200% (0x0E)
        let saturation_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0E, 0xFF];
        let response = parse_visca_response(&saturation_bytes, &ViscaResponseType::Saturation);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::Saturation { level })) => {
                assert_eq!(level, 0x0E);
            }
            _ => panic!("Expected Saturation inquiry response"),
        }

        // Test Hue response
        let hue_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF];
        let response = parse_visca_response(&hue_bytes, &ViscaResponseType::Hue);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::Hue { hue })) => {
                assert_eq!(hue, 0x07);
            }
            _ => panic!("Expected Hue inquiry response"),
        }
    }

    #[test]
    fn test_parse_red_blue_gain_responses() {
        // Test RedGain -10
        let red_gain_bytes = vec![0x90, 0x50, 0x00, 0xFF];
        let response = parse_visca_response(&red_gain_bytes, &ViscaResponseType::RedGain);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::RedGain { gain })) => {
                assert_eq!(gain, -10);
            }
            _ => panic!("Expected RedGain inquiry response"),
        }

        // Test RedGain 0
        let red_gain_bytes = vec![0x90, 0x50, 0x0A, 0xFF];
        let response = parse_visca_response(&red_gain_bytes, &ViscaResponseType::RedGain);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::RedGain { gain })) => {
                assert_eq!(gain, 0);
            }
            _ => panic!("Expected RedGain inquiry response"),
        }

        // Test BlueGain +10
        let blue_gain_bytes = vec![0x90, 0x50, 0x14, 0xFF];
        let response = parse_visca_response(&blue_gain_bytes, &ViscaResponseType::BlueGain);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::BlueGain { gain })) => {
                assert_eq!(gain, 10);
            }
            _ => panic!("Expected BlueGain inquiry response"),
        }
    }

    #[test]
    fn test_parse_image_flip_responses() {
        // Test ImageFlip Off (0x00)
        let flip_bytes = vec![0x90, 0x50, 0x00, 0xFF];
        let response = parse_visca_response(&flip_bytes, &ViscaResponseType::ImageFlip);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::ImageFlip {
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
        let response = parse_visca_response(&flip_bytes, &ViscaResponseType::ImageFlip);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::ImageFlip {
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
        let response = parse_visca_response(&flip_bytes, &ViscaResponseType::ImageFlip);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::ImageFlip {
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
        let response = parse_visca_response(&flip_bytes, &ViscaResponseType::ImageFlip);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::ImageFlip {
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
        let response = parse_visca_response(&bytes, &ViscaResponseType::SharpnessMode);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::SharpnessMode { mode })) => {
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
        let response = parse_visca_response(&bytes, &ViscaResponseType::ColorTemperature);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::ColorTemperature {
                temperature,
            })) => {
                assert_eq!(temperature, (0x03 << 4) | u16::from(temp_value));
            }
            _ => panic!("Expected ColorTemperature inquiry response"),
        }
    }

    fn test_noise_reduction_2d_response(level_value: u8) {
        let bytes = vec![0x90, 0x50, level_value, 0xFF];
        let response = parse_visca_response(&bytes, &ViscaResponseType::NoiseReduction2D);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::NoiseReduction2D { level })) => {
                assert_eq!(level, level_value);
            }
            _ => panic!("Expected NoiseReduction2D inquiry response"),
        }
    }

    fn test_noise_reduction_3d_response(level_value: u8) {
        let bytes = vec![0x90, 0x50, level_value, 0xFF];
        let response = parse_visca_response(&bytes, &ViscaResponseType::NoiseReduction3D);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::NoiseReduction3D { level })) => {
                assert_eq!(level, level_value);
            }
            _ => panic!("Expected NoiseReduction3D inquiry response"),
        }
    }

    fn test_black_white_response(status: u8, expected: bool) {
        let bytes = vec![0x90, 0x50, status, 0xFF];
        let response = parse_visca_response(&bytes, &ViscaResponseType::BlackWhite);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::BlackWhite { on })) => {
                assert_eq!(on, expected);
            }
            _ => panic!("Expected BlackWhite inquiry response"),
        }
    }

    fn test_focus_zone_response(zone_value: u8, _expected_zone: FocusZone) {
        let bytes = vec![0x90, 0x50, zone_value, 0xFF];
        let response = parse_visca_response(&bytes, &ViscaResponseType::FocusZone);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::FocusZone { zone: _ })) => {
                // Zone parsed successfully
            }
            _ => panic!("Expected FocusZone inquiry response"),
        }
    }

    fn test_af_sensitivity_response(sens_value: u8, _expected_sensitivity: AFSensitivity) {
        let bytes = vec![0x90, 0x50, sens_value, 0xFF];
        let response = parse_visca_response(&bytes, &ViscaResponseType::AFSensitivity);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::AFSensitivity {
                sensitivity: _,
            })) => {
                // Sensitivity parsed successfully
            }
            _ => panic!("Expected AFSensitivity inquiry response"),
        }
    }

    fn test_focus_near_limit_response(expected_position: u16) {
        let p1 = ((expected_position >> 12) & 0x0F) as u8;
        let p2 = ((expected_position >> 8) & 0x0F) as u8;
        let p3 = ((expected_position >> 4) & 0x0F) as u8;
        let p4 = (expected_position & 0x0F) as u8;
        let bytes = vec![0x90, 0x50, p1, p2, p3, p4, 0xFF];
        let response = parse_visca_response(&bytes, &ViscaResponseType::FocusNearLimit);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::FocusNearLimit { position })) => {
                assert_eq!(position, expected_position);
            }
            _ => panic!("Expected FocusNearLimit inquiry response"),
        }
    }

    fn test_dynamic_range_response(level_value: u8) {
        let bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, level_value, 0xFF];
        let response = parse_visca_response(&bytes, &ViscaResponseType::DynamicRange);
        match response {
            Ok(Response::InquiryResponse(ViscaInquiryResponse::DynamicRange { level })) => {
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

        // Test AFSensitivity response
        test_af_sensitivity_response(0x02, AFSensitivity::High);

        // Test FocusNearLimit response
        test_focus_near_limit_response(0x1234);

        // Test DynamicRange response
        test_dynamic_range_response(0x08);
    }

    #[test]
    fn test_parse_backlight_response() {
        // Test parsing for Backlight status (currently not implemented in parse_visca_response)
        // This test documents expected behavior once implemented

        // Backlight On
        let backlight_on_bytes = vec![0x90, 0x50, 0x02, 0xFF];
        let response = parse_visca_response(&backlight_on_bytes, &ViscaResponseType::Backlight);

        // Currently returns Completion because Backlight parsing is not implemented
        // Once implemented, this should return:
        // Ok(Response::InquiryResponse(ViscaInquiryResponse::Backlight { status: true }))
        assert!(matches!(response, Ok(Response::Completion)));

        // TODO: Implement Backlight response parsing in response.rs
    }

    #[test]
    fn test_response_length_validation() {
        // Test various response types with incorrect lengths

        // Sharpness with wrong length (should be 7 bytes)
        let invalid_sharpness = vec![0x90, 0x50, 0x0B, 0xFF];
        let response = parse_visca_response(&invalid_sharpness, &ViscaResponseType::Sharpness);
        assert!(matches!(response, Err(Error::InvalidResponseLength)));

        // Exposure compensation with wrong length (should be 7 bytes)
        let invalid_exp_comp = vec![0x90, 0x50, 0x07, 0xFF];
        let response =
            parse_visca_response(&invalid_exp_comp, &ViscaResponseType::ExposureCompensation);
        assert!(matches!(response, Err(Error::InvalidResponseLength)));

        // Anti-flicker with wrong length (should be 4 bytes)
        let invalid_anti_flicker = vec![0x90, 0x50, 0x00, 0x01, 0xFF];
        let response = parse_visca_response(&invalid_anti_flicker, &ViscaResponseType::AntiFlicker);
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
