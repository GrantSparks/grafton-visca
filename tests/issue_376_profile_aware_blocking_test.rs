//! Tests for issue #376: Make the blocking runtime profile-aware
//!
//! This test verifies that the blocking runtime uses profile-specific
//! coordinate system conversion for pan/tilt responses, matching the
//! behavior of the async runtime.

#[cfg(all(test, not(feature = "mode-async"), feature = "test-utils"))]
mod profile_aware_blocking_tests {
    use grafton_visca::{
        camera::profiles::{PtzOpticsG2, SonyBRC300},
        capabilities::ProtocolStyle,
        command::{inquiry::PanTiltPositionInquiry, InquiryResponse},
        runtime::blocking_runner::BlockingRunner,
        testing::testkit::scripted_transport::{ScriptedSyncTransport, Step},
        timeout::{CommandCategory, TimeoutConfig},
        CameraId,
    };

    /// Helper to create a pan/tilt position response frame.
    ///
    /// Creates a DataReply frame with 8-byte pan/tilt payload in camera coordinates.
    fn create_pan_tilt_response(pan_u16: u16, tilt_u16: u16) -> Vec<u8> {
        // DataReply frame: 90 50 pppp tttt FF
        // Where pppp and tttt are 4 nibbles each
        let mut frame = vec![0x90, 0x50];

        // Convert u16 values to 4 nibbles each
        frame.push(((pan_u16 >> 12) & 0x0F) as u8);
        frame.push(((pan_u16 >> 8) & 0x0F) as u8);
        frame.push(((pan_u16 >> 4) & 0x0F) as u8);
        frame.push((pan_u16 & 0x0F) as u8);

        frame.push(((tilt_u16 >> 12) & 0x0F) as u8);
        frame.push(((tilt_u16 >> 8) & 0x0F) as u8);
        frame.push(((tilt_u16 >> 4) & 0x0F) as u8);
        frame.push((tilt_u16 & 0x0F) as u8);

        frame.push(0xFF); // Terminator
        frame
    }

    #[test]
    fn test_blocking_pan_tilt_decoding_signed_centered() {
        // PtzOpticsG2 uses SignedCentered coordinate system (default)
        // Camera coordinates are already in signed format
        let mut runner =
            BlockingRunner::<PtzOpticsG2>::new(ProtocolStyle::RawVisca, TimeoutConfig::default());

        // Create transport with scripted response
        // Camera coordinates: pan=0x1234, tilt=0x5678 (signed values)
        let response_frame = create_pan_tilt_response(0x1234, 0x5678);
        let mut transport = ScriptedSyncTransport::new(vec![Step::OnSend {
            matches: None,
            responses: vec![response_frame],
        }]);

        // Send inquiry command
        let inquiry = PanTiltPositionInquiry;
        let result = runner.send_command(
            &mut transport,
            &inquiry,
            CameraId::CAMERA_1,
            CommandCategory::Quick,
        );

        match result {
            Ok(response) => {
                if let grafton_visca::command::response::ViscaResponse::Inquiry(
                    InquiryResponse::PanTiltPosition { pan, tilt },
                ) = response
                {
                    // For SignedCentered, values should be interpreted as signed i16
                    assert_eq!(pan, 0x1234_i16);
                    assert_eq!(tilt, 0x5678_i16);
                } else {
                    panic!("Expected PanTiltPosition response, got: {:?}", response);
                }
            }
            Err(e) => panic!("Command failed: {:?}", e),
        }
    }

    #[test]
    fn test_blocking_pan_tilt_decoding_unsigned_centered() {
        // SonyBRC300 uses UnsignedCentered coordinate system
        // Camera coordinates need conversion: 0x8000 is center (0 logical)
        let mut runner =
            BlockingRunner::<SonyBRC300>::new(ProtocolStyle::RawVisca, TimeoutConfig::default());

        // Create transport with scripted response
        // Camera coordinates: pan=0x8000 (center), tilt=0x9000 (slightly up)
        let response_frame = create_pan_tilt_response(0x8000, 0x9000);
        let mut transport = ScriptedSyncTransport::new(vec![Step::OnSend {
            matches: None,
            responses: vec![response_frame],
        }]);

        // Send inquiry command
        let inquiry = PanTiltPositionInquiry;
        let result = runner.send_command(
            &mut transport,
            &inquiry,
            CameraId::CAMERA_1,
            CommandCategory::Quick,
        );

        match result {
            Ok(response) => {
                if let grafton_visca::command::response::ViscaResponse::Inquiry(
                    InquiryResponse::PanTiltPosition { pan, tilt },
                ) = response
                {
                    // For UnsignedCentered, 0x8000 should convert to 0 logical
                    // 0x9000 should convert to 0x1000 logical (0x9000 - 0x8000)
                    assert_eq!(pan, 0, "Pan at center (0x8000) should be 0 logical");
                    assert_eq!(tilt, 0x1000, "Tilt at 0x9000 should be 0x1000 logical");
                } else {
                    panic!("Expected PanTiltPosition response, got: {:?}", response);
                }
            }
            Err(e) => panic!("Command failed: {:?}", e),
        }
    }

    #[test]
    fn test_blocking_pan_tilt_conversion_matches_coordinate_system() {
        // Test that the blocking runtime uses the profile's coordinate system
        // This is a more direct test of the conversion logic

        // Test edge cases for UnsignedCentered conversion
        let test_cases = [
            (0x0000, 0x0000, -0x8000_i16, -0x8000_i16), // Min camera coords -> min logical
            (0x8000, 0x8000, 0x0000, 0x0000),           // Center camera -> center logical
            (0xFFFF, 0xFFFF, 0x7FFF, 0x7FFF),           // Max camera -> max logical
            (0x7000, 0x9000, -0x1000, 0x1000),          // Mixed values
        ];

        for (pan_cam, tilt_cam, expected_pan, expected_tilt) in test_cases {
            // Create a fresh runner for each test case to avoid state issues
            let mut runner = BlockingRunner::<SonyBRC300>::new(
                ProtocolStyle::RawVisca,
                TimeoutConfig::default(),
            );
            let response_frame = create_pan_tilt_response(pan_cam, tilt_cam);
            let mut transport = ScriptedSyncTransport::new(vec![Step::OnSend {
                matches: None,
                responses: vec![response_frame],
            }]);

            let inquiry = PanTiltPositionInquiry;
            let result = runner.send_command(
                &mut transport,
                &inquiry,
                CameraId::CAMERA_1,
                CommandCategory::Quick,
            );

            match result {
                Ok(response) => {
                    if let grafton_visca::command::response::ViscaResponse::Inquiry(
                        InquiryResponse::PanTiltPosition { pan, tilt },
                    ) = response
                    {
                        assert_eq!(
                            pan, expected_pan,
                            "Pan conversion failed for camera value 0x{:04X}",
                            pan_cam
                        );
                        assert_eq!(
                            tilt, expected_tilt,
                            "Tilt conversion failed for camera value 0x{:04X}",
                            tilt_cam
                        );
                    } else {
                        panic!("Expected PanTiltPosition response");
                    }
                }
                Err(e) => panic!(
                    "Command failed for test case ({:04X}, {:04X}): {:?}",
                    pan_cam, tilt_cam, e
                ),
            }
        }
    }

    #[test]
    fn test_blocking_inquiry_with_completion() {
        // Test that blocking runner handles completion frames correctly
        // with profile-aware decoding for subsequent data replies

        let mut runner =
            BlockingRunner::<PtzOpticsG2>::new(ProtocolStyle::RawVisca, TimeoutConfig::default());

        // Create transport with data reply for home position (0,0)
        let mut transport = ScriptedSyncTransport::new(vec![Step::OnSend {
            matches: None,
            responses: vec![vec![
                0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF,
            ]],
        }]);

        let inquiry = PanTiltPositionInquiry;
        let result = runner.send_command(
            &mut transport,
            &inquiry,
            CameraId::CAMERA_1,
            CommandCategory::Quick,
        );

        match result {
            Ok(response) => {
                if let grafton_visca::command::response::ViscaResponse::Inquiry(
                    InquiryResponse::PanTiltPosition { pan, tilt },
                ) = response
                {
                    assert_eq!(pan, 0);
                    assert_eq!(tilt, 0);
                } else {
                    panic!("Expected PanTiltPosition response, got: {:?}", response);
                }
            }
            Err(e) => panic!("Command failed: {:?}", e),
        }
    }
}
