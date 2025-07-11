//! Enhanced model validation tests using the new testing infrastructure.
//!
//! These tests use MockTransport and ProtocolValidator to thoroughly test
//! camera model-specific command validation and behavior.

mod common;

use crate::common::{
    patterns, CommandFixtures, MockResponse, MockTransport, MockTransportBuilder,
    ProtocolValidator, ScenarioBuilder, ValidationMode,
};
use grafton_visca::{
    camera::{methods::*, Camera},
    command::{
        focus::{FocusCommand, FocusSpeed},
        pan_tilt::{PanTiltCommand, PanTiltSpeed},
        power::{Power, PowerCommand},
        preset::{PresetAction, PresetCommand},
        zoom::{ZoomCommand, ZoomSpeed},
        Command,
    },
    profiles::{CameraModel, PTZOpticsG2},
    timeout::CommandCategory,
    types::{FocusPosition, PresetId, ZoomPosition},
    Error,
};

/// Helper function to assert a command is valid for G2
fn assert_valid_for_g2<C: Command>(command: &C) {
    assert!(command.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
}

/// Helper function to assert a command is invalid for G2
fn assert_invalid_for_g2<C: Command>(command: &C) {
    assert!(command
        .validate_for_model(CameraModel::PTZOpticsG2)
        .is_err());
}

#[test]
fn test_power_commands_g2_validation() {
    // Use MockTransportBuilder for power command validation
    let mock = MockTransportBuilder::new()
        .connected(true)
        .expect(
            &patterns::power::ON,
            vec![
                MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
            ],
        )
        .expect(
            &patterns::power::STANDBY,
            vec![
                MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
            ],
        )
        .build();

    let mut validator = ProtocolValidator::new(ValidationMode::Strict);
    let mut camera = Camera::<PTZOpticsG2, _>::new(mock.clone());

    // Test that all power commands are valid for G2
    let power_on = PowerCommand { power: Power::On };
    let power_standby = PowerCommand {
        power: Power::Standby,
    };

    assert_valid_for_g2(&power_on);
    assert_valid_for_g2(&power_standby);

    // Execute commands through camera to test actual usage
    camera.power_on().unwrap();
    camera.power_off().unwrap();

    // Validate protocol compliance
    let sent_commands = mock.sent_history();
    for cmd in &sent_commands {
        validator.validate_command(cmd).unwrap();
    }

    // MockTransportBuilder automatically verifies expectations when dropped
}

#[test]
fn test_zoom_commands_g2_validation_comprehensive() {
    // Use MockTransportBuilder for zoom command validation
    let zoom_commands = CommandFixtures::zoom_commands();
    let mut builder = MockTransportBuilder::new().connected(true);

    for (_name, cmd_bytes) in &zoom_commands {
        builder = builder.expect(
            cmd_bytes,
            vec![
                MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
            ],
        );
    }

    // Test basic zoom commands
    assert_valid_for_g2(&ZoomCommand::Stop);
    assert_valid_for_g2(&ZoomCommand::TeleStandard);
    assert_valid_for_g2(&ZoomCommand::WideStandard);

    // Test variable speed zoom commands
    for speed in 0..=7 {
        let zoom_speed = ZoomSpeed::new(speed).unwrap();
        assert_valid_for_g2(&ZoomCommand::TeleVariable(zoom_speed));
        assert_valid_for_g2(&ZoomCommand::WideVariable(zoom_speed));
    }

    // Test zoom position commands for G2 limits
    // G2 has 20X optical zoom, max position is 0x7000
    let valid_positions = vec![
        0x0000, // Wide end
        0x3000, // Mid position
        0x7000, // 20X optical limit (G2 max)
    ];

    for pos in valid_positions {
        builder = builder.expect(
            &[
                0x81,
                0x01,
                0x04,
                0x47,
                ((pos >> 12) & 0x0F) as u8,
                ((pos >> 8) & 0x0F) as u8,
                ((pos >> 4) & 0x0F) as u8,
                (pos & 0x0F) as u8,
                0xFF,
            ],
            vec![
                MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
            ],
        );

        let zoom_pos = ZoomPosition::new(pos).unwrap();
        assert_valid_for_g2(&ZoomCommand::Position(zoom_pos));
    }

    let mock = builder.build();
    let mut camera = Camera::<PTZOpticsG2, _>::new(mock.clone());

    // Execute commands to verify integration
    camera.zoom_stop().unwrap();
    camera.zoom_in().unwrap();
    camera.zoom_out().unwrap();
    camera.zoom_in_variable(ZoomSpeed::new(5).unwrap()).unwrap();
    camera
        .zoom_out_variable(ZoomSpeed::new(3).unwrap())
        .unwrap();

    // Test zoom positions
    for pos in [0x0000, 0x3000, 0x7000] {
        let normalized = pos as f32 / 0x7000 as f32;
        camera.zoom_absolute(normalized).unwrap();
    }

    // MockTransportBuilder automatically verifies expectations when dropped
}

#[test]
fn test_pan_tilt_commands_g2_validation() {
    let mut mock = MockTransport::new();

    // Test pan/tilt speed validation for G2
    for pan_speed in 0x01..=0x18 {
        // G2 max pan speed
        for tilt_speed in 0x01..=0x14 {
            // G2 max tilt speed
            let pan_speed_obj = PanTiltSpeed::new(pan_speed).unwrap();
            let tilt_speed_obj = PanTiltSpeed::new(tilt_speed).unwrap();

            let cmd = PanTiltCommand::Variable {
                pan_direction: grafton_visca::command::pan_tilt::PanDirection::Right,
                tilt_direction: grafton_visca::command::pan_tilt::TiltDirection::Up,
                pan_speed: pan_speed_obj,
                tilt_speed: tilt_speed_obj,
            };

            assert_valid_for_g2(&cmd);
        }
    }

    // Test invalid speeds (beyond G2 limits)
    let invalid_pan_speed = PanTiltSpeed::new(0x19); // Beyond G2 pan max
    let invalid_tilt_speed = PanTiltSpeed::new(0x15); // Beyond G2 tilt max

    // These should fail at speed creation for G2, but let's test if they existed
    assert!(invalid_pan_speed.is_err());
    assert!(invalid_tilt_speed.is_err());

    // Test home command
    mock.expect_command(&patterns::pan_tilt::HOME)
        .described_as("pan/tilt home - G2 validation")
        .will_ack(1)
        .then_complete(1);

    let mut camera = Camera::<PTZOpticsG2, _>::new(mock.clone());
    camera.pan_tilt_home().unwrap();

    // MockTransportBuilder automatically verifies expectations when dropped
}

#[test]
fn test_focus_commands_g2_validation() {
    let mut mock = MockTransport::new();

    // Test basic focus commands
    assert_valid_for_g2(&FocusCommand::Stop);
    assert_valid_for_g2(&FocusCommand::Far);
    assert_valid_for_g2(&FocusCommand::Near);
    assert_valid_for_g2(&FocusCommand::Auto);
    assert_valid_for_g2(&FocusCommand::Manual);

    // Test focus speeds
    for speed in 0..=7 {
        let focus_speed = FocusSpeed::new(speed).unwrap();
        assert_valid_for_g2(&FocusCommand::FarWithSpeed(focus_speed));
        assert_valid_for_g2(&FocusCommand::NearWithSpeed(focus_speed));
    }

    // Test focus position validation for G2
    // G2 uses range 0x1000-0xF000
    let valid_positions = vec![
        0x1000, // Minimum
        0x8000, // Middle
        0xF000, // Maximum
    ];

    for pos in valid_positions {
        mock.expect_command(&[
            0x81,
            0x01,
            0x04,
            0x48,
            ((pos >> 12) & 0x0F) as u8,
            ((pos >> 8) & 0x0F) as u8,
            ((pos >> 4) & 0x0F) as u8,
            (pos & 0x0F) as u8,
            0xFF,
        ])
        .described_as(&format!("focus position 0x{:04X}", pos))
        .will_ack(1)
        .then_complete(1);

        let focus_pos = FocusPosition::new(pos).unwrap();
        assert_valid_for_g2(&FocusCommand::Position(focus_pos));
    }

    let mut camera = Camera::<PTZOpticsG2, _>::new(mock.clone());

    // Execute focus commands to verify integration
    for pos in [0x1000, 0x8000, 0xF000] {
        let normalized = (pos - 0x1000) as f32 / (0xF000 - 0x1000) as f32;
        camera.focus_absolute(normalized).unwrap();
    }

    // MockTransportBuilder automatically verifies expectations when dropped
}

#[test]
fn test_preset_commands_g2_validation() {
    let mut mock = MockTransport::new();

    // G2 supports presets 0-127
    let valid_presets = vec![0, 1, 2, 50, 99, 127];

    for preset_num in valid_presets {
        // Set expectation for preset set
        mock.expect_command(&[0x81, 0x01, 0x04, 0x3F, 0x01, preset_num, 0xFF])
            .described_as(&format!("preset set {}", preset_num))
            .will_ack(1)
            .then_complete(1);

        // Set expectation for preset recall
        mock.expect_command(&[0x81, 0x01, 0x04, 0x3F, 0x02, preset_num, 0xFF])
            .described_as(&format!("preset recall {}", preset_num))
            .will_ack(1)
            .then_complete(1);

        let preset_id = PresetId::new(preset_num).unwrap();
        let set_cmd = PresetCommand {
            action: PresetAction::Set,
            preset_number: preset_id,
        };
        let recall_cmd = PresetCommand {
            action: PresetAction::Recall,
            preset_number: preset_id,
        };

        assert_valid_for_g2(&set_cmd);
        assert_valid_for_g2(&recall_cmd);
    }

    let mut camera = Camera::<PTZOpticsG2, _>::new(mock.clone());

    // Execute preset commands
    for preset_num in [0, 1, 2, 50, 99, 127] {
        let preset_id = PresetId::new(preset_num).unwrap();
        camera.preset_set(preset_id).unwrap();
        camera.preset_recall(preset_id).unwrap();
    }

    // MockTransportBuilder automatically verifies expectations when dropped
}

#[test]
fn test_command_categories_for_g2() {
    // Test that commands have appropriate categories for timeout handling
    assert_eq!(
        PowerCommand { power: Power::On }.command_category(),
        CommandCategory::Quick
    );
    assert_eq!(
        ZoomCommand::Stop.command_category(),
        CommandCategory::Movement
    );
    assert_eq!(
        FocusCommand::Auto.command_category(),
        CommandCategory::Movement
    );

    let preset_cmd = PresetCommand {
        action: PresetAction::Recall,
        preset_number: PresetId::new(1).unwrap(),
    };
    assert_eq!(preset_cmd.command_category(), CommandCategory::Preset);
}

#[test]
fn test_model_specific_behavior_simulation() {
    // Create a scenario that simulates G2-specific timing behavior
    let scenario = ScenarioBuilder::new("G2 Behavior Simulation")
        .description("Test G2-specific timing and response patterns")
        .expect_power_on()
        .expect_home()
        .expect_preset_recall(1)
        .build();

    let mut mock = MockTransport::new();
    scenario.apply_to(&mut mock).unwrap();

    let mut camera = Camera::<PTZOpticsG2, _>::new(mock.clone());

    // Execute the G2-specific sequence
    camera.power_on().unwrap();
    camera.pan_tilt_home().unwrap();
    camera.preset_recall(PresetId::new(1).unwrap()).unwrap();

    // MockTransportBuilder automatically verifies expectations when dropped
}

#[test]
fn test_protocol_validation_with_g2_commands() {
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);

    // Test that all G2 commands pass strict protocol validation
    let commands = vec![
        PowerCommand { power: Power::On }.to_bytes().unwrap(),
        ZoomCommand::Stop.to_bytes().unwrap(),
        FocusCommand::Auto.to_bytes().unwrap(),
    ];

    for cmd in commands {
        validator.validate_command(&cmd).unwrap();
    }

    // Ensure validator state is clean
    assert!(validator.all_sockets_free());

    let summary = validator.get_summary();
    assert_eq!(summary.commands_sent, 3);
    assert_eq!(summary.responses_received, 0); // No responses in this test
}

#[test]
fn test_edge_cases_for_g2_validation() {
    // Test boundary conditions specific to G2

    // Zoom position at exact G2 limit
    let max_zoom = ZoomPosition::new(0x7000).unwrap();
    assert_valid_for_g2(&ZoomCommand::Position(max_zoom));

    // Focus position at G2 boundaries
    let min_focus = FocusPosition::new(0x1000).unwrap();
    let max_focus = FocusPosition::new(0xF000).unwrap();
    assert_valid_for_g2(&FocusCommand::Position(min_focus));
    assert_valid_for_g2(&FocusCommand::Position(max_focus));

    // Maximum preset number for G2
    let max_preset = PresetId::new(127).unwrap();
    let preset_cmd = PresetCommand {
        action: PresetAction::Set,
        preset_number: max_preset,
    };
    assert_valid_for_g2(&preset_cmd);
}

#[test]
fn test_invalid_commands_for_g2() {
    // Test commands that should be invalid for G2 model
    // (Most validation happens at type creation, but test what we can)

    // These would fail at construction time, which is the intended behavior
    assert!(ZoomPosition::new(0x7001).is_err()); // Beyond G2 zoom limit
    assert!(FocusPosition::new(0x0FFF).is_err()); // Below G2 focus limit
    assert!(FocusPosition::new(0xF001).is_err()); // Above G2 focus limit
    assert!(PresetId::new(128).is_err()); // Beyond G2 preset limit
}

#[test]
fn test_comprehensive_g2_command_sequence() {
    // Test a realistic sequence of G2 operations with validation
    let scenario = ScenarioBuilder::new("Comprehensive G2 Test")
        .description("Full G2 camera operation sequence")
        .expect_power_on()
        .expect_home()
        .expect_command(
            &ZoomCommand::Position(ZoomPosition::new(0x4000).unwrap())
                .to_bytes()
                .unwrap(),
            vec![
                MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                MockResponse::Immediate(patterns::responses::COMPLETE_1.to_vec()),
            ],
        )
        .expect_command(
            &FocusCommand::Auto.to_bytes().unwrap(),
            vec![
                MockResponse::Immediate(patterns::responses::ACK_2.to_vec()),
                MockResponse::Immediate(patterns::responses::COMPLETE_2.to_vec()),
            ],
        )
        .expect_preset_recall(5)
        .build();

    let mut mock = MockTransport::new();
    scenario.apply_to(&mut mock).unwrap();

    let mut camera = Camera::<PTZOpticsG2, _>::new(mock.clone());

    // Execute the full sequence
    camera.power_on().unwrap();
    camera.pan_tilt_home().unwrap();
    camera.zoom_absolute(0.5).unwrap(); // Mid zoom (0x4000)
    camera.focus_auto().unwrap();
    camera.preset_recall(PresetId::new(5).unwrap()).unwrap();

    // MockTransportBuilder automatically verifies expectations when dropped
}
