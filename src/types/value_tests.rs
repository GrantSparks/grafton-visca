#![allow(clippy::expect_used)]

use super::*;
use crate::wire::WireValue;

#[test]
fn union_domains_preserve_codes_while_constructors_are_checked() {
    for code in u8::MIN..=u8::MAX {
        let shutter = ShutterSpeed::new(code).expect("every shutter byte is syntactically valid");
        let bright = BrightnessLevel::new(code).expect("every bright byte is syntactically valid");
        assert_eq!(shutter.value(), code);
        assert_eq!(bright.value(), code);
        assert_eq!(
            ShutterSpeed::from_wire(code).expect("shutter code"),
            shutter
        );
        assert_eq!(
            BrightnessLevel::from_wire(code).expect("bright code"),
            bright
        );
    }
    assert_eq!(FocusPosition::MIN.value(), 0);
    assert_eq!(FocusPosition::MAX.value(), u16::MAX);
    assert_eq!(
        FocusPosition::new(u16::MAX).expect("focus field").to_wire(),
        u16::MAX
    );
    assert!(BrightnessLevel::try_from(256_u16).is_err());
    assert_eq!(
        BrightnessLevel::try_from(255_u16).expect("one byte"),
        BrightnessLevel::MAX
    );
}

#[test]
fn centered_values_round_trip_every_valid_code_and_reject_other_codes() {
    for code in u8::MIN..=u8::MAX {
        if code <= 14 {
            let value = ExposureCompensationLevel::from_wire(code).expect("compensation code");
            assert_eq!(value.value(), i8::try_from(code).expect("small code") - 7);
            assert_eq!(value.to_wire(), code);
        } else {
            assert!(ExposureCompensationLevel::from_wire(code).is_err());
        }
        if code <= 20 {
            let value = RgbTuning::from_wire(code).expect("tuning code");
            assert_eq!(value.value(), i8::try_from(code).expect("small code") - 10);
            assert_eq!(value.to_wire(), code);
        } else {
            assert!(RgbTuning::from_wire(code).is_err());
        }
    }
    assert_eq!(RgbTuning::NEUTRAL.to_wire(), 10);
    assert!(RgbTuning::new(-11).is_err());
    assert!(RgbTuning::new(11).is_err());
}

#[test]
fn exact_speed_inputs_and_levels_remain_distinct() {
    let exact = PanTiltSpeed::new(
        PanSpeed::new(50).expect("union pan speed"),
        TiltSpeed::new(25).expect("union tilt speed"),
    );
    let input: Speed<PanTiltSpeed> = exact.into();
    assert_eq!(input, Speed::Exact(exact));
    let coarse: Speed<PanTiltSpeed> = SpeedLevel::Fast.into();
    assert_eq!(coarse, Speed::Level(SpeedLevel::Fast));
    #[cfg(any(feature = "blocking", feature = "async"))]
    assert_eq!(SpeedLevel::Fast.baseline_speeds(), (18, 15, 6, 6));
    let zoom = ZoomSpeed::new(7).expect("zoom speed");
    let focus = FocusSpeed::new(0).expect("focus speed");
    assert_eq!(Speed::<ZoomSpeed>::from(zoom), Speed::Exact(zoom));
    assert_eq!(Speed::<FocusSpeed>::from(focus), Speed::Exact(focus));
}

#[test]
fn enums_preserve_their_documented_setting_bytes() {
    assert_eq!(NdiQuality::High.to_wire(), 1);
    assert_eq!(NdiQuality::Off.to_wire(), 4);
    assert_eq!(TallyMode::Flash.to_wire(), 1);
    assert_eq!(TallyMode::On.to_wire(), 2);
    assert_eq!(TallyMode::Off.to_wire(), 3);
    assert_eq!(TallyBrightness::Low.to_wire(), 4);
    assert_eq!(TallyBrightness::High.to_wire(), 5);
    assert!(TallyMode::from_wire(0).is_err());
    assert_eq!(FStop::F5_6.to_string(), "F5.6");
    assert_eq!(FStop::Closed.to_string(), "Closed");
}

#[cfg(feature = "serde")]
#[test]
fn merged_values_and_speed_inputs_round_trip_with_validated_domains() {
    let gain = RgbGain::new(255).expect("gain");
    assert_eq!(serde_json::to_string(&gain).expect("serialize gain"), "255");
    assert_eq!(
        serde_json::from_str::<RgbGain>("255").expect("deserialize gain"),
        gain
    );
    assert!(serde_json::from_str::<RgbGain>("256").is_err());
    assert!(serde_json::from_str::<RgbTuning>("11").is_err());
    assert!(serde_json::from_str::<GainLevel>("26").is_err());
    let exact = Speed::Exact(ZoomSpeed::new(7).expect("zoom speed"));
    let coarse: Speed<ZoomSpeed> = SpeedLevel::Medium.into();
    for input in [exact, coarse] {
        let json = serde_json::to_string(&input).expect("serialize speed");
        assert_eq!(
            serde_json::from_str::<Speed<ZoomSpeed>>(&json).expect("deserialize speed"),
            input
        );
    }
    assert_eq!(
        serde_json::to_string(&TallyMode::Flash).expect("serialize mode"),
        "\"flash\""
    );
}
