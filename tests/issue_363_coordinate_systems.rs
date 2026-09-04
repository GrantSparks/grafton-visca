//! Profile-aware pan/tilt position decoding, including Sony BRC-300's
//! nonstandard five-nibble pan field.

use grafton_visca::{
    camera::profiles::{GenericVisca, NearusBRC300, SonyBRC300},
    capabilities::{CoordinateSystem, PanTilt, PanTiltWireCodec},
    command::{InquiryData, InquiryKind, Response},
    PanTiltPositionRaw,
};

fn parsed<P: grafton_visca::capabilities::Profile>(payload: &[u8]) -> Response {
    let mut frame = Vec::with_capacity(payload.len() + 3);
    frame.extend_from_slice(&[0x90, 0x50]);
    frame.extend_from_slice(payload);
    frame.push(0xFF);
    Response::parse_with_profile::<P>(&frame, &InquiryKind::PanTiltPosition)
        .expect("profile-aware pan/tilt position should parse")
}

fn pan_tilt(response: Response) -> (i32, i32) {
    match response {
        Response::Inquiry(InquiryData::PanTiltPosition { pan, tilt }) => (pan, tilt),
        _ => panic!("expected PanTiltPosition inquiry response"),
    }
}

#[test]
fn standard_visca_signed_centered_profile_decodes_both_signs() {
    assert_eq!(
        pan_tilt(parsed::<GenericVisca>(&[
            0x00, 0x00, 0x01, 0x00, 0x0f, 0x0f, 0x0f, 0x00,
        ])),
        (0x10, -0x10)
    );
}

#[test]
fn sony_brc300_decodes_documented_nine_nibble_center() {
    assert_eq!(
        pan_tilt(parsed::<SonyBRC300>(&[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        (0, 0)
    );
}

#[test]
fn sony_brc300_decodes_documented_signed_endpoints() {
    // BRC-300 manual p. 22: `08A58`/`F75A8` pan and `493D`/`E796` tilt.
    assert_eq!(
        pan_tilt(parsed::<SonyBRC300>(&[
            0x00, 0x08, 0x0A, 0x05, 0x08, 0x04, 0x09, 0x03, 0x0D,
        ])),
        (0x08A58, 0x493D)
    );
    assert_eq!(
        pan_tilt(parsed::<SonyBRC300>(&[
            0x0F, 0x07, 0x05, 0x0A, 0x08, 0x0E, 0x07, 0x09, 0x06,
        ])),
        (-0x08A58, -0x186A)
    );
}

#[test]
fn sony_brc300_inquiry_endpoints_use_library_axis_polarity() {
    // BRC-300 p. 22: positive raw endpoints are Left/Up, while negative raw
    // endpoints are Right/Down. The library exposes right as positive pan and
    // up as negative tilt degrees.
    let (pan, tilt) = pan_tilt(parsed::<SonyBRC300>(&[
        0x00, 0x08, 0x0A, 0x05, 0x08, 0x04, 0x09, 0x03, 0x0D,
    ]));
    let left_up = PanTiltPositionRaw::new(pan, tilt).as_degrees_with_profile(&SonyBRC300);
    assert!(left_up.pan.0 < 0.0);
    assert!(left_up.tilt.0 < 0.0);
    assert!((left_up.pan.0 + 0x08A58 as f32 / 208.0).abs() < f32::EPSILON);
    assert!((left_up.tilt.0 + 0x493D as f32 / 208.0).abs() < f32::EPSILON);

    let (pan, tilt) = pan_tilt(parsed::<SonyBRC300>(&[
        0x0F, 0x07, 0x05, 0x0A, 0x08, 0x0E, 0x07, 0x09, 0x06,
    ]));
    let right_down = PanTiltPositionRaw::new(pan, tilt).as_degrees_with_profile(&SonyBRC300);
    assert!(right_down.pan.0 > 0.0);
    assert!(right_down.tilt.0 > 0.0);
    assert!((right_down.pan.0 - 0x08A58 as f32 / 208.0).abs() < f32::EPSILON);
    assert!((right_down.tilt.0 - 0x186A as f32 / 208.0).abs() < f32::EPSILON);
}

#[test]
fn nearus_does_not_inherit_sony_brc300_framing_without_evidence() {
    // Nearus retains its existing unsigned-centered metadata, but uses the
    // conservative standard 4+4 VISCA codec rather than Sony's 5+4 frame.
    assert_eq!(
        pan_tilt(parsed::<NearusBRC300>(&[
            0x08, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00,
        ])),
        (0, 0)
    );
    assert_eq!(
        NearusBRC300::PAN_TILT_WIRE_CODEC,
        PanTiltWireCodec::StandardVisca
    );
}

#[test]
fn real_profile_wire_facts_are_distinct() {
    assert_eq!(
        GenericVisca::COORDINATE_SYSTEM,
        CoordinateSystem::SignedCentered
    );
    assert_eq!(
        SonyBRC300::COORDINATE_SYSTEM,
        CoordinateSystem::SignedCentered
    );
    assert_eq!(
        SonyBRC300::PAN_TILT_WIRE_CODEC,
        PanTiltWireCodec::SonyBrc300
    );
    assert_eq!(SonyBRC300::PAN_RANGE.min(), -0x08A58);
    assert_eq!(SonyBRC300::PAN_RANGE.max(), 0x08A58);
    assert_eq!(SonyBRC300::TILT_RANGE.min(), -0x186A);
    assert_eq!(SonyBRC300::TILT_RANGE.max(), 0x493D);
}
