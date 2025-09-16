//! Test for issue #363: Make async runtime inquiry decoding profile-aware.
//!
//! This test verifies that the async runtime correctly uses profile-aware
//! decoding for pan/tilt position inquiries, properly handling coordinate
//! system conversion for cameras with unsigned-centered coordinates.

use grafton_visca::{
    camera::profiles::{GenericVisca, SonyBRC300},
    capabilities::{CoordinateSystem, PanTilt},
    command::{
        response::{lift_inquiry_for, payload::Payload, Response, ResponseKind},
        InquiryResponse,
    },
    protocol::response::{BasicKind, BasicResponse},
};

#[test]
fn test_lift_inquiry_for_signed_centered() {
    // Create a DataReply response with pan/tilt at center (0x0000, 0x0000 for signed-centered)
    let payload = vec![
        0x00, 0x00, 0x00, 0x00, // Pan: 0x0000 (center for signed)
        0x00, 0x00, 0x00, 0x00, // Tilt: 0x0000 (center for signed)
    ];

    let basic = BasicResponse {
        kind: BasicKind::DataReply,
        socket: None,
        payload: Payload::new(&payload),
    };

    let response_type = ResponseKind::PanTiltPosition;

    // Use profile-aware lifting with GenericVisca (signed-centered)
    let response = lift_inquiry_for::<GenericVisca>(&basic, Some(&response_type))
        .expect("Should parse successfully");

    // Verify the response is correctly interpreted as (0, 0)
    match response {
        Response::Inquiry(InquiryResponse::PanTiltPosition { pan, tilt }) => {
            assert_eq!(pan, 0, "Pan should be 0 at center for signed-centered");
            assert_eq!(tilt, 0, "Tilt should be 0 at center for signed-centered");
        }
        _ => panic!("Expected PanTiltPosition inquiry response"),
    }
}

#[test]
fn test_lift_inquiry_for_unsigned_centered() {
    // Create a DataReply response with pan/tilt at center (0x8000, 0x8000 for unsigned-centered)
    let payload = vec![
        0x08, 0x00, 0x00, 0x00, // Pan: 0x8000 (center for unsigned)
        0x08, 0x00, 0x00, 0x00, // Tilt: 0x8000 (center for unsigned)
    ];

    let basic = BasicResponse {
        kind: BasicKind::DataReply,
        socket: None,
        payload: Payload::new(&payload),
    };

    let response_type = ResponseKind::PanTiltPosition;

    // Use profile-aware lifting with SonyBRC300 (unsigned-centered)
    let response = lift_inquiry_for::<SonyBRC300>(&basic, Some(&response_type))
        .expect("Should parse successfully");

    // Verify the response is correctly interpreted as (0, 0)
    match response {
        Response::Inquiry(InquiryResponse::PanTiltPosition { pan, tilt }) => {
            assert_eq!(pan, 0, "Pan should be 0 at center for unsigned-centered");
            assert_eq!(tilt, 0, "Tilt should be 0 at center for unsigned-centered");
        }
        _ => panic!("Expected PanTiltPosition inquiry response"),
    }
}

#[test]
fn test_coordinate_conversion_extremes() {
    // Test maximum positive pan/tilt for unsigned-centered
    // 0xFFFF should convert to 32767 in host units
    let payload = vec![
        0x0F, 0x0F, 0x0F, 0x0F, // Pan: 0xFFFF
        0x0F, 0x0F, 0x0F, 0x0F, // Tilt: 0xFFFF
    ];

    let basic = BasicResponse {
        kind: BasicKind::DataReply,
        socket: None,
        payload: Payload::new(&payload),
    };

    let response_type = ResponseKind::PanTiltPosition;

    // Use profile-aware lifting for unsigned-centered
    let response = lift_inquiry_for::<SonyBRC300>(&basic, Some(&response_type))
        .expect("Should parse successfully");

    match response {
        Response::Inquiry(InquiryResponse::PanTiltPosition { pan, tilt }) => {
            assert_eq!(
                pan, 32767,
                "Pan should be 32767 at max for unsigned-centered"
            );
            assert_eq!(
                tilt, 32767,
                "Tilt should be 32767 at max for unsigned-centered"
            );
        }
        _ => panic!("Expected PanTiltPosition inquiry response"),
    }

    // Test minimum (0x0000 should convert to -32768)
    let payload = vec![
        0x00, 0x00, 0x00, 0x00, // Pan: 0x0000
        0x00, 0x00, 0x00, 0x00, // Tilt: 0x0000
    ];

    let basic = BasicResponse {
        kind: BasicKind::DataReply,
        socket: None,
        payload: Payload::new(&payload),
    };

    let response = lift_inquiry_for::<SonyBRC300>(&basic, Some(&response_type))
        .expect("Should parse successfully");

    match response {
        Response::Inquiry(InquiryResponse::PanTiltPosition { pan, tilt }) => {
            assert_eq!(
                pan, -32768,
                "Pan should be -32768 at min for unsigned-centered"
            );
            assert_eq!(
                tilt, -32768,
                "Tilt should be -32768 at min for unsigned-centered"
            );
        }
        _ => panic!("Expected PanTiltPosition inquiry response"),
    }
}

// Note: Full async runtime integration testing would require more complex test setup.
// The key fix is demonstrated by the lift_inquiry_for tests above which show
// that the profile-aware lifting works correctly for both coordinate systems.
// The async runtime now uses lift_inquiry_for<P> instead of lift_inquiry,
// ensuring correct coordinate conversion for all profiles.

#[test]
fn test_real_profiles_coordinate_systems() {
    // Verify that real camera profiles have the expected coordinate systems
    assert_eq!(
        GenericVisca::COORDINATE_SYSTEM,
        CoordinateSystem::SignedCentered,
        "GenericVisca should use signed-centered coordinates"
    );

    assert_eq!(
        SonyBRC300::COORDINATE_SYSTEM,
        CoordinateSystem::UnsignedCentered,
        "SonyBRC300 should use unsigned-centered coordinates"
    );
}
