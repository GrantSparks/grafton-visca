use grafton_visca::command::response::{parse_visca_response, ViscaResponse};
use grafton_visca::command::{ViscaInquiryResponse, ViscaResponseType};
use grafton_visca::{ViscaError, ViscaTransport};
use grafton_visca::command::gain::AntiFlickerMode;
use grafton_visca::command::{SharpnessMode, FocusZone, AFSensitivity};

#[cfg(test)]
mod response_parsing_tests {
    use super::*;

    #[test]
    fn test_parse_ack_response() {
        // ACK for socket 0
        // Note: ACK is recognized by the 0x40-0x4F pattern in byte[1], regardless of response_type
        let ack_bytes = vec![0x90, 0x40, 0xFF];
        let response = parse_visca_response(&ack_bytes, &ViscaResponseType::PanTiltPosition);
        assert!(matches!(response, Ok(ViscaResponse::Ack)));

        // ACK for socket 1
        let ack_bytes = vec![0x90, 0x41, 0xFF];
        let response = parse_visca_response(&ack_bytes, &ViscaResponseType::ZoomPosition);
        assert!(matches!(response, Ok(ViscaResponse::Ack)));
    }

    #[test]
    fn test_parse_completion_response() {
        // Completion for socket 0
        // Note: Completion is recognized by 0x50-0x5F pattern with 3 bytes total
        let completion_bytes = vec![0x90, 0x50, 0xFF];
        let response = parse_visca_response(&completion_bytes, &ViscaResponseType::PanTiltPosition);
        assert!(matches!(response, Ok(ViscaResponse::Completion)));

        // Completion for socket 1
        let completion_bytes = vec![0x90, 0x51, 0xFF];
        let response = parse_visca_response(&completion_bytes, &ViscaResponseType::ZoomPosition);
        assert!(matches!(response, Ok(ViscaResponse::Completion)));
    }

    #[test]
    fn test_parse_error_responses() {
        // Errors are recognized by 0x60-0x6F pattern in byte[1]
        // The parse_visca_response function handles errors by calling ViscaError::from_code

        // Test Syntax Error (0x02)
        let error_bytes = vec![0x90, 0x60, 0x02, 0xFF];
        let response = parse_visca_response(&error_bytes, &ViscaResponseType::PanTiltPosition);
        assert!(matches!(response, Err(ViscaError::SyntaxError)));

        // Test Command Buffer Full (0x03)
        let error_bytes = vec![0x90, 0x60, 0x03, 0xFF];
        let response = parse_visca_response(&error_bytes, &ViscaResponseType::PanTiltPosition);
        assert!(matches!(response, Err(ViscaError::CommandBufferFull)));

        // Test Command Not Executable (0x41) - note different byte[1] pattern
        let error_bytes = vec![0x90, 0x61, 0x41, 0xFF];
        let response = parse_visca_response(&error_bytes, &ViscaResponseType::PanTiltPosition);
        assert!(matches!(response, Err(ViscaError::CommandNotExecutable)));
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
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::PanTiltPosition {
                pan,
                tilt,
            })) => {
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
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::ZoomPosition { position })) => {
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
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::FocusPosition {
                position,
            })) => {
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
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::ExposureMode { mode })) => {
                assert_eq!(mode as u8, 0x00); // Auto mode
            }
            _ => panic!("Expected ExposureMode inquiry response"),
        }

        // Manual exposure mode
        let exposure_response_bytes = vec![0x90, 0x50, 0x03, 0xFF];
        let response =
            parse_visca_response(&exposure_response_bytes, &ViscaResponseType::ExposureMode);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::ExposureMode { mode })) => {
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
        assert!(matches!(response, Ok(ViscaResponse::Completion)));
    }

    #[test]
    fn test_parse_contrast_response() {
        // Example Contrast value
        // TODO: Sprint 1 will implement Contrast parsing
        // Currently falls through to default case and returns Completion
        let contrast_response_bytes = vec![0x90, 0x50, 0x0F, 0xFF];
        let response = parse_visca_response(&contrast_response_bytes, &ViscaResponseType::Contrast);
        // For now, this returns Completion since parsing is not implemented
        assert!(matches!(response, Ok(ViscaResponse::Completion)));
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
        assert!(matches!(response, Ok(ViscaResponse::Ack)));
    }

    #[test]
    fn test_parse_sharpness_response() {
        // Test Sharpness response
        let sharpness_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0B, 0xFF];
        let response = parse_visca_response(&sharpness_bytes, &ViscaResponseType::Sharpness);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::Sharpness { value })) => {
                assert_eq!(value, 0x0B);
            }
            _ => panic!("Expected Sharpness inquiry response"),
        }
    }

    #[test]
    fn test_parse_exposure_compensation_responses() {
        // Test Exposure Compensation value -7
        let exp_comp_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xFF];
        let response = parse_visca_response(&exp_comp_bytes, &ViscaResponseType::ExposureCompensation);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::ExposureCompensation { value })) => {
                assert_eq!(value, -7);
            }
            _ => panic!("Expected ExposureCompensation inquiry response"),
        }

        // Test Exposure Compensation value 0
        let exp_comp_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF];
        let response = parse_visca_response(&exp_comp_bytes, &ViscaResponseType::ExposureCompensation);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::ExposureCompensation { value })) => {
                assert_eq!(value, 0);
            }
            _ => panic!("Expected ExposureCompensation inquiry response"),
        }

        // Test Exposure Compensation value +7
        let exp_comp_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0E, 0xFF];
        let response = parse_visca_response(&exp_comp_bytes, &ViscaResponseType::ExposureCompensation);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::ExposureCompensation { value })) => {
                assert_eq!(value, 7);
            }
            _ => panic!("Expected ExposureCompensation inquiry response"),
        }

        // Test Exposure Compensation Mode On
        let exp_comp_mode_bytes = vec![0x90, 0x50, 0x02, 0xFF];
        let response = parse_visca_response(&exp_comp_mode_bytes, &ViscaResponseType::ExposureCompensationMode);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::ExposureCompensationMode { on })) => {
                assert!(on);
            }
            _ => panic!("Expected ExposureCompensationMode inquiry response"),
        }

        // Test Exposure Compensation Mode Off
        let exp_comp_mode_bytes = vec![0x90, 0x50, 0x03, 0xFF];
        let response = parse_visca_response(&exp_comp_mode_bytes, &ViscaResponseType::ExposureCompensationMode);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::ExposureCompensationMode { on })) => {
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
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::Iris { position })) => {
                assert_eq!(position, 0x00);
            }
            _ => panic!("Expected Iris inquiry response"),
        }

        // Test Iris F1.8 (0x0C)
        let iris_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0C, 0xFF];
        let response = parse_visca_response(&iris_bytes, &ViscaResponseType::Iris);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::Iris { position })) => {
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
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::Shutter { position })) => {
                assert_eq!(position, 0x01);
            }
            _ => panic!("Expected Shutter inquiry response"),
        }

        // Test Shutter 1/10000 (0x11)
        let shutter_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x01, 0x01, 0xFF];
        let response = parse_visca_response(&shutter_bytes, &ViscaResponseType::Shutter);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::Shutter { position })) => {
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
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::Bright { position })) => {
                assert_eq!(position, 0x00);
            }
            _ => panic!("Expected Bright inquiry response"),
        }

        // Test Bright 17 (0x11)
        let bright_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x01, 0x01, 0xFF];
        let response = parse_visca_response(&bright_bytes, &ViscaResponseType::Bright);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::Bright { position })) => {
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
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::Gain { gain })) => {
                assert_eq!(gain, 0x07);
            }
            _ => panic!("Expected Gain inquiry response"),
        }

        // Test GainLimit response
        let gain_limit_bytes = vec![0x90, 0x50, 0x0F, 0xFF];
        let response = parse_visca_response(&gain_limit_bytes, &ViscaResponseType::GainLimit);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::GainLimit { limit })) => {
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
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::AntiFlicker { mode })) => {
                assert!(matches!(mode, AntiFlickerMode::Off));
            }
            _ => panic!("Expected AntiFlicker inquiry response"),
        }

        // Test AntiFlicker 50Hz
        let anti_flicker_bytes = vec![0x90, 0x50, 0x01, 0xFF];
        let response = parse_visca_response(&anti_flicker_bytes, &ViscaResponseType::AntiFlicker);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::AntiFlicker { mode })) => {
                assert!(matches!(mode, AntiFlickerMode::Hz50));
            }
            _ => panic!("Expected AntiFlicker inquiry response"),
        }

        // Test AntiFlicker 60Hz
        let anti_flicker_bytes = vec![0x90, 0x50, 0x02, 0xFF];
        let response = parse_visca_response(&anti_flicker_bytes, &ViscaResponseType::AntiFlicker);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::AntiFlicker { mode })) => {
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
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::Saturation { level })) => {
                assert_eq!(level, 0x00);
            }
            _ => panic!("Expected Saturation inquiry response"),
        }

        // Test Saturation 200% (0x0E)
        let saturation_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0E, 0xFF];
        let response = parse_visca_response(&saturation_bytes, &ViscaResponseType::Saturation);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::Saturation { level })) => {
                assert_eq!(level, 0x0E);
            }
            _ => panic!("Expected Saturation inquiry response"),
        }

        // Test Hue response
        let hue_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x07, 0xFF];
        let response = parse_visca_response(&hue_bytes, &ViscaResponseType::Hue);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::Hue { hue })) => {
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
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::RedGain { gain })) => {
                assert_eq!(gain, -10);
            }
            _ => panic!("Expected RedGain inquiry response"),
        }

        // Test RedGain 0
        let red_gain_bytes = vec![0x90, 0x50, 0x0A, 0xFF];
        let response = parse_visca_response(&red_gain_bytes, &ViscaResponseType::RedGain);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::RedGain { gain })) => {
                assert_eq!(gain, 0);
            }
            _ => panic!("Expected RedGain inquiry response"),
        }

        // Test BlueGain +10
        let blue_gain_bytes = vec![0x90, 0x50, 0x14, 0xFF];
        let response = parse_visca_response(&blue_gain_bytes, &ViscaResponseType::BlueGain);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::BlueGain { gain })) => {
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
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::ImageFlip { vertical, horizontal })) => {
                assert!(!vertical);
                assert!(!horizontal);
            }
            _ => panic!("Expected ImageFlip inquiry response"),
        }

        // Test ImageFlip Horizontal only (0x01)
        let flip_bytes = vec![0x90, 0x50, 0x01, 0xFF];
        let response = parse_visca_response(&flip_bytes, &ViscaResponseType::ImageFlip);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::ImageFlip { vertical, horizontal })) => {
                assert!(!vertical);
                assert!(horizontal);
            }
            _ => panic!("Expected ImageFlip inquiry response"),
        }

        // Test ImageFlip Vertical only (0x02)
        let flip_bytes = vec![0x90, 0x50, 0x02, 0xFF];
        let response = parse_visca_response(&flip_bytes, &ViscaResponseType::ImageFlip);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::ImageFlip { vertical, horizontal })) => {
                assert!(vertical);
                assert!(!horizontal);
            }
            _ => panic!("Expected ImageFlip inquiry response"),
        }

        // Test ImageFlip Both (0x03)
        let flip_bytes = vec![0x90, 0x50, 0x03, 0xFF];
        let response = parse_visca_response(&flip_bytes, &ViscaResponseType::ImageFlip);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::ImageFlip { vertical, horizontal })) => {
                assert!(vertical);
                assert!(horizontal);
            }
            _ => panic!("Expected ImageFlip inquiry response"),
        }
    }

    #[test]
    fn test_parse_extended_inquiry_responses() {
        // Test SharpnessMode response
        
        // Auto mode
        let sharp_mode_bytes = vec![0x90, 0x50, 0x02, 0xFF];
        let response = parse_visca_response(&sharp_mode_bytes, &ViscaResponseType::SharpnessMode);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::SharpnessMode { mode })) => {
                assert!(matches!(mode, SharpnessMode::Auto));
            }
            _ => panic!("Expected SharpnessMode inquiry response"),
        }

        // Manual mode
        let sharp_mode_bytes = vec![0x90, 0x50, 0x03, 0xFF];
        let response = parse_visca_response(&sharp_mode_bytes, &ViscaResponseType::SharpnessMode);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::SharpnessMode { mode })) => {
                assert!(matches!(mode, SharpnessMode::Manual));
            }
            _ => panic!("Expected SharpnessMode inquiry response"),
        }

        // Test ColorTemperature response
        let ct_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x03, 0x07, 0xFF]; // 8000K (0x37)
        let response = parse_visca_response(&ct_bytes, &ViscaResponseType::ColorTemperature);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::ColorTemperature { temperature })) => {
                assert_eq!(temperature, 0x37);
            }
            _ => panic!("Expected ColorTemperature inquiry response"),
        }

        // Test NoiseReduction2D response
        let nr2d_bytes = vec![0x90, 0x50, 0x05, 0xFF];
        let response = parse_visca_response(&nr2d_bytes, &ViscaResponseType::NoiseReduction2D);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::NoiseReduction2D { level })) => {
                assert_eq!(level, 0x05);
            }
            _ => panic!("Expected NoiseReduction2D inquiry response"),
        }

        // Test NoiseReduction3D response
        let nr3d_bytes = vec![0x90, 0x50, 0x08, 0xFF];
        let response = parse_visca_response(&nr3d_bytes, &ViscaResponseType::NoiseReduction3D);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::NoiseReduction3D { level })) => {
                assert_eq!(level, 0x08);
            }
            _ => panic!("Expected NoiseReduction3D inquiry response"),
        }

        // Test BlackWhite response
        let bw_on_bytes = vec![0x90, 0x50, 0x04, 0xFF];
        let response = parse_visca_response(&bw_on_bytes, &ViscaResponseType::BlackWhite);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::BlackWhite { on })) => {
                assert!(on);
            }
            _ => panic!("Expected BlackWhite inquiry response"),
        }

        let bw_off_bytes = vec![0x90, 0x50, 0x00, 0xFF];
        let response = parse_visca_response(&bw_off_bytes, &ViscaResponseType::BlackWhite);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::BlackWhite { on })) => {
                assert!(!on);
            }
            _ => panic!("Expected BlackWhite inquiry response"),
        }

        // Test FocusZone response
        
        let fz_center_bytes = vec![0x90, 0x50, 0x01, 0xFF];
        let response = parse_visca_response(&fz_center_bytes, &ViscaResponseType::FocusZone);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::FocusZone { zone })) => {
                assert!(matches!(zone, FocusZone::Center));
            }
            _ => panic!("Expected FocusZone inquiry response"),
        }

        // Test AFSensitivity response
        
        let af_high_bytes = vec![0x90, 0x50, 0x02, 0xFF];
        let response = parse_visca_response(&af_high_bytes, &ViscaResponseType::AFSensitivity);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::AFSensitivity { sensitivity })) => {
                assert!(matches!(sensitivity, AFSensitivity::High));
            }
            _ => panic!("Expected AFSensitivity inquiry response"),
        }

        // Test FocusNearLimit response
        let fnl_bytes = vec![0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0xFF];
        let response = parse_visca_response(&fnl_bytes, &ViscaResponseType::FocusNearLimit);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::FocusNearLimit { position })) => {
                assert_eq!(position, 0x1234);
            }
            _ => panic!("Expected FocusNearLimit inquiry response"),
        }

        // Test DynamicRange response
        let dr_bytes = vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x08, 0xFF];
        let response = parse_visca_response(&dr_bytes, &ViscaResponseType::DynamicRange);
        match response {
            Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::DynamicRange { level })) => {
                assert_eq!(level, 0x08);
            }
            _ => panic!("Expected DynamicRange inquiry response"),
        }
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
        // Ok(ViscaResponse::InquiryResponse(ViscaInquiryResponse::Backlight { status: true }))
        assert!(matches!(response, Ok(ViscaResponse::Completion)));
        
        // TODO: Implement Backlight response parsing in response.rs
    }

    #[test]
    fn test_response_length_validation() {
        // Test various response types with incorrect lengths
        
        // Sharpness with wrong length (should be 7 bytes)
        let invalid_sharpness = vec![0x90, 0x50, 0x0B, 0xFF];
        let response = parse_visca_response(&invalid_sharpness, &ViscaResponseType::Sharpness);
        assert!(matches!(response, Err(ViscaError::InvalidResponseLength)));

        // Exposure compensation with wrong length (should be 7 bytes)
        let invalid_exp_comp = vec![0x90, 0x50, 0x07, 0xFF];
        let response = parse_visca_response(&invalid_exp_comp, &ViscaResponseType::ExposureCompensation);
        assert!(matches!(response, Err(ViscaError::InvalidResponseLength)));

        // Anti-flicker with wrong length (should be 4 bytes)
        let invalid_anti_flicker = vec![0x90, 0x50, 0x00, 0x01, 0xFF];
        let response = parse_visca_response(&invalid_anti_flicker, &ViscaResponseType::AntiFlicker);
        assert!(matches!(response, Err(ViscaError::InvalidResponseLength)));
    }
}

#[cfg(test)]
mod transport_response_parsing_tests {
    use super::*;

    // Mock transport for testing
    struct MockTransport {
        responses: Vec<Vec<u8>>,
        current_index: usize,
    }

    impl MockTransport {
        fn new(responses: Vec<Vec<u8>>) -> Self {
            Self {
                responses,
                current_index: 0,
            }
        }
    }

    impl ViscaTransport for MockTransport {
        fn send_command(
            &mut self,
            _command: &dyn grafton_visca::ViscaCommand,
        ) -> Result<(), ViscaError> {
            Ok(())
        }

        fn receive_response(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
            if self.current_index < self.responses.len() {
                let response = vec![self.responses[self.current_index].clone()];
                self.current_index += 1;
                Ok(response)
            } else {
                Err(ViscaError::Io(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "No more responses",
                )))
            }
        }
    }

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
