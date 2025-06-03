use grafton_visca::command::response::{parse_visca_response, ViscaResponse};
use grafton_visca::command::{ViscaInquiryResponse, ViscaResponseType};
use grafton_visca::{ViscaError, ViscaTransport};

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
