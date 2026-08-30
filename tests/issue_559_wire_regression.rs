//! Normative wire regressions identified during PR #559 review.
//!
//! These assertions use absolute VISCA frames and the public request/response
//! entry points, so they do not merely repeat the generated inquiry metadata.

#![allow(clippy::expect_used)]

use grafton_visca::{
    command::{
        Brightness, BrightnessInquiry, FocusZone, FocusZoneInquiry, InquiryKind,
        PictureEffectInquiry, PictureEffectMode, Response, ResponseParser, UsbAudio,
        UsbAudioInquiry,
    },
    profiles::{PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyFR7},
    types::BrightnessLevel,
    CameraId, Error, ProfileSpec, Request,
};

fn wire<R: Request + ?Sized>(request: &R) -> Vec<u8> {
    let mut buffer = vec![0_u8; R::MAX_SIZE];
    let written = request
        .write_into(CameraId::CAMERA_1, &mut buffer)
        .expect("golden request must encode");
    buffer.truncate(written);
    buffer
}

fn decode<C: ResponseParser>(kind: InquiryKind, frame: &[u8]) -> C::Response {
    let response = Response::parse_with_type(frame, &kind).expect("golden response must parse");
    C::from_response(response).expect("golden response must have the expected type")
}

fn rejects_trailing_payload<C: ResponseParser>(kind: InquiryKind, frame: &[u8]) {
    assert!(
        Response::parse_with_type(frame, &kind)
            .and_then(C::from_response)
            .is_err(),
        "{kind:?} must reject a reply with trailing payload bytes"
    );
}

#[test]
fn brightness_direct_paths_and_inquiry_match_the_04_4d_family() {
    let level = BrightnessLevel::new(0x11).expect("brightness level is valid");
    let direct = [0x81, 0x01, 0x04, 0x4D, 0x00, 0x00, 0x01, 0x01, 0xFF];

    assert_eq!(wire(&Brightness::SetLevel(level)), direct);
    assert_eq!(wire(&Brightness::Direct(level)), direct);
    let mut camera_2_buffer = [0_u8; Brightness::MAX_SIZE];
    let camera_2_written = Brightness::Direct(level)
        .write_into(CameraId::CAMERA_2, &mut camera_2_buffer)
        .expect("camera 2 brightness direct must encode");
    assert_eq!(
        &camera_2_buffer[..camera_2_written],
        &[0x82, 0x01, 0x04, 0x4D, 0x00, 0x00, 0x01, 0x01, 0xFF]
    );
    assert_eq!(wire(&BrightnessInquiry), [0x81, 0x09, 0x04, 0x4D, 0xFF]);
    assert_eq!(
        decode::<BrightnessInquiry>(
            InquiryKind::Brightness,
            &[0x90, 0x50, 0x00, 0x00, 0x01, 0x01, 0xFF],
        ),
        level,
    );
}

#[test]
fn focus_zone_inquiry_uses_af_zone_opcode_and_decodes_positions() {
    assert_eq!(wire(&FocusZoneInquiry), [0x81, 0x09, 0x04, 0xAA, 0xFF]);
    assert_eq!(
        decode::<FocusZoneInquiry>(InquiryKind::FocusZone, &[0x90, 0x50, 0x00, 0xFF]),
        FocusZone::Top,
    );
    assert_eq!(
        decode::<FocusZoneInquiry>(InquiryKind::FocusZone, &[0x90, 0x50, 0x01, 0xFF]),
        FocusZone::Center,
    );
    assert_eq!(
        decode::<FocusZoneInquiry>(InquiryKind::FocusZone, &[0x90, 0x50, 0x02, 0xFF]),
        FocusZone::Bottom,
    );
}

#[test]
fn picture_effect_inquiry_uses_the_direct_command_opcode_and_decodes_modes() {
    assert_eq!(wire(&PictureEffectInquiry), [0x81, 0x09, 0x04, 0x63, 0xFF]);
    assert_eq!(
        decode::<PictureEffectInquiry>(InquiryKind::PictureEffect, &[0x90, 0x50, 0x02, 0xFF]),
        PictureEffectMode::Off,
    );
    assert_eq!(
        decode::<PictureEffectInquiry>(InquiryKind::PictureEffect, &[0x90, 0x50, 0x04, 0xFF]),
        PictureEffectMode::BlackAndWhite,
    );
    rejects_trailing_payload::<PictureEffectInquiry>(
        InquiryKind::PictureEffect,
        &[0x90, 0x50, 0x02, 0x00, 0xFF],
    );
}

#[test]
fn usb_audio_inquiry_matches_uac_wire_and_its_02_on_03_off_polarity() {
    assert_eq!(wire(&UsbAudioInquiry), [0x81, 0x2A, 0x02, 0xA0, 0x04, 0xFF]);
    assert_eq!(
        wire(&UsbAudio::On),
        [0x81, 0x2A, 0x02, 0xA0, 0x04, 0x02, 0xFF]
    );
    assert_eq!(
        wire(&UsbAudio::Off),
        [0x81, 0x2A, 0x02, 0xA0, 0x04, 0x03, 0xFF]
    );
    assert!(decode::<UsbAudioInquiry>(
        InquiryKind::UsbAudio,
        &[0x90, 0x50, 0x02, 0xFF],
    ));
    assert!(!decode::<UsbAudioInquiry>(
        InquiryKind::UsbAudio,
        &[0x90, 0x50, 0x03, 0xFF],
    ));
    rejects_trailing_payload::<UsbAudioInquiry>(
        InquiryKind::UsbAudio,
        &[0x90, 0x50, 0x02, 0x00, 0xFF],
    );
}

#[test]
fn focus_zone_and_usb_audio_inquiries_require_their_source_backed_profile_gates() {
    let g2 = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile");
    FocusZoneInquiry
        .validate_for_profile(&g2)
        .expect("G2 documents the focus-zone response");
    UsbAudioInquiry
        .validate_for_profile(&g2)
        .expect("G2 documents USB audio");

    let g3 = ProfileSpec::from_compile_time::<PtzOpticsG3>().expect("G3 profile");
    assert!(matches!(
        FocusZoneInquiry.validate_for_profile(&g3),
        Err(Error::FeatureNotSupported { .. })
    ));
    assert!(matches!(
        UsbAudioInquiry.validate_for_profile(&g3),
        Err(Error::FeatureNotSupported { .. })
    ));

    let ptzoptics_30x = ProfileSpec::from_compile_time::<PtzOptics30X>().expect("30X profile");
    FocusZoneInquiry
        .validate_for_profile(&ptzoptics_30x)
        .expect("30X documents the focus-zone response");
    UsbAudioInquiry
        .validate_for_profile(&ptzoptics_30x)
        .expect("30X documents USB audio");

    let fr7 = ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile");
    for result in [
        FocusZoneInquiry.validate_for_profile(&fr7),
        UsbAudioInquiry.validate_for_profile(&fr7),
        PictureEffectInquiry.validate_for_profile(&fr7),
    ] {
        assert!(matches!(result, Err(Error::FeatureNotSupported { .. })));
    }
}
