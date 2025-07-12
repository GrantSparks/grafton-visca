//! Comprehensive encoding tests moved from src/command/tests/encoding_tests.rs
//!
//! These tests verify VISCA command encoding using the enhanced testing infrastructure
//! with golden vector validation and pattern matching.

mod common;

use crate::common::{
    patterns::{inquiry, responses},
    MockResponse, MockTransportBuilder, ProtocolValidator, ValidationMode,
};
use grafton_visca::{
    command::*,
    transport::BlockingTransport,
    types::{DynamicRangeLevel, IrisLevel, PanSpeed, TiltSpeed, ZoomPosition},
    EncodeVisca,
};

#[test]
fn test_power_commands_encoding() {
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);

    // Power On
    let power_on = PowerCommand { power: Power::On };
    let expected = &[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF];
    assert_eq!(
        power_on.try_into_vec().unwrap(),
        expected,
        "Power On command should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Power Standby
    let power_standby = PowerCommand {
        power: Power::Standby,
    };
    let expected = &[0x81, 0x01, 0x04, 0x00, 0x03, 0xFF];
    assert_eq!(
        power_standby.try_into_vec().unwrap(),
        expected,
        "Power Standby command should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();
}

#[test]
fn test_pan_tilt_home_encoding() {
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);

    let home_cmd = PanTilt::Home;
    let expected = &[0x81, 0x01, 0x06, 0x04, 0xFF];
    assert_eq!(
        home_cmd.try_into_vec().unwrap(),
        expected,
        "Pan/Tilt Home command should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();
}

#[test]
fn test_pan_tilt_move_directions_encoding() {
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);

    // Test Up direction
    let move_up = PanTilt::Move {
        direction: PanTiltDirection::Up,
        pan_speed: PanSpeed::new(0x0F).unwrap(),
        tilt_speed: TiltSpeed::new(0x0F).unwrap(),
    };
    let expected = &[0x81, 0x01, 0x06, 0x01, 0x0F, 0x0F, 0x03, 0x01, 0xFF];
    assert_eq!(
        move_up.try_into_vec().unwrap(),
        expected,
        "Pan/Tilt Move Up should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Test DownRight direction
    let move_down_right = PanTilt::Move {
        direction: PanTiltDirection::DownRight,
        pan_speed: PanSpeed::new(0x0F).unwrap(), // max nibble speed
        tilt_speed: TiltSpeed::new(0x0F).unwrap(), // max nibble speed
    };
    let expected = &[0x81, 0x01, 0x06, 0x01, 0x0F, 0x0F, 0x02, 0x02, 0xFF];
    assert_eq!(
        move_down_right.try_into_vec().unwrap(),
        expected,
        "Pan/Tilt Move DownRight with max speeds should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Test Stop
    let stop = PanTilt::Move {
        direction: PanTiltDirection::Stop,
        pan_speed: PanSpeed::new(0).unwrap(),
        tilt_speed: TiltSpeed::new(0).unwrap(),
    };
    let expected = &[0x81, 0x01, 0x06, 0x01, 0x00, 0x00, 0x03, 0x03, 0xFF];
    assert_eq!(
        stop.try_into_vec().unwrap(),
        expected,
        "Pan/Tilt Stop should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();
}

#[test]
fn test_pan_tilt_speed_limits() {
    // Test pan speed out of range
    assert!(
        PanSpeed::new(0x19).is_err(),
        "Pan speed above 0x18 should return error"
    );

    // Test tilt speed out of range
    assert!(
        TiltSpeed::new(0x15).is_err(),
        "Tilt speed above 0x14 should return error"
    );
}

#[test]
fn test_zoom_commands_encoding() {
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);

    // Zoom Stop
    let zoom_stop = Zoom::Stop;
    let expected = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xFF];
    assert_eq!(
        zoom_stop.try_into_vec().unwrap(),
        expected,
        "Zoom Stop should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Zoom In
    let zoom_in = Zoom::TeleStd;
    let expected = &[0x81, 0x01, 0x04, 0x07, 0x02, 0xFF];
    assert_eq!(
        zoom_in.try_into_vec().unwrap(),
        expected,
        "Zoom In should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Zoom Out
    let zoom_out = Zoom::WideStd;
    let expected = &[0x81, 0x01, 0x04, 0x07, 0x03, 0xFF];
    assert_eq!(
        zoom_out.try_into_vec().unwrap(),
        expected,
        "Zoom Out should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Zoom In WithSpeed with speed
    let zoom_in_var = Zoom::TeleVariable(ZoomSpeed::new(5).unwrap());
    let expected = &[0x81, 0x01, 0x04, 0x07, 0x25, 0xFF]; // 0x20 | 5 = 0x25
    assert_eq!(
        zoom_in_var.try_into_vec().unwrap(),
        expected,
        "Zoom In WithSpeed with speed 5 should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Zoom Out WithSpeed with max speed
    let zoom_out_var = Zoom::WideVariable(ZoomSpeed::new(7).unwrap());
    let expected = &[0x81, 0x01, 0x04, 0x07, 0x37, 0xFF]; // 0x30 | 7 = 0x37
    assert_eq!(
        zoom_out_var.try_into_vec().unwrap(),
        expected,
        "Zoom Out WithSpeed with max speed should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();
}

#[test]
fn test_zoom_speed_validation() {
    // Test zoom speed out of range
    let zoom_result = ZoomSpeed::new(8);
    assert!(
        zoom_result.is_err(),
        "Zoom speed above 7 should return error"
    );
}

#[test]
fn test_zoom_position_encoding() {
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);

    // Test a specific zoom position
    let zoom_position = Zoom::Position(ZoomPosition::new(0x1234).unwrap());
    let expected = &[0x81, 0x01, 0x04, 0x47, 0x01, 0x02, 0x03, 0x04, 0xFF];
    assert_eq!(
        zoom_position.try_into_vec().unwrap(),
        expected,
        "Zoom Position position 0x1234 should be split into nibbles correctly"
    );
    validator.validate_command(expected).unwrap();

    // Test maximum zoom position (0x7000 for 20X optical)
    let zoom_max = Zoom::Position(ZoomPosition::new(0x7000).unwrap());
    let expected = &[0x81, 0x01, 0x04, 0x47, 0x07, 0x00, 0x00, 0x00, 0xFF];
    assert_eq!(
        zoom_max.try_into_vec().unwrap(),
        expected,
        "Zoom Position max position should be split into nibbles correctly"
    );
    validator.validate_command(expected).unwrap();
}

#[test]
fn test_preset_commands_encoding() {
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);

    // Preset Reset
    let preset_reset = PresetCommand {
        action: PresetAction::Reset,
        preset_number: PresetNumber::new(0).unwrap(),
    };
    let expected = &[0x81, 0x01, 0x04, 0x3F, 0x00, 0x00, 0xFF];
    assert_eq!(
        preset_reset.try_into_vec().unwrap(),
        expected,
        "Preset Reset should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Preset Set
    let preset_set = PresetCommand {
        action: PresetAction::Set,
        preset_number: PresetNumber::new(5).unwrap(),
    };
    let expected = &[0x81, 0x01, 0x04, 0x3F, 0x01, 0x05, 0xFF];
    assert_eq!(
        preset_set.try_into_vec().unwrap(),
        expected,
        "Preset Set position 5 should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Preset Recall
    let preset_recall = PresetCommand {
        action: PresetAction::Recall,
        preset_number: PresetNumber::new(15).unwrap(), // valid preset number
    };
    let expected = &[0x81, 0x01, 0x04, 0x3F, 0x02, 0x0F, 0xFF];
    assert_eq!(
        preset_recall.try_into_vec().unwrap(),
        expected,
        "Preset Recall position 15 should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();
}

#[test]
fn test_preset_number_validation() {
    // Test preset number out of range
    let preset_result = PresetNumber::new(90);
    assert!(
        preset_result.is_err(),
        "Preset number 90 should return error (max is 89)"
    );
}

#[test]
fn test_focus_commands_encoding() {
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);

    // Focus Stop
    let focus_stop = Focus::Stop;
    let expected = &[0x81, 0x01, 0x04, 0x08, 0x00, 0xFF];
    assert_eq!(
        focus_stop.try_into_vec().unwrap(),
        expected,
        "Focus Stop should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Focus Far
    let focus_far = Focus::Far;
    let expected = &[0x81, 0x01, 0x04, 0x08, 0x02, 0xFF];
    assert_eq!(
        focus_far.try_into_vec().unwrap(),
        expected,
        "Focus Far should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Focus Near WithSpeed
    let focus_speed = FocusSpeed::new(3).unwrap();
    let focus_near_var = Focus::NearWithSpeed(focus_speed);
    let expected = &[0x81, 0x01, 0x04, 0x08, 0x33, 0xFF]; // 0x30 | 3 = 0x33
    assert_eq!(
        focus_near_var.try_into_vec().unwrap(),
        expected,
        "Focus Near WithSpeed with speed 3 should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Focus Auto
    let focus_auto = Focus::Auto;
    let expected = &[0x81, 0x01, 0x04, 0x38, 0x02, 0xFF];
    assert_eq!(
        focus_auto.try_into_vec().unwrap(),
        expected,
        "Focus Auto should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Focus One Push Trigger
    let focus_one_push = Focus::OnePushTrigger;
    let expected = &[0x81, 0x01, 0x04, 0x18, 0x01, 0xFF];
    assert_eq!(
        focus_one_push.try_into_vec().unwrap(),
        expected,
        "Focus One Push Trigger should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();
}

#[test]
fn test_focus_speed_validation() {
    // Test focus speed out of range
    assert!(
        FocusSpeed::new(8).is_err(),
        "Focus speed above 7 should return error"
    );
}

#[test]
fn test_exposure_mode_commands_encoding() {
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);

    // Auto Exposure
    let exposure_auto = ExposureCommand {
        mode: ExposureMode::Auto,
    };
    let expected = &[0x81, 0x01, 0x04, 0x39, 0x00, 0xFF];
    assert_eq!(
        exposure_auto.try_into_vec().unwrap(),
        expected,
        "Auto Exposure mode should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Manual Exposure
    let exposure_manual = ExposureCommand {
        mode: ExposureMode::Manual,
    };
    let expected = &[0x81, 0x01, 0x04, 0x39, 0x03, 0xFF];
    assert_eq!(
        exposure_manual.try_into_vec().unwrap(),
        expected,
        "Manual Exposure mode should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Shutter Priority
    let exposure_shutter = ExposureCommand {
        mode: ExposureMode::Shutter,
    };
    let expected = &[0x81, 0x01, 0x04, 0x39, 0x0A, 0xFF];
    assert_eq!(
        exposure_shutter.try_into_vec().unwrap(),
        expected,
        "Shutter Priority mode should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();
}

#[test]
fn test_inquiry_commands_encoding() {
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);

    // Pan/Tilt Position Inquiry
    let pt_inquiry = PanTiltPositionInquiry;
    assert_eq!(
        pt_inquiry.try_into_vec().unwrap(),
        inquiry::PAN_TILT_POSITION,
        "Pan/Tilt Position inquiry should produce correct byte sequence"
    );
    validator
        .validate_command(&pt_inquiry.try_into_vec().unwrap())
        .unwrap();

    // Zoom Position Inquiry
    let zoom_inquiry = ZoomPositionInquiry;
    assert_eq!(
        zoom_inquiry.try_into_vec().unwrap(),
        inquiry::ZOOM_POSITION,
        "Zoom Position inquiry should produce correct byte sequence"
    );
    validator
        .validate_command(&zoom_inquiry.try_into_vec().unwrap())
        .unwrap();

    // Focus Position Inquiry
    let focus_inquiry = FocusPositionInquiry;
    assert_eq!(
        focus_inquiry.try_into_vec().unwrap(),
        inquiry::FOCUS_POSITION,
        "Focus Position inquiry should produce correct byte sequence"
    );
    validator
        .validate_command(&focus_inquiry.try_into_vec().unwrap())
        .unwrap();

    // Exposure Mode Inquiry
    let exposure_inquiry = ExposureModeInquiry;
    assert_eq!(
        exposure_inquiry.try_into_vec().unwrap(),
        inquiry::EXPOSURE_MODE,
        "Exposure Mode inquiry should produce correct byte sequence"
    );
    validator
        .validate_command(&exposure_inquiry.try_into_vec().unwrap())
        .unwrap();
}

#[test]
fn test_exposure_compensation_commands_encoding() {
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);

    // Exposure Compensation On
    let exp_comp_on = ExposureCompensation::On;
    let expected = &[0x81, 0x01, 0x04, 0x3E, 0x02, 0xFF];
    assert_eq!(
        exp_comp_on.try_into_vec().unwrap(),
        expected,
        "Exposure Compensation On should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Exposure Compensation Off
    let exp_comp_off = ExposureCompensation::Off;
    let expected = &[0x81, 0x01, 0x04, 0x3E, 0x03, 0xFF];
    assert_eq!(
        exp_comp_off.try_into_vec().unwrap(),
        expected,
        "Exposure Compensation Off should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Exposure Compensation Reset
    let exp_comp_reset = ExposureCompensation::Reset;
    let expected = &[0x81, 0x01, 0x04, 0x0E, 0x00, 0xFF];
    assert_eq!(
        exp_comp_reset.try_into_vec().unwrap(),
        expected,
        "Exposure Compensation Reset should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Exposure Compensation Direct -7
    let exp_comp_neg7 =
        ExposureCompensation::SetLevel(ExposureCompensationLevel::new(-7).unwrap());
    let expected = &[0x81, 0x01, 0x04, 0x4E, 0x00, 0x00, 0x00, 0x00, 0xFF];
    assert_eq!(
        exp_comp_neg7.try_into_vec().unwrap(),
        expected,
        "Exposure Compensation Direct -7 should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Exposure Compensation Direct +7
    let exp_comp_pos7 =
        ExposureCompensation::SetLevel(ExposureCompensationLevel::new(7).unwrap());
    let expected = &[0x81, 0x01, 0x04, 0x4E, 0x00, 0x00, 0x00, 0x0E, 0xFF];
    assert_eq!(
        exp_comp_pos7.try_into_vec().unwrap(),
        expected,
        "Exposure Compensation Direct +7 should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();
}

#[test]
fn test_dynamic_range_command_encoding() {
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);

    // Dynamic Range level 0
    let dr_0 = DynamicRange::SetLevel(DynamicRangeLevel::new(0).unwrap());
    let expected = &[0x81, 0x01, 0x04, 0x25, 0x00, 0x00, 0x00, 0x00, 0xFF];
    assert_eq!(
        dr_0.try_into_vec().unwrap(),
        expected,
        "Dynamic Range level 0 should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Dynamic Range level 8
    let dr_8 = DynamicRange::SetLevel(DynamicRangeLevel::new(8).unwrap());
    let expected = &[0x81, 0x01, 0x04, 0x25, 0x00, 0x00, 0x00, 0x08, 0xFF];
    assert_eq!(
        dr_8.try_into_vec().unwrap(),
        expected,
        "Dynamic Range level 8 should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Dynamic Range out of range
    let dr_result = DynamicRangeLevel::new(9);
    assert!(
        dr_result.is_err(),
        "Dynamic Range level 9 should return error"
    );
}

#[test]
fn test_iris_commands_encoding() {
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);

    // Iris Reset
    let iris_reset = Iris::Reset;
    let expected = &[0x81, 0x01, 0x04, 0x0B, 0x00, 0xFF];
    assert_eq!(
        iris_reset.try_into_vec().unwrap(),
        expected,
        "Iris Reset should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Iris Up
    let iris_up = Iris::Up;
    let expected = &[0x81, 0x01, 0x04, 0x0B, 0x02, 0xFF];
    assert_eq!(
        iris_up.try_into_vec().unwrap(),
        expected,
        "Iris Up should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();

    // Iris Direct F1.8
    let iris_f18 = Iris::SetAperture(IrisLevel::new(0x0C).unwrap());
    let expected = &[0x81, 0x01, 0x04, 0x4B, 0x00, 0x00, 0x00, 0x0C, 0xFF];
    assert_eq!(
        iris_f18.try_into_vec().unwrap(),
        expected,
        "Iris Direct F1.8 should produce correct byte sequence"
    );
    validator.validate_command(expected).unwrap();
}

#[test]
fn test_using_mock_transport_for_encoding() {
    // Test that our encoded commands work with MockTransportBuilder
    let power_on_cmd = &[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF];

    let mut mock = MockTransportBuilder::new()
        .connected(true)
        .expect(
            power_on_cmd,
            vec![
                MockResponse::Immediate(responses::ACK_1.to_vec()),
                MockResponse::Immediate(responses::COMPLETE_1.to_vec()),
            ],
        )
        .build();

    // Verify the mock transport accepts our encoded command
    let result = mock.send(power_on_cmd);
    assert!(result.is_ok());
}

#[test]
fn test_nibble_encoding_patterns() {
    // Test that position values are properly split into nibbles
    let test_values = [
        (0x0000, [0x00, 0x00, 0x00, 0x00]),
        (0x1234, [0x01, 0x02, 0x03, 0x04]),
        (0x6789, [0x06, 0x07, 0x08, 0x09]),
        (0x7000, [0x07, 0x00, 0x00, 0x00]),
    ];

    for (value, expected_nibbles) in test_values {
        let zoom_cmd = Zoom::Position(ZoomPosition::new(value).unwrap());
        let bytes = zoom_cmd.try_into_vec().unwrap();

        // Bytes 4-7 contain the position nibbles
        assert_eq!(
            &bytes[4..8],
            &expected_nibbles,
            "Position value 0x{:04X} should be encoded as nibbles {:02X?}",
            value,
            expected_nibbles
        );
    }
}
