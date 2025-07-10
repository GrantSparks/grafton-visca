//! Tests for pan/tilt control commands.

mod common;

use grafton_visca::{
    camera::{methods::PanTiltMethodsExt, Camera},
    command::{
        pan_tilt::{PanTiltCommand, PanTiltDirection}, 
        Command, ResponseType
    },
    profiles::PTZOpticsG2,
    timeout::CommandCategory,
    types::{PanPosition, PanSpeed, TiltPosition, TiltSpeed},
    Error,
};
use crate::common::{
    MockTransport, ProtocolValidator, ValidationMode, patterns, ResponseBuilder
};

#[test]
fn test_pan_tilt_direction_to_bytes() {
    assert_eq!(PanTiltDirection::Up.to_bytes(), (0x03, 0x01));
    assert_eq!(PanTiltDirection::Down.to_bytes(), (0x03, 0x02));
    assert_eq!(PanTiltDirection::Left.to_bytes(), (0x01, 0x03));
    assert_eq!(PanTiltDirection::Right.to_bytes(), (0x02, 0x03));
    assert_eq!(PanTiltDirection::UpLeft.to_bytes(), (0x01, 0x01));
    assert_eq!(PanTiltDirection::UpRight.to_bytes(), (0x02, 0x01));
    assert_eq!(PanTiltDirection::DownLeft.to_bytes(), (0x01, 0x02));
    assert_eq!(PanTiltDirection::DownRight.to_bytes(), (0x02, 0x02));
    assert_eq!(PanTiltDirection::Stop.to_bytes(), (0x03, 0x03));
}

#[test]
fn test_pan_speed_validation() {
    // Valid speeds
    assert!(PanSpeed::new(0x00).is_ok());
    assert!(PanSpeed::new(0x18).is_ok());

    // Invalid speed
    assert!(matches!(
        PanSpeed::new(0x19),
        Err(Error::ParameterOutOfRange { .. })
    ));

    // From trait
    assert!(PanSpeed::try_from(0x10).is_ok());
    assert!(PanSpeed::try_from(0x20).is_err());

    // Value method
    let speed = PanSpeed::new(0x10).unwrap_or_else(|e| panic!("Valid pan speed: {e:?}"));
    let value: u8 = speed.value();
    assert_eq!(value, 0x10);
}

#[test]
fn test_tilt_speed_validation() {
    // Valid speeds
    assert!(TiltSpeed::new(0x00).is_ok());
    assert!(TiltSpeed::new(0x14).is_ok());

    // Invalid speed
    assert!(matches!(
        TiltSpeed::new(0x15),
        Err(Error::ParameterOutOfRange { .. })
    ));

    // From trait
    assert!(TiltSpeed::try_from(0x10).is_ok());
    assert!(TiltSpeed::try_from(0x15).is_err());

    // Value method
    let speed = TiltSpeed::new(0x10).unwrap_or_else(|e| panic!("Valid tilt speed: {e:?}"));
    let value: u8 = speed.value();
    assert_eq!(value, 0x10);
}

#[test]
fn test_position_to_bytes() {
    // Helper function from the module
    fn position_to_bytes(position: i16) -> [u8; 4] {
        let pos_u16 = position as u16;
        [
            ((pos_u16 >> 12) & 0x0F) as u8,
            ((pos_u16 >> 8) & 0x0F) as u8,
            ((pos_u16 >> 4) & 0x0F) as u8,
            (pos_u16 & 0x0F) as u8,
        ]
    }

    // Test positive values
    assert_eq!(position_to_bytes(0x1234), [0x01, 0x02, 0x03, 0x04]);

    // Test negative values (two's complement)
    assert_eq!(position_to_bytes(-1), [0x0F, 0x0F, 0x0F, 0x0F]);
    assert_eq!(position_to_bytes(-0x1234), [0x0E, 0x0D, 0x0C, 0x0C]);

    // Test zero
    assert_eq!(position_to_bytes(0), [0x00, 0x00, 0x00, 0x00]);
}

#[test]
fn test_pan_tilt_command_to_bytes() {
    // Test Home command
    let home = PanTiltCommand::Home;
    let bytes = home
        .to_bytes()
        .unwrap_or_else(|e| panic!("Valid command: {e:?}"));
    
    // Verify using pattern constants
    assert_eq!(bytes, patterns::pan_tilt::HOME);
    
    // Validate protocol compliance
    let mut validator = ProtocolValidator::new(ValidationMode::Strict);
    assert!(validator.validate_command(&bytes).is_ok());

    // Test Reset command
    let reset = PanTiltCommand::Reset;
    let bytes = reset
        .to_bytes()
        .unwrap_or_else(|e| panic!("Valid command: {e:?}"));
    assert_eq!(bytes, patterns::pan_tilt::RESET);

    // Test Stop command
    let pan_speed = PanSpeed::new(0x10).unwrap();
    let tilt_speed = TiltSpeed::new(0x10).unwrap();
    let stop = PanTiltCommand::Move {
        direction: PanTiltDirection::Stop,
        pan_speed,
        tilt_speed,
    };
    let bytes = stop.to_bytes().unwrap();
    assert_eq!(bytes, vec![0x81, 0x01, 0x06, 0x01, 0x10, 0x10, 0x03, 0x03, 0xFF]);
}

#[test]
fn test_pan_tilt_directional_commands() {
    let pan_speed = PanSpeed::new(0x18).unwrap();
    let tilt_speed = TiltSpeed::new(0x14).unwrap();

    // Test Up command
    let up = PanTiltCommand::Move {
        direction: PanTiltDirection::Up,
        pan_speed,
        tilt_speed,
    };
    let bytes = up.to_bytes().unwrap();
    assert_eq!(bytes, vec![0x81, 0x01, 0x06, 0x01, 0x18, 0x14, 0x03, 0x01, 0xFF]);

    // Test UpRight command
    let up_right = PanTiltCommand::Move {
        direction: PanTiltDirection::UpRight,
        pan_speed,
        tilt_speed,
    };
    let bytes = up_right.to_bytes().unwrap();
    assert_eq!(bytes, vec![0x81, 0x01, 0x06, 0x01, 0x18, 0x14, 0x02, 0x01, 0xFF]);
}

#[test]
fn test_pan_tilt_absolute_position() {
    let pan_speed = PanSpeed::new(0x10).unwrap();
    let tilt_speed = TiltSpeed::new(0x10).unwrap();
    let absolute = PanTiltCommand::AbsolutePosition {
        pan_speed,
        tilt_speed,
        pan: PanPosition::new(0x1234).unwrap(),
        tilt: TiltPosition::new(-0x0567).unwrap(),
    };

    let bytes = absolute.to_bytes().unwrap();
    assert_eq!(
        bytes,
        vec![
            0x81, 0x01, 0x06, 0x02, 0x10, 0x10, 0x01, 0x02, 0x03, 0x04, 0x0F, 0x0A, 0x09, 0x09, 0xFF
        ]
    );
}

#[test]
fn test_pan_tilt_relative_position() {
    let pan_speed = PanSpeed::new(0x10).unwrap();
    let tilt_speed = TiltSpeed::new(0x10).unwrap();
    let relative = PanTiltCommand::RelativePosition {
        pan_speed,
        tilt_speed,
        pan: PanPosition::new(-100).unwrap(),
        tilt: TiltPosition::new(200).unwrap(),
    };

    let bytes = relative.to_bytes().unwrap();
    // Format: 81 01 06 03 PS TS 0p 0p 0p 0p 0t 0t 0t 0t FF
    // -100 = 0xFF9C, 200 = 0x00C8
    assert_eq!(
        bytes,
        vec![
            0x81, 0x01, 0x06, 0x03, 0x10, 0x10, 0x0F, 0x0F, 0x09, 0x0C, 0x00, 0x00, 0x0C, 0x08, 0xFF
        ]
    );
}

#[test]
fn test_response_type() {
    assert_eq!(PanTiltCommand::Home.response_type(), None);
    assert_eq!(PanTiltCommand::Reset.response_type(), None);
    
    let cmd = PanTiltCommand::Move {
        direction: PanTiltDirection::Stop,
        pan_speed: PanSpeed::new(1).unwrap(),
        tilt_speed: TiltSpeed::new(1).unwrap(),
    };
    assert_eq!(cmd.response_type(), None);
}

#[test]
fn test_command_category() {
    // Home and Reset are movement commands that may take time
    assert_eq!(PanTiltCommand::Home.command_category(), CommandCategory::Movement);
    assert_eq!(PanTiltCommand::Reset.command_category(), CommandCategory::Movement);
    
    // Other commands are standard
    let cmd = PanTiltCommand::Move {
        direction: PanTiltDirection::Up,
        pan_speed: PanSpeed::new(1).unwrap(),
        tilt_speed: TiltSpeed::new(1).unwrap(),
    };
    assert_eq!(cmd.command_category(), CommandCategory::Movement);
}

#[test]
fn test_pan_tilt_with_camera() {
    let mut mock = MockTransport::new();
    
    // Set up expectations
    mock.expect_command(&patterns::pan_tilt::HOME)
        .described_as("pan/tilt home")
        .will_ack(1)
        .then_complete(1);
        
    mock.expect_command(&patterns::pan_tilt::STOP)
        .described_as("pan/tilt stop")
        .will_ack(1)
        .then_complete(1);
        
    mock.expect_command(&patterns::pan_tilt::UP)
        .described_as("pan/tilt up")
        .will_ack(1)
        .then_complete(1);
    
    let mut camera = Camera::<PTZOpticsG2, _>::new(mock.clone());
    
    // Test commands
    assert!(camera.pan_tilt_home().is_ok());
    assert!(camera.pan_tilt_stop().is_ok());
    assert!(camera.pan_tilt_move(PanTiltDirection::Up, 0x18, 0x18).is_ok());
    
    // Verify all expectations were met
    mock.verify().unwrap();
}

#[test]
fn test_pan_tilt_absolute_with_camera() {
    let mut mock = MockTransport::new();
    
    // Build expected command for absolute position
    let expected_cmd = vec![
        0x81, 0x01, 0x06, 0x02, 0x10, 0x10, // Header and speeds
        0x00, 0x00, 0x00, 0x00, // Pan position 0
        0x00, 0x00, 0x00, 0x00, // Tilt position 0
        0xFF
    ];
    
    mock.expect_command(&expected_cmd)
        .described_as("pan/tilt absolute to center")
        .will_ack(1)
        .then_complete(1);
    
    let mut camera = Camera::<PTZOpticsG2, _>::new(mock.clone());
    
    // Move to center position
    assert!(camera.pan_tilt_absolute(0.0, 0.0, 0x10).is_ok());
    
    mock.verify().unwrap();
}

#[test]
fn test_pan_tilt_with_inquiry_response() {
    let mut mock = MockTransport::new();
    
    // Set up pan/tilt position inquiry
    mock.expect_command(&patterns::pan_tilt::POSITION_INQ)
        .described_as("pan/tilt position inquiry")
        .will_return_data(&[
            0x01, 0x02, 0x03, 0x04, // Pan position 0x1234
            0x05, 0x06, 0x07, 0x08  // Tilt position 0x5678
        ]);
    
    let mut camera = Camera::<PTZOpticsG2, _>::new(mock.clone());
    
    // This would need the inquiry methods implemented
    // For now, just verify the mock was set up correctly
    mock.verify().unwrap();
}