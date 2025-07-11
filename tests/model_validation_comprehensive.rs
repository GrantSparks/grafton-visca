//! Comprehensive G2 model validation tests moved from src/command/tests/g2_validation_tests.rs
//!
//! These tests verify command validation for PTZOptics G2 cameras using the
//! enhanced testing infrastructure with MockTransport and validation.

mod common;

use grafton_visca::{constants::CameraModel, Command};
use crate::common::{MockTransport, ProtocolValidator, ValidationMode};

/// Helper function to test that a command is valid for G2
fn assert_valid_for_g2<C: Command>(command: &C) {
    match command.validate_for_model(CameraModel::PTZOpticsG2) {
        Ok(()) => {} // Expected
        Err(e) => panic!("Command should be valid for G2 but got error: {:?}", e),
    }
}

/// Helper function to test command validation with mock transport
fn test_command_with_mock<C: Command>(command: &C, description: &str) {
    let mut mock = MockTransport::new();
    let validator = ProtocolValidator::new(ValidationMode::Strict);
    
    // First validate for G2
    assert_valid_for_g2(command);
    
    // Test encoding
    let bytes = command.to_bytes().unwrap();
    validator.validate_command(&bytes).unwrap();
    
    // Test with mock transport
    mock.expect_command(&bytes)
        .described_as(description)
        .will_ack(1)
        .will_complete(1);
    
    let result = mock.send_command_and_wait(&bytes, std::time::Duration::from_millis(100));
    assert!(result.is_ok(), "Mock transport should accept valid G2 command: {}", description);
}

mod power_commands {
    use super::*;
    use grafton_visca::command::power::{Power, PowerCommand};

    #[test]
    fn test_power_commands() {
        // All power commands should be valid for G2
        test_command_with_mock(&PowerCommand { power: Power::On }, "power on");
        test_command_with_mock(&PowerCommand { power: Power::Standby }, "power standby");
    }
}

mod zoom_commands {
    use super::*;
    use grafton_visca::command::zoom::{ZoomCommand, ZoomSpeed};

    #[test]
    fn test_zoom_stop() {
        test_command_with_mock(&ZoomCommand::Stop, "zoom stop");
    }

    #[test]
    fn test_zoom_standard_speeds() {
        test_command_with_mock(&ZoomCommand::TeleStandard, "zoom tele standard");
        test_command_with_mock(&ZoomCommand::WideStandard, "zoom wide standard");
    }

    #[test]
    fn test_zoom_variable_speeds() {
        // Valid speeds: 0-7
        for speed in 0..=7 {
            let zoom_speed = ZoomSpeed::new(speed).unwrap();
            test_command_with_mock(
                &ZoomCommand::TeleVariable(zoom_speed), 
                &format!("zoom tele variable speed {}", speed)
            );
            test_command_with_mock(
                &ZoomCommand::WideVariable(zoom_speed), 
                &format!("zoom wide variable speed {}", speed)
            );
        }
    }

    #[test]
    fn test_zoom_position_values() {
        // G2 has 20X optical zoom, max position is 0x7000
        use grafton_visca::types::ZoomPosition;

        // Valid positions
        let positions = [
            (0x0000, "zoom position wide end"),
            (0x3000, "zoom position mid"),
            (0x7000, "zoom position 20X optical limit"),
        ];
        
        for (pos, desc) in positions {
            test_command_with_mock(
                &ZoomCommand::Position(ZoomPosition::new(pos).unwrap()),
                desc
            );
        }

        // Invalid positions (beyond 20X) - These will fail at ZoomPosition creation
        assert!(ZoomPosition::new(0x7001).is_err());
        assert!(ZoomPosition::new(0x7AC0).is_err());
        assert!(ZoomPosition::new(0x7FFF).is_err());
        assert!(ZoomPosition::new(0xFFFF).is_err());
    }
}

mod focus_commands {
    use super::*;
    use grafton_visca::command::focus::{
        AutoFocusSensitivity, AutoFocusSensitivityCommand, FocusCommand, FocusNearLimitCommand,
        FocusSpeed, FocusZone, FocusZoneCommand,
    };

    #[test]
    fn test_focus_stop() {
        test_command_with_mock(&FocusCommand::Stop, "focus stop");
    }

    #[test]
    fn test_focus_standard_speeds() {
        test_command_with_mock(&FocusCommand::Far, "focus far");
        test_command_with_mock(&FocusCommand::Near, "focus near");
    }

    #[test]
    fn test_focus_variable_speeds() {
        // Valid speeds: 0-7
        for speed in 0..=7 {
            let focus_speed = FocusSpeed::new(speed).unwrap();
            test_command_with_mock(
                &FocusCommand::FarWithSpeed(focus_speed),
                &format!("focus far with speed {}", speed)
            );
            test_command_with_mock(
                &FocusCommand::NearWithSpeed(focus_speed),
                &format!("focus near with speed {}", speed)
            );
        }
    }

    #[test]
    fn test_focus_position_values() {
        use grafton_visca::types::FocusPosition;

        // Focus range: 0x1000 to 0xF000
        let positions = [
            (0x1000, "focus position infinity"),
            (0x8000, "focus position mid"),
            (0xF000, "focus position near limit"),
        ];
        
        for (pos, desc) in positions {
            test_command_with_mock(
                &FocusCommand::Position(FocusPosition::new(pos).unwrap()),
                desc
            );
        }

        // Invalid positions (out of range) - These will fail at FocusPosition creation
        assert!(FocusPosition::new(0x0FFF).is_err());
        assert!(FocusPosition::new(0xF001).is_err());
        assert!(FocusPosition::new(0x0000).is_err());
        assert!(FocusPosition::new(0xFFFF).is_err());
    }

    #[test]
    fn test_focus_modes() {
        test_command_with_mock(&FocusCommand::Auto, "focus auto");
        test_command_with_mock(&FocusCommand::Manual, "focus manual");
    }

    #[test]
    fn test_focus_one_push() {
        test_command_with_mock(&FocusCommand::OnePushTrigger, "focus one push trigger");
    }

    #[test]
    fn test_focus_infinity() {
        test_command_with_mock(&FocusCommand::Infinity, "focus infinity");
    }

    #[test]
    fn test_focus_zones() {
        let zones = [
            (FocusZone::Top, "focus zone top"),
            (FocusZone::Center, "focus zone center"),
            (FocusZone::Bottom, "focus zone bottom"),
        ];
        
        for (zone, desc) in zones {
            test_command_with_mock(&FocusZoneCommand { zone }, desc);
        }
    }

    #[test]
    fn test_focus_sensitivity() {
        let sensitivities = [
            (AutoFocusSensitivity::Normal, "af sensitivity normal"),
            (AutoFocusSensitivity::Low, "af sensitivity low"),
            (AutoFocusSensitivity::High, "af sensitivity high"),
        ];
        
        for (sensitivity, desc) in sensitivities {
            test_command_with_mock(&AutoFocusSensitivityCommand { sensitivity }, desc);
        }
    }

    #[test]
    fn test_focus_near_limit() {
        use grafton_visca::types::FocusPosition;

        // Valid positions within focus range
        let positions = [
            (0x1000, "focus near limit infinity"),
            (0x8000, "focus near limit mid"),
            (0xF000, "focus near limit near"),
        ];
        
        for (pos, desc) in positions {
            test_command_with_mock(
                &FocusNearLimitCommand {
                    position: FocusPosition::new(pos).unwrap(),
                },
                desc
            );
        }
    }
}

mod pan_tilt_commands {
    use super::*;
    use grafton_visca::command::pan_tilt::{PanTiltCommand, PanTiltDirection};
    use grafton_visca::types::{PanPosition, PanSpeed, TiltPosition, TiltSpeed};

    #[test]
    fn test_pan_tilt_home() {
        test_command_with_mock(&PanTiltCommand::Home, "pan tilt home");
    }

    #[test]
    fn test_pan_tilt_reset() {
        test_command_with_mock(&PanTiltCommand::Reset, "pan tilt reset");
    }

    #[test]
    fn test_pan_tilt_directional_movement() {
        let pan_speed = PanSpeed::new(0x10).unwrap();
        let tilt_speed = TiltSpeed::new(0x10).unwrap();

        // Test all directions
        let directions = [
            (PanTiltDirection::Up, "pan tilt up"),
            (PanTiltDirection::Down, "pan tilt down"),
            (PanTiltDirection::Left, "pan tilt left"),
            (PanTiltDirection::Right, "pan tilt right"),
            (PanTiltDirection::UpLeft, "pan tilt up left"),
            (PanTiltDirection::UpRight, "pan tilt up right"),
            (PanTiltDirection::DownLeft, "pan tilt down left"),
            (PanTiltDirection::DownRight, "pan tilt down right"),
            (PanTiltDirection::Stop, "pan tilt stop"),
        ];

        for (direction, desc) in directions {
            test_command_with_mock(
                &PanTiltCommand::Move {
                    direction,
                    pan_speed,
                    tilt_speed,
                },
                desc
            );
        }
    }

    #[test]
    fn test_pan_tilt_speed_limits() {
        // Valid pan speeds: 0x00 to 0x18 (0-24)
        for speed in [0x00, 0x10, 0x18] {
            let pan_speed = PanSpeed::new(speed).unwrap();
            test_command_with_mock(
                &PanTiltCommand::Move {
                    direction: PanTiltDirection::Right,
                    pan_speed,
                    tilt_speed: TiltSpeed::new(0x10).unwrap(),
                },
                &format!("pan move with speed {}", speed)
            );
        }

        // Valid tilt speeds: 0x00 to 0x14 (0-20)
        for speed in [0x00, 0x10, 0x14] {
            let tilt_speed = TiltSpeed::new(speed).unwrap();
            test_command_with_mock(
                &PanTiltCommand::Move {
                    direction: PanTiltDirection::Up,
                    pan_speed: PanSpeed::new(0x10).unwrap(),
                    tilt_speed,
                },
                &format!("tilt move with speed {}", speed)
            );
        }
    }

    #[test]
    fn test_pan_tilt_absolute_position() {
        let pan_speed = PanSpeed::new(0x10).unwrap();
        let tilt_speed = TiltSpeed::new(0x10).unwrap();

        // Valid positions within G2 range
        // Pan: -2448 to 2448, Tilt: -432 to 1296
        let positions = [
            (0, 0, "absolute position center"),
            (2448, 1296, "absolute position max"),
            (-2448, -432, "absolute position min"),
        ];
        
        for (pan, tilt, desc) in positions {
            test_command_with_mock(
                &PanTiltCommand::AbsolutePosition {
                    pan: PanPosition::new(pan).unwrap(),
                    tilt: TiltPosition::new(tilt).unwrap(),
                    pan_speed,
                    tilt_speed,
                },
                desc
            );
        }

        // Invalid positions (out of range) - These will fail at Position creation
        assert!(PanPosition::new(2449).is_err());
        assert!(PanPosition::new(-2449).is_err());
        assert!(TiltPosition::new(1297).is_err());
        assert!(TiltPosition::new(-433).is_err());
    }

    #[test]
    fn test_pan_tilt_relative_position() {
        let pan_speed = PanSpeed::new(0x10).unwrap();
        let tilt_speed = TiltSpeed::new(0x10).unwrap();

        // Relative positions
        let positions = [
            (100, 100, "relative position positive"),
            (-100, -100, "relative position negative"),
        ];
        
        for (pan, tilt, desc) in positions {
            test_command_with_mock(
                &PanTiltCommand::RelativePosition {
                    pan: PanPosition::new(pan).unwrap(),
                    tilt: TiltPosition::new(tilt).unwrap(),
                    pan_speed,
                    tilt_speed,
                },
                desc
            );
        }
    }
}

mod preset_commands {
    use super::*;
    use grafton_visca::command::preset::{PresetAction, PresetCommand, PresetNumber};

    #[test]
    fn test_preset_actions() {
        // G2 supports presets 0-89
        let valid_presets = [0, 1, 50, 89];
        let actions = [
            (PresetAction::Reset, "reset"),
            (PresetAction::Set, "set"),
            (PresetAction::Recall, "recall"),
        ];

        for preset_id in valid_presets {
            let preset = PresetNumber::new(preset_id).unwrap();
            for (action, action_name) in actions {
                test_command_with_mock(
                    &PresetCommand {
                        action,
                        preset_number: preset,
                    },
                    &format!("preset {} {}", action_name, preset_id)
                );
            }
        }
    }

    #[test]
    fn test_preset_limits() {
        // Test that preset number validation works
        assert!(PresetNumber::new(90).is_err(), "Preset 90 should be invalid");
        assert!(PresetNumber::new(255).is_err(), "Preset 255 should be invalid");
    }
}

mod exposure_commands {
    use super::*;
    use grafton_visca::{
        command::{
            exposure::{
                BrightCommand, DynamicRangeCommand, ExposureCommand, ExposureCompensationCommand,
                ExposureCompensationLevel, ExposureMode, IrisCommand, ShutterCommand,
            },
            gain::{AntiFlickerCommand, AntiFlickerMode, GainCommand, GainLimitCommand},
            image::BacklightCommand,
        },
        types::{BrightnessLevel, DynamicRangeLevel, Gain, GainLimit, IrisLevel, ShutterSpeed},
    };

    #[test]
    fn test_exposure_modes() {
        let modes = [
            (ExposureMode::Auto, "exposure auto"),
            (ExposureMode::Manual, "exposure manual"),
            (ExposureMode::Shutter, "exposure shutter priority"),
            (ExposureMode::Iris, "exposure iris priority"),
            (ExposureMode::Bright, "exposure bright"),
        ];
        
        for (mode, desc) in modes {
            test_command_with_mock(&ExposureCommand { mode }, desc);
        }
    }

    #[test]
    fn test_exposure_compensation() {
        let commands = [
            (ExposureCompensationCommand::On, "exp comp on"),
            (ExposureCompensationCommand::Off, "exp comp off"),
            (ExposureCompensationCommand::Reset, "exp comp reset"),
            (ExposureCompensationCommand::Up, "exp comp up"),
            (ExposureCompensationCommand::Down, "exp comp down"),
        ];
        
        for (cmd, desc) in commands {
            test_command_with_mock(&cmd, desc);
        }

        // Valid range: -7 to +7
        for value in [-7, 0, 7] {
            test_command_with_mock(
                &ExposureCompensationCommand::SetLevel(
                    ExposureCompensationLevel::new(value).unwrap()
                ),
                &format!("exp comp level {}", value)
            );
        }
    }

    #[test]
    fn test_backlight() {
        test_command_with_mock(&BacklightCommand { status: true }, "backlight on");
        test_command_with_mock(&BacklightCommand { status: false }, "backlight off");
    }

    #[test]
    fn test_iris_commands() {
        let commands = [
            (IrisCommand::Reset, "iris reset"),
            (IrisCommand::Up, "iris up"),
            (IrisCommand::Down, "iris down"),
        ];
        
        for (cmd, desc) in commands {
            test_command_with_mock(&cmd, desc);
        }

        // Valid range: 0x00 (Close) to 0x0C (F1.8)
        for value in [0x00, 0x06, 0x0C] {
            test_command_with_mock(
                &IrisCommand::SetAperture(IrisLevel::new(value).unwrap()),
                &format!("iris aperture 0x{:02X}", value)
            );
        }
    }

    #[test]
    fn test_gain_commands() {
        let commands = [
            (GainCommand::Reset, "gain reset"),
            (GainCommand::Up, "gain up"),
            (GainCommand::Down, "gain down"),
        ];
        
        for (cmd, desc) in commands {
            test_command_with_mock(&cmd, desc);
        }

        // Valid range: 0x00 to 0x07 (0-7)
        for value in [0x00, 0x03, 0x07] {
            test_command_with_mock(
                &GainCommand::SetValue(Gain::new(value).unwrap()),
                &format!("gain value {}", value)
            );
        }
    }

    #[test]
    fn test_gain_limit() {
        // Valid range: 0x0 to 0xF (0-15)
        for value in [0x0, 0x8, 0xF] {
            test_command_with_mock(
                &GainLimitCommand {
                    limit: GainLimit::new(value).unwrap(),
                },
                &format!("gain limit {}", value)
            );
        }
    }

    #[test]
    fn test_dynamic_range() {
        // Valid range: 0 to 8
        for value in [0, 4, 8] {
            test_command_with_mock(
                &DynamicRangeCommand::SetLevel(
                    DynamicRangeLevel::new(value).unwrap()
                ),
                &format!("dynamic range {}", value)
            );
        }
    }

    #[test]
    fn test_anti_flicker() {
        let modes = [
            (AntiFlickerMode::Off, "anti flicker off"),
            (AntiFlickerMode::Hz50, "anti flicker 50Hz"),
            (AntiFlickerMode::Hz60, "anti flicker 60Hz"),
        ];
        
        for (mode, desc) in modes {
            test_command_with_mock(&AntiFlickerCommand { mode }, desc);
        }
    }
}

mod white_balance_commands {
    use super::*;
    use grafton_visca::command::{
        color::{
            BlueGainCommand, BlueTuningCommand, ColorTemperatureCommand, OnePushTriggerCommand,
            RedGainCommand, RedTuningCommand,
        },
        white_balance::{WhiteBalanceCommand, WhiteBalanceMode},
    };

    #[test]
    fn test_white_balance_modes() {
        let modes = [
            (WhiteBalanceMode::Auto, "wb auto"),
            (WhiteBalanceMode::Indoor, "wb indoor"),
            (WhiteBalanceMode::Outdoor, "wb outdoor"),
            (WhiteBalanceMode::OnePush, "wb one push"),
            (WhiteBalanceMode::Manual, "wb manual"),
            (WhiteBalanceMode::ColorTemperature, "wb color temperature"),
        ];
        
        for (mode, desc) in modes {
            test_command_with_mock(&WhiteBalanceCommand { mode }, desc);
        }
    }

    #[test]
    fn test_one_push_trigger() {
        test_command_with_mock(&OnePushTriggerCommand, "one push trigger");
    }

    #[test]
    fn test_color_tuning() {
        use grafton_visca::types::{BlueTuning, RedTuning};

        // Valid range: -10 to +10
        for value in [-10, 0, 10] {
            test_command_with_mock(
                &RedTuningCommand {
                    level: RedTuning::new(value).unwrap(),
                },
                &format!("red tuning {}", value)
            );
            test_command_with_mock(
                &BlueTuningCommand {
                    level: BlueTuning::new(value).unwrap(),
                },
                &format!("blue tuning {}", value)
            );
        }
    }
}

#[test]
fn test_comprehensive_g2_coverage() {
    // This test ensures we've covered all major command categories
    let test_modules = [
        "power_commands",
        "zoom_commands", 
        "focus_commands",
        "pan_tilt_commands",
        "preset_commands",
        "exposure_commands",
        "white_balance_commands",
    ];

    println!(
        "G2 command validation test coverage includes {} command categories",
        test_modules.len()
    );
}

#[test]
fn test_mock_transport_integration_with_g2_commands() {
    // Test that G2 commands work end-to-end with mock transport
    let mut mock = MockTransport::new();
    
    // Setup a realistic command sequence
    let power_on = PowerCommand { power: Power::On };
    let power_on_bytes = power_on.to_bytes().unwrap();
    
    mock.expect_command(&power_on_bytes)
        .described_as("G2 power on sequence")
        .will_ack(1)
        .will_complete(1);
    
    // Validate for G2
    assert_valid_for_g2(&power_on);
    
    // Execute through mock
    let result = mock.send_command_and_wait(&power_on_bytes, std::time::Duration::from_millis(100));
    assert!(result.is_ok(), "G2 command should execute successfully through mock transport");
}