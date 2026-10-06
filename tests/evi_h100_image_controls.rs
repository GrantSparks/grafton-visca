//! Sony EVI-H100 (R8) image controls: what is granted matches R8 byte for
//! byte, and what R8 documents with a different layout is withheld.

#![allow(clippy::expect_used)]

use grafton_visca::{
    capabilities::TypedSupportSurface,
    command::{
        HueCommand, HueInquiry, NoiseReduction2D, NoiseReduction2DInquiry, NoiseReduction2DMode,
        NoiseReduction2DModeCommand, NoiseReduction2DModeInquiry, PictureEffectCommand,
        PictureEffectInquiry, PictureEffectMode, SaturationCommand, SaturationInquiry,
        SharpnessModeInquiry,
    },
    profiles::SonyEVIH100,
    types::{HueLevel, NoiseReduction2DLevel, SaturationLevel},
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
        let command =
            NoiseReduction2D::with_level(NoiseReduction2DLevel::new(level).expect("level"));
        command
            .validate_for_profile(&profile)
            .unwrap_or_else(|error| panic!("NR level {level}: {error}"));
        assert_eq!(wire(&command), [0x81, 0x01, 0x04, 0x53, level, 0xFF]);
    }
    // `CAM_NRInq` `8x 09 04 53 FF`.
    NoiseReduction2DInquiry
        .validate_for_profile(&profile)
        .expect("NR level inquiry");
    assert_eq!(
        wire(&NoiseReduction2DInquiry),
        [0x81, 0x09, 0x04, 0x53, 0xFF]
    );

    // R8 lists no `04 50` auto/manual mode.
    assert!(matches!(
        NoiseReduction2DModeCommand::new(NoiseReduction2DMode::Auto).validate_for_profile(&profile),
        Err(Error::FeatureNotSupported { .. })
    ));
    assert!(matches!(
        NoiseReduction2DModeInquiry.validate_for_profile(&profile),
        Err(Error::FeatureNotSupported { .. })
    ));
}

#[test]
fn r8_color_gain_and_hue_are_the_saturation_and_hue_controls() {
    let profile = profile();
    // `CAM_ColorGain` Direct `8x 01 04 49 00 00 00 0p FF`, 0h..Eh.
    let saturation = SaturationCommand::new(SaturationLevel::new(0x0E).expect("level"));
    saturation
        .validate_for_profile(&profile)
        .expect("color gain");
    assert_eq!(
        wire(&saturation),
        [0x81, 0x01, 0x04, 0x49, 0x00, 0x00, 0x00, 0x0E, 0xFF]
    );
    SaturationInquiry
        .validate_for_profile(&profile)
        .expect("color gain inquiry");
    assert_eq!(wire(&SaturationInquiry), [0x81, 0x09, 0x04, 0x49, 0xFF]);

    // `CAM_ColorHue` Direct `8x 01 04 4F 00 00 00 0p FF`, 0h..Eh.
    let hue = HueCommand::new(HueLevel::new(0x0E).expect("level"));
    hue.validate_for_profile(&profile).expect("color hue");
    assert_eq!(
        wire(&hue),
        [0x81, 0x01, 0x04, 0x4F, 0x00, 0x00, 0x00, 0x0E, 0xFF]
    );
    HueInquiry
        .validate_for_profile(&profile)
        .expect("color hue inquiry");
    assert_eq!(wire(&HueInquiry), [0x81, 0x09, 0x04, 0x4F, 0xFF]);
}

#[test]
fn r8_aperture_and_picture_effect_are_metadata_only() {
    let profile = profile();
    let capabilities = profile.capabilities();
    assert_eq!(capabilities.sharpness_range, Some(0x00..=0x0F));
    assert!(capabilities.has_picture_effect);
    assert!(!capabilities.supports_typed(TypedSupportSurface::SharpnessControl));
    assert!(!capabilities.supports_typed(TypedSupportSurface::PictureEffect));

    // The typed sharpness surface also sends the `04 05` mode R8 lacks, and
    // the typed picture-effect inquiry reads R8's Neg.Art `02` as Off.
    assert!(matches!(
        SharpnessModeInquiry.validate_for_profile(&profile),
        Err(Error::FeatureNotSupported { .. })
    ));
    assert!(matches!(
        PictureEffectCommand::new(PictureEffectMode::BlackAndWhite).validate_for_profile(&profile),
        Err(Error::FeatureNotSupported { .. })
    ));
    assert!(matches!(
        PictureEffectInquiry.validate_for_profile(&profile),
        Err(Error::FeatureNotSupported { .. })
    ));
}
