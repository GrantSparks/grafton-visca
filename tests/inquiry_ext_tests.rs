#[cfg(test)]
mod tests {
    use grafton_visca::command::exposure::ExposureMode;
    use grafton_visca::command::white_balance::WhiteBalanceMode;
    use grafton_visca::{ViscaCommand, ViscaError, ViscaInquiryExt, ViscaTransport};
    use std::collections::VecDeque;

    /// Mock transport for testing inquiry extension methods
    struct MockTransport {
        responses: VecDeque<Result<Vec<Vec<u8>>, ViscaError>>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                responses: VecDeque::new(),
            }
        }

        fn queue_response(&mut self, response: Vec<u8>) {
            self.responses.push_back(Ok(vec![response]));
        }

        fn queue_error(&mut self, error: ViscaError) {
            self.responses.push_back(Err(error));
        }
    }

    impl ViscaTransport for MockTransport {
        fn send_command(&mut self, _command: &dyn ViscaCommand) -> Result<(), ViscaError> {
            Ok(())
        }

        fn receive_response(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
            self.responses.pop_front().unwrap_or(Ok(vec![]))
        }
    }

    #[test]
    fn test_get_power_state_on() {
        let mut transport = MockTransport::new();
        // Queue ACK and Power ON response
        transport.queue_response(vec![0x90, 0x41, 0xFF]); // ACK
        transport.queue_response(vec![0x90, 0x50, 0x02, 0xFF]); // Power ON

        let result = transport.get_power_state().unwrap();
        assert!(result);
    }

    #[test]
    fn test_get_power_state_off() {
        let mut transport = MockTransport::new();
        // Queue ACK and Power OFF response
        transport.queue_response(vec![0x90, 0x41, 0xFF]); // ACK
        transport.queue_response(vec![0x90, 0x50, 0x03, 0xFF]); // Power OFF

        let result = transport.get_power_state().unwrap();
        assert!(!result);
    }

    #[test]
    fn test_get_pan_tilt_position() {
        let mut transport = MockTransport::new();
        // Queue ACK and Pan/Tilt position response
        transport.queue_response(vec![0x90, 0x41, 0xFF]); // ACK
        transport.queue_response(vec![
            0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0xFF,
        ]);

        let (pan, tilt) = transport.get_pan_tilt_position().unwrap();
        assert_eq!(pan, 0x1234);
        assert_eq!(tilt, 0x5678);
    }

    #[test]
    fn test_get_zoom_position() {
        let mut transport = MockTransport::new();
        // Queue ACK and Zoom position response
        transport.queue_response(vec![0x90, 0x41, 0xFF]); // ACK
        transport.queue_response(vec![0x90, 0x50, 0x04, 0x00, 0x00, 0x00, 0xFF]); // Zoom at 0x4000

        let zoom = transport.get_zoom_position().unwrap();
        assert_eq!(zoom, 0x4000);
    }

    #[test]
    fn test_get_exposure_mode() {
        let mut transport = MockTransport::new();
        // Queue ACK and Exposure mode response (Auto)
        transport.queue_response(vec![0x90, 0x41, 0xFF]); // ACK
        transport.queue_response(vec![0x90, 0x50, 0x00, 0xFF]); // Auto mode

        let mode = transport.get_exposure_mode().unwrap();
        // ExposureMode doesn't implement PartialEq, so we check the discriminant
        assert!(matches!(mode, ExposureMode::Auto));
    }

    #[test]
    fn test_get_white_balance_mode() {
        let mut transport = MockTransport::new();
        // Queue ACK and White Balance mode response (Auto)
        transport.queue_response(vec![0x90, 0x41, 0xFF]); // ACK
        transport.queue_response(vec![0x90, 0x50, 0x00, 0xFF]); // Auto mode

        let mode = transport.get_white_balance_mode().unwrap();
        // WhiteBalanceMode doesn't implement PartialEq, so we check the discriminant
        assert!(matches!(mode, WhiteBalanceMode::Auto));
    }

    #[test]
    fn test_get_exposure_compensation() {
        let mut transport = MockTransport::new();
        // Queue ACK and Exposure compensation response
        transport.queue_response(vec![0x90, 0x41, 0xFF]); // ACK
        transport.queue_response(vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x0E, 0xFF]); // +7 compensation

        let compensation = transport.get_exposure_compensation().unwrap();
        assert_eq!(compensation, 7);
    }

    #[test]
    fn test_get_image_flip() {
        let mut transport = MockTransport::new();
        // Queue ACK and Image flip response (both on)
        transport.queue_response(vec![0x90, 0x41, 0xFF]); // ACK
        transport.queue_response(vec![0x90, 0x50, 0x03, 0xFF]); // Both vertical and horizontal flip

        let (vertical, horizontal) = transport.get_image_flip().unwrap();
        assert!(vertical);
        assert!(horizontal);
    }

    // NOTE: test_get_camera_state is commented out because the response parsing
    // for Luminance and Contrast commands is not fully implemented in the
    // current version of the library. The high-level inquiry API is designed
    // to work once these parsers are added.

    // #[test]
    // fn test_get_camera_state() {
    //     // Full camera state test would go here
    // }

    #[test]
    fn test_error_handling() {
        let mut transport = MockTransport::new();
        // Queue an error response
        transport.queue_error(ViscaError::Timeout);

        let result = transport.get_power_state();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ViscaError::Timeout));
    }
}
