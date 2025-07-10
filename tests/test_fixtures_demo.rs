//! Demonstration of test fixtures and generators usage.
//!
//! This test file shows how to use CommandFixtures and generators
//! from the test_utils module for comprehensive testing.

#![cfg(test)]

mod common;

use grafton_visca::{
    camera::{methods::*, Camera},
    command::Command,
    profiles::PTZOpticsG2,
    Result,
};
use crate::common::{
    MockTransport, ProtocolValidator, ValidationMode,
    patterns, ScenarioBuilder, MockResponse, CommandFixtures, generators,
};
use std::time::Duration;

#[test]
fn test_all_power_commands_with_fixtures() {
    let mut mock = MockTransport::new();
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);
    
    // Get all power command fixtures
    let power_cmds = CommandFixtures::power_commands();
    
    // Set up expectations for each command
    for (name, cmd_bytes) in &power_cmds {
        mock.expect_command(cmd_bytes)
            .described_as(name)
            .will_ack(1)
            .then_complete(1);
            
        // Validate each command
        assert!(
            validator.validate_command(cmd_bytes).is_ok(),
            "Command '{}' should be protocol compliant", 
            name
        );
    }
    
    let mut camera = Camera::<PTZOpticsG2, _>::new(mock.clone());
    
    // Execute the commands
    camera.power_on().unwrap();
    camera.power_off().unwrap();
    
    // Verify all expectations were met
    mock.verify().unwrap();
}

#[test]
fn test_zoom_commands_with_fixtures() {
    let mut mock = MockTransport::new();
    let zoom_cmds = CommandFixtures::zoom_commands();
    
    // Set up mock for all zoom commands
    for (name, cmd_bytes) in &zoom_cmds {
        mock.expect_command(cmd_bytes)
            .described_as(name)
            .will_ack(1)
            .then_complete(1);
    }
    
    let mut camera = Camera::<PTZOpticsG2, _>::new(mock.clone());
    
    // Execute zoom commands
    camera.zoom_stop().unwrap();
    camera.zoom_in().unwrap();
    camera.zoom_out().unwrap();
    // Note: Variable speed zoom is not available in the current API
    // Using standard zoom commands instead
    camera.zoom_in().unwrap();
    camera.zoom_out().unwrap();
    
    mock.verify().unwrap();
}

#[test]
fn test_preset_commands_with_fixtures() {
    let preset_cmds = CommandFixtures::preset_commands();
    let mut mock = MockTransport::new();
    
    // Set up expectations for preset commands
    for (name, cmd_bytes) in &preset_cmds {
        mock.expect_command(cmd_bytes)
            .described_as(name)
            .will_ack(1)
            .then_complete(1);
    }
    
    let mut camera = Camera::<PTZOpticsG2, _>::new(mock.clone());
    
    // Test preset operations
    camera.preset_set(1.into()).unwrap();
    camera.preset_recall(1.into()).unwrap();
    camera.preset_set(2.into()).unwrap();
    camera.preset_recall(2.into()).unwrap();
    
    mock.verify().unwrap();
}

#[test]
fn test_edge_case_commands() {
    let edge_cases = CommandFixtures::edge_case_commands();
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);
    
    for (name, cmd_bytes) in edge_cases {
        println!("Testing edge case: {}", name);
        
        // Validate protocol compliance even for edge cases
        let result = validator.validate_command(&cmd_bytes);
        
        // Some edge cases might be intentionally invalid
        if result.is_err() {
            println!("  Edge case '{}' is invalid: {:?}", name, result);
        } else {
            println!("  Edge case '{}' is valid", name);
        }
    }
}

#[test]
fn test_zoom_positions_with_generator() {
    let zoom_positions = generators::zoom_positions();
    let mut mock = MockTransport::new();
    
    // Test a subset of zoom positions
    for position in zoom_positions.iter().take(5) {
        // Build expected command bytes for zoom position
        let cmd_bytes = vec![
            0x81, 0x01, 0x04, 0x47,
            ((position >> 12) & 0x0F) as u8,
            ((position >> 8) & 0x0F) as u8,
            ((position >> 4) & 0x0F) as u8,
            (position & 0x0F) as u8,
            0xFF
        ];
        
        mock.expect_command(&cmd_bytes)
            .described_as(&format!("zoom to position 0x{:04X}", position))
            .will_ack(1)
            .then_complete(1);
    }
    
    let mut camera = Camera::<PTZOpticsG2, _>::new(mock.clone());
    
    // Execute zoom position commands
    for position in zoom_positions.iter().take(5) {
        // Convert position to normalized value (0.0 - 1.0)
        let normalized = *position as f32 / 0x4000 as f32;
        camera.zoom_absolute(normalized).unwrap();
    }
    
    mock.verify().unwrap();
}

#[test]
fn test_pan_tilt_positions_with_generator() {
    let positions = generators::pan_tilt_positions();
    let speeds = generators::speed_values();
    
    // Create a scenario for pan/tilt testing
    let mut scenario = ScenarioBuilder::new("Pan/Tilt Position Test");
    
    // Add expectations for a few position movements
    for ((pan, tilt), &speed) in positions.iter().take(3).zip(speeds.iter()) {
        let cmd_bytes = build_pan_tilt_absolute_command(*pan, *tilt, speed, speed);
        
        scenario = scenario.expect_command(
            &cmd_bytes,
            vec![
                MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                MockResponse::Delayed(patterns::responses::COMPLETE_1.to_vec(), Duration::from_millis(200))
            ]
        );
    }
    
    let mut mock = MockTransport::new();
    scenario.build().apply_to(&mut mock).unwrap();
    
    let mut camera = Camera::<PTZOpticsG2, _>::new(mock.clone());
    
    // Execute movements
    for ((pan, tilt), &speed) in positions.iter().take(3).zip(speeds.iter()) {
        // Convert i16 degrees to f32
        camera.pan_tilt_absolute(*pan as f32, *tilt as f32, speed).unwrap();
    }
    
    mock.verify().unwrap();
}

#[test]
fn test_comprehensive_command_sequence() {
    // Test a realistic sequence of camera operations
    let scenario = ScenarioBuilder::new("Comprehensive Camera Test")
        .description("Initialize camera and perform various operations")
        // Power on
        .expect_power_on()
        // Home position
        .expect_home()
        // Zoom operations
        .expect_command(
            &patterns::zoom::STOP,
            vec![
                MockResponse::Immediate(patterns::responses::ACK_1.to_vec()),
                MockResponse::Delayed(patterns::responses::COMPLETE_1.to_vec(), Duration::from_millis(50))
            ]
        )
        // Preset recall
        .expect_preset_recall(1)
        // Focus operations from fixtures
        .expect_command(
            &CommandFixtures::focus_commands()[0].1, // Auto focus
            vec![
                MockResponse::Immediate(patterns::responses::ACK_2.to_vec()),
                MockResponse::Delayed(patterns::responses::COMPLETE_2.to_vec(), Duration::from_millis(100))
            ]
        )
        .build();
    
    let mut mock = MockTransport::new();
    scenario.apply_to(&mut mock).unwrap();
    
    let mut camera = Camera::<PTZOpticsG2, _>::new(mock.clone());
    
    // Execute the sequence
    camera.power_on().unwrap();
    camera.pan_tilt_home().unwrap();
    camera.zoom_stop().unwrap();
    camera.preset_recall(1.into()).unwrap();
    camera.focus_auto().unwrap();
    
    mock.verify().unwrap();
}

// Helper function to build pan/tilt absolute command
fn build_pan_tilt_absolute_command(pan: i16, tilt: i16, pan_speed: u8, tilt_speed: u8) -> Vec<u8> {
    let pan_bytes = position_to_bytes(pan);
    let tilt_bytes = position_to_bytes(tilt);
    
    vec![
        0x81, 0x01, 0x06, 0x02, pan_speed, tilt_speed,
        pan_bytes[0], pan_bytes[1], pan_bytes[2], pan_bytes[3],
        tilt_bytes[0], tilt_bytes[1], tilt_bytes[2], tilt_bytes[3],
        0xFF
    ]
}

fn position_to_bytes(pos: i16) -> [u8; 4] {
    let unsigned = pos as u16;
    [
        ((unsigned >> 12) & 0x0F) as u8,
        ((unsigned >> 8) & 0x0F) as u8,
        ((unsigned >> 4) & 0x0F) as u8,
        (unsigned & 0x0F) as u8,
    ]
}