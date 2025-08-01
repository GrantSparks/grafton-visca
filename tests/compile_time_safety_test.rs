//! Integration test demonstrating compile-time capability checking with the generic API.

#![allow(clippy::expect_used)]

use async_trait::async_trait;
use grafton_visca::camera::methods::{
    FocusOpsBlocking, PanTiltOpsBlocking, PowerOpsBlocking, PresetsOpsBlocking, ZoomOpsBlocking,
};
use grafton_visca::prelude::blocking::{GenericViscaCam, PTZOpticsG2Cam, SonyFR7Cam};
use grafton_visca::transport::{unified::BlockingTransportWrapper, UnifiedTransport};
use grafton_visca::{capabilities::*, Camera, Error, PresetNumber};
use std::sync::Mutex;

// Mock transport that returns proper VISCA responses
#[derive(Debug)]
struct MockTransport {
    response_sequence: Mutex<Vec<Vec<u8>>>,
    response_index: Mutex<usize>,
}

impl MockTransport {
    fn new() -> Self {
        Self {
            // Default sequence: ACK followed by Completion for each command
            // Provide enough responses for multiple commands
            response_sequence: Mutex::new(vec![
                vec![0x90, 0x41, 0xFF], // ACK (socket 1) for power_on
                vec![0x90, 0x51, 0xFF], // Completion (socket 1) for power_on
                vec![0x90, 0x41, 0xFF], // ACK (socket 1) for zoom_stop
                vec![0x90, 0x51, 0xFF], // Completion (socket 1) for zoom_stop
                vec![0x90, 0x41, 0xFF], // ACK (socket 1) for next command
                vec![0x90, 0x51, 0xFF], // Completion (socket 1) for next command
            ]),
            response_index: Mutex::new(0),
        }
    }

    fn new_with_sony_envelope() -> Self {
        Self {
            // Sony encapsulated responses with 8-byte header
            response_sequence: Mutex::new(vec![
                // ACK with Sony header: [0x01, 0x11, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00] + [0x90, 0x41, 0xFF]
                vec![
                    0x01, 0x11, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x90, 0x41, 0xFF,
                ],
                // Completion with Sony header
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

#[async_trait]
impl grafton_visca::transport::Transport for MockTransport {
    async fn send(&self, _data: &[u8]) -> Result<(), Error> {
        Ok(())
    }

    async fn recv(&self) -> Result<bytes::Bytes, Error> {
        let sequence = self
            .response_sequence
            .lock()
            .expect("MockTransport mutex poisoned");
        let mut index = self
            .response_index
            .lock()
            .expect("MockTransport mutex poisoned");

        let response = if *index < sequence.len() {
            sequence[*index].clone()
        } else {
            // Reset to beginning for next command
            *index = 0;
            sequence[0].clone()
        };

        *index += 1;
        Ok(bytes::Bytes::from(response))
    }
}

impl grafton_visca::transport::BlockingTransport for MockTransport {
    fn send_blocking(&self, _data: &[u8]) -> Result<(), Error> {
        // For tests, just use the same logic as async send
        Ok(())
    }

    fn recv_blocking(&self) -> Result<bytes::Bytes, Error> {
        let sequence = self
            .response_sequence
            .lock()
            .expect("MockTransport mutex poisoned");
        let mut index = self
            .response_index
            .lock()
            .expect("MockTransport mutex poisoned");

        let response = if *index < sequence.len() {
            sequence[*index].clone()
        } else {
            // Reset to beginning for next command
            *index = 0;
            sequence[0].clone()
        };

        *index += 1;
        Ok(bytes::Bytes::from(response))
    }
}

#[test]
fn test_ptzoptics_g2_capabilities() {
    let camera = PTZOpticsG2Cam::new_blocking(MockTransport::new());

    // These methods exist for PTZOpticsG2 - checked at compile time
    assert!(PowerOpsBlocking::power_on(&camera).is_ok());
    assert!(PanTiltOpsBlocking::pan_tilt_home(&camera).is_ok());
    assert!(ZoomOpsBlocking::zoom_stop(&camera).is_ok());
    assert!(FocusOpsBlocking::focus_auto(&camera).is_ok());
    assert!(PresetsOpsBlocking::preset_recall(&camera, PresetNumber::new(1).unwrap()).is_ok());

    // This would NOT compile - G2 doesn't implement NDFilter trait!
    // camera.set_nd_filter_mode(NDFilterMode::Clear).unwrap(); // COMPILE ERROR!
}

#[test]
fn test_sony_fr7_has_nd_filter() {
    let camera = SonyFR7Cam::new_blocking(MockTransport::new_with_sony_envelope());

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
    fn adjust_nd_filter<P, T>(_camera: &Camera<P, T>) -> Result<(), Error>
    where
        P: Profile + NDFilter,
        T: UnifiedTransport,
    {
        // ND filter methods would be available here
        Ok(())
    }

    let fr7 = SonyFR7Cam::new_blocking(MockTransport::new_with_sony_envelope());
    let _g2 = PTZOpticsG2Cam::new_blocking(MockTransport::new());

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
        camera: &Camera<P, BlockingTransportWrapper<MockTransport>>,
    ) -> Result<(), Error>
    where
        P: Profile,
    {
        // Use blocking operations through the blocking traits
        PowerOpsBlocking::power_on(camera)?;
        ZoomOpsBlocking::zoom_stop(camera)?;
        Ok(())
    }

    // Function that requires motion sync capability
    fn motion_sync_control<P>(
        _camera: &Camera<P, BlockingTransportWrapper<MockTransport>>,
    ) -> Result<(), Error>
    where
        P: Profile + MotionSync,
    {
        // Motion sync methods would be available here
        Ok(())
    }

    let g2 = PTZOpticsG2Cam::new_blocking(MockTransport::new());
    let fr7 = SonyFR7Cam::new_blocking(MockTransport::new_with_sony_envelope());
    let generic = GenericViscaCam::new_blocking(MockTransport::new());

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
