//! Normative wire regressions identified during PR #559 review.
//!
//! These assertions use absolute VISCA frames and the public request/response
//! entry points, so they do not merely repeat the generated inquiry metadata.

#![allow(clippy::expect_used)]

use grafton_visca::{
    capabilities::TypedSupportSurface,
    command::{
        AutoWhiteBalanceSensitivityInquiry, Brightness, BrightnessInquiry, ColorTemperatureInquiry,
        DefogLevelInquiry, FocusRangeInquiry, FocusZone, FocusZoneCommand, FocusZoneInquiry,
        GammaInquiry, InquiryKind, NdFilterInquiry, NoiseReduction2D, NoiseReduction2DInquiry,
        NoiseReduction2DMode, NoiseReduction2DModeCommand, NoiseReduction2DModeInquiry,
        NoiseReduction3D, NoiseReduction3DInquiry, PictureEffectInquiry, PictureEffectMode,
        Response, ResponseParser, SharpnessModeInquiry, UsbAudio, UsbAudioInquiry, VersionInfo,
        VersionInquiry,
    },
    profiles::{
        GenericVisca, NearusBRC300, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300,
        SonyBRCH900, SonyEVIH100, SonyFR7,
    },
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

fn inquiry_only_noise_reduction_profile() -> grafton_visca::Result<ProfileSpec> {
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

#[derive(Clone, Copy)]
enum OneByteInquiryParser {
    Sharpness,
    NdFilter,
    FocusRange,
    DefogLevel,
    AutoWhiteBalanceSensitivity,
    Gamma,
}

impl OneByteInquiryParser {
    fn parse(self, response: Response) -> Result<(), Error> {
        match self {
            Self::Sharpness => SharpnessModeInquiry::from_response(response).map(|_| ()),
            Self::NdFilter => NdFilterInquiry::from_response(response).map(|_| ()),
            Self::FocusRange => FocusRangeInquiry::from_response(response).map(|_| ()),
            Self::DefogLevel => DefogLevelInquiry::from_response(response).map(|_| ()),
            Self::AutoWhiteBalanceSensitivity => {
                AutoWhiteBalanceSensitivityInquiry::from_response(response).map(|_| ())
            }
            Self::Gamma => GammaInquiry::from_response(response).map(|_| ()),
        }
    }
}

struct OneByteInquiryCase {
    name: &'static str,
    kind: InquiryKind,
    parser: OneByteInquiryParser,
    valid: &'static [u8],
    trailing: &'static [u8],
}

#[test]
fn fixed_one_byte_inquiries_accept_one_byte_and_reject_trailing_payload() {
    // The source tables describe these replies as one-byte values. Keep the
    // valid and malformed forms together so every decoder gets both checks.
    let cases = [
        OneByteInquiryCase {
            name: "sharpness mode",
            kind: InquiryKind::SharpnessMode,
            parser: OneByteInquiryParser::Sharpness,
            valid: &[0x90, 0x50, 0x02, 0xFF],
            trailing: &[0x90, 0x50, 0x02, 0x00, 0xFF],
        },
        OneByteInquiryCase {
            name: "ND filter",
            kind: InquiryKind::NdFilter,
            parser: OneByteInquiryParser::NdFilter,
            valid: &[0x90, 0x50, 0x00, 0xFF],
            trailing: &[0x90, 0x50, 0x00, 0x00, 0xFF],
        },
        OneByteInquiryCase {
            name: "focus range",
            kind: InquiryKind::FocusRange,
            parser: OneByteInquiryParser::FocusRange,
            valid: &[0x90, 0x50, 0x00, 0xFF],
            trailing: &[0x90, 0x50, 0x00, 0x00, 0xFF],
        },
        OneByteInquiryCase {
            name: "defog level",
            kind: InquiryKind::DefogLevel,
            parser: OneByteInquiryParser::DefogLevel,
            valid: &[0x90, 0x50, 0x00, 0xFF],
            trailing: &[0x90, 0x50, 0x00, 0x00, 0xFF],
        },
        OneByteInquiryCase {
            name: "AWB sensitivity",
            kind: InquiryKind::AutoWhiteBalanceSensitivity,
            parser: OneByteInquiryParser::AutoWhiteBalanceSensitivity,
            valid: &[0x90, 0x50, 0x00, 0xFF],
            trailing: &[0x90, 0x50, 0x00, 0x00, 0xFF],
        },
        OneByteInquiryCase {
            name: "gamma",
            kind: InquiryKind::Gamma,
            parser: OneByteInquiryParser::Gamma,
            valid: &[0x90, 0x50, 0x00, 0xFF],
            trailing: &[0x90, 0x50, 0x00, 0x00, 0xFF],
        },
    ];

    for case in cases {
        let valid_response = Response::parse_with_type(case.valid, &case.kind)
            .unwrap_or_else(|error| panic!("{} valid reply failed: {error:?}", case.name));
        case.parser.parse(valid_response).unwrap_or_else(|error| {
            panic!("{} valid reply failed typed decoding: {error:?}", case.name)
        });

        let error = Response::parse_with_type(case.trailing, &case.kind)
            .and_then(|response| case.parser.parse(response))
            .expect_err("trailing payload must be rejected");
        assert!(
            matches!(
                error,
                Error::InvalidResponseLength {
                    expected: 1,
                    actual: 2,
                    ..
                }
            ),
            "{} trailing reply returned {error:?}",
            case.name
        );
    }
}

#[test]
fn brightness_set_and_inquiry_match_the_04_4d_family() {
    let level = BrightnessLevel::new(0x11);
    let direct = [0x81, 0x01, 0x04, 0x4D, 0x00, 0x00, 0x01, 0x01, 0xFF];

    assert_eq!(wire(&Brightness::SetLevel(level)), direct);
    let mut camera_2_buffer = [0_u8; Brightness::MAX_SIZE];
    let camera_2_written = Brightness::SetLevel(level)
        .write_into(CameraId::CAMERA_2, &mut camera_2_buffer)
        .expect("camera 2 brightness set must encode");
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

/// PTZOptics G2 bench regression (#795, 2026-10-04): every camera on the bench
/// (PT30X/PT20X/PT12X-NDI G2, firmware ARM 6.3.51THI, 6.3.76THI, 6.4.18SHI)
/// answered the focus-zone inquiry with `90 50 03 FF`, which rc.3 rejected as
/// an unknown zone.
#[test]
fn focus_zone_inquiry_decodes_hardware_observed_zone_03() {
    assert_eq!(
        decode::<FocusZoneInquiry>(InquiryKind::FocusZone, &[0x90, 0x50, 0x03, 0xFF]),
        FocusZone::Zone03,
    );
    rejects_trailing_payload::<FocusZoneInquiry>(
        InquiryKind::FocusZone,
        &[0x90, 0x50, 0x03, 0x00, 0xFF],
    );
    // Values beyond the documented and observed set stay rejected, so a reply
    // the library cannot vouch for never becomes a guessed zone.
    assert!(
        Response::parse_with_type(&[0x90, 0x50, 0x04, 0xFF], &InquiryKind::FocusZone)
            .and_then(FocusZoneInquiry::from_response)
            .is_err()
    );
}

/// The same bench cameras accept `81 01 04 AA 03 FF` (ACK `90 42`, completion
/// `90 52`) and read `03` back, so the typed setter must emit it.
#[test]
fn focus_zone_command_encodes_hardware_observed_zone_03() {
    assert_eq!(
        wire(&FocusZoneCommand::new(FocusZone::Zone03)),
        [0x81, 0x01, 0x04, 0xAA, 0x03, 0xFF],
    );
}

/// Every zone a camera can report can be written back unchanged: the
/// inquiry reply byte and the setter parameter byte are the same value.
#[test]
fn every_focus_zone_round_trips_between_inquiry_and_setter() {
    for (byte, zone) in [
        (0x00, FocusZone::Top),
        (0x01, FocusZone::Center),
        (0x02, FocusZone::Bottom),
        (0x03, FocusZone::Zone03),
    ] {
        let read = decode::<FocusZoneInquiry>(InquiryKind::FocusZone, &[0x90, 0x50, byte, 0xFF]);
        assert_eq!(read, zone);
        assert_eq!(u8::from(read), byte);
        assert_eq!(FocusZone::try_from(byte).expect("known zone"), zone);
        assert_eq!(
            wire(&FocusZoneCommand::new(read)),
            [0x81, 0x01, 0x04, 0xAA, byte, 0xFF],
            "{zone:?} must be restorable from its own inquiry reply",
        );
    }
}

/// Focus-zone values are an evidence-backed per-profile list: the G2 family
/// (G2 and legacy 30X) adds `Zone03` from the PTZOptics G2 bench (#795), G3
/// keeps the documented zones, and profiles without the surface list none.
/// Sending is gated per value; decoding a reply is not.
#[test]
fn focus_zone_values_are_gated_per_profile_while_decoding_stays_universal() {
    let documented = [FocusZone::Top, FocusZone::Center, FocusZone::Bottom];
    let g2_family = [
        FocusZone::Top,
        FocusZone::Center,
        FocusZone::Bottom,
        FocusZone::Zone03,
    ];
    for (name, profile, expected) in [
        (
            "PtzOpticsG2",
            ProfileSpec::from_compile_time::<PtzOpticsG2>(),
            &g2_family[..],
        ),
        (
            "PtzOptics30X",
            ProfileSpec::from_compile_time::<PtzOptics30X>(),
            &g2_family[..],
        ),
        (
            "PtzOpticsG3",
            ProfileSpec::from_compile_time::<PtzOpticsG3>(),
            &documented[..],
        ),
        (
            "SonyFR7",
            ProfileSpec::from_compile_time::<SonyFR7>(),
            &[][..],
        ),
        (
            "GenericVisca",
            ProfileSpec::from_compile_time::<GenericVisca>(),
            &[][..],
        ),
    ] {
        let profile = profile.expect("built-in profile");
        assert_eq!(profile.capabilities().focus_zones, expected, "{name}");
        for zone in g2_family {
            let admitted = FocusZoneCommand::new(zone).validate_for_profile(&profile);
            assert_eq!(
                admitted.is_ok(),
                expected.contains(&zone),
                "{name} {zone:?}"
            );
        }
    }

    let g3 = ProfileSpec::from_compile_time::<PtzOpticsG3>().expect("G3 profile");
    let error = FocusZoneCommand::new(FocusZone::Zone03)
        .validate_for_profile(&g3)
        .expect_err("G3 has no evidence for focus-zone value 03");
    assert!(
        matches!(
            &error,
            Error::InvalidParameter { parameter: "focus_zone", value, .. } if value == "03"
        ),
        "unexpected error: {error:?}"
    );
    assert_eq!(
        decode::<FocusZoneInquiry>(InquiryKind::FocusZone, &[0x90, 0x50, 0x03, 0xFF]),
        FocusZone::Zone03,
        "decoding a G3 reply of 03 is not gated: it sends nothing",
    );
}

/// The Sony `CAM_VersionInq` reply (`y0 50 GG GG HH HH JJ JJ KK FF`, R12) is
/// the only layout `system().version()` decodes.
#[test]
fn version_inquiry_decodes_the_sony_reply_layout() {
    assert_eq!(wire(&VersionInquiry), [0x81, 0x09, 0x00, 0x02, 0xFF]);
    assert_eq!(
        decode::<VersionInquiry>(
            InquiryKind::Version,
            &[0x90, 0x50, 0x00, 0x01, 0x04, 0x0F, 0x01, 0x23, 0x02, 0xFF],
        ),
        VersionInfo {
            vendor: 0x0001,
            model: 0x040F,
            rom_version: 0x0123,
            max_socket: 0x02,
        },
    );
}

/// #828 M1: only the PTZOptics G2 color-temperature reply layout is sourced
/// (one data byte, `y0 50 pq FF`, PTZOptics G2 bench inquiry test #498). The
/// typed inquiry needs `HasColorTemperatureInquiry`, and every other profile
/// refuses it before I/O, including those that keep the color-temperature
/// controls.
#[test]
fn color_temperature_inquiry_is_gated_to_profiles_with_a_sourced_reply_layout() {
    for (name, profile) in [
        (
            "PtzOpticsG2",
            ProfileSpec::from_compile_time::<PtzOpticsG2>(),
        ),
        (
            "PtzOptics30X",
            ProfileSpec::from_compile_time::<PtzOptics30X>(),
        ),
    ] {
        let profile = profile.expect("built-in profile");
        assert!(profile
            .capabilities()
            .supports_typed(TypedSupportSurface::ColorTemperatureInquiry));
        ColorTemperatureInquiry
            .validate_for_profile(&profile)
            .unwrap_or_else(|error| panic!("{name} must admit the inquiry: {error}"));
    }

    for (name, profile, has_controls) in [
        (
            "PtzOpticsG3",
            ProfileSpec::from_compile_time::<PtzOpticsG3>(),
            true,
        ),
        (
            "SonyBRCH900",
            ProfileSpec::from_compile_time::<SonyBRCH900>(),
            true,
        ),
        (
            "SonyEVIH100",
            ProfileSpec::from_compile_time::<SonyEVIH100>(),
            false,
        ),
        (
            "SonyFR7",
            ProfileSpec::from_compile_time::<SonyFR7>(),
            false,
        ),
        (
            "GenericVisca",
            ProfileSpec::from_compile_time::<GenericVisca>(),
            false,
        ),
    ] {
        let profile = profile.expect("built-in profile");
        let capabilities = profile.capabilities();
        assert_eq!(
            capabilities.supports_typed(TypedSupportSurface::ColorTemperature),
            has_controls,
            "{name} color-temperature controls",
        );
        assert!(
            !capabilities.supports_typed(TypedSupportSurface::ColorTemperatureInquiry),
            "{name} must not advertise an unsourced color-temperature reply",
        );
        assert!(
            matches!(
                ColorTemperatureInquiry.validate_for_profile(&profile),
                Err(Error::FeatureNotSupported { .. })
            ),
            "{name} must refuse the color-temperature inquiry before any I/O",
        );
    }
}

/// PTZOptics G2 bench regression (#795, 2026-10-04): the cameras answer
/// `81 09 00 02 FF` with the unsourced 2-byte payload `00 52`. No source
/// defines it, so the typed inquiry is not offered to PTZOptics profiles (the
/// static accessor needs `HasVersionInquiry`, and the erased/runtime path
/// refuses before I/O), while the Sony-format profiles keep it.
#[test]
fn version_inquiry_is_gated_to_profiles_with_the_sony_reply_layout() {
    for (name, profile) in [
        (
            "PtzOpticsG2",
            ProfileSpec::from_compile_time::<PtzOpticsG2>(),
        ),
        (
            "PtzOpticsG3",
            ProfileSpec::from_compile_time::<PtzOpticsG3>(),
        ),
        (
            "PtzOptics30X",
            ProfileSpec::from_compile_time::<PtzOptics30X>(),
        ),
    ] {
        let profile = profile.expect("built-in profile");
        assert!(
            !profile
                .capabilities()
                .supports_typed(TypedSupportSurface::VersionInquiry),
            "{name} must not advertise the Sony-format version inquiry",
        );
        assert!(
            matches!(
                VersionInquiry.validate_for_profile(&profile),
                Err(Error::FeatureNotSupported { .. })
            ),
            "{name} must refuse the typed version inquiry before any I/O",
        );
    }

    for (name, profile) in [
        ("SonyFR7", ProfileSpec::from_compile_time::<SonyFR7>()),
        (
            "SonyBRCH900",
            ProfileSpec::from_compile_time::<SonyBRCH900>(),
        ),
        (
            "SonyEVIH100",
            ProfileSpec::from_compile_time::<SonyEVIH100>(),
        ),
        ("SonyBRC300", ProfileSpec::from_compile_time::<SonyBRC300>()),
        (
            "NearusBRC300",
            ProfileSpec::from_compile_time::<NearusBRC300>(),
        ),
        (
            "GenericVisca",
            ProfileSpec::from_compile_time::<GenericVisca>(),
        ),
    ] {
        let profile = profile.expect("built-in profile");
        assert!(
            profile
                .capabilities()
                .supports_typed(TypedSupportSurface::VersionInquiry),
            "{name} must advertise the Sony-format version inquiry",
        );
        VersionInquiry
            .validate_for_profile(&profile)
            .unwrap_or_else(|error| panic!("{name} must admit the version inquiry: {error}"));
    }

    // The observed PTZOptics reply is not mistaken for a Sony reply.
    let error = Response::parse_with_type(&[0x90, 0x50, 0x00, 0x52, 0xFF], &InquiryKind::Version)
        .and_then(VersionInquiry::from_response)
        .expect_err("a 2-byte payload is not the 7-byte Sony layout");
    assert_eq!(
        error.to_string(),
        "Invalid response length: expected 7 bytes, got 2 (payload: 00 52)"
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
fn inquiry_only_noise_reduction_runtime_profile_is_rejected_at_construction() {
    let error = inquiry_only_noise_reduction_profile()
        .expect_err("an inquiry-only NR profile violates the paired-surface invariant");
    assert!(matches!(error, Error::InvalidRequest(message)
        if message.contains("noise-reduction metadata and paired inquiry/control typed support must agree")));
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
    assert!(g3_capabilities.has_2d_nr);
    assert!(g3_capabilities.has_3d_nr);
    assert!(g3_capabilities.supports_typed(TypedSupportSurface::NoiseReduction2D));
    assert!(g3_capabilities.supports_typed(TypedSupportSurface::NoiseReduction3D));
    assert!(g3_capabilities.supports_typed(TypedSupportSurface::NoiseReduction2DControl));
    assert!(g3_capabilities.supports_typed(TypedSupportSurface::NoiseReduction3DControl));
}
