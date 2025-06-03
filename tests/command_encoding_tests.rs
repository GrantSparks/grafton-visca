use grafton_visca::command::*;
use grafton_visca::ViscaCommand;

#[cfg(test)]
mod golden_vector_tests {
    use super::*;
    use grafton_visca::command::pan_tilt::{PanSpeed, PanTiltDirection, TiltSpeed};
    use grafton_visca::command::power::Power;
    use grafton_visca::command::preset::PresetAction;

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
        let zoom_tele_var = ZoomCommand::TeleVariable(5);
        assert_eq!(
            zoom_tele_var.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x07, 0x25, 0xFF], // 0x20 | 5 = 0x25
            "Zoom Tele Variable with speed 5 should produce correct byte sequence"
        );

        // Zoom Wide Variable with max speed
        let zoom_wide_var = ZoomCommand::WideVariable(7);
        assert_eq!(
            zoom_wide_var.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x07, 0x37, 0xFF], // 0x30 | 7 = 0x37
            "Zoom Wide Variable with max speed should produce correct byte sequence"
        );
    }

    #[test]
    fn test_zoom_speed_validation() {
        // Test zoom speed out of range
        let zoom_invalid = ZoomCommand::TeleVariable(8);
        assert!(
            zoom_invalid.to_bytes().is_err(),
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
            preset_number: 0,
        };
        assert_eq!(
            preset_reset.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x3F, 0x00, 0x00, 0xFF],
            "Preset Reset should produce correct byte sequence"
        );

        // Preset Set
        let preset_set = PresetCommand {
            action: PresetAction::Set,
            preset_number: 5,
        };
        assert_eq!(
            preset_set.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x3F, 0x01, 0x05, 0xFF],
            "Preset Set position 5 should produce correct byte sequence"
        );

        // Preset Recall
        let preset_recall = PresetCommand {
            action: PresetAction::Recall,
            preset_number: 89, // max preset number
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
        let invalid_preset = PresetCommand {
            action: PresetAction::Set,
            preset_number: 90,
        };
        assert!(
            invalid_preset.to_bytes().is_err(),
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
}
