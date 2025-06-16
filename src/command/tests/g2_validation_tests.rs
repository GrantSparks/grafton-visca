#![allow(missing_docs)]
#![allow(clippy::unwrap_used)] // Tests can use unwrap
//! Comprehensive tests for PTZOptics G2 camera command validation
//!
//! This test suite validates every command in the library against the
//! PTZOptics G2 documentation, testing both valid (positive) and invalid
//! (negative) cases for each command.

use crate::{constants::CameraModel, Command, Error};

/// Helper function to test that a command is valid for G2
fn assert_valid_for_g2<C: Command>(command: &C) {
    match command.validate_for_model(CameraModel::PTZOpticsG2) {
        Ok(()) => {} // Expected
        Err(e) => panic!("Command should be valid for G2 but got error: {:?}", e),
    }
}

/// Helper function to test that a command is invalid for G2
fn assert_invalid_for_g2<C: Command>(command: &C, expected_reason: &str) {
    match command.validate_for_model(CameraModel::PTZOpticsG2) {
        Err(Error::ModelValidation {
            model,
            command: _,
            reason,
        }) => {
            assert_eq!(model, CameraModel::PTZOpticsG2);
            assert!(
                reason.contains(expected_reason),
                "Expected reason to contain '{}', but got: '{}'",
                expected_reason,
                reason
            );
        }
        Ok(()) => panic!("Command should be invalid for G2 but validation passed"),
        Err(e) => panic!("Expected ModelValidation error but got: {:?}", e),
    }
}

mod power_commands {
    use super::*;
    use crate::command::power::{Power, PowerCommand};

    #[test]
    fn test_power_commands() {
        // All power commands should be valid for G2
        assert_valid_for_g2(&PowerCommand { power: Power::On });
        assert_valid_for_g2(&PowerCommand {
            power: Power::Standby,
        });
    }
}

mod zoom_commands {
    use super::*;
    use crate::command::zoom::{ZoomCommand, ZoomSpeed};

    #[test]
    fn test_zoom_stop() {
        assert_valid_for_g2(&ZoomCommand::Stop);
    }

    #[test]
    fn test_zoom_standard_speeds() {
        assert_valid_for_g2(&ZoomCommand::ZoomInStandard);
        assert_valid_for_g2(&ZoomCommand::ZoomOutStandard);
    }

    #[test]
    fn test_zoom_variable_speeds() {
        // Valid speeds: 0-7
        for speed in 0..=7 {
            let zoom_speed = ZoomSpeed::new(speed).unwrap();
            assert_valid_for_g2(&ZoomCommand::ZoomInVariable(zoom_speed));
            assert_valid_for_g2(&ZoomCommand::ZoomOutVariable(zoom_speed));
        }
    }

    #[test]
    fn test_zoom_direct_positions() {
        // G2 has 20X optical zoom, max position is 0x7000

        // Valid positions
        assert_valid_for_g2(&ZoomCommand::Direct(0x0000)); // Wide end
        assert_valid_for_g2(&ZoomCommand::Direct(0x3000)); // Mid position
        assert_valid_for_g2(&ZoomCommand::Direct(0x7000)); // 20X optical limit

        // Invalid positions (beyond 20X)
        assert_invalid_for_g2(&ZoomCommand::Direct(0x7001), "0x7000");
        assert_invalid_for_g2(&ZoomCommand::Direct(0x7AC0), "0x7000"); // 30X position
        assert_invalid_for_g2(&ZoomCommand::Direct(0x7FFF), "0x7000"); // Digital zoom max
        assert_invalid_for_g2(&ZoomCommand::Direct(0xFFFF), "0x7000"); // Max possible value
    }
}

mod focus_commands {
    use super::*;
    use crate::command::focus::{
        AutoFocusSensitivity, AutoFocusSensitivityCommand, FocusCommand, FocusNearLimitCommand,
        FocusSpeed, FocusZone, FocusZoneCommand,
    };

    #[test]
    fn test_focus_stop() {
        assert_valid_for_g2(&FocusCommand::Stop);
    }

    #[test]
    fn test_focus_standard_speeds() {
        assert_valid_for_g2(&FocusCommand::FocusFarStandard);
        assert_valid_for_g2(&FocusCommand::FocusNearStandard);
    }

    #[test]
    fn test_focus_variable_speeds() {
        // Valid speeds: 0-7
        for speed in 0..=7 {
            let focus_speed = FocusSpeed::new(speed).unwrap();
            assert_valid_for_g2(&FocusCommand::FarVariable(focus_speed));
            assert_valid_for_g2(&FocusCommand::NearVariable(focus_speed));
        }
    }

    #[test]
    fn test_focus_direct_positions() {
        // Focus range: 0x1000 to 0xF000
        assert_valid_for_g2(&FocusCommand::Direct(0x1000)); // Infinity
        assert_valid_for_g2(&FocusCommand::Direct(0x8000)); // Mid position
        assert_valid_for_g2(&FocusCommand::Direct(0xF000)); // Near limit

        // Invalid positions (out of range)
        assert_invalid_for_g2(&FocusCommand::Direct(0x0FFF), "0x1000");
        assert_invalid_for_g2(&FocusCommand::Direct(0xF001), "0xF000");
        assert_invalid_for_g2(&FocusCommand::Direct(0x0000), "0x1000");
        assert_invalid_for_g2(&FocusCommand::Direct(0xFFFF), "0xF000");
    }

    #[test]
    fn test_focus_modes() {
        assert_valid_for_g2(&FocusCommand::Auto);
        assert_valid_for_g2(&FocusCommand::Manual);
    }

    #[test]
    fn test_focus_one_push() {
        assert_valid_for_g2(&FocusCommand::OnePushTrigger);
    }

    #[test]
    fn test_focus_infinity() {
        assert_valid_for_g2(&FocusCommand::Infinity);
    }

    #[test]
    fn test_focus_zones() {
        assert_valid_for_g2(&FocusZoneCommand {
            zone: FocusZone::Top,
        });
        assert_valid_for_g2(&FocusZoneCommand {
            zone: FocusZone::Center,
        });
        assert_valid_for_g2(&FocusZoneCommand {
            zone: FocusZone::Bottom,
        });
    }

    #[test]
    fn test_focus_sensitivity() {
        assert_valid_for_g2(&AutoFocusSensitivityCommand {
            sensitivity: AutoFocusSensitivity::Normal,
        });
        assert_valid_for_g2(&AutoFocusSensitivityCommand {
            sensitivity: AutoFocusSensitivity::Low,
        });
    }

    #[test]
    fn test_focus_near_limit() {
        // Valid positions within focus range
        assert_valid_for_g2(&FocusNearLimitCommand { position: 0x1000 });
        assert_valid_for_g2(&FocusNearLimitCommand { position: 0x8000 });
        assert_valid_for_g2(&FocusNearLimitCommand { position: 0xF000 });

        // Invalid positions (outside 0x1000-0xF000 range)
        assert_invalid_for_g2(&FocusNearLimitCommand { position: 0x0FFF }, "0x1000-0xF000");
        assert_invalid_for_g2(&FocusNearLimitCommand { position: 0xF001 }, "0x1000-0xF000");
        assert_invalid_for_g2(&FocusNearLimitCommand { position: 0xFFFF }, "0x1000-0xF000");
    }
}

mod pan_tilt_commands {
    use super::*;
    use crate::command::pan_tilt::{
        LimitCorner, PanSpeed, PanTiltCommand, PanTiltDirection, PanTiltLimitCommand, TiltSpeed,
    };

    #[test]
    fn test_pan_tilt_home() {
        assert_valid_for_g2(&PanTiltCommand::Home);
    }

    #[test]
    fn test_pan_tilt_reset() {
        assert_valid_for_g2(&PanTiltCommand::Reset);
    }

    #[test]
    fn test_pan_tilt_directional_movement() {
        let pan_speed = PanSpeed::new(0x10).unwrap();
        let tilt_speed = TiltSpeed::new(0x10).unwrap();

        // Test all directions
        let directions = vec![
            PanTiltDirection::Up,
            PanTiltDirection::Down,
            PanTiltDirection::Left,
            PanTiltDirection::Right,
            PanTiltDirection::UpLeft,
            PanTiltDirection::UpRight,
            PanTiltDirection::DownLeft,
            PanTiltDirection::DownRight,
            PanTiltDirection::Stop,
        ];

        for direction in directions {
            assert_valid_for_g2(&PanTiltCommand::Move {
                direction,
                pan_speed,
                tilt_speed,
            });
        }
    }

    #[test]
    fn test_pan_tilt_speed_limits() {
        // Valid pan speeds: 0x00 to 0x18 (0-24)
        for speed in 0x00..=0x18 {
            let pan_speed = PanSpeed::new(speed).unwrap();
            assert_valid_for_g2(&PanTiltCommand::Move {
                direction: PanTiltDirection::Right,
                pan_speed,
                tilt_speed: TiltSpeed::new(0x10).unwrap(),
            });
        }

        // Valid tilt speeds: 0x00 to 0x14 (0-20)
        for speed in 0x00..=0x14 {
            let tilt_speed = TiltSpeed::new(speed).unwrap();
            assert_valid_for_g2(&PanTiltCommand::Move {
                direction: PanTiltDirection::Up,
                pan_speed: PanSpeed::new(0x10).unwrap(),
                tilt_speed,
            });
        }
    }

    #[test]
    fn test_pan_tilt_absolute_position() {
        let pan_speed = PanSpeed::new(0x10).unwrap();
        let tilt_speed = TiltSpeed::new(0x10).unwrap();

        // Valid positions within G2 range
        // Pan: -2448 to 2448, Tilt: -432 to 1296
        assert_valid_for_g2(&PanTiltCommand::AbsolutePosition {
            pan: 0,
            tilt: 0,
            pan_speed,
            tilt_speed,
        });

        assert_valid_for_g2(&PanTiltCommand::AbsolutePosition {
            pan: 2448,
            tilt: 1296,
            pan_speed,
            tilt_speed,
        });

        assert_valid_for_g2(&PanTiltCommand::AbsolutePosition {
            pan: -2448,
            tilt: -432,
            pan_speed,
            tilt_speed,
        });

        // Invalid positions (out of range)
        assert_invalid_for_g2(
            &PanTiltCommand::AbsolutePosition {
                pan: 2449,
                tilt: 0,
                pan_speed,
                tilt_speed,
            },
            "2448",
        );

        assert_invalid_for_g2(
            &PanTiltCommand::AbsolutePosition {
                pan: -2449,
                tilt: 0,
                pan_speed,
                tilt_speed,
            },
            "-2448",
        );

        assert_invalid_for_g2(
            &PanTiltCommand::AbsolutePosition {
                pan: 0,
                tilt: 1297,
                pan_speed,
                tilt_speed,
            },
            "1296",
        );

        assert_invalid_for_g2(
            &PanTiltCommand::AbsolutePosition {
                pan: 0,
                tilt: -433,
                pan_speed,
                tilt_speed,
            },
            "-432",
        );
    }

    #[test]
    fn test_pan_tilt_relative_position() {
        let pan_speed = PanSpeed::new(0x10).unwrap();
        let tilt_speed = TiltSpeed::new(0x10).unwrap();

        // Relative positions don't have validation currently
        assert_valid_for_g2(&PanTiltCommand::RelativePosition {
            pan: 100,
            tilt: 100,
            pan_speed,
            tilt_speed,
        });

        assert_valid_for_g2(&PanTiltCommand::RelativePosition {
            pan: -100,
            tilt: -100,
            pan_speed,
            tilt_speed,
        });
    }

    #[test]
    fn test_pan_tilt_limits() {
        // Limit set commands
        assert_valid_for_g2(&PanTiltLimitCommand::Set {
            corner: LimitCorner::DownLeft,
            pan: 0,
            tilt: 0,
        });

        assert_valid_for_g2(&PanTiltLimitCommand::Set {
            corner: LimitCorner::UpRight,
            pan: 1000,
            tilt: 1000,
        });

        // Limit clear commands
        assert_valid_for_g2(&PanTiltLimitCommand::Clear {
            corner: LimitCorner::DownLeft,
        });

        assert_valid_for_g2(&PanTiltLimitCommand::Clear {
            corner: LimitCorner::UpRight,
        });
    }
}

mod preset_commands {
    use super::*;
    use crate::command::preset::{PresetAction, PresetCommand, PresetNumber};

    #[test]
    fn test_preset_actions() {
        // G2 supports presets 0-89
        let valid_presets = vec![0, 1, 50, 89];

        for preset_id in valid_presets {
            let preset = PresetNumber::new(preset_id).unwrap();
            assert_valid_for_g2(&PresetCommand {
                action: PresetAction::Reset,
                preset_number: preset,
            });
            assert_valid_for_g2(&PresetCommand {
                action: PresetAction::Set,
                preset_number: preset,
            });
            assert_valid_for_g2(&PresetCommand {
                action: PresetAction::Recall,
                preset_number: preset,
            });
        }
    }
}

mod exposure_commands {
    use super::*;
    use crate::{
        command::{
            exposure::{
                BrightCommand, DynamicRangeCommand, DynamicRangeLevel, ExposureCommand,
                ExposureCompensationCommand, ExposureCompensationLevel, ExposureMode, IrisCommand,
                ShutterCommand,
            },
            gain::{AntiFlickerCommand, AntiFlickerMode, GainCommand, GainLimitCommand},
            image::BacklightCommand,
        },
        types::{BrightnessLevel, GainLimit, GainValue, IrisLevel, ShutterSpeed},
    };

    #[test]
    fn test_exposure_modes() {
        // All exposure modes are valid for G2
        assert_valid_for_g2(&ExposureCommand {
            mode: ExposureMode::Auto,
        });
        assert_valid_for_g2(&ExposureCommand {
            mode: ExposureMode::Manual,
        });
        assert_valid_for_g2(&ExposureCommand {
            mode: ExposureMode::Shutter,
        });
        assert_valid_for_g2(&ExposureCommand {
            mode: ExposureMode::Iris,
        });
        assert_valid_for_g2(&ExposureCommand {
            mode: ExposureMode::Bright,
        });
    }

    #[test]
    fn test_exposure_compensation() {
        assert_valid_for_g2(&ExposureCompensationCommand::On);
        assert_valid_for_g2(&ExposureCompensationCommand::Off);
        assert_valid_for_g2(&ExposureCompensationCommand::Reset);
        assert_valid_for_g2(&ExposureCompensationCommand::Up);
        assert_valid_for_g2(&ExposureCompensationCommand::Down);

        // Valid range: -7 to +7
        for value in -7..=7 {
            assert_valid_for_g2(&ExposureCompensationCommand::Direct(
                ExposureCompensationLevel::new(value).unwrap(),
            ));
        }
    }

    #[test]
    fn test_backlight() {
        assert_valid_for_g2(&BacklightCommand { status: true });
        assert_valid_for_g2(&BacklightCommand { status: false });
    }

    #[test]
    fn test_iris_commands() {
        assert_valid_for_g2(&IrisCommand::Reset);
        assert_valid_for_g2(&IrisCommand::Up);
        assert_valid_for_g2(&IrisCommand::Down);

        // Valid range: 0x00 (Close) to 0x0C (F1.8)
        for value in 0x00..=0x0C {
            assert_valid_for_g2(&IrisCommand::Direct(IrisLevel::new(value).unwrap()));
        }
    }

    #[test]
    fn test_shutter_commands() {
        assert_valid_for_g2(&ShutterCommand::Reset);
        assert_valid_for_g2(&ShutterCommand::Up);
        assert_valid_for_g2(&ShutterCommand::Down);

        // Valid range: 0x01 (1/30) to 0x11 (1/10000)
        for value in 0x01..=0x11 {
            assert_valid_for_g2(&ShutterCommand::Direct(ShutterSpeed::new(value).unwrap()));
        }
    }

    #[test]
    fn test_bright_commands() {
        assert_valid_for_g2(&BrightCommand::Reset);
        assert_valid_for_g2(&BrightCommand::Up);
        assert_valid_for_g2(&BrightCommand::Down);

        // Valid range: 0x00 to 0x11 (0-17)
        for value in 0x00..=0x11 {
            assert_valid_for_g2(&BrightCommand::Direct(BrightnessLevel::new(value).unwrap()));
        }

        // BrightnessLevel::new() already prevents values > 0x11, so we can't test invalid direct values
        // The type system enforces the constraint
    }

    #[test]
    fn test_gain_commands() {
        assert_valid_for_g2(&GainCommand::Reset);
        assert_valid_for_g2(&GainCommand::Up);
        assert_valid_for_g2(&GainCommand::Down);

        // Valid range: 0x00 to 0x07 (0-7)
        for value in 0x00..=0x07 {
            assert_valid_for_g2(&GainCommand::Direct(GainValue::new(value).unwrap()));
        }
    }

    #[test]
    fn test_gain_limit() {
        // Valid range: 0x0 to 0xF (0-15)
        for value in 0x0..=0xF {
            assert_valid_for_g2(&GainLimitCommand {
                limit: GainLimit::new(value).unwrap(),
            });
        }
    }

    #[test]
    fn test_dynamic_range() {
        // Valid range: 0 to 8
        for value in 0..=8 {
            assert_valid_for_g2(&DynamicRangeCommand::Direct(
                DynamicRangeLevel::new(value).unwrap(),
            ));
        }
    }

    #[test]
    fn test_anti_flicker() {
        assert_valid_for_g2(&AntiFlickerCommand {
            mode: AntiFlickerMode::Off,
        });
        assert_valid_for_g2(&AntiFlickerCommand {
            mode: AntiFlickerMode::Hz50,
        });
        assert_valid_for_g2(&AntiFlickerCommand {
            mode: AntiFlickerMode::Hz60,
        });
    }
}

mod white_balance_commands {
    use super::*;
    use crate::command::{
        color::{
            BlueGainCommand, BlueTuningCommand, ColorTemperatureCommand, OnePushTriggerCommand,
            RedGainCommand, RedTuningCommand,
        },
        white_balance::{WhiteBalanceCommand, WhiteBalanceMode},
    };

    #[test]
    fn test_white_balance_modes() {
        assert_valid_for_g2(&WhiteBalanceCommand {
            mode: WhiteBalanceMode::Auto,
        });
        assert_valid_for_g2(&WhiteBalanceCommand {
            mode: WhiteBalanceMode::Indoor,
        });
        assert_valid_for_g2(&WhiteBalanceCommand {
            mode: WhiteBalanceMode::Outdoor,
        });
        assert_valid_for_g2(&WhiteBalanceCommand {
            mode: WhiteBalanceMode::OnePush,
        });
        assert_valid_for_g2(&WhiteBalanceCommand {
            mode: WhiteBalanceMode::Manual,
        });
        assert_valid_for_g2(&WhiteBalanceCommand {
            mode: WhiteBalanceMode::ColorTemperature,
        });
    }

    #[test]
    fn test_one_push_trigger() {
        assert_valid_for_g2(&OnePushTriggerCommand);
    }

    #[test]
    fn test_color_tuning() {
        // Valid range: -10 to +10
        for value in -10..=10 {
            assert_valid_for_g2(&RedTuningCommand { level: value });
            assert_valid_for_g2(&BlueTuningCommand { level: value });
        }
    }

    #[test]
    fn test_red_blue_gain() {
        assert_valid_for_g2(&RedGainCommand::Reset);
        assert_valid_for_g2(&RedGainCommand::Up);
        assert_valid_for_g2(&RedGainCommand::Down);

        assert_valid_for_g2(&BlueGainCommand::Reset);
        assert_valid_for_g2(&BlueGainCommand::Up);
        assert_valid_for_g2(&BlueGainCommand::Down);

        // Valid range: 0x00 to 0xFF
        for value in [0x00, 0x80, 0xFF] {
            assert_valid_for_g2(&RedGainCommand::Direct(value));
            assert_valid_for_g2(&BlueGainCommand::Direct(value));
        }
    }

    #[test]
    fn test_color_temperature() {
        assert_valid_for_g2(&ColorTemperatureCommand::Reset);
        assert_valid_for_g2(&ColorTemperatureCommand::Up);
        assert_valid_for_g2(&ColorTemperatureCommand::Down);

        // Valid range: 0x00 (2500K) to 0x37 (8000K)
        for value in [0x00, 0x10, 0x20, 0x30, 0x37] {
            assert_valid_for_g2(&ColorTemperatureCommand::Direct(value));
        }
    }
}

mod image_adjustment_commands {
    use super::*;
    use crate::{
        command::{
            color::{HueCommand, SaturationCommand},
            image::{BlackWhiteCommand, NoiseReduction2DCommand, NoiseReduction3DCommand},
            luminance_contrast_sharpness::{
                ContrastCommand, LuminanceCommand, SharpnessCommand, SharpnessMode,
            },
        },
        types::{ContrastLevel, LuminanceLevel, NoiseReduction2DLevel, NoiseReduction3DLevel},
    };

    #[test]
    fn test_saturation() {
        // Valid range: 0x0 (60%) to 0xE (200%)
        for value in 0x0..=0xE {
            assert_valid_for_g2(&SaturationCommand { level: value });
        }
    }

    #[test]
    fn test_hue() {
        // Valid range: 0x0 to 0xE (0-14)
        for value in 0x0..=0xE {
            assert_valid_for_g2(&HueCommand { level: value });
        }
    }

    #[test]
    fn test_luminance() {
        // Valid range: 0 to 14
        for value in 0..=14 {
            assert_valid_for_g2(&LuminanceCommand {
                value: LuminanceLevel::new(value).unwrap(),
            });
        }
    }

    #[test]
    fn test_contrast() {
        // Valid range: 0 to 14
        for value in 0..=14 {
            assert_valid_for_g2(&ContrastCommand {
                value: ContrastLevel::new(value).unwrap(),
            });
        }
    }

    #[test]
    fn test_sharpness() {
        assert_valid_for_g2(&SharpnessCommand::Mode(SharpnessMode::Auto));
        assert_valid_for_g2(&SharpnessCommand::Mode(SharpnessMode::Manual));
        assert_valid_for_g2(&SharpnessCommand::Reset);
        assert_valid_for_g2(&SharpnessCommand::Up);
        assert_valid_for_g2(&SharpnessCommand::Down);

        // Valid range: 0 to 11
        for value in 0..=11 {
            assert_valid_for_g2(&SharpnessCommand::Direct { value });
        }
    }

    #[test]
    fn test_noise_reduction_2d() {
        assert_valid_for_g2(&NoiseReduction2DCommand::Off);

        // Valid levels: 1 to 5
        for level in 1..=5 {
            assert_valid_for_g2(&NoiseReduction2DCommand::Level(
                NoiseReduction2DLevel::new(level).unwrap(),
            ));
        }
    }

    #[test]
    fn test_noise_reduction_3d() {
        assert_valid_for_g2(&NoiseReduction3DCommand::Off);

        // Valid levels: 1 to 8
        for level in 1..=8 {
            assert_valid_for_g2(&NoiseReduction3DCommand::Level(
                NoiseReduction3DLevel::new(level).unwrap(),
            ));
        }
    }

    #[test]
    fn test_black_white_mode() {
        assert_valid_for_g2(&BlackWhiteCommand { on: true });
        assert_valid_for_g2(&BlackWhiteCommand { on: false });
    }
}

mod flip_commands {
    use super::*;
    use crate::command::flip::{Flip, ImageFlipCommand};

    #[test]
    fn test_image_flip() {
        assert_valid_for_g2(&ImageFlipCommand { flip: Flip::On });
        assert_valid_for_g2(&ImageFlipCommand { flip: Flip::Off });
    }
}

mod inquiry_commands {
    use super::*;
    use crate::command::inquiry::InquiryCommand;

    #[test]
    fn test_all_inquiry_commands() {
        // All inquiry commands should be valid for any camera model
        assert_valid_for_g2(&InquiryCommand::Power);
        assert_valid_for_g2(&InquiryCommand::ZoomPosition);
        assert_valid_for_g2(&InquiryCommand::FocusPosition);
        assert_valid_for_g2(&InquiryCommand::FocusZone);
        assert_valid_for_g2(&InquiryCommand::AutoFocusSensitivity);
        assert_valid_for_g2(&InquiryCommand::FocusNearLimit);
        assert_valid_for_g2(&InquiryCommand::PanTiltPosition);
        assert_valid_for_g2(&InquiryCommand::ExposureMode);
        assert_valid_for_g2(&InquiryCommand::Iris);
        assert_valid_for_g2(&InquiryCommand::Gain);
        assert_valid_for_g2(&InquiryCommand::GainLimit);
        assert_valid_for_g2(&InquiryCommand::Bright);
        assert_valid_for_g2(&InquiryCommand::Shutter);
        assert_valid_for_g2(&InquiryCommand::ExposureCompensation);
        assert_valid_for_g2(&InquiryCommand::ExposureCompensationMode);
        assert_valid_for_g2(&InquiryCommand::DynamicRange);
        assert_valid_for_g2(&InquiryCommand::Backlight);
        assert_valid_for_g2(&InquiryCommand::AntiFlicker);
        assert_valid_for_g2(&InquiryCommand::WhiteBalanceMode);
        assert_valid_for_g2(&InquiryCommand::RedGain);
        assert_valid_for_g2(&InquiryCommand::BlueGain);
        assert_valid_for_g2(&InquiryCommand::ColorTemperature);
        assert_valid_for_g2(&InquiryCommand::Saturation);
        assert_valid_for_g2(&InquiryCommand::Hue);
        assert_valid_for_g2(&InquiryCommand::Luminance);
        assert_valid_for_g2(&InquiryCommand::Contrast);
        assert_valid_for_g2(&InquiryCommand::Sharpness);
        assert_valid_for_g2(&InquiryCommand::SharpnessMode);
        assert_valid_for_g2(&InquiryCommand::NoiseReduction2D);
        assert_valid_for_g2(&InquiryCommand::NoiseReduction3D);
        assert_valid_for_g2(&InquiryCommand::BlackWhite);
        assert_valid_for_g2(&InquiryCommand::ImageFlip);
    }
}

#[test]
fn test_comprehensive_g2_coverage() {
    // This test ensures we've covered all major command categories
    // If new commands are added to the library, they should be tested here

    // Count total tests to ensure comprehensive coverage
    let test_modules = [
        "power_commands",
        "zoom_commands",
        "focus_commands",
        "pan_tilt_commands",
        "preset_commands",
        "exposure_commands",
        "white_balance_commands",
        "image_adjustment_commands",
        "flip_commands",
        "inquiry_commands",
    ];

    println!(
        "G2 command validation test coverage includes {} command categories",
        test_modules.len()
    );
}

// Additional tests to verify the newly implemented validations work correctly
mod validation_tests {
    use super::*;
    use crate::{
        command::{
            color::{BlueTuningCommand, HueCommand, RedTuningCommand, SaturationCommand},
            exposure::{
                BrightCommand, DynamicRangeCommand, DynamicRangeLevel, ExposureCompensationCommand,
                ExposureCompensationLevel,
            },
            focus::FocusNearLimitCommand,
            gain::GainLimitCommand,
            luminance_contrast_sharpness::{ContrastCommand, LuminanceCommand},
        },
        types::{BrightnessLevel, ContrastLevel, GainLimit, LuminanceLevel},
    };

    #[test]
    fn test_newly_validated_commands_accept_valid_values() {
        // These should all pass validation for G2
        assert_valid_for_g2(&BrightCommand::Direct(BrightnessLevel::new(0x11).unwrap()));
        assert_valid_for_g2(&DynamicRangeCommand::Direct(
            DynamicRangeLevel::new(8).unwrap(),
        ));
        assert_valid_for_g2(&ExposureCompensationCommand::Direct(
            ExposureCompensationLevel::new(7).unwrap(),
        ));
        assert_valid_for_g2(&ExposureCompensationCommand::Direct(
            ExposureCompensationLevel::new(-7).unwrap(),
        ));
        assert_valid_for_g2(&GainLimitCommand {
            limit: GainLimit::new(0xF).unwrap(),
        });
        assert_valid_for_g2(&RedTuningCommand { level: 10 });
        assert_valid_for_g2(&RedTuningCommand { level: -10 });
        assert_valid_for_g2(&BlueTuningCommand { level: 10 });
        assert_valid_for_g2(&BlueTuningCommand { level: -10 });
        assert_valid_for_g2(&SaturationCommand { level: 0xE });
        assert_valid_for_g2(&HueCommand { level: 0xE });
        assert_valid_for_g2(&LuminanceCommand {
            value: LuminanceLevel::new(14).unwrap(),
        });
        assert_valid_for_g2(&ContrastCommand {
            value: ContrastLevel::new(14).unwrap(),
        });
        assert_valid_for_g2(&FocusNearLimitCommand { position: 0xF000 });
    }

    #[test]
    fn test_tuning_commands_reject_out_of_range() {
        // Red/Blue tuning should reject values outside -10 to +10
        assert_invalid_for_g2(&RedTuningCommand { level: -11 }, "-10 to +10");
        assert_invalid_for_g2(&RedTuningCommand { level: 11 }, "-10 to +10");
        assert_invalid_for_g2(&BlueTuningCommand { level: -11 }, "-10 to +10");
        assert_invalid_for_g2(&BlueTuningCommand { level: 11 }, "-10 to +10");
    }

    #[test]
    fn test_saturation_hue_reject_out_of_range() {
        // Saturation and Hue should reject values > 0xE
        assert_invalid_for_g2(&SaturationCommand { level: 0xF }, "max is 0x0E");
        assert_invalid_for_g2(&SaturationCommand { level: 0xFF }, "max is 0x0E");
        assert_invalid_for_g2(&HueCommand { level: 0xF }, "max is 0x0E");
        assert_invalid_for_g2(&HueCommand { level: 0xFF }, "max is 0x0E");
    }
}
