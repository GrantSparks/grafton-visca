//! Sony EVI-H100 (R8) image controls: what is granted matches R8 byte for
//! byte, and what R8 documents with a different layout is withheld.

#![allow(clippy::expect_used)]

use grafton_visca::wire::WireValue;
use grafton_visca::{
    capabilities::TypedSupportSurface,
    profiles::{ProfileSpec, SonyEviH100},
    request::Request,
    types::{
        CameraId, HueLevel, NoiseReduction2D, NoiseReduction2DMode, PictureEffectMode,
        SaturationLevel, SharpnessLevel, SharpnessMode,
    },
    Error,
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
    ProfileSpec::from_compile_time::<SonyEviH100>().expect("EVI-H100 profile")
}

#[test]
fn r8_noise_reduction_is_the_2d_level_family() {
    let profile = profile();
    // `CAM_NR` `8x 01 04 53 0p FF`, p = 0 (off) and 1..5.
    for level in 0..=5 {
        let command = grafton_visca::command::image::SetNoiseReduction2d::new(
            NoiseReduction2D::from_wire(level).expect("level or Off"),
        );
        command
            .validate_for_target(
                &grafton_visca::config::TargetRegistration::new(
                    grafton_visca::types::CameraId::CAMERA_1,
                    profile.clone(),
                )
                .expect("target registration"),
            )
            .unwrap_or_else(|error| panic!("NR level {level}: {error}"));
        assert_eq!(wire(&command), [0x81, 0x01, 0x04, 0x53, level, 0xFF]);
    }
    // `CAM_NRInq` `8x 09 04 53 FF`.
    grafton_visca::command::image::NoiseReduction2dInquiry
        .validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                profile.clone(),
            )
            .expect("target registration"),
        )
        .expect("NR level inquiry");
    assert_eq!(
        wire(&grafton_visca::command::image::NoiseReduction2dInquiry),
        [0x81, 0x09, 0x04, 0x53, 0xFF]
    );

    // R8 lists no `04 50` auto/manual mode.
    assert!(matches!(
        grafton_visca::command::image::SetNoiseReduction2dMode::new(NoiseReduction2DMode::Auto)
            .validate_for_target(
                &grafton_visca::config::TargetRegistration::new(
                    grafton_visca::types::CameraId::CAMERA_1,
                    profile.clone()
                )
                .expect("target registration")
            ),
        Err(Error::FeatureNotSupported { .. })
    ));
    assert!(matches!(
        grafton_visca::command::image::NoiseReduction2dModeInquiry.validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
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
    let saturation = grafton_visca::command::image::SetSaturation::new(
        SaturationLevel::new(0x0E).expect("level"),
    );
    saturation
        .validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                profile.clone(),
            )
            .expect("target registration"),
        )
        .expect("color gain");
    assert_eq!(
        wire(&saturation),
        [0x81, 0x01, 0x04, 0x49, 0x00, 0x00, 0x00, 0x0E, 0xFF]
    );
    grafton_visca::command::image::SaturationInquiry
        .validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                profile.clone(),
            )
            .expect("target registration"),
        )
        .expect("color gain inquiry");
    assert_eq!(
        wire(&grafton_visca::command::image::SaturationInquiry),
        [0x81, 0x09, 0x04, 0x49, 0xFF]
    );

    // `CAM_ColorHue` Direct `8x 01 04 4F 00 00 00 0p FF`, 0h..Eh.
    let hue = grafton_visca::command::image::SetHue::new(HueLevel::new(0x0E).expect("level"));
    hue.validate_for_target(
        &grafton_visca::config::TargetRegistration::new(
            grafton_visca::types::CameraId::CAMERA_1,
            profile.clone(),
        )
        .expect("target registration"),
    )
    .expect("color hue");
    assert_eq!(
        wire(&hue),
        [0x81, 0x01, 0x04, 0x4F, 0x00, 0x00, 0x00, 0x0E, 0xFF]
    );
    grafton_visca::command::image::HueInquiry
        .validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                profile.clone(),
            )
            .expect("target registration"),
        )
        .expect("color hue inquiry");
    assert_eq!(
        wire(&grafton_visca::command::image::HueInquiry),
        [0x81, 0x09, 0x04, 0x4F, 0xFF]
    );
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
    assert!(capabilities.supports(TypedSupportSurface::Sharpness));
    assert!(!capabilities.supports(TypedSupportSurface::SharpnessMode));

    let target = grafton_visca::config::TargetRegistration::new(CameraId::CAMERA_1, profile)
        .expect("target registration");
    // R8 CAM_Aperture Direct: `8x 01 04 42 00 00 00 0p FF`, 0h..Fh.
    for level in 0..=0x0f {
        let command = grafton_visca::command::image::SetSharpness::new(
            SharpnessLevel::new(level).expect("R8 aperture level"),
        );
        command
            .validate_for_target(&target)
            .expect("sourced aperture direct");
        assert_eq!(wire(&command), [0x81, 1, 4, 0x42, 0, 0, 0, level, 0xff]);
    }
    assert!(SharpnessLevel::new(0x10).is_err());
    for (validation, bytes, operation) in [
        (
            grafton_visca::command::image::SharpnessReset.validate_for_target(&target),
            wire(&grafton_visca::command::image::SharpnessReset),
            0,
        ),
        (
            grafton_visca::command::image::SharpnessUp.validate_for_target(&target),
            wire(&grafton_visca::command::image::SharpnessUp),
            2,
        ),
        (
            grafton_visca::command::image::SharpnessDown.validate_for_target(&target),
            wire(&grafton_visca::command::image::SharpnessDown),
            3,
        ),
    ] {
        validation.expect("sourced aperture adjustment");
        assert_eq!(bytes, [0x81, 1, 4, 2, operation, 0xff]);
    }
    grafton_visca::command::image::SharpnessInquiry
        .validate_for_target(&target)
        .expect("sourced aperture inquiry");
    assert_eq!(
        wire(&grafton_visca::command::image::SharpnessInquiry),
        [0x81, 9, 4, 0x42, 0xff]
    );

    // R8's aperture level family does not establish `04 05`/`09 04 05` mode.
    for mode in [SharpnessMode::Auto, SharpnessMode::Manual] {
        assert!(matches!(
            grafton_visca::command::image::SetSharpnessMode::new(mode).validate_for_target(&target),
            Err(Error::FeatureNotSupported { .. })
        ));
    }
    assert!(matches!(
        grafton_visca::command::image::SharpnessModeInquiry.validate_for_target(&target),
        Err(Error::FeatureNotSupported { .. })
    ));
}

#[test]
fn r8_picture_effect_is_metadata_only() {
    let profile = profile();
    assert!(profile
        .capabilities()
        .image
        .as_ref()
        .is_some_and(|image| image.picture_effect));
    assert!(!profile
        .capabilities()
        .supports(TypedSupportSurface::PictureEffect));

    // The typed picture-effect inquiry reads R8's Neg.Art `02` as Off.
    assert!(matches!(
        grafton_visca::command::image::SetPictureEffect::new(PictureEffectMode::BlackAndWhite)
            .validate_for_target(
                &grafton_visca::config::TargetRegistration::new(
                    grafton_visca::types::CameraId::CAMERA_1,
                    profile.clone()
                )
                .expect("target registration")
            ),
        Err(Error::FeatureNotSupported { .. })
    ));
    assert!(matches!(
        grafton_visca::command::image::PictureEffectInquiry.validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                profile.clone()
            )
            .expect("target registration")
        ),
        Err(Error::FeatureNotSupported { .. })
    ));
}
