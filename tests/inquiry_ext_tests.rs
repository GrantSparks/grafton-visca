#[cfg(test)]
mod tests {
    use grafton_visca::command::exposure::ExposureMode;
    use grafton_visca::command::white_balance::WhiteBalanceMode;
    use grafton_visca::{
        Command, Error, Response, Transport, InquiryExt, InquiryResponse,
    };
    use std::collections::VecDeque;

    /// Mock device for testing inquiry extension methods
    struct MockDevice {
        responses: VecDeque<Result<Response, Error>>,
    }

    impl MockDevice {
        const fn new() -> Self {
            Self {
                responses: VecDeque::new(),
            }
        }

        fn queue_inquiry_response(&mut self, response: InquiryResponse) {
            self.responses
                .push_back(Ok(Response::InquiryResponse(response)));
        }

        fn queue_error(&mut self, error: Error) {
            self.responses.push_back(Err(error));
        }
    }

    impl Transport for MockDevice {
        fn execute_command(&mut self, _command: &dyn Command) -> Result<Response, Error> {
            self.responses.pop_front().unwrap_or(Err(Error::Timeout))
        }
    }

    #[test]
    fn test_get_power_state_on() {
        let mut device = MockDevice::new();
        // Queue Power ON response
        device.queue_inquiry_response(InquiryResponse::Power { on: true });

        let result = device.get_power_state().unwrap();
        assert!(result);
    }

    #[test]
    fn test_get_power_state_off() {
        let mut device = MockDevice::new();
        // Queue Power OFF response
        device.queue_inquiry_response(InquiryResponse::Power { on: false });

        let result = device.get_power_state().unwrap();
        assert!(!result);
    }

    #[test]
    fn test_get_pan_tilt_position() {
        let mut device = MockDevice::new();
        // Queue Pan/Tilt position response
        device.queue_inquiry_response(InquiryResponse::PanTiltPosition {
            pan: 0x1234,
            tilt: 0x5678,
        });

        let (pan, tilt) = device.get_pan_tilt_position().unwrap();
        assert_eq!(pan, 0x1234);
        assert_eq!(tilt, 0x5678);
    }

    #[test]
    fn test_get_zoom_position() {
        let mut device = MockDevice::new();
        // Queue Zoom position response
        device.queue_inquiry_response(InquiryResponse::ZoomPosition { position: 0x4000 });

        let zoom = device.get_zoom_position().unwrap();
        assert_eq!(zoom, 0x4000);
    }

    #[test]
    fn test_get_exposure_mode() {
        let mut device = MockDevice::new();
        // Queue Exposure mode response (Auto)
        device.queue_inquiry_response(InquiryResponse::ExposureMode {
            mode: ExposureMode::Auto,
        });

        let mode = device.get_exposure_mode().unwrap();
        // ExposureMode doesn't implement PartialEq, so we check the discriminant
        assert!(matches!(mode, ExposureMode::Auto));
    }

    #[test]
    fn test_get_white_balance_mode() {
        let mut device = MockDevice::new();
        // Queue White Balance mode response (Auto)
        device.queue_inquiry_response(InquiryResponse::WhiteBalance {
            mode: WhiteBalanceMode::Auto,
        });

        let mode = device.get_white_balance_mode().unwrap();
        // WhiteBalanceMode doesn't implement PartialEq, so we check the discriminant
        assert!(matches!(mode, WhiteBalanceMode::Auto));
    }

    #[test]
    fn test_get_exposure_compensation() {
        let mut device = MockDevice::new();
        // Queue Exposure compensation response (+7 compensation)
        device.queue_inquiry_response(InquiryResponse::ExposureCompensation { value: 7 });

        let compensation = device.get_exposure_compensation().unwrap();
        assert_eq!(compensation, 7);
    }

    #[test]
    fn test_get_image_flip() {
        let mut device = MockDevice::new();
        // Queue Image flip response (both on)
        device.queue_inquiry_response(InquiryResponse::ImageFlip {
            vertical: true,
            horizontal: true,
        });

        let (vertical, horizontal) = device.get_image_flip().unwrap();
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
        let mut device = MockDevice::new();
        // Queue an error response
        device.queue_error(Error::CommandTimeout {
            duration: std::time::Duration::from_secs(5),
            command: "Power inquiry".to_string(),
        });

        let result = device.get_power_state();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::CommandTimeout { .. }));
    }
}
