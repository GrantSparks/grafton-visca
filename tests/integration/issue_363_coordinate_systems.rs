//! Profile-aware pan/tilt position decoding, including Sony BRC-300's
//! nonstandard five-nibble pan field.

use grafton_visca::{
    capabilities::{CoordinateSystem, PanTiltWireCodec},
    command::pan_tilt,
    profiles::{CompileTimeProfile, GenericVisca, NearusBrc300, ProfileSpec, SonyBrc300},
    request::Inquiry,
    types::PanTiltUnits,
    wire::Response,
};

fn parsed<P: CompileTimeProfile>(payload: &[u8]) -> PanTiltUnits {
    let mut frame = Vec::with_capacity(payload.len() + 3);
    frame.extend_from_slice(&[0x90, 0x50]);
    frame.extend_from_slice(payload);
    frame.push(0xFF);
    Response::parse(&frame).expect("pan/tilt response frame should parse");
    let profile = ProfileSpec::from_compile_time::<P>().expect("validated profile");
    pan_tilt::PositionInquiry
        .decoder_for_profile(&profile)
        .decode(&frame[2..frame.len() - 1])
        .expect("profile-aware pan/tilt position should parse")
}

fn pan_tilt(position: PanTiltUnits) -> (i32, i32) {
    position.raw_values()
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
        pan_tilt(parsed::<SonyBrc300>(&[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        (0, 0)
    );
}

#[test]
fn sony_brc300_decodes_documented_signed_endpoints() {
    // BRC-300 manual p. 22: `08A58`/`F75A8` pan and `493D`/`E796` tilt.
    assert_eq!(
        pan_tilt(parsed::<SonyBrc300>(&[
            0x00, 0x08, 0x0A, 0x05, 0x08, 0x04, 0x09, 0x03, 0x0D,
        ])),
        (0x08A58, 0x493D)
    );
    assert_eq!(
        pan_tilt(parsed::<SonyBrc300>(&[
            0x0F, 0x07, 0x05, 0x0A, 0x08, 0x0E, 0x07, 0x09, 0x06,
        ])),
        (-0x08A58, -0x186A)
    );
}

#[test]
fn sony_brc300_inquiry_endpoints_use_library_axis_polarity() {
    // BRC-300 p. 22: positive raw endpoints are Left/Up, while negative raw
    // endpoints are Right/Down. The library exposes right as positive pan and
    // up as positive tilt degrees, so only the pan axis is reversed.
    let (pan, tilt) = pan_tilt(parsed::<SonyBrc300>(&[
        0x00, 0x08, 0x0A, 0x05, 0x08, 0x04, 0x09, 0x03, 0x0D,
    ]));
    let conversion = ProfileSpec::from_compile_time::<SonyBrc300>()
        .unwrap()
        .pan_tilt_coordinates()
        .unwrap();
    let left_up = conversion
        .to_degrees(PanTiltUnits::new(pan, tilt))
        .expect("finite coordinates");
    assert!(left_up.pan.value() < 0.0);
    assert!(left_up.tilt.value() > 0.0);
    assert!((left_up.pan.value() + 0x08A58 as f32 / 208.0).abs() < f32::EPSILON);
    assert!((left_up.tilt.value() - 0x493D as f32 / 208.0).abs() < f32::EPSILON);

    let (pan, tilt) = pan_tilt(parsed::<SonyBrc300>(&[
        0x0F, 0x07, 0x05, 0x0A, 0x08, 0x0E, 0x07, 0x09, 0x06,
    ]));
    let right_down = conversion
        .to_degrees(PanTiltUnits::new(pan, tilt))
        .expect("finite coordinates");
    assert!(right_down.pan.value() > 0.0);
    assert!(right_down.tilt.value() < 0.0);
    assert!((right_down.pan.value() - 0x08A58 as f32 / 208.0).abs() < f32::EPSILON);
    assert!((right_down.tilt.value() + 0x186A as f32 / 208.0).abs() < f32::EPSILON);
}

#[test]
fn nearus_uses_the_brc300_frame_its_command_list_documents() {
    // R21 (the BRC-300/300P command list text) documents the same five-nibble
    // pan field, signed endpoints and limits as the Sony BRC-300 manual.
    assert_eq!(
        pan_tilt(parsed::<NearusBrc300>(&[
            0x00, 0x08, 0x0A, 0x05, 0x08, 0x04, 0x09, 0x03, 0x0D,
        ])),
        (0x08A58, 0x493D)
    );
    let nearus = ProfileSpec::from_compile_time::<NearusBrc300>().unwrap();
    let sony = ProfileSpec::from_compile_time::<SonyBrc300>().unwrap();
    let nearus = nearus.capabilities().pan_tilt.as_ref().unwrap();
    let sony = sony.capabilities().pan_tilt.as_ref().unwrap();
    assert_eq!(
        nearus.coordinates.wire_codec(),
        PanTiltWireCodec::SonyBrc300
    );
    assert_eq!(
        nearus.coordinates.coordinate_system(),
        CoordinateSystem::SignedCentered
    );
    assert_eq!(nearus.pan_range, sony.pan_range);
    assert_eq!(nearus.tilt_ranges.range(None), sony.tilt_ranges.range(None));
    assert_eq!(nearus.pan_speed.max(), 0x18);
    assert_eq!(nearus.tilt_speed.max(), 0x18);
}

#[test]
fn real_profile_wire_facts_are_distinct() {
    let generic = ProfileSpec::from_compile_time::<GenericVisca>().unwrap();
    let sony = ProfileSpec::from_compile_time::<SonyBrc300>().unwrap();
    let generic = generic.capabilities().pan_tilt.as_ref().unwrap();
    let sony = sony.capabilities().pan_tilt.as_ref().unwrap();
    assert_eq!(
        generic.coordinates.coordinate_system(),
        CoordinateSystem::SignedCentered
    );
    assert_eq!(
        sony.coordinates.coordinate_system(),
        CoordinateSystem::SignedCentered
    );
    assert_eq!(sony.coordinates.wire_codec(), PanTiltWireCodec::SonyBrc300);
    assert_eq!(sony.pan_range.min(), -0x08A58);
    assert_eq!(sony.pan_range.max(), 0x08A58);
    assert_eq!(sony.tilt_ranges.range(None).min(), -0x186A);
    assert_eq!(sony.tilt_ranges.range(None).max(), 0x493D);
}
