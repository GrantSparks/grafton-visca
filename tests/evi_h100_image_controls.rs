//! Sony EVI-H100 (R8) image controls: what is granted matches R8 byte for
//! byte, and what R8 documents with a different layout is withheld.

#![allow(clippy::expect_used)]

use grafton_visca::wire::WireValue;
use grafton_visca::{
    capabilities::TypedSupportSurface,
    command::{
        HueCommand, HueInquiry, NoiseReduction2D, NoiseReduction2DInquiry, NoiseReduction2DMode,
        NoiseReduction2DModeCommand, NoiseReduction2DModeInquiry, PictureEffectCommand,
        PictureEffectInquiry, PictureEffectMode, SaturationCommand, SaturationInquiry, Sharpness,
        SharpnessMode, SharpnessModeInquiry, SharpnessPositionInquiry,
    },
    profiles::SonyEVIH100,
    types::{HueLevel, SaturationLevel, SharpnessLevel},
    CameraId, Error, ProfileSpec, Request,
};

fn wire<R: Request + ?Sized>(request: &R) -> Vec<u8> {
    let mut buffer = vec![0_u8; R::MAX_SIZE];
    let written = request
        .write_into(CameraId::CAMERA_1, &mut buffer)
        .expect("request must encode");
    buffer.truncate(written);
    buffer
}

fn profile() -> ProfileSpec {
    ProfileSpec::from_compile_time::<SonyEVIH100>().expect("EVI-H100 profile")
}

#[test]
fn r8_noise_reduction_is_the_2d_level_family() {
    let profile = profile();
    // `CAM_NR` `8x 01 04 53 0p FF`, p = 0 (off) and 1..5.
    for level in 0..=5 {
        let command = NoiseReduction2D::from_wire(level).expect("level or Off");
        command
            .validate_for_target(
                &grafton_visca::TargetRegistration::new(
                    grafton_visca::CameraId::CAMERA_1,
                    profile.clone(),
                )
                .expect("target registration"),
            )
            .unwrap_or_else(|error| panic!("NR level {level}: {error}"));
        assert_eq!(wire(&command), [0x81, 0x01, 0x04, 0x53, level, 0xFF]);
    }
    // `CAM_NRInq` `8x 09 04 53 FF`.
    NoiseReduction2DInquiry
        .validate_for_target(
            &grafton_visca::TargetRegistration::new(
                grafton_visca::CameraId::CAMERA_1,
                profile.clone(),
            )
            .expect("target registration"),
        )
        .expect("NR level inquiry");
    assert_eq!(
        wire(&NoiseReduction2DInquiry),
        [0x81, 0x09, 0x04, 0x53, 0xFF]
    );

    // R8 lists no `04 50` auto/manual mode.
    assert!(matches!(
        NoiseReduction2DModeCommand::new(NoiseReduction2DMode::Auto).validate_for_target(
            &grafton_visca::TargetRegistration::new(
                grafton_visca::CameraId::CAMERA_1,
                profile.clone()
            )
            .expect("target registration")
        ),
        Err(Error::FeatureNotSupported { .. })
    ));
    assert!(matches!(
        NoiseReduction2DModeInquiry.validate_for_target(
            &grafton_visca::TargetRegistration::new(
                grafton_visca::CameraId::CAMERA_1,
                profile.clone()
            )
            .expect("target registration")
        ),
        Err(Error::FeatureNotSupported { .. })
    ));
}

#[test]
fn r8_color_gain_and_hue_are_the_saturation_and_hue_controls() {
    let profile = profile();
    // `CAM_ColorGain` Direct `8x 01 04 49 00 00 00 0p FF`, 0h..Eh.
    let saturation = SaturationCommand::new(SaturationLevel::new(0x0E).expect("level"));
    saturation
        .validate_for_target(
            &grafton_visca::TargetRegistration::new(
                grafton_visca::CameraId::CAMERA_1,
                profile.clone(),
            )
            .expect("target registration"),
        )
        .expect("color gain");
    assert_eq!(
        wire(&saturation),
        [0x81, 0x01, 0x04, 0x49, 0x00, 0x00, 0x00, 0x0E, 0xFF]
    );
    SaturationInquiry
        .validate_for_target(
            &grafton_visca::TargetRegistration::new(
                grafton_visca::CameraId::CAMERA_1,
                profile.clone(),
            )
            .expect("target registration"),
        )
        .expect("color gain inquiry");
    assert_eq!(wire(&SaturationInquiry), [0x81, 0x09, 0x04, 0x49, 0xFF]);

    // `CAM_ColorHue` Direct `8x 01 04 4F 00 00 00 0p FF`, 0h..Eh.
    let hue = HueCommand::new(HueLevel::new(0x0E).expect("level"));
    hue.validate_for_target(
        &grafton_visca::TargetRegistration::new(grafton_visca::CameraId::CAMERA_1, profile.clone())
            .expect("target registration"),
    )
    .expect("color hue");
    assert_eq!(
        wire(&hue),
        [0x81, 0x01, 0x04, 0x4F, 0x00, 0x00, 0x00, 0x0E, 0xFF]
    );
    HueInquiry
        .validate_for_target(
            &grafton_visca::TargetRegistration::new(
                grafton_visca::CameraId::CAMERA_1,
                profile.clone(),
            )
            .expect("target registration"),
        )
        .expect("color hue inquiry");
    assert_eq!(wire(&HueInquiry), [0x81, 0x09, 0x04, 0x4F, 0xFF]);
}

#[test]
fn r8_aperture_levels_are_typed_without_a_sharpness_mode_grant() {
    let profile = profile();
    let capabilities = profile.capabilities();
    assert_eq!(
        capabilities
            .image
            .as_ref()
            .expect("image domain")
            .sharpness_range
            .map(|range| range.as_inclusive()),
        Some(0x00..=0x0F)
    );
    assert!(capabilities.supports_typed(TypedSupportSurface::SharpnessControl));
    assert!(!capabilities.supports_typed(TypedSupportSurface::SharpnessMode));

    let target = grafton_visca::TargetRegistration::new(CameraId::CAMERA_1, profile)
        .expect("target registration");
    // R8 CAM_Aperture Direct: `8x 01 04 42 00 00 00 0p FF`, 0h..Fh.
    for level in 0..=0x0f {
        let command = Sharpness::SetLevel {
            value: SharpnessLevel::new(level).expect("R8 aperture level"),
        };
        command
            .validate_for_target(&target)
            .expect("sourced aperture direct");
        assert_eq!(wire(&command), [0x81, 1, 4, 0x42, 0, 0, 0, level, 0xff]);
    }
    assert!(SharpnessLevel::new(0x10).is_err());
    for (command, operation) in [
        (Sharpness::Reset, 0),
        (Sharpness::Up, 2),
        (Sharpness::Down, 3),
    ] {
        command
            .validate_for_target(&target)
            .expect("sourced aperture adjustment");
        assert_eq!(wire(&command), [0x81, 1, 4, 2, operation, 0xff]);
    }
    SharpnessPositionInquiry
        .validate_for_target(&target)
        .expect("sourced aperture inquiry");
    assert_eq!(wire(&SharpnessPositionInquiry), [0x81, 9, 4, 0x42, 0xff]);

    // R8's aperture level family does not establish `04 05`/`09 04 05` mode.
    for mode in [SharpnessMode::Auto, SharpnessMode::Manual] {
        assert!(matches!(
            Sharpness::Mode(mode).validate_for_target(&target),
            Err(Error::FeatureNotSupported { .. })
        ));
    }
    assert!(matches!(
        SharpnessModeInquiry.validate_for_target(&target),
        Err(Error::FeatureNotSupported { .. })
    ));
}

#[test]
fn r8_picture_effect_is_metadata_only() {
    let profile = profile();
    assert!(profile.capabilities().has_picture_effect());
    assert!(!profile
        .capabilities()
        .supports_typed(TypedSupportSurface::PictureEffect));

    // The typed picture-effect inquiry reads R8's Neg.Art `02` as Off.
    assert!(matches!(
        PictureEffectCommand::new(PictureEffectMode::BlackAndWhite).validate_for_target(
            &grafton_visca::TargetRegistration::new(
                grafton_visca::CameraId::CAMERA_1,
                profile.clone()
            )
            .expect("target registration")
        ),
        Err(Error::FeatureNotSupported { .. })
    ));
    assert!(matches!(
        PictureEffectInquiry.validate_for_target(
            &grafton_visca::TargetRegistration::new(
                grafton_visca::CameraId::CAMERA_1,
                profile.clone()
            )
            .expect("target registration")
        ),
        Err(Error::FeatureNotSupported { .. })
    ));
}
