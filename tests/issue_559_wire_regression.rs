//! Normative wire regressions identified during PR #559 review.
//!
//! These assertions use absolute VISCA frames and the public request/response
//! entry points, so they do not merely repeat the generated inquiry metadata.

#![allow(clippy::expect_used)]

use grafton_visca::wire::WireValue;

use grafton_visca::{
    capabilities::TypedSupportSurface,
    command::{
        image::{SetNoiseReduction2d, SetNoiseReduction3d},
        streaming::SetUsbAudio,
    },
    profiles::{
        GenericVisca, NearusBrc300, ProfileSpec, PtzOptics30x, PtzOpticsG2, PtzOpticsG3,
        SonyBrc300, SonyBrcH900, SonyEviH100, SonyFr7,
    },
    request::{Inquiry, Request},
    types::{
        BrightnessLevel, CameraId, FocusZone, NoiseReduction2D, NoiseReduction2DLevel,
        NoiseReduction2DMode, NoiseReduction3D, NoiseReduction3DLevel, PictureEffectMode,
        VersionInfo,
    },
    wire::Response,
    Error,
};

fn wire<R: Request + ?Sized>(request: &R) -> Vec<u8> {
    let mut buffer = vec![0_u8; R::MAX_SIZE];
    let written = request
        .write_into(CameraId::CAMERA_1, &mut buffer)
        .expect("golden request must encode");
    buffer.truncate(written);
    buffer
}

fn noise_reduction_profile_without_grants() -> grafton_visca::Result<ProfileSpec> {
    let source = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile");
    let mut capabilities = source.capabilities().clone();
    capabilities.profile_id = None;
    capabilities.model_name = "NR without unified grants".into();
    capabilities.typed_support = capabilities
        .typed_support
        .without(TypedSupportSurface::NoiseReduction2D)
        .without(TypedSupportSurface::NoiseReduction2DMode)
        .without(TypedSupportSurface::NoiseReduction3D);

    let mut builder = ProfileSpec::builder(capabilities)
        .transports(source.transports())
        .envelope(source.envelope())
        .timing(source.timing())
        .maximum_command_sockets(source.maximum_command_sockets())
        .supports_operation_complete(source.supports_operation_complete())
        .supports_command_cancel(source.supports_command_cancel())
        .position_inquiries(source.position_inquiries());
    for encoding in source.encodings().iter() {
        builder = builder.encoding(encoding);
    }
    builder.build()
}

fn decode_result<I: Inquiry>(inquiry: I, frame: &[u8]) -> Result<I::Response, Error> {
    Response::parse(frame)?;
    if frame.get(1) != Some(&0x50) {
        return Err(Error::UnexpectedResponseType);
    }
    let payload = frame
        .get(2..frame.len().saturating_sub(1))
        .ok_or(Error::InvalidResponseFormat)?;
    inquiry.decoder().decode(payload)
}

fn decode<I: Inquiry>(inquiry: I, frame: &[u8]) -> I::Response {
    decode_result(inquiry, frame).expect("golden response must have the expected type")
}

fn rejects_trailing_payload<I: Inquiry>(inquiry: I, frame: &[u8]) {
    assert!(
        decode_result(inquiry, frame).is_err(),
        "inquiry must reject trailing payload bytes"
    );
}

#[derive(Clone, Copy)]
enum OneByteInquiryParser {
    Sharpness,
    AutoWhiteBalanceSensitivity,
    Gamma,
}

impl OneByteInquiryParser {
    fn parse(self, frame: &[u8]) -> Result<(), Error> {
        match self {
            Self::Sharpness => {
                decode_result(grafton_visca::command::image::SharpnessModeInquiry, frame)
                    .map(|_| ())
            }
            Self::AutoWhiteBalanceSensitivity => decode_result(
                grafton_visca::command::white_balance::SensitivityInquiry,
                frame,
            )
            .map(|_| ()),
            Self::Gamma => {
                decode_result(grafton_visca::command::image::GammaInquiry, frame).map(|_| ())
            }
        }
    }
}

struct OneByteInquiryCase {
    name: &'static str,
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
            parser: OneByteInquiryParser::Sharpness,
            valid: &[0x90, 0x50, 0x02, 0xFF],
            trailing: &[0x90, 0x50, 0x02, 0x00, 0xFF],
        },
        OneByteInquiryCase {
            name: "AWB sensitivity",
            parser: OneByteInquiryParser::AutoWhiteBalanceSensitivity,
            valid: &[0x90, 0x50, 0x00, 0xFF],
            trailing: &[0x90, 0x50, 0x00, 0x00, 0xFF],
        },
        OneByteInquiryCase {
            name: "gamma",
            parser: OneByteInquiryParser::Gamma,
            valid: &[0x90, 0x50, 0x00, 0xFF],
            trailing: &[0x90, 0x50, 0x00, 0x00, 0xFF],
        },
    ];

    for case in cases {
        case.parser.parse(case.valid).unwrap_or_else(|error| {
            panic!("{} valid reply failed typed decoding: {error:?}", case.name)
        });

        let error = case
            .parser
            .parse(case.trailing)
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
    let level = BrightnessLevel::new(0x11).expect("valid value");
    let direct = [0x81, 0x01, 0x04, 0x4D, 0x00, 0x00, 0x01, 0x01, 0xFF];

    assert_eq!(
        wire(&grafton_visca::command::exposure::SetBrightness::new(level)),
        direct
    );
    let mut camera_2_buffer = [0_u8; grafton_visca::command::exposure::SetBrightness::MAX_SIZE];
    let camera_2_written = grafton_visca::command::exposure::SetBrightness::new(level)
        .write_into(CameraId::CAMERA_2, &mut camera_2_buffer)
        .expect("camera 2 brightness set must encode");
    assert_eq!(
        &camera_2_buffer[..camera_2_written],
        &[0x82, 0x01, 0x04, 0x4D, 0x00, 0x00, 0x01, 0x01, 0xFF]
    );
    assert_eq!(
        wire(&grafton_visca::command::exposure::BrightnessInquiry),
        [0x81, 0x09, 0x04, 0x4D, 0xFF]
    );
    assert_eq!(
        decode(
            grafton_visca::command::exposure::BrightnessInquiry,
            &[0x90, 0x50, 0x00, 0x00, 0x01, 0x01, 0xFF],
        ),
        level,
    );
}

#[test]
fn focus_zone_inquiry_uses_af_zone_opcode_and_decodes_positions() {
    assert_eq!(
        wire(&grafton_visca::command::focus::ZoneInquiry),
        [0x81, 0x09, 0x04, 0xAA, 0xFF]
    );
    assert_eq!(
        decode(
            grafton_visca::command::focus::ZoneInquiry,
            &[0x90, 0x50, 0x00, 0xFF]
        ),
        FocusZone::Top,
    );
    assert_eq!(
        decode(
            grafton_visca::command::focus::ZoneInquiry,
            &[0x90, 0x50, 0x01, 0xFF]
        ),
        FocusZone::Center,
    );
    assert_eq!(
        decode(
            grafton_visca::command::focus::ZoneInquiry,
            &[0x90, 0x50, 0x02, 0xFF]
        ),
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
        decode(
            grafton_visca::command::focus::ZoneInquiry,
            &[0x90, 0x50, 0x03, 0xFF]
        ),
        FocusZone::Zone03,
    );
    rejects_trailing_payload(
        grafton_visca::command::focus::ZoneInquiry,
        &[0x90, 0x50, 0x03, 0x00, 0xFF],
    );
    // Values beyond the documented and observed set stay rejected, so a reply
    // the library cannot vouch for never becomes a guessed zone.
    assert!(decode_result(
        grafton_visca::command::focus::ZoneInquiry,
        &[0x90, 0x50, 0x04, 0xFF]
    )
    .is_err());
}

/// The same bench cameras accept `81 01 04 AA 03 FF` (ACK `90 42`, completion
/// `90 52`) and read `03` back, so the typed setter must emit it.
#[test]
fn focus_zone_command_encodes_hardware_observed_zone_03() {
    assert_eq!(
        wire(&grafton_visca::command::focus::SetZone::new(
            FocusZone::Zone03
        )),
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
        let read = decode(
            grafton_visca::command::focus::ZoneInquiry,
            &[0x90, 0x50, byte, 0xFF],
        );
        assert_eq!(read, zone);
        assert_eq!(read.to_wire(), byte);
        assert_eq!(FocusZone::from_wire(byte).expect("known zone"), zone);
        assert_eq!(
            wire(&grafton_visca::command::focus::SetZone::new(read)),
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
            "PtzOptics30x",
            ProfileSpec::from_compile_time::<PtzOptics30x>(),
            &g2_family[..],
        ),
        (
            "PtzOpticsG3",
            ProfileSpec::from_compile_time::<PtzOpticsG3>(),
            &documented[..],
        ),
        (
            "SonyFr7",
            ProfileSpec::from_compile_time::<SonyFr7>(),
            &[][..],
        ),
        (
            "GenericVisca",
            ProfileSpec::from_compile_time::<GenericVisca>(),
            &[][..],
        ),
    ] {
        let profile = profile.expect("built-in profile");
        assert_eq!(
            profile
                .capabilities()
                .focus
                .as_ref()
                .expect("focus domain")
                .zones,
            expected,
            "{name}"
        );
        for zone in g2_family {
            let admitted = grafton_visca::command::focus::SetZone::new(zone).validate_for_target(
                &grafton_visca::config::TargetRegistration::new(
                    grafton_visca::types::CameraId::CAMERA_1,
                    profile.clone(),
                )
                .expect("target registration"),
            );
            assert_eq!(
                admitted.is_ok(),
                expected.contains(&zone),
                "{name} {zone:?}"
            );
        }
    }

    let g3 = ProfileSpec::from_compile_time::<PtzOpticsG3>().expect("G3 profile");
    let error = grafton_visca::command::focus::SetZone::new(FocusZone::Zone03)
        .validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                g3.clone(),
            )
            .expect("target registration"),
        )
        .expect_err("G3 has no evidence for focus-zone value 03");
    assert!(
        matches!(
            &error,
            Error::InvalidParameter { parameter: "focus_zone", value, .. } if value == "03"
        ),
        "unexpected error: {error:?}"
    );
    assert_eq!(
        decode(
            grafton_visca::command::focus::ZoneInquiry,
            &[0x90, 0x50, 0x03, 0xFF]
        ),
        FocusZone::Zone03,
        "decoding a G3 reply of 03 is not gated: it sends nothing",
    );
}

/// The Sony `CAM_VersionInq` reply (`y0 50 GG GG HH HH JJ JJ KK FF`, R12) is
/// the only layout `system().version()` decodes.
#[test]
fn version_inquiry_decodes_the_sony_reply_layout() {
    assert_eq!(
        wire(&grafton_visca::command::system::VersionInquiry),
        [0x81, 0x09, 0x00, 0x02, 0xFF]
    );
    assert_eq!(
        decode(
            grafton_visca::command::system::VersionInquiry,
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
            "PtzOptics30x",
            ProfileSpec::from_compile_time::<PtzOptics30x>(),
        ),
    ] {
        let profile = profile.expect("built-in profile");
        assert!(profile
            .capabilities()
            .supports(TypedSupportSurface::ColorTemperatureInquiry));
        grafton_visca::command::white_balance::ColorTemperatureInquiry
            .validate_for_target(
                &grafton_visca::config::TargetRegistration::new(
                    grafton_visca::types::CameraId::CAMERA_1,
                    profile.clone(),
                )
                .expect("target registration"),
            )
            .unwrap_or_else(|error| panic!("{name} must admit the inquiry: {error}"));
    }

    for (name, profile, has_controls) in [
        (
            "PtzOpticsG3",
            ProfileSpec::from_compile_time::<PtzOpticsG3>(),
            true,
        ),
        (
            "SonyBrcH900",
            ProfileSpec::from_compile_time::<SonyBrcH900>(),
            false,
        ),
        (
            "SonyEviH100",
            ProfileSpec::from_compile_time::<SonyEviH100>(),
            false,
        ),
        (
            "SonyFr7",
            ProfileSpec::from_compile_time::<SonyFr7>(),
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
            capabilities.supports(TypedSupportSurface::ColorTemperature),
            has_controls,
            "{name} color-temperature controls",
        );
        assert!(
            !capabilities.supports(TypedSupportSurface::ColorTemperatureInquiry),
            "{name} must not advertise an unsourced color-temperature reply",
        );
        assert!(
            matches!(
                grafton_visca::command::white_balance::ColorTemperatureInquiry.validate_for_target(
                    &grafton_visca::config::TargetRegistration::new(
                        grafton_visca::types::CameraId::CAMERA_1,
                        profile.clone()
                    )
                    .expect("target registration")
                ),
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
            "PtzOptics30x",
            ProfileSpec::from_compile_time::<PtzOptics30x>(),
        ),
    ] {
        let profile = profile.expect("built-in profile");
        assert!(
            !profile
                .capabilities()
                .supports(TypedSupportSurface::VersionInquiry),
            "{name} must not advertise the Sony-format version inquiry",
        );
        assert!(
            matches!(
                grafton_visca::command::system::VersionInquiry.validate_for_target(
                    &grafton_visca::config::TargetRegistration::new(
                        grafton_visca::types::CameraId::CAMERA_1,
                        profile.clone()
                    )
                    .expect("target registration")
                ),
                Err(Error::FeatureNotSupported { .. })
            ),
            "{name} must refuse the typed version inquiry before any I/O",
        );
    }

    for (name, profile) in [
        ("SonyFr7", ProfileSpec::from_compile_time::<SonyFr7>()),
        (
            "SonyBrcH900",
            ProfileSpec::from_compile_time::<SonyBrcH900>(),
        ),
        (
            "SonyEviH100",
            ProfileSpec::from_compile_time::<SonyEviH100>(),
        ),
        ("SonyBrc300", ProfileSpec::from_compile_time::<SonyBrc300>()),
        (
            "NearusBrc300",
            ProfileSpec::from_compile_time::<NearusBrc300>(),
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
                .supports(TypedSupportSurface::VersionInquiry),
            "{name} must advertise the Sony-format version inquiry",
        );
        grafton_visca::command::system::VersionInquiry
            .validate_for_target(
                &grafton_visca::config::TargetRegistration::new(
                    grafton_visca::types::CameraId::CAMERA_1,
                    profile.clone(),
                )
                .expect("target registration"),
            )
            .unwrap_or_else(|error| panic!("{name} must admit the version inquiry: {error}"));
    }

    // The observed PTZOptics reply is not mistaken for a Sony reply.
    let error = decode_result(
        grafton_visca::command::system::VersionInquiry,
        &[0x90, 0x50, 0x00, 0x52, 0xFF],
    )
    .expect_err("a 2-byte payload is not the 7-byte Sony layout");
    assert_eq!(
        error.to_string(),
        "Invalid response length: expected 7 bytes, got 2 (payload: 00 52)"
    );
}

#[test]
fn picture_effect_inquiry_uses_the_direct_command_opcode_and_decodes_modes() {
    assert_eq!(
        wire(&grafton_visca::command::image::PictureEffectInquiry),
        [0x81, 0x09, 0x04, 0x63, 0xFF]
    );
    assert_eq!(
        decode(
            grafton_visca::command::image::PictureEffectInquiry,
            &[0x90, 0x50, 0x02, 0xFF]
        ),
        PictureEffectMode::Off,
    );
    assert_eq!(
        decode(
            grafton_visca::command::image::PictureEffectInquiry,
            &[0x90, 0x50, 0x04, 0xFF]
        ),
        PictureEffectMode::BlackAndWhite,
    );
    rejects_trailing_payload(
        grafton_visca::command::image::PictureEffectInquiry,
        &[0x90, 0x50, 0x02, 0x00, 0xFF],
    );
}

#[test]
fn usb_audio_inquiry_matches_uac_wire_and_its_02_on_03_off_polarity() {
    assert_eq!(
        wire(&grafton_visca::command::streaming::UsbAudioInquiry),
        [0x81, 0x2A, 0x02, 0xA0, 0x04, 0xFF]
    );
    assert_eq!(
        wire(&SetUsbAudio::new(true)),
        [0x81, 0x2A, 0x02, 0xA0, 0x04, 0x02, 0xFF]
    );
    assert_eq!(
        wire(&SetUsbAudio::new(false)),
        [0x81, 0x2A, 0x02, 0xA0, 0x04, 0x03, 0xFF]
    );
    assert!(decode(
        grafton_visca::command::streaming::UsbAudioInquiry,
        &[0x90, 0x50, 0x02, 0xFF],
    ));
    assert!(!decode(
        grafton_visca::command::streaming::UsbAudioInquiry,
        &[0x90, 0x50, 0x03, 0xFF],
    ));
    rejects_trailing_payload(
        grafton_visca::command::streaming::UsbAudioInquiry,
        &[0x90, 0x50, 0x02, 0x00, 0xFF],
    );
}

#[test]
fn noise_reduction_inquiries_match_the_documented_2d_mode_and_level_registers() {
    assert_eq!(
        wire(&grafton_visca::command::image::NoiseReduction2dModeInquiry),
        [0x81, 0x09, 0x04, 0x50, 0xFF]
    );
    assert_eq!(
        wire(&grafton_visca::command::image::NoiseReduction2dInquiry),
        [0x81, 0x09, 0x04, 0x53, 0xFF]
    );
    assert_eq!(
        wire(&grafton_visca::command::image::NoiseReduction3dInquiry),
        [0x81, 0x09, 0x04, 0x54, 0xFF]
    );

    assert_eq!(
        decode(
            grafton_visca::command::image::NoiseReduction2dModeInquiry,
            &[0x90, 0x50, 0x02, 0xFF],
        ),
        NoiseReduction2DMode::Auto,
    );
    assert_eq!(
        decode(
            grafton_visca::command::image::NoiseReduction2dModeInquiry,
            &[0x90, 0x50, 0x03, 0xFF],
        ),
        NoiseReduction2DMode::Manual,
    );
    assert!(decode_result(
        grafton_visca::command::image::NoiseReduction2dModeInquiry,
        &[0x90, 0x50, 0x00, 0xFF]
    )
    .is_err());

    assert_eq!(
        decode(
            grafton_visca::command::image::NoiseReduction2dInquiry,
            &[0x90, 0x50, 0x00, 0xFF],
        ),
        NoiseReduction2D::Off,
    );
    assert_eq!(
        decode(
            grafton_visca::command::image::NoiseReduction2dInquiry,
            &[0x90, 0x50, 0x05, 0xFF],
        ),
        NoiseReduction2D::Level(NoiseReduction2DLevel::MAX),
    );
    assert!(decode_result(
        grafton_visca::command::image::NoiseReduction2dInquiry,
        &[0x90, 0x50, 0x06, 0xFF]
    )
    .is_err());

    assert_eq!(
        decode(
            grafton_visca::command::image::NoiseReduction3dInquiry,
            &[0x90, 0x50, 0x00, 0xFF],
        )
        .to_wire(),
        0,
    );
    assert_eq!(
        decode(
            grafton_visca::command::image::NoiseReduction3dInquiry,
            &[0x90, 0x50, 0x05, 0xFF],
        )
        .to_wire(),
        5,
    );
    // This public parser has no profile context, so it accepts the value
    // type's full syntactic register domain. Session decoding narrows current
    // G2/G3 replies independently.
    assert_eq!(
        decode(
            grafton_visca::command::image::NoiseReduction3dInquiry,
            &[0x90, 0x50, 0x08, 0xFF],
        )
        .to_wire(),
        8,
    );
    Response::parse(&[0x90, 0x50, 0x09, 0xFF])
        .expect("well-formed frame must reach the typed inquiry conversion");
    assert!(decode_result(
        grafton_visca::command::image::NoiseReduction3dInquiry,
        &[0x90, 0x50, 0x09, 0xFF],
    )
    .is_err());
}

#[test]
fn noise_reduction_controls_match_the_documented_absolute_visca_frames() {
    assert_eq!(
        wire(
            &grafton_visca::command::image::SetNoiseReduction2dMode::new(
                NoiseReduction2DMode::Auto
            )
        ),
        [0x81, 0x01, 0x04, 0x50, 0x02, 0xFF]
    );
    assert_eq!(
        wire(
            &grafton_visca::command::image::SetNoiseReduction2dMode::new(
                NoiseReduction2DMode::Manual
            )
        ),
        [0x81, 0x01, 0x04, 0x50, 0x03, 0xFF]
    );
    assert_eq!(
        wire(&SetNoiseReduction2d::new(NoiseReduction2D::with_level(
            NoiseReduction2DLevel::new(3).expect("2D level"),
        ))),
        [0x81, 0x01, 0x04, 0x53, 0x03, 0xFF]
    );
    assert_eq!(
        wire(&SetNoiseReduction2d::new(NoiseReduction2D::Off)),
        [0x81, 0x01, 0x04, 0x53, 0x00, 0xFF]
    );
    assert_eq!(
        wire(&SetNoiseReduction3d::new(NoiseReduction3D::with_level(
            NoiseReduction3DLevel::new(8).expect("3D level 8"),
        ))),
        [0x81, 0x01, 0x04, 0x54, 0x08, 0xFF]
    );
    assert_eq!(
        wire(&SetNoiseReduction3d::new(NoiseReduction3D::Off)),
        [0x81, 0x01, 0x04, 0x54, 0x00, 0xFF]
    );
}

#[test]
fn noise_reduction_grants_admit_or_refuse_both_queries_and_settings() {
    let source = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile");
    let supported = grafton_visca::config::TargetRegistration::new(CameraId::CAMERA_1, source)
        .expect("supported target");
    for result in [
        grafton_visca::command::image::NoiseReduction2dInquiry.validate_for_target(&supported),
        grafton_visca::command::image::NoiseReduction3dInquiry.validate_for_target(&supported),
        SetNoiseReduction2d::new(NoiseReduction2D::Off).validate_for_target(&supported),
        SetNoiseReduction3d::new(NoiseReduction3D::Off).validate_for_target(&supported),
    ] {
        result.expect("a unified grant permits both read and write");
    }
    let profile = noise_reduction_profile_without_grants().expect("withheld NR grants");
    let target = grafton_visca::config::TargetRegistration::new(CameraId::CAMERA_1, profile)
        .expect("withheld target");
    for result in [
        grafton_visca::command::image::NoiseReduction2dInquiry.validate_for_target(&target),
        grafton_visca::command::image::NoiseReduction3dInquiry.validate_for_target(&target),
        SetNoiseReduction2d::new(NoiseReduction2D::Off).validate_for_target(&target),
        SetNoiseReduction3d::new(NoiseReduction3D::Off).validate_for_target(&target),
    ] {
        assert!(matches!(result, Err(Error::FeatureNotSupported { .. })));
    }
}

#[test]
fn focus_zone_and_usb_audio_inquiries_require_their_source_backed_profile_gates() {
    let g2 = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile");
    grafton_visca::command::focus::ZoneInquiry
        .validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                g2.clone(),
            )
            .expect("target registration"),
        )
        .expect("G2 documents the focus-zone response");
    grafton_visca::command::streaming::UsbAudioInquiry
        .validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                g2.clone(),
            )
            .expect("target registration"),
        )
        .expect("G2 documents USB audio");

    let g3 = ProfileSpec::from_compile_time::<PtzOpticsG3>().expect("G3 profile");
    assert!(matches!(
        grafton_visca::command::focus::ZoneInquiry.validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                g3.clone()
            )
            .expect("target registration")
        ),
        Err(Error::FeatureNotSupported { .. })
    ));
    assert!(matches!(
        grafton_visca::command::streaming::UsbAudioInquiry.validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                g3.clone()
            )
            .expect("target registration")
        ),
        Err(Error::FeatureNotSupported { .. })
    ));

    let ptzoptics_30x = ProfileSpec::from_compile_time::<PtzOptics30x>().expect("30X profile");
    grafton_visca::command::focus::ZoneInquiry
        .validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                ptzoptics_30x.clone(),
            )
            .expect("target registration"),
        )
        .expect("30X documents the focus-zone response");
    grafton_visca::command::streaming::UsbAudioInquiry
        .validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                ptzoptics_30x.clone(),
            )
            .expect("target registration"),
        )
        .expect("30X documents USB audio");
    for inquiry in [
        grafton_visca::command::image::NoiseReduction2dModeInquiry.validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                g2.clone(),
            )
            .expect("target registration"),
        ),
        grafton_visca::command::image::NoiseReduction2dInquiry.validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                g2.clone(),
            )
            .expect("target registration"),
        ),
        grafton_visca::command::image::NoiseReduction3dInquiry.validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                g2.clone(),
            )
            .expect("target registration"),
        ),
        grafton_visca::command::image::NoiseReduction2dModeInquiry.validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                g3.clone(),
            )
            .expect("target registration"),
        ),
        grafton_visca::command::image::NoiseReduction2dInquiry.validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                g3.clone(),
            )
            .expect("target registration"),
        ),
        grafton_visca::command::image::NoiseReduction3dInquiry.validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                g3.clone(),
            )
            .expect("target registration"),
        ),
        grafton_visca::command::image::NoiseReduction2dModeInquiry.validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                ptzoptics_30x.clone(),
            )
            .expect("target registration"),
        ),
        grafton_visca::command::image::NoiseReduction2dInquiry.validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                ptzoptics_30x.clone(),
            )
            .expect("target registration"),
        ),
        grafton_visca::command::image::NoiseReduction3dInquiry.validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                ptzoptics_30x.clone(),
            )
            .expect("target registration"),
        ),
    ] {
        inquiry.expect("G2, G3, and legacy 30X document the NR inquiry");
    }

    for profile in [&g2, &g3, &ptzoptics_30x] {
        let capabilities = profile.capabilities();
        assert!(capabilities.supports(TypedSupportSurface::NoiseReduction2D));
        assert!(capabilities.supports(TypedSupportSurface::NoiseReduction3D));

        grafton_visca::command::image::SetNoiseReduction2dMode::new(NoiseReduction2DMode::Manual)
            .validate_for_target(
                &grafton_visca::config::TargetRegistration::new(
                    grafton_visca::types::CameraId::CAMERA_1,
                    profile.clone(),
                )
                .expect("target registration"),
            )
            .expect("NR mode control must validate");
        SetNoiseReduction2d::new(NoiseReduction2D::with_level(NoiseReduction2DLevel::MAX))
            .validate_for_target(
                &grafton_visca::config::TargetRegistration::new(
                    grafton_visca::types::CameraId::CAMERA_1,
                    profile.clone(),
                )
                .expect("target registration"),
            )
            .expect("2D NR control must validate");
        SetNoiseReduction3d::new(NoiseReduction3D::with_level(NoiseReduction3DLevel::MAX))
            .validate_for_target(
                &grafton_visca::config::TargetRegistration::new(
                    grafton_visca::types::CameraId::CAMERA_1,
                    profile.clone(),
                )
                .expect("target registration"),
            )
            .expect("3D NR control must validate at level 8");
    }

    let fr7 = ProfileSpec::from_compile_time::<SonyFr7>().expect("FR7 profile");
    for result in [
        grafton_visca::command::focus::ZoneInquiry.validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                fr7.clone(),
            )
            .expect("target registration"),
        ),
        grafton_visca::command::streaming::UsbAudioInquiry.validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                fr7.clone(),
            )
            .expect("target registration"),
        ),
        grafton_visca::command::image::PictureEffectInquiry.validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                fr7.clone(),
            )
            .expect("target registration"),
        ),
        grafton_visca::command::image::NoiseReduction2dModeInquiry.validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                fr7.clone(),
            )
            .expect("target registration"),
        ),
        grafton_visca::command::image::NoiseReduction2dInquiry.validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                fr7.clone(),
            )
            .expect("target registration"),
        ),
        grafton_visca::command::image::NoiseReduction3dInquiry.validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                fr7.clone(),
            )
            .expect("target registration"),
        ),
    ] {
        assert!(matches!(result, Err(Error::FeatureNotSupported { .. })));
    }

    for control in [
        grafton_visca::command::image::SetNoiseReduction2dMode::new(NoiseReduction2DMode::Manual)
            .validate_for_target(
                &grafton_visca::config::TargetRegistration::new(
                    grafton_visca::types::CameraId::CAMERA_1,
                    fr7.clone(),
                )
                .expect("target registration"),
            ),
        SetNoiseReduction2d::new(NoiseReduction2D::with_level(NoiseReduction2DLevel::MAX))
            .validate_for_target(
                &grafton_visca::config::TargetRegistration::new(
                    grafton_visca::types::CameraId::CAMERA_1,
                    fr7.clone(),
                )
                .expect("target registration"),
            ),
        SetNoiseReduction3d::new(NoiseReduction3D::with_level(NoiseReduction3DLevel::MAX))
            .validate_for_target(
                &grafton_visca::config::TargetRegistration::new(
                    grafton_visca::types::CameraId::CAMERA_1,
                    fr7.clone(),
                )
                .expect("target registration"),
            ),
    ] {
        assert!(matches!(control, Err(Error::FeatureNotSupported { .. })));
    }

    // R14 directly documents the G3 inquiry rows, so this is a regression
    // guard against accidentally retaining the former inquiry denial.
    let g3_capabilities = g3.capabilities();
    assert!(g3_capabilities.supports(TypedSupportSurface::NoiseReduction2D));
    assert!(g3_capabilities.supports(TypedSupportSurface::NoiseReduction3D));
}

/// R7 v4.00 p.14: two zero padding bytes precede the variable-ND density code.
#[test]
fn fr7_variable_nd_direct_preserves_both_padding_bytes_and_exact_bound() {
    use grafton_visca::{
        request::Request,
        types::{CameraId, NdFilterDensity},
    };

    assert_eq!(grafton_visca::command::nd_filter::SetDensity::MAX_SIZE, 10);
    for density in 0..=0x14 {
        let request = grafton_visca::command::nd_filter::SetDensity::new(
            NdFilterDensity::new(density).expect("sourced density"),
        );
        let mut frame = [0; 10];
        assert_eq!(
            request
                .write_into(CameraId::CAMERA_1, &mut frame)
                .expect("exact-sized buffer"),
            10
        );
        assert_eq!(
            frame,
            [
                0x81,
                0x01,
                0x7E,
                0x04,
                0x42,
                0,
                0,
                (density >> 4) as u8,
                (density & 0x0F) as u8,
                0xFF
            ]
        );
        assert!(request.write_into(CameraId::CAMERA_1, &mut [0; 9]).is_err());
    }
}
