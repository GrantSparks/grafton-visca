//! Golden full-frame inquiry replies through the public frame parser and typed decoders.

use grafton_visca::{
    command::{exposure, focus, image, pan_tilt, power, tally, white_balance, zoom},
    profiles::{GenericVisca, ProfileSpec, PtzOpticsG2, SonyFr7},
    request::Inquiry,
    types::{AutoWhiteBalanceSensitivity, ImageFlipMode, WhiteBalanceMode},
    wire::{Nibbles, Payload, Response, WireValue},
    Error,
};

fn payload(frame: &[u8]) -> Result<&[u8], Error> {
    // Validate framing before passing the data field to the query's decoder.
    Response::parse(frame)?;
    if frame.get(1) != Some(&0x50) {
        return Err(Error::UnexpectedResponseType);
    }
    frame
        .get(2..frame.len().saturating_sub(1))
        .ok_or(Error::InvalidResponseFormat)
}

fn decode<I: Inquiry>(query: I, frame: &[u8]) -> Result<I::Response, Error> {
    query.decoder().decode(payload(frame)?)
}

#[test]
fn white_balance_mode_inquiry_preserves_existing_values_and_decodes_fr7_atw() {
    for (wire, expected) in [
        (0x00, WhiteBalanceMode::Auto),
        (0x01, WhiteBalanceMode::Indoor),
        (0x02, WhiteBalanceMode::Outdoor),
        (0x03, WhiteBalanceMode::OnePush),
        (0x04, WhiteBalanceMode::Atw),
        (0x05, WhiteBalanceMode::Manual),
        (0x20, WhiteBalanceMode::ColorTemperature),
    ] {
        assert_eq!(
            decode(white_balance::ModeInquiry, &[0x90, 0x50, wire, 0xff]).expect("supported mode"),
            expected
        );
    }
}

#[test]
fn sony_fr7_profile_path_advertises_and_decodes_atw() {
    let fr7 = ProfileSpec::from_compile_time::<SonyFr7>().expect("FR7");
    let generic = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic");
    assert!(fr7
        .capabilities()
        .white_balance
        .as_ref()
        .expect("WB")
        .modes
        .contains(&WhiteBalanceMode::Atw));
    assert!(!generic
        .capabilities()
        .white_balance
        .as_ref()
        .expect("WB")
        .modes
        .contains(&WhiteBalanceMode::Atw));
    assert_eq!(
        white_balance::ModeInquiry
            .decoder_for_profile(&fr7)
            .expect("profile-bound decoder")
            .decode(payload(&[0x90, 0x50, 0x04, 0xff]).expect("frame"))
            .expect("FR7 ATW"),
        WhiteBalanceMode::Atw
    );
}

#[test]
fn test_parse_power_inquiry_responses() {
    assert!(decode(power::StatusInquiry, &[0x90, 0x50, 0x02, 0xff])
        .expect("power on")
        .is_on());
    assert!(!decode(power::StatusInquiry, &[0x90, 0x50, 0x03, 0xff])
        .expect("power off")
        .is_on());
}

#[test]
fn test_parse_pan_tilt_position_inquiry() {
    let profile =
        ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("standard coordinate profile");
    let decoder = pan_tilt::PositionInquiry
        .decoder_for_profile(&profile)
        .expect("profile-bound decoder");
    let standard = [0x90, 0x50, 0, 1, 2, 3, 0, 4, 5, 6, 0xff];
    let position = decoder
        .decode(payload(&standard).expect("frame"))
        .expect("position");
    assert_eq!(position.raw_values(), (0x0123, 0x0456));
    let negative = [
        0x90, 0x50, 0x0f, 0x0f, 0x0f, 0x0f, 0x0f, 0x0f, 0x0e, 0x0d, 0xff,
    ];
    let position = decoder
        .decode(payload(&negative).expect("frame"))
        .expect("negative position");
    assert_eq!(position.raw_values(), (-1, -19));
}

#[test]
fn test_parse_zoom_position_inquiry() {
    assert_eq!(
        decode(zoom::PositionInquiry, &[0x90, 0x50, 1, 2, 3, 4, 0xff])
            .expect("zoom")
            .value(),
        0x1234
    );
    let raw_maximum = [0x90, 0x50, 0x0f, 0x0f, 0x0f, 0x0f, 0xff];
    let raw_payload = payload(&raw_maximum).expect("valid 16-bit reply frame");
    assert_eq!(
        Nibbles::<4>::try_from(Payload::new(raw_payload))
            .expect("four nibbles")
            .u16_quad(0),
        Some(0xffff)
    );
    assert!(
        decode(zoom::PositionInquiry, &raw_maximum).is_err(),
        "raw 16-bit maximum exceeds the checked position domain"
    );
    assert_eq!(
        decode(
            zoom::PositionInquiry,
            &[0x90, 0x50, 0x07, 0x0f, 0x0f, 0x0f, 0xff]
        )
        .expect("maximum checked zoom")
        .value(),
        0x7fff
    );
}

#[test]
fn test_parse_focus_position_inquiry() {
    assert_eq!(
        decode(focus::PositionInquiry, &[0x90, 0x50, 5, 6, 7, 8, 0xff])
            .expect("focus")
            .value(),
        0x5678
    );
}

#[test]
fn test_parse_backlight_inquiry() {
    assert!(decode(
        exposure::BacklightCompensationInquiry,
        &[0x90, 0x50, 2, 0xff]
    )
    .expect("backlight on"));
}

#[test]
fn test_parse_image_flip_inquiry() {
    let both = decode(image::FlipModeInquiry, &[0x90, 0x50, 3, 0xff]).expect("both flips");
    assert!(matches!(
        both,
        ImageFlipMode::Vertical | ImageFlipMode::Both
    ));
    assert!(matches!(
        both,
        ImageFlipMode::Horizontal | ImageFlipMode::Both
    ));
    let vertical = decode(image::FlipModeInquiry, &[0x90, 0x50, 2, 0xff]).expect("vertical only");
    assert!(matches!(
        vertical,
        ImageFlipMode::Vertical | ImageFlipMode::Both
    ));
    assert!(matches!(
        vertical,
        ImageFlipMode::Off | ImageFlipMode::Vertical
    ));
}

#[test]
fn test_parse_brightness_inquiry() {
    assert_eq!(
        decode(
            exposure::BrightnessInquiry,
            &[0x90, 0x50, 0x0, 0x0, 0x0, 0x8, 0xff]
        )
        .expect("brightness response")
        .value(),
        8
    );
}

#[test]
fn test_parse_gain_level_inquiry() {
    assert_eq!(
        decode(
            exposure::GainInquiry,
            &[0x90, 0x50, 0x0, 0x0, 0x0, 0x5, 0xff]
        )
        .expect("gain_level response")
        .value(),
        5
    );
}

#[test]
fn test_parse_contrast_inquiry() {
    assert_eq!(
        decode(
            image::ContrastInquiry,
            &[0x90, 0x50, 0x0, 0x0, 0x0, 0xc, 0xff]
        )
        .expect("contrast response")
        .value(),
        12
    );
}

#[test]
fn test_parse_luminance_inquiry() {
    assert_eq!(
        decode(
            image::LuminanceInquiry,
            &[0x90, 0x50, 0x0, 0x0, 0x0, 0x7, 0xff]
        )
        .expect("luminance response")
        .value(),
        7
    );
}

#[test]
fn test_parse_sharpness_position_inquiry() {
    assert_eq!(
        decode(
            image::SharpnessInquiry,
            &[0x90, 0x50, 0x0, 0x0, 0x0, 0x8, 0xff]
        )
        .expect("sharpness_position response")
        .value(),
        8
    );
}

#[test]
fn test_parse_iris_inquiry() {
    assert_eq!(
        decode(
            exposure::IrisInquiry,
            &[0x90, 0x50, 0x0, 0x0, 0x0, 0xa, 0xff]
        )
        .expect("iris response")
        .value(),
        10
    );
}

#[test]
fn test_parse_iris_inquiry_preserves_both_position_nibbles() {
    assert_eq!(
        decode(
            exposure::IrisInquiry,
            &[0x90, 0x50, 0x0, 0x0, 0x1, 0xe, 0xff]
        )
        .expect("iris_inquiry_preserves_both_position_nibbles response")
        .value(),
        30
    );
}

#[test]
fn test_parse_shutter_inquiry() {
    assert_eq!(
        decode(
            exposure::ShutterInquiry,
            &[0x90, 0x50, 0x0, 0x0, 0x0, 0x7, 0xff]
        )
        .expect("shutter response")
        .value(),
        7
    );
}

#[test]
fn test_parse_noise_reduction_2d_inquiry() {
    assert_eq!(
        decode(image::NoiseReduction2dInquiry, &[0x90, 0x50, 0x3, 0xff])
            .expect("noise_reduction_2d response")
            .to_wire(),
        3
    );
}

#[test]
fn test_parse_noise_reduction_3d_inquiry() {
    assert_eq!(
        decode(image::NoiseReduction3dInquiry, &[0x90, 0x50, 0x5, 0xff])
            .expect("noise_reduction_3d response")
            .to_wire(),
        5
    );
}

#[test]
fn test_parse_saturation_inquiry() {
    assert_eq!(
        decode(
            image::SaturationInquiry,
            &[0x90, 0x50, 0x0, 0x0, 0x0, 0xa, 0xff]
        )
        .expect("saturation response")
        .value(),
        10
    );
}

#[test]
fn test_parse_hue_inquiry() {
    assert_eq!(
        decode(image::HueInquiry, &[0x90, 0x50, 0x0, 0x0, 0x0, 0x7, 0xff])
            .expect("hue response")
            .value(),
        7
    );
}

#[test]
fn test_parse_noise_reduction_3d_inquiry_full_public_domain() {
    for wire in 0..=8 {
        assert_eq!(
            decode(image::NoiseReduction3dInquiry, &[0x90, 0x50, wire, 0xff])
                .expect("public level domain")
                .to_wire(),
            wire
        );
    }
}

#[test]
fn test_parse_auto_white_balance_sensitivity_inquiry() {
    for (frame, expected) in [
        ([0x90, 0x50, 0x00, 0xff], AutoWhiteBalanceSensitivity::High),
        (
            [0x90, 0x50, 0x01, 0xff],
            AutoWhiteBalanceSensitivity::Normal,
        ),
        ([0x90, 0x50, 0x02, 0xff], AutoWhiteBalanceSensitivity::Low),
    ] {
        assert_eq!(
            decode(white_balance::SensitivityInquiry, &frame).expect("AWB sensitivity"),
            expected
        );
    }
    assert!(decode(white_balance::SensitivityInquiry, &[0x90, 0x50, 0x03, 0xff]).is_err());
}

#[test]
fn test_parse_tally_red_inquiry() {
    assert!(decode(tally::RedInquiry, &[0x90, 0x50, 2, 0xff]).expect("red on"));
    assert!(!decode(tally::RedInquiry, &[0x90, 0x50, 3, 0xff]).expect("red off"));
}

#[test]
fn test_parse_tally_green_inquiry() {
    assert!(decode(tally::GreenInquiry, &[0x90, 0x50, 2, 0xff]).expect("green on"));
    assert!(!decode(tally::GreenInquiry, &[0x90, 0x50, 3, 0xff]).expect("green off"));
}

#[test]
fn test_error_handling() {
    assert!(
        decode(power::StatusInquiry, &[0x90, 0x50, 2]).is_err(),
        "missing terminator"
    );
    let profile = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("profile");
    let short = [0x90, 0x50, 0, 1, 0xff];
    assert!(
        pan_tilt::PositionInquiry
            .decoder_for_profile(&profile)
            .expect("profile-bound decoder")
            .decode(payload(&short).expect("well-formed frame"))
            .is_err(),
        "insufficient coordinate data"
    );
    assert!(decode(power::StatusInquiry, &[]).is_err(), "empty frame");
}
