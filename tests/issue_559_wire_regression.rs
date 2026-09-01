//! Normative wire regressions identified during PR #559 review.
//!
//! These assertions use absolute VISCA frames and the public request/response
//! entry points, so they do not merely repeat the generated inquiry metadata.

#![allow(clippy::expect_used)]

use grafton_visca::{
    capabilities::TypedSupportSurface,
    command::{
        Brightness, BrightnessInquiry, FocusZone, FocusZoneInquiry, InquiryKind, NoiseReduction2D,
        NoiseReduction2DInquiry, NoiseReduction2DMode, NoiseReduction2DModeCommand,
        NoiseReduction2DModeInquiry, NoiseReduction3D, NoiseReduction3DInquiry,
        PictureEffectInquiry, PictureEffectMode, Response, ResponseParser, UsbAudio,
        UsbAudioInquiry,
    },
    profiles::{PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyFR7},
    types::{BrightnessLevel, NoiseReduction2DLevel, NoiseReduction3DLevel},
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

fn inquiry_only_noise_reduction_profile() -> ProfileSpec {
    let source = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile");
    let coordinates = source
        .pan_tilt_coordinates()
        .expect("G2 pan/tilt conversion");
    let mut capabilities = source.capabilities().clone();
    capabilities.profile_id = None;
    capabilities.model_name = "NR inquiry without NR control".into();
    capabilities.typed_support = capabilities
        .typed_support
        .without(TypedSupportSurface::NoiseReduction2DControl)
        .without(TypedSupportSurface::NoiseReduction3DControl);

    ProfileSpec::builder(capabilities)
        .pan_tilt_coordinates(
            coordinates.coordinate_system(),
            coordinates.pan_degrees_to_units(),
            coordinates.tilt_degrees_to_units(),
        )
        .pan_tilt_wire_codec(coordinates.wire_codec())
        .transports(source.transports())
        .envelope(source.envelope())
        .timing(source.timing())
        .maximum_command_sockets(source.maximum_command_sockets())
        .supports_operation_complete(source.supports_operation_complete())
        .supports_command_cancel(source.supports_command_cancel())
        .preset_recall_axes(source.preset_recall_axes())
        .position_inquiries(source.position_inquiries())
        .build()
        .expect("inquiry-only NR runtime profile")
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
fn noise_reduction_inquiries_match_the_documented_2d_mode_and_level_registers() {
    assert_eq!(
        wire(&NoiseReduction2DModeInquiry),
        [0x81, 0x09, 0x04, 0x50, 0xFF]
    );
    assert_eq!(
        wire(&NoiseReduction2DInquiry),
        [0x81, 0x09, 0x04, 0x53, 0xFF]
    );
    assert_eq!(
        wire(&NoiseReduction3DInquiry),
        [0x81, 0x09, 0x04, 0x54, 0xFF]
    );

    assert_eq!(
        decode::<NoiseReduction2DModeInquiry>(
            InquiryKind::NoiseReduction2DMode,
            &[0x90, 0x50, 0x02, 0xFF],
        ),
        NoiseReduction2DMode::Auto,
    );
    assert_eq!(
        decode::<NoiseReduction2DModeInquiry>(
            InquiryKind::NoiseReduction2DMode,
            &[0x90, 0x50, 0x03, 0xFF],
        ),
        NoiseReduction2DMode::Manual,
    );
    assert!(Response::parse_with_type(
        &[0x90, 0x50, 0x00, 0xFF],
        &InquiryKind::NoiseReduction2DMode,
    )
    .and_then(NoiseReduction2DModeInquiry::from_response)
    .is_err());

    assert_eq!(
        decode::<NoiseReduction2DInquiry>(InquiryKind::NoiseReduction2D, &[0x90, 0x50, 0x00, 0xFF],),
        NoiseReduction2DLevel::MIN,
    );
    assert_eq!(
        decode::<NoiseReduction2DInquiry>(InquiryKind::NoiseReduction2D, &[0x90, 0x50, 0x05, 0xFF],),
        NoiseReduction2DLevel::MAX,
    );
    assert!(
        Response::parse_with_type(&[0x90, 0x50, 0x06, 0xFF], &InquiryKind::NoiseReduction2D,)
            .and_then(NoiseReduction2DInquiry::from_response)
            .is_err()
    );

    assert_eq!(
        decode::<NoiseReduction3DInquiry>(
            InquiryKind::NoiseReduction3D,
            &[0x90, 0x50, 0x00, 0xFF],
        )
        .value(),
        0,
    );
    assert_eq!(
        decode::<NoiseReduction3DInquiry>(
            InquiryKind::NoiseReduction3D,
            &[0x90, 0x50, 0x05, 0xFF],
        )
        .value(),
        5,
    );
    // This public parser has no profile context, so it accepts the value
    // type's full legacy-compatible domain. Session decoding narrows current
    // G2/G3 replies independently.
    assert_eq!(
        decode::<NoiseReduction3DInquiry>(
            InquiryKind::NoiseReduction3D,
            &[0x90, 0x50, 0x08, 0xFF],
        )
        .value(),
        8,
    );
    let response =
        Response::parse_with_type(&[0x90, 0x50, 0x09, 0xFF], &InquiryKind::NoiseReduction3D)
            .expect("well-formed frame must reach the typed inquiry conversion");
    assert!(NoiseReduction3DInquiry::from_response(response).is_err());
}

#[test]
fn noise_reduction_controls_match_the_documented_absolute_visca_frames() {
    assert_eq!(
        wire(&NoiseReduction2DModeCommand::new(
            NoiseReduction2DMode::Auto
        )),
        [0x81, 0x01, 0x04, 0x50, 0x02, 0xFF]
    );
    assert_eq!(
        wire(&NoiseReduction2DModeCommand::new(
            NoiseReduction2DMode::Manual
        )),
        [0x81, 0x01, 0x04, 0x50, 0x03, 0xFF]
    );
    assert_eq!(
        wire(&NoiseReduction2D::with_level(
            NoiseReduction2DLevel::new(3).expect("2D level"),
        )),
        [0x81, 0x01, 0x04, 0x53, 0x03, 0xFF]
    );
    assert_eq!(
        wire(&NoiseReduction2D::off()),
        [0x81, 0x01, 0x04, 0x53, 0x00, 0xFF]
    );
    assert_eq!(
        wire(&NoiseReduction3D::with_level(
            NoiseReduction3DLevel::new(8).expect("3D level 8"),
        )),
        [0x81, 0x01, 0x04, 0x54, 0x08, 0xFF]
    );
    assert_eq!(
        wire(&NoiseReduction3D::off()),
        [0x81, 0x01, 0x04, 0x54, 0x00, 0xFF]
    );
}

#[test]
fn direct_noise_reduction_requests_reject_an_inquiry_only_runtime_profile() {
    let profile = inquiry_only_noise_reduction_profile();
    assert!(profile
        .capabilities()
        .supports_typed(TypedSupportSurface::NoiseReduction2D));
    assert!(profile
        .capabilities()
        .supports_typed(TypedSupportSurface::NoiseReduction3D));
    assert!(!profile
        .capabilities()
        .supports_typed(TypedSupportSurface::NoiseReduction2DControl));
    assert!(!profile
        .capabilities()
        .supports_typed(TypedSupportSurface::NoiseReduction3DControl));

    for request in [
        NoiseReduction2DModeCommand::new(NoiseReduction2DMode::Manual)
            .validate_for_profile(&profile),
        NoiseReduction2D::with_level(NoiseReduction2DLevel::MAX).validate_for_profile(&profile),
        NoiseReduction3D::with_level(NoiseReduction3DLevel::MAX).validate_for_profile(&profile),
    ] {
        assert!(
            matches!(request, Err(Error::FeatureNotSupported { .. })),
            "the control surface, not the independent inquiry surface, must gate requests"
        );
    }
}

#[test]
fn a_mutable_profile_id_cannot_launder_the_legacy_nr3d_reply_domain() {
    let source = ProfileSpec::from_compile_time::<PtzOptics30X>().expect("legacy 30X profile");
    let coordinates = source
        .pan_tilt_coordinates()
        .expect("legacy 30X pan/tilt conversion");
    let mut capabilities = source.capabilities().clone();
    assert_eq!(
        capabilities.profile_id,
        Some(grafton_visca::profiles::ProfileId::PtzOptics30X)
    );
    capabilities.model_name = "forged legacy NR domain".into();

    let error = ProfileSpec::builder(capabilities)
        .pan_tilt_coordinates(
            coordinates.coordinate_system(),
            coordinates.pan_degrees_to_units(),
            coordinates.tilt_degrees_to_units(),
        )
        .pan_tilt_wire_codec(coordinates.wire_codec())
        .transports(source.transports())
        .envelope(source.envelope())
        .timing(source.timing())
        .maximum_command_sockets(source.maximum_command_sockets())
        .supports_operation_complete(source.supports_operation_complete())
        .supports_command_cancel(source.supports_command_cancel())
        .preset_recall_axes(source.preset_recall_axes())
        .position_inquiries(source.position_inquiries())
        .build()
        .expect_err("a built-in identity claim must require every registry profile fact");
    assert!(matches!(
        error,
        Error::InvalidRequest(message)
            if message == "built-in profile identity does not match runtime profile facts"
    ));
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
    for inquiry in [
        NoiseReduction2DModeInquiry.validate_for_profile(&g2),
        NoiseReduction2DInquiry.validate_for_profile(&g2),
        NoiseReduction3DInquiry.validate_for_profile(&g2),
        NoiseReduction2DModeInquiry.validate_for_profile(&g3),
        NoiseReduction2DInquiry.validate_for_profile(&g3),
        NoiseReduction3DInquiry.validate_for_profile(&g3),
        NoiseReduction2DModeInquiry.validate_for_profile(&ptzoptics_30x),
        NoiseReduction2DInquiry.validate_for_profile(&ptzoptics_30x),
        NoiseReduction3DInquiry.validate_for_profile(&ptzoptics_30x),
    ] {
        inquiry.expect("G2, G3, and legacy 30X document the NR inquiry");
    }

    for profile in [&g2, &g3, &ptzoptics_30x] {
        let capabilities = profile.capabilities();
        assert!(capabilities.has_noise_reduction);
        assert!(capabilities.has_2d_nr);
        assert!(capabilities.has_3d_nr);
        assert!(capabilities.supports_typed(TypedSupportSurface::NoiseReduction2D));
        assert!(capabilities.supports_typed(TypedSupportSurface::NoiseReduction3D));
        assert!(capabilities.supports_typed(TypedSupportSurface::NoiseReduction2DControl));
        assert!(capabilities.supports_typed(TypedSupportSurface::NoiseReduction3DControl));

        NoiseReduction2DModeCommand::new(NoiseReduction2DMode::Manual)
            .validate_for_profile(profile)
            .expect("NR mode control must validate");
        NoiseReduction2D::with_level(NoiseReduction2DLevel::MAX)
            .validate_for_profile(profile)
            .expect("2D NR control must validate");
        NoiseReduction3D::with_level(NoiseReduction3DLevel::MAX)
            .validate_for_profile(profile)
            .expect("3D NR control must validate at level 8");
    }

    let fr7 = ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile");
    for result in [
        FocusZoneInquiry.validate_for_profile(&fr7),
        UsbAudioInquiry.validate_for_profile(&fr7),
        PictureEffectInquiry.validate_for_profile(&fr7),
        NoiseReduction2DModeInquiry.validate_for_profile(&fr7),
        NoiseReduction2DInquiry.validate_for_profile(&fr7),
        NoiseReduction3DInquiry.validate_for_profile(&fr7),
    ] {
        assert!(matches!(result, Err(Error::FeatureNotSupported { .. })));
    }

    for control in [
        NoiseReduction2DModeCommand::new(NoiseReduction2DMode::Manual).validate_for_profile(&fr7),
        NoiseReduction2D::with_level(NoiseReduction2DLevel::MAX).validate_for_profile(&fr7),
        NoiseReduction3D::with_level(NoiseReduction3DLevel::MAX).validate_for_profile(&fr7),
    ] {
        assert!(matches!(control, Err(Error::FeatureNotSupported { .. })));
    }

    // R14 directly documents the G3 inquiry rows, so this is a regression
    // guard against accidentally retaining the former inquiry denial.
    let g3_capabilities = g3.capabilities();
    assert!(g3_capabilities.has_noise_reduction);
    assert!(g3_capabilities.has_2d_nr);
    assert!(g3_capabilities.has_3d_nr);
    assert!(g3_capabilities.supports_typed(TypedSupportSurface::NoiseReduction2D));
    assert!(g3_capabilities.supports_typed(TypedSupportSurface::NoiseReduction3D));
    assert!(g3_capabilities.supports_typed(TypedSupportSurface::NoiseReduction2DControl));
    assert!(g3_capabilities.supports_typed(TypedSupportSurface::NoiseReduction3DControl));
}
