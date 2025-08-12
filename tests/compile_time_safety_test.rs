//! Integration test demonstrating compile-time capability checking with the generic API.

#![allow(clippy::expect_used)]
#![cfg(not(feature = "async"))]

/// VISCA command terminator byte.
const VISCA_TERMINATOR: u8 = 0xFF;

use grafton_visca::prelude::blocking::{GenericViscaCam, PTZOpticsG2Cam, SonyFR7Cam};
use grafton_visca::transport::BlockingTransport;
use grafton_visca::{capabilities::*, Error, PresetNumber};
use std::sync::Mutex;

// Mock transport that returns proper VISCA responses
#[derive(Debug)]
struct MockTransport {
    response_sequence: Mutex<Vec<Vec<u8>>>,
    response_index: Mutex<usize>,
}

// Remove async Transport impl since we're in blocking mode

impl MockTransport {
    fn new() -> Self {
        Self {
            // Default sequence: ACK followed by Completion for each command
            // Provide enough responses for multiple commands
            response_sequence: Mutex::new(vec![
                vec![0x90, 0x41, VISCA_TERMINATOR], // ACK (socket 1) for power_on
                vec![0x90, 0x51, VISCA_TERMINATOR], // Completion (socket 1) for power_on
                vec![0x90, 0x41, VISCA_TERMINATOR], // ACK (socket 1) for pan_tilt_home
                vec![0x90, 0x51, VISCA_TERMINATOR], // Completion (socket 1) for pan_tilt_home
                vec![0x90, 0x41, VISCA_TERMINATOR], // ACK (socket 1) for zoom_stop
                vec![0x90, 0x51, VISCA_TERMINATOR], // Completion (socket 1) for zoom_stop
                vec![0x90, 0x41, VISCA_TERMINATOR], // ACK (socket 1) for focus_auto
                vec![0x90, 0x51, VISCA_TERMINATOR], // Completion (socket 1) for focus_auto
                vec![0x90, 0x41, VISCA_TERMINATOR], // ACK (socket 1) for preset_recall
                vec![0x90, 0x51, VISCA_TERMINATOR], // Completion (socket 1) for preset_recall
            ]),
            response_index: Mutex::new(0),
        }
    }

    fn new_with_sony_envelope() -> Self {
        Self {
            // Sony encapsulated responses with 8-byte header
            response_sequence: Mutex::new(vec![
                // ACK with Sony header for power_on
                vec![
                    0x01, 0x11, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x90, 0x41, 0xFF,
                ],
                // Completion with Sony header for power_on
                vec![
                    0x01, 0x11, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x90, 0x51, 0xFF,
                ],
                // ACK for pan_tilt_home
                vec![
                    0x01, 0x11, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x90, 0x41, 0xFF,
                ],
                // Completion for pan_tilt_home
                vec![
                    0x01, 0x11, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x90, 0x51, 0xFF,
                ],
                // ACK for zoom_stop
                vec![
                    0x01, 0x11, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x90, 0x41, 0xFF,
                ],
                // Completion for zoom_stop
                vec![
                    0x01, 0x11, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x90, 0x51, 0xFF,
                ],
            ]),
            response_index: Mutex::new(0),
        }
    }
}

// Implement BlockingTransport for MockTransport
impl BlockingTransport for MockTransport {
    fn send_blocking(&self, _bytes: &[u8]) -> Result<(), Error> {
        Ok(())
    }

    fn recv_blocking(&self) -> Result<bytes::Bytes, Error> {
        let response_sequence = self.response_sequence.lock().unwrap();
        let mut response_index = self.response_index.lock().unwrap();

        if *response_index < response_sequence.len() {
            let response = response_sequence[*response_index].clone();
            *response_index += 1;
            Ok(bytes::Bytes::from(response))
        } else {
            Err(Error::Timeout)
        }
    }

    fn recv_blocking_with_timeout(
        &self,
        _duration: core::time::Duration,
    ) -> Result<bytes::Bytes, Error> {
        // For testing, just return the next response from the queue
        let response_sequence = self.response_sequence.lock().unwrap();
        let mut response_index = self.response_index.lock().unwrap();

        if *response_index < response_sequence.len() {
            let response = response_sequence[*response_index].clone();
            *response_index += 1;
            Ok(bytes::Bytes::from(response))
        } else {
            // Return a default ACK response if no more responses
            Ok(bytes::Bytes::from(vec![
                0x01, 0x11, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x90, 0x41, 0xFF,
            ]))
        }
    }
}

#[test]
fn test_ptzoptics_g2_capabilities() {
    let camera = PTZOpticsG2Cam::from_transport(MockTransport::new());

    // These methods exist for PTZOpticsG2 - checked at compile time
    assert!(camera.power_on().is_ok());
    assert!(camera.pan_tilt_home().is_ok());
    assert!(camera.zoom_stop().is_ok());
    assert!(camera.focus_auto().is_ok());
    assert!(camera.preset_recall(PresetNumber::new(1).unwrap()).is_ok());

    // This would NOT compile - G2 doesn't implement NDFilter trait!
    // camera.set_nd_filter_mode(NDFilterMode::Clear).unwrap(); // COMPILE ERROR!
}

#[test]
fn test_sony_fr7_has_nd_filter() {
    let camera = SonyFR7Cam::from_transport(MockTransport::new_with_sony_envelope());

    // FR7 has all standard features
    assert!(camera.power_on().is_ok());
    assert!(camera.pan_tilt_home().is_ok());
    assert!(camera.zoom_stop().is_ok());

    // PLUS ND filter support! This compiles because SonyFR7 implements NDFilter
    // Note: The command API would be available here for ND filter control
}

// This test demonstrates compile-time capability checking
#[test]
fn test_compile_time_capability_checking() {
    // This function can only be called with cameras that have ND filter support
    fn adjust_nd_filter<P, T>(
        _camera: &grafton_visca::camera::Camera<grafton_visca::camera::BlockingMode, P, T>,
    ) -> Result<(), Error>
    where
        P: Profile + NDFilter,
        T: BlockingTransport + Send + Sync + 'static,
    {
        // ND filter methods would be available here
        Ok(())
    }

    let fr7 = SonyFR7Cam::from_transport(MockTransport::new_with_sony_envelope());
    let _g2 = PTZOpticsG2Cam::from_transport(MockTransport::new());

    // This compiles - FR7 has NDFilter
    assert!(adjust_nd_filter(&fr7).is_ok());

    // This would NOT compile - G2 doesn't have NDFilter
    // adjust_nd_filter(&g2); // COMPILE ERROR!

    // The compiler prevents calling unsupported methods at compile time
}

#[test]
fn test_generic_functions_with_trait_bounds() {
    // Function that works with any camera
    fn basic_control<P>(
        camera: &grafton_visca::camera::Camera<
            grafton_visca::camera::BlockingMode,
            P,
            MockTransport,
        >,
    ) -> Result<(), Error>
    where
        P: Profile,
    {
        // Use blocking operations directly on the camera
        camera.power_on()?;
        camera.zoom_stop()?;
        Ok(())
    }

    // Function that requires motion sync capability
    fn motion_sync_control<P>(
        _camera: &grafton_visca::camera::Camera<
            grafton_visca::camera::BlockingMode,
            P,
            MockTransport,
        >,
    ) -> Result<(), Error>
    where
        P: Profile + MotionSync,
    {
        // Motion sync methods would be available here
        Ok(())
    }

    let g2 = PTZOpticsG2Cam::from_transport(MockTransport::new());
    let fr7 = SonyFR7Cam::from_transport(MockTransport::new_with_sony_envelope());
    let generic = GenericViscaCam::from_transport(MockTransport::new());

    // Test each camera individually to isolate the issue
    println!("Testing G2 camera...");
    assert!(basic_control(&g2).is_ok());

    println!("Testing FR7 camera...");
    let fr7_result = basic_control(&fr7);
    if let Err(e) = &fr7_result {
        println!("FR7 error: {e:?}");
    }
    assert!(fr7_result.is_ok());

    println!("Testing generic camera...");
    assert!(basic_control(&generic).is_ok());

    // Only G2 can use motion_sync_control (it implements MotionSync)
    assert!(motion_sync_control(&g2).is_ok());
    // Note: FR7 doesn't have MotionSync in current implementation

    // This would NOT compile - GenericVisca doesn't implement MotionSync
    // motion_sync_control(&generic); // COMPILE ERROR!
}
