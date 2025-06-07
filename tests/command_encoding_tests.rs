use grafton_visca::command::*;
use grafton_visca::ViscaCommand;

#[cfg(test)]
mod golden_vector_tests {
    use super::*;
    use grafton_visca::command::exposure::{DynamicRangeLevel, ExposureCompensationLevel};
    use grafton_visca::command::pan_tilt::{PanSpeed, PanTiltDirection, TiltSpeed};
    use grafton_visca::command::power::Power;
    use grafton_visca::command::preset::{PresetAction, PresetNumber};
    use grafton_visca::command::zoom::ZoomSpeed;

    #[test]
    fn test_power_commands() {
        // Power On
        let power_on = PowerCommand { power: Power::On };
        assert_eq!(
            power_on.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF],
            "Power On command should produce correct byte sequence"
        );

        // Power Standby
        let power_standby = PowerCommand {
            power: Power::Standby,
        };
        assert_eq!(
            power_standby.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF],
            "Power Standby command should produce correct byte sequence"
        );
    }

    #[test]
    fn test_pan_tilt_home() {
        let home_cmd = PanTiltCommand::Home;
        assert_eq!(
            home_cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x06, 0x04, 0xFF],
            "Pan/Tilt Home command should produce correct byte sequence"
        );
    }

    #[test]
    fn test_pan_tilt_move_directions() {
        // Test Up direction
        let move_up = PanTiltCommand::Move {
            direction: PanTiltDirection::Up,
            pan_speed: PanSpeed::new(0x10).unwrap(),
            tilt_speed: TiltSpeed::new(0x10).unwrap(),
        };
        assert_eq!(
            move_up.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x06, 0x01, 0x10, 0x10, 0x03, 0x01, 0xFF],
            "Pan/Tilt Move Up should produce correct byte sequence"
        );

        // Test DownRight direction
        let move_down_right = PanTiltCommand::Move {
            direction: PanTiltDirection::DownRight,
            pan_speed: PanSpeed::new(0x18).unwrap(), // max pan speed
            tilt_speed: TiltSpeed::new(0x14).unwrap(), // max tilt speed
        };
        assert_eq!(
            move_down_right.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x06, 0x01, 0x18, 0x14, 0x02, 0x02, 0xFF],
            "Pan/Tilt Move DownRight with max speeds should produce correct byte sequence"
        );

        // Test Stop
        let stop = PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0).unwrap(),
            tilt_speed: TiltSpeed::new(0).unwrap(),
        };
        assert_eq!(
            stop.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x06, 0x01, 0x00, 0x00, 0x03, 0x03, 0xFF],
            "Pan/Tilt Stop should produce correct byte sequence"
        );
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
    fn test_zoom_commands() {
        // Zoom Stop
        let zoom_stop = ZoomCommand::Stop;
        assert_eq!(
            zoom_stop.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xFF],
            "Zoom Stop should produce correct byte sequence"
        );

        // Zoom Tele Standard
        let zoom_tele = ZoomCommand::TeleStandard;
        assert_eq!(
            zoom_tele.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF],
            "Zoom Tele Standard should produce correct byte sequence"
        );

        // Zoom Wide Standard
        let zoom_wide = ZoomCommand::WideStandard;
        assert_eq!(
            zoom_wide.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x07, 0x03, 0xFF],
            "Zoom Wide Standard should produce correct byte sequence"
        );

        // Zoom Tele Variable with speed
        let zoom_tele_var = ZoomCommand::TeleVariable(ZoomSpeed::new(5).unwrap());
        assert_eq!(
            zoom_tele_var.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x07, 0x25, 0xFF], // 0x20 | 5 = 0x25
            "Zoom Tele Variable with speed 5 should produce correct byte sequence"
        );

        // Zoom Wide Variable with max speed
        let zoom_wide_var = ZoomCommand::WideVariable(ZoomSpeed::new(7).unwrap());
        assert_eq!(
            zoom_wide_var.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x07, 0x37, 0xFF], // 0x30 | 7 = 0x37
            "Zoom Wide Variable with max speed should produce correct byte sequence"
        );
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
    fn test_zoom_direct_position() {
        // Test a specific zoom position
        let zoom_direct = ZoomCommand::Direct(0x1234);
        assert_eq!(
            zoom_direct.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x47, 0x01, 0x02, 0x03, 0x04, 0xFF],
            "Zoom Direct position 0x1234 should be split into nibbles correctly"
        );

        // Test maximum zoom position
        let zoom_max = ZoomCommand::Direct(0xFFFF);
        assert_eq!(
            zoom_max.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x47, 0x0F, 0x0F, 0x0F, 0x0F, 0xFF],
            "Zoom Direct max position should be split into nibbles correctly"
        );
    }

    #[test]
    fn test_preset_commands() {
        // Preset Reset
        let preset_reset = PresetCommand {
            action: PresetAction::Reset,
            preset_number: PresetNumber::new(0).unwrap(),
        };
        assert_eq!(
            preset_reset.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x3F, 0x00, 0x00, 0xFF],
            "Preset Reset should produce correct byte sequence"
        );

        // Preset Set
        let preset_set = PresetCommand {
            action: PresetAction::Set,
            preset_number: PresetNumber::new(5).unwrap(),
        };
        assert_eq!(
            preset_set.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x3F, 0x01, 0x05, 0xFF],
            "Preset Set position 5 should produce correct byte sequence"
        );

        // Preset Recall
        let preset_recall = PresetCommand {
            action: PresetAction::Recall,
            preset_number: PresetNumber::new(89).unwrap(), // max preset number
        };
        assert_eq!(
            preset_recall.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x3F, 0x02, 0x59, 0xFF],
            "Preset Recall position 89 should produce correct byte sequence"
        );
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
    fn test_focus_commands() {
        // Focus Stop
        let focus_stop = FocusCommand::Stop;
        assert_eq!(
            focus_stop.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x08, 0x00, 0xFF],
            "Focus Stop should produce correct byte sequence"
        );

        // Focus Far Standard
        let focus_far = FocusCommand::FarStandard;
        assert_eq!(
            focus_far.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x08, 0x02, 0xFF],
            "Focus Far Standard should produce correct byte sequence"
        );

        // Focus Near Variable
        let focus_near_var = FocusCommand::NearVariable(3);
        assert_eq!(
            focus_near_var.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x08, 0x33, 0xFF], // 0x30 | 3 = 0x33
            "Focus Near Variable with speed 3 should produce correct byte sequence"
        );

        // Focus Auto
        let focus_auto = FocusCommand::Auto;
        assert_eq!(
            focus_auto.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x38, 0x02, 0xFF],
            "Focus Auto should produce correct byte sequence"
        );

        // Focus One Push Trigger
        let focus_one_push = FocusCommand::OnePushTrigger;
        assert_eq!(
            focus_one_push.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x18, 0x01, 0xFF],
            "Focus One Push Trigger should produce correct byte sequence"
        );
    }

    #[test]
    fn test_focus_speed_validation() {
        // Test focus speed out of range
        let focus_invalid = FocusCommand::FarVariable(8);
        assert!(
            focus_invalid.to_bytes().is_err(),
            "Focus speed above 7 should return error"
        );
    }

    #[test]
    fn test_exposure_mode_commands() {
        // Auto Exposure
        let exposure_auto = ExposureCommand {
            mode: ExposureMode::Auto,
        };
        assert_eq!(
            exposure_auto.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x39, 0x00, 0xFF],
            "Auto Exposure mode should produce correct byte sequence"
        );

        // Manual Exposure
        let exposure_manual = ExposureCommand {
            mode: ExposureMode::Manual,
        };
        assert_eq!(
            exposure_manual.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x39, 0x03, 0xFF],
            "Manual Exposure mode should produce correct byte sequence"
        );

        // Shutter Priority
        let exposure_shutter = ExposureCommand {
            mode: ExposureMode::Shutter,
        };
        assert_eq!(
            exposure_shutter.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x39, 0x0A, 0xFF],
            "Shutter Priority mode should produce correct byte sequence"
        );
    }

    #[test]
    fn test_inquiry_commands() {
        // Pan/Tilt Position Inquiry
        let pt_inquiry = InquiryCommand::PanTiltPosition;
        assert_eq!(
            pt_inquiry.to_bytes().unwrap(),
            vec![0x81, 0x09, 0x06, 0x12, 0xFF],
            "Pan/Tilt Position inquiry should produce correct byte sequence"
        );

        // Zoom Position Inquiry
        let zoom_inquiry = InquiryCommand::ZoomPosition;
        assert_eq!(
            zoom_inquiry.to_bytes().unwrap(),
            vec![0x81, 0x09, 0x04, 0x47, 0xFF],
            "Zoom Position inquiry should produce correct byte sequence"
        );

        // Focus Position Inquiry
        let focus_inquiry = InquiryCommand::FocusPosition;
        assert_eq!(
            focus_inquiry.to_bytes().unwrap(),
            vec![0x81, 0x09, 0x04, 0x48, 0xFF],
            "Focus Position inquiry should produce correct byte sequence"
        );

        // Exposure Mode Inquiry
        let exposure_inquiry = InquiryCommand::ExposureMode;
        assert_eq!(
            exposure_inquiry.to_bytes().unwrap(),
            vec![0x81, 0x09, 0x04, 0x39, 0xFF],
            "Exposure Mode inquiry should produce correct byte sequence"
        );
    }

    #[test]
    fn test_exposure_compensation_commands() {
        // Exposure Compensation On
        let exp_comp_on = ExposureCompensationCommand::On;
        assert_eq!(
            exp_comp_on.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x3E, 0x02, 0xFF],
            "Exposure Compensation On should produce correct byte sequence"
        );

        // Exposure Compensation Off
        let exp_comp_off = ExposureCompensationCommand::Off;
        assert_eq!(
            exp_comp_off.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x3E, 0x03, 0xFF],
            "Exposure Compensation Off should produce correct byte sequence"
        );

        // Exposure Compensation Reset
        let exp_comp_reset = ExposureCompensationCommand::Reset;
        assert_eq!(
            exp_comp_reset.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x0E, 0x00, 0xFF],
            "Exposure Compensation Reset should produce correct byte sequence"
        );

        // Exposure Compensation Direct -7
        let exp_comp_neg7 =
            ExposureCompensationCommand::Direct(ExposureCompensationLevel::new(-7).unwrap());
        assert_eq!(
            exp_comp_neg7.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x4E, 0x00, 0x00, 0x00, 0x00, 0xFF],
            "Exposure Compensation Direct -7 should produce correct byte sequence"
        );

        // Exposure Compensation Direct +7
        let exp_comp_pos7 =
            ExposureCompensationCommand::Direct(ExposureCompensationLevel::new(7).unwrap());
        assert_eq!(
            exp_comp_pos7.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x4E, 0x00, 0x00, 0x00, 0x0E, 0xFF],
            "Exposure Compensation Direct +7 should produce correct byte sequence"
        );
    }

    #[test]
    fn test_dynamic_range_command() {
        // Dynamic Range level 0
        let dr_0 = DynamicRangeCommand::Direct(DynamicRangeLevel::new(0).unwrap());
        assert_eq!(
            dr_0.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x25, 0x00, 0x00, 0x00, 0x00, 0xFF],
            "Dynamic Range level 0 should produce correct byte sequence"
        );

        // Dynamic Range level 8
        let dr_8 = DynamicRangeCommand::Direct(DynamicRangeLevel::new(8).unwrap());
        assert_eq!(
            dr_8.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x25, 0x00, 0x00, 0x00, 0x08, 0xFF],
            "Dynamic Range level 8 should produce correct byte sequence"
        );

        // Dynamic Range out of range
        let dr_result = DynamicRangeLevel::new(9);
        assert!(
            dr_result.is_err(),
            "Dynamic Range level 9 should return error"
        );
    }

    #[test]
    fn test_iris_commands() {
        // Iris Reset
        let iris_reset = IrisCommand::Reset;
        assert_eq!(
            iris_reset.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x0B, 0x00, 0xFF],
            "Iris Reset should produce correct byte sequence"
        );

        // Iris Up
        let iris_up = IrisCommand::Up;
        assert_eq!(
            iris_up.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x0B, 0x02, 0xFF],
            "Iris Up should produce correct byte sequence"
        );

        // Iris Direct F1.8
        let iris_f18 = IrisCommand::Direct(0x0C);
        assert_eq!(
            iris_f18.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x4B, 0x00, 0x00, 0x00, 0x0C, 0xFF],
            "Iris Direct F1.8 should produce correct byte sequence"
        );
    }

    #[test]
    fn test_shutter_commands() {
        // Shutter Reset
        let shutter_reset = ShutterCommand::Reset;
        assert_eq!(
            shutter_reset.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x0A, 0x00, 0xFF],
            "Shutter Reset should produce correct byte sequence"
        );

        // Shutter Direct 1/30
        let shutter_30 = ShutterCommand::Direct(0x01);
        assert_eq!(
            shutter_30.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x4A, 0x00, 0x00, 0x00, 0x01, 0xFF],
            "Shutter Direct 1/30 should produce correct byte sequence"
        );

        // Shutter Direct 1/10000
        let shutter_10000 = ShutterCommand::Direct(0x11);
        assert_eq!(
            shutter_10000.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x4A, 0x00, 0x00, 0x01, 0x01, 0xFF],
            "Shutter Direct 1/10000 should produce correct byte sequence"
        );
    }

    #[test]
    fn test_bright_commands() {
        // Bright Reset
        let bright_reset = BrightCommand::Reset;
        assert_eq!(
            bright_reset.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x0D, 0x00, 0xFF],
            "Bright Reset should produce correct byte sequence"
        );

        // Bright Direct 17
        let bright_17 = BrightCommand::Direct(0x11);
        assert_eq!(
            bright_17.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x0D, 0x00, 0x00, 0x01, 0x01, 0xFF],
            "Bright Direct 17 should produce correct byte sequence"
        );
    }

    #[test]
    fn test_gain_commands() {
        // Gain Reset
        let gain_reset = GainCommand::Reset;
        assert_eq!(
            gain_reset.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x0C, 0x00, 0xFF],
            "Gain Reset should produce correct byte sequence"
        );

        // Gain Direct 7
        let gain_7 = GainCommand::Direct(0x07);
        assert_eq!(
            gain_7.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x0C, 0x00, 0x00, 0x00, 0x07, 0xFF],
            "Gain Direct 7 should produce correct byte sequence"
        );

        // Gain Limit
        let gain_limit = GainLimitCommand { limit: 0x0F };
        assert_eq!(
            gain_limit.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x2C, 0x0F, 0xFF],
            "Gain Limit 15 should produce correct byte sequence"
        );

        // Anti-Flicker 50Hz
        let anti_flicker = AntiFlickerCommand {
            mode: AntiFlickerMode::Hz50,
        };
        assert_eq!(
            anti_flicker.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x23, 0x01, 0xFF],
            "Anti-Flicker 50Hz should produce correct byte sequence"
        );
    }

    #[test]
    fn test_color_commands() {
        // One-Push Trigger
        let one_push = OnePushTriggerCommand;
        assert_eq!(
            one_push.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x10, 0x05, 0xFF],
            "One-Push Trigger should produce correct byte sequence"
        );

        // Red Tuning -10
        let red_neg10 = RedTuningCommand { level: -10 };
        assert_eq!(
            red_neg10.to_bytes().unwrap(),
            vec![0x81, 0x0A, 0x01, 0x12, 0x00, 0xFF],
            "Red Tuning -10 should produce correct byte sequence"
        );

        // Red Tuning +10
        let red_pos10 = RedTuningCommand { level: 10 };
        assert_eq!(
            red_pos10.to_bytes().unwrap(),
            vec![0x81, 0x0A, 0x01, 0x12, 0x14, 0xFF],
            "Red Tuning +10 should produce correct byte sequence"
        );

        // Blue Tuning 0
        let blue_0 = BlueTuningCommand { level: 0 };
        assert_eq!(
            blue_0.to_bytes().unwrap(),
            vec![0x81, 0x0A, 0x01, 0x13, 0x0A, 0xFF],
            "Blue Tuning 0 should produce correct byte sequence"
        );

        // Saturation 200%
        let sat_200 = SaturationCommand { level: 0x0E };
        assert_eq!(
            sat_200.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x49, 0x00, 0x00, 0x00, 0x0E, 0xFF],
            "Saturation 200% should produce correct byte sequence"
        );

        // Hue 14
        let hue_14 = HueCommand { level: 0x0E };
        assert_eq!(
            hue_14.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x4F, 0x00, 0x00, 0x00, 0x0E, 0xFF],
            "Hue 14 should produce correct byte sequence"
        );
    }

    #[test]
    fn test_pan_tilt_advanced_commands() {
        // Pan/Tilt Reset
        let pt_reset = PanTiltCommand::Reset;
        assert_eq!(
            pt_reset.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x06, 0x05, 0xFF],
            "Pan/Tilt Reset should produce correct byte sequence"
        );

        // Absolute Position
        let abs_pos = PanTiltCommand::AbsolutePosition {
            pan: 0x1234,
            tilt: 0x5678,
            pan_speed: PanSpeed::new(0x10).unwrap(),
            tilt_speed: TiltSpeed::new(0x10).unwrap(),
        };
        assert_eq!(
            abs_pos.to_bytes().unwrap(),
            vec![
                0x81, 0x01, 0x06, 0x02, 0x10, 0x10, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                0xFF
            ],
            "Absolute Position should produce correct byte sequence"
        );

        // Relative Position
        let rel_pos = PanTiltCommand::RelativePosition {
            pan: -100,
            tilt: 200,
            pan_speed: PanSpeed::new(0x08).unwrap(),
            tilt_speed: TiltSpeed::new(0x08).unwrap(),
        };
        // Convert signed to unsigned for VISCA protocol encoding
        let pan_bytes = -100i16 as u16;
        let tilt_bytes = 200u16;
        assert_eq!(
            rel_pos.to_bytes().unwrap(),
            vec![
                0x81,
                0x01,
                0x06,
                0x03,
                0x08,
                0x08,
                ((pan_bytes >> 12) & 0x0F) as u8,
                ((pan_bytes >> 8) & 0x0F) as u8,
                ((pan_bytes >> 4) & 0x0F) as u8,
                (pan_bytes & 0x0F) as u8,
                ((tilt_bytes >> 12) & 0x0F) as u8,
                ((tilt_bytes >> 8) & 0x0F) as u8,
                ((tilt_bytes >> 4) & 0x0F) as u8,
                (tilt_bytes & 0x0F) as u8,
                0xFF
            ],
            "Relative Position should produce correct byte sequence"
        );
    }

    #[test]
    fn test_pan_tilt_limit_commands() {
        use grafton_visca::command::pan_tilt::LimitCorner;

        // Limit Set
        let limit_set = PanTiltLimitCommand::Set {
            corner: LimitCorner::DownLeft,
            pan: 0x1000,
            tilt: 0x0800,
        };
        assert_eq!(
            limit_set.to_bytes().unwrap(),
            vec![
                0x81, 0x01, 0x06, 0x07, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00,
                0xFF
            ],
            "Pan/Tilt Limit Set should produce correct byte sequence"
        );

        // Limit Clear
        let limit_clear = PanTiltLimitCommand::Clear {
            corner: LimitCorner::UpRight,
        };
        assert_eq!(
            limit_clear.to_bytes().unwrap(),
            vec![
                0x81, 0x01, 0x06, 0x07, 0x01, 0x01, 0x07, 0x0F, 0x0F, 0x0F, 0x07, 0x0F, 0x0F, 0x0F,
                0xFF
            ],
            "Pan/Tilt Limit Clear should produce correct byte sequence"
        );
    }

    #[test]
    fn test_new_inquiry_commands() {
        // Sharpness Inquiry
        let sharp_inq = InquiryCommand::Sharpness;
        assert_eq!(
            sharp_inq.to_bytes().unwrap(),
            vec![0x81, 0x09, 0x04, 0x42, 0xFF],
            "Sharpness inquiry should produce correct byte sequence"
        );

        // Iris Inquiry
        let iris_inq = InquiryCommand::Iris;
        assert_eq!(
            iris_inq.to_bytes().unwrap(),
            vec![0x81, 0x09, 0x04, 0x4B, 0xFF],
            "Iris inquiry should produce correct byte sequence"
        );

        // Gain Inquiry
        let gain_inq = InquiryCommand::Gain;
        assert_eq!(
            gain_inq.to_bytes().unwrap(),
            vec![0x81, 0x09, 0x04, 0x4C, 0xFF],
            "Gain inquiry should produce correct byte sequence"
        );

        // Red Gain Inquiry
        let red_gain_inq = InquiryCommand::RedGain;
        assert_eq!(
            red_gain_inq.to_bytes().unwrap(),
            vec![0x81, 0x09, 0x0A, 0x12, 0xFF],
            "Red Gain inquiry should produce correct byte sequence"
        );

        // Saturation Inquiry
        let sat_inq = InquiryCommand::Saturation;
        assert_eq!(
            sat_inq.to_bytes().unwrap(),
            vec![0x81, 0x09, 0x04, 0x49, 0xFF],
            "Saturation inquiry should produce correct byte sequence"
        );
    }

    #[test]
    fn test_sharpness_mode_commands() {
        // Sharpness Mode Auto
        let sharp_auto = SharpnessCommand::Mode(SharpnessMode::Auto);
        assert_eq!(
            sharp_auto.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x05, 0x02, 0xFF],
            "Sharpness Mode Auto should produce correct byte sequence"
        );

        // Sharpness Mode Manual
        let sharp_manual = SharpnessCommand::Mode(SharpnessMode::Manual);
        assert_eq!(
            sharp_manual.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x05, 0x03, 0xFF],
            "Sharpness Mode Manual should produce correct byte sequence"
        );

        // Sharpness Reset
        let sharp_reset = SharpnessCommand::Reset;
        assert_eq!(
            sharp_reset.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x02, 0x00, 0xFF],
            "Sharpness Reset should produce correct byte sequence"
        );

        // Sharpness Up
        let sharp_up = SharpnessCommand::Up;
        assert_eq!(
            sharp_up.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x02, 0x02, 0xFF],
            "Sharpness Up should produce correct byte sequence"
        );

        // Sharpness Direct 11
        let sharp_11 = SharpnessCommand::Direct { value: 11 };
        assert_eq!(
            sharp_11.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x42, 0x00, 0x00, 0x00, 0x0B, 0xFF],
            "Sharpness Direct 11 should produce correct byte sequence"
        );
    }

    #[test]
    fn test_color_temperature_commands() {
        // Color Temperature Reset
        let ct_reset = ColorTemperatureCommand::Reset;
        assert_eq!(
            ct_reset.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x20, 0x00, 0xFF],
            "Color Temperature Reset should produce correct byte sequence"
        );

        // Color Temperature Up
        let ct_up = ColorTemperatureCommand::Up;
        assert_eq!(
            ct_up.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x20, 0x02, 0xFF],
            "Color Temperature Up should produce correct byte sequence"
        );

        // Color Temperature Direct 8000K (0x37)
        let ct_8000k = ColorTemperatureCommand::Direct(0x37);
        assert_eq!(
            ct_8000k.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x20, 0x00, 0x00, 0x03, 0x07, 0xFF],
            "Color Temperature Direct 8000K should produce correct byte sequence"
        );
    }

    #[test]
    fn test_red_blue_gain_commands() {
        // Red Gain Reset
        let red_reset = RedGainCommand::Reset;
        assert_eq!(
            red_reset.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x03, 0x00, 0xFF],
            "Red Gain Reset should produce correct byte sequence"
        );

        // Red Gain Direct 0xFF
        let red_ff = RedGainCommand::Direct(0xFF);
        assert_eq!(
            red_ff.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x43, 0x00, 0x00, 0x0F, 0x0F, 0xFF],
            "Red Gain Direct 0xFF should produce correct byte sequence"
        );

        // Blue Gain Up
        let blue_up = BlueGainCommand::Up;
        assert_eq!(
            blue_up.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x04, 0x02, 0xFF],
            "Blue Gain Up should produce correct byte sequence"
        );

        // Blue Gain Direct 0x80
        let blue_80 = BlueGainCommand::Direct(0x80);
        assert_eq!(
            blue_80.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x44, 0x00, 0x00, 0x08, 0x00, 0xFF],
            "Blue Gain Direct 0x80 should produce correct byte sequence"
        );
    }

    #[test]
    fn test_noise_reduction_commands() {
        // 2D Noise Reduction Off
        let nr2d_off = NoiseReduction2DCommand::Off;
        assert_eq!(
            nr2d_off.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x53, 0x00, 0xFF],
            "2D Noise Reduction Off should produce correct byte sequence"
        );

        // 2D Noise Reduction Level 5
        let nr2d_5 = NoiseReduction2DCommand::Level(5);
        assert_eq!(
            nr2d_5.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x53, 0x05, 0xFF],
            "2D Noise Reduction Level 5 should produce correct byte sequence"
        );

        // 3D Noise Reduction Level 8
        let nr3d_8 = NoiseReduction3DCommand::Level(8);
        assert_eq!(
            nr3d_8.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x54, 0x08, 0xFF],
            "3D Noise Reduction Level 8 should produce correct byte sequence"
        );
    }

    #[test]
    fn test_black_white_mode() {
        // Black & White Mode On
        let bw_on = BlackWhiteCommand { on: true };
        assert_eq!(
            bw_on.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x01, 0x04, 0xFF],
            "Black & White Mode On should produce correct byte sequence"
        );

        // Black & White Mode Off
        let bw_off = BlackWhiteCommand { on: false };
        assert_eq!(
            bw_off.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x01, 0x00, 0xFF],
            "Black & White Mode Off should produce correct byte sequence"
        );
    }

    #[test]
    fn test_image_flip_combined() {
        // Image Flip Off
        let flip_off = ImageFlipCombinedCommand {
            mode: ImageFlipMode::Off,
        };
        assert_eq!(
            flip_off.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x61, 0x00, 0xFF],
            "Image Flip Off should produce correct byte sequence"
        );

        // Image Flip Both
        let flip_both = ImageFlipCombinedCommand {
            mode: ImageFlipMode::Both,
        };
        assert_eq!(
            flip_both.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x61, 0x03, 0xFF],
            "Image Flip Both should produce correct byte sequence"
        );
    }

    #[test]
    fn test_focus_advanced_commands() {
        // Focus Zone Center
        let fz_center = FocusZoneCommand {
            zone: FocusZone::Center,
        };
        assert_eq!(
            fz_center.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x3C, 0x01, 0xFF],
            "Focus Zone Center should produce correct byte sequence"
        );

        // AF Sensitivity High
        let af_high = AFSensitivityCommand {
            sensitivity: AFSensitivity::High,
        };
        assert_eq!(
            af_high.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x58, 0x02, 0xFF],
            "AF Sensitivity High should produce correct byte sequence"
        );

        // Focus Near Limit
        let fnl = FocusNearLimitCommand { position: 0x1234 };
        assert_eq!(
            fnl.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x28, 0x01, 0x02, 0x03, 0x04, 0xFF],
            "Focus Near Limit should produce correct byte sequence"
        );
    }

    #[test]
    fn test_new_inquiry_commands_extended() {
        // Sharpness Mode Inquiry
        let sharp_mode_inq = InquiryCommand::SharpnessMode;
        assert_eq!(
            sharp_mode_inq.to_bytes().unwrap(),
            vec![0x81, 0x09, 0x04, 0x05, 0xFF],
            "Sharpness Mode inquiry should produce correct byte sequence"
        );

        // Color Temperature Inquiry
        let ct_inq = InquiryCommand::ColorTemperature;
        assert_eq!(
            ct_inq.to_bytes().unwrap(),
            vec![0x81, 0x09, 0x04, 0x20, 0xFF],
            "Color Temperature inquiry should produce correct byte sequence"
        );

        // Dynamic Range Inquiry
        let dr_inq = InquiryCommand::DynamicRange;
        assert_eq!(
            dr_inq.to_bytes().unwrap(),
            vec![0x81, 0x09, 0x04, 0x25, 0xFF],
            "Dynamic Range inquiry should produce correct byte sequence"
        );
    }
}
