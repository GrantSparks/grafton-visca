//! Tests to verify that command enums produce the expected bytes.
//!
//! These tests ensure that the high-level command enums generate the correct
//! VISCA protocol bytes according to the specification.

use grafton_visca::{
    command::{
        focus::{FocusCommand, FocusSpeed},
        pan_tilt::{PanTiltCommand, PanTiltDirection},
        zoom::{ZoomCommand, ZoomSpeed},
        Command,
    },
    types::{PanPosition, PanSpeed, TiltPosition, TiltSpeed},
};

#[test]
fn test_zoom_command_consistency() {
    // Zoom Stop
    let zoom_stop = ZoomCommand::Stop;
    assert_eq!(
        zoom_stop.to_bytes().unwrap(),
        vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xFF]
    );

    // Zoom Tele Standard
    let zoom_tele_std = ZoomCommand::TeleStandard;
    assert_eq!(
        zoom_tele_std.to_bytes().unwrap(),
        vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]
    );

    // Zoom Wide Standard  
    let zoom_wide_std = ZoomCommand::WideStandard;
    assert_eq!(
        zoom_wide_std.to_bytes().unwrap(),
        vec![0x81, 0x01, 0x04, 0x07, 0x03, 0xFF]
    );

    // Zoom Tele Variable
    let zoom_tele = ZoomCommand::TeleVariable(ZoomSpeed::new(5).unwrap());
    assert_eq!(
        zoom_tele.to_bytes().unwrap(),
        vec![0x81, 0x01, 0x04, 0x07, 0x25, 0xFF]
    );

    // Zoom Wide Variable
    let zoom_wide = ZoomCommand::WideVariable(ZoomSpeed::new(3).unwrap());
    assert_eq!(
        zoom_wide.to_bytes().unwrap(),
        vec![0x81, 0x01, 0x04, 0x07, 0x33, 0xFF]
    );
}

#[test]
fn test_focus_command_consistency() {
    // Focus Auto
    let focus_auto = FocusCommand::Auto;
    assert_eq!(
        focus_auto.to_bytes().unwrap(),
        vec![0x81, 0x01, 0x04, 0x38, 0x02, 0xFF]
    );

    // Focus Manual
    let focus_manual = FocusCommand::Manual;
    assert_eq!(
        focus_manual.to_bytes().unwrap(),
        vec![0x81, 0x01, 0x04, 0x38, 0x03, 0xFF]
    );

    // Focus Stop
    let focus_stop = FocusCommand::Stop;
    assert_eq!(
        focus_stop.to_bytes().unwrap(),
        vec![0x81, 0x01, 0x04, 0x08, 0x00, 0xFF]
    );

    // Focus Near Standard
    let focus_near_std = FocusCommand::Near;
    assert_eq!(
        focus_near_std.to_bytes().unwrap(),
        vec![0x81, 0x01, 0x04, 0x08, 0x03, 0xFF]
    );

    // Focus Far Standard
    let focus_far_std = FocusCommand::Far;
    assert_eq!(
        focus_far_std.to_bytes().unwrap(),
        vec![0x81, 0x01, 0x04, 0x08, 0x02, 0xFF]
    );

    // Focus Near Variable
    let focus_near = FocusCommand::NearWithSpeed(FocusSpeed::new(4).unwrap());
    assert_eq!(
        focus_near.to_bytes().unwrap(),
        vec![0x81, 0x01, 0x04, 0x08, 0x34, 0xFF]
    );

    // Focus Far Variable
    let focus_far = FocusCommand::FarWithSpeed(FocusSpeed::new(6).unwrap());
    assert_eq!(
        focus_far.to_bytes().unwrap(),
        vec![0x81, 0x01, 0x04, 0x08, 0x26, 0xFF]
    );

    // Focus One Push Trigger
    let focus_one_push = FocusCommand::OnePushTrigger;
    assert_eq!(
        focus_one_push.to_bytes().unwrap(),
        vec![0x81, 0x01, 0x04, 0x18, 0x01, 0xFF]
    );

    // Focus Infinity
    let focus_infinity = FocusCommand::Infinity;
    assert_eq!(
        focus_infinity.to_bytes().unwrap(),
        vec![0x81, 0x01, 0x04, 0x18, 0x02, 0xFF]
    );
}

#[test]
fn test_pan_tilt_command_consistency() {
    // Pan/Tilt Home
    let pt_home = PanTiltCommand::Home;
    assert_eq!(
        pt_home.to_bytes().unwrap(),
        vec![0x81, 0x01, 0x06, 0x04, 0xFF]
    );

    // Pan/Tilt Reset
    let pt_reset = PanTiltCommand::Reset;
    assert_eq!(
        pt_reset.to_bytes().unwrap(),
        vec![0x81, 0x01, 0x06, 0x05, 0xFF]
    );

    // Pan/Tilt Stop
    let pt_stop = PanTiltCommand::Move {
        direction: PanTiltDirection::Stop,
        pan_speed: PanSpeed::new(0).unwrap(),
        tilt_speed: TiltSpeed::new(0).unwrap(),
    };
    assert_eq!(
        pt_stop.to_bytes().unwrap(),
        vec![0x81, 0x01, 0x06, 0x01, 0x00, 0x00, 0x03, 0x03, 0xFF]
    );

    // Pan/Tilt Up
    let pt_up = PanTiltCommand::Move {
        direction: PanTiltDirection::Up,
        pan_speed: PanSpeed::new(0x10).unwrap(),
        tilt_speed: TiltSpeed::new(0x10).unwrap(),
    };
    assert_eq!(
        pt_up.to_bytes().unwrap(),
        vec![0x81, 0x01, 0x06, 0x01, 0x10, 0x10, 0x03, 0x01, 0xFF]
    );

    // Pan/Tilt DownRight
    let pt_downright = PanTiltCommand::Move {
        direction: PanTiltDirection::DownRight,
        pan_speed: PanSpeed::new(0x08).unwrap(),
        tilt_speed: TiltSpeed::new(0x08).unwrap(),
    };
    assert_eq!(
        pt_downright.to_bytes().unwrap(),
        vec![0x81, 0x01, 0x06, 0x01, 0x08, 0x08, 0x02, 0x02, 0xFF]
    );

    // Pan/Tilt Absolute Position
    let pt_absolute = PanTiltCommand::AbsolutePosition {
        pan: PanPosition::new(0).unwrap(),
        tilt: TiltPosition::new(0).unwrap(),
        pan_speed: PanSpeed::new(0x10).unwrap(),
        tilt_speed: TiltSpeed::new(0x10).unwrap(),
    };
    assert_eq!(
        pt_absolute.to_bytes().unwrap(),
        vec![0x81, 0x01, 0x06, 0x02, 0x10, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF]
    );

    // Pan/Tilt Relative Position
    let pt_relative = PanTiltCommand::RelativePosition {
        pan: PanPosition::new(100).unwrap(),
        tilt: TiltPosition::new(50).unwrap(),
        pan_speed: PanSpeed::new(0x08).unwrap(),
        tilt_speed: TiltSpeed::new(0x08).unwrap(),
    };
    let bytes = pt_relative.to_bytes().unwrap();
    assert_eq!(bytes[0], 0x81);
    assert_eq!(bytes[1], 0x01);
    assert_eq!(bytes[2], 0x06);
    assert_eq!(bytes[3], 0x03);
    assert_eq!(bytes[4], 0x08);
    assert_eq!(bytes[5], 0x08);
    // Pan/tilt position bytes would be encoded here
    assert_eq!(bytes[14], 0xFF);
}

#[test]
fn test_response_type_consistency() {
    // Most action commands return None for response_type
    assert!(ZoomCommand::Stop.response_type().is_none());
    // TeleStandard and WideStandard have response types
    assert!(ZoomCommand::TeleStandard.response_type().is_some());
    assert!(ZoomCommand::WideStandard.response_type().is_some());
    // Variable speed zoom commands don't have response types
    assert!(ZoomCommand::TeleVariable(ZoomSpeed::new(5).unwrap()).response_type().is_none());
    assert!(ZoomCommand::WideVariable(ZoomSpeed::new(5).unwrap()).response_type().is_none());
    
    assert!(FocusCommand::Auto.response_type().is_none());
    assert!(FocusCommand::Stop.response_type().is_none());
    assert!(PanTiltCommand::Home.response_type().is_none());
    assert!(PanTiltCommand::Reset.response_type().is_none());
    assert!(
        PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0).unwrap(),
            tilt_speed: TiltSpeed::new(0).unwrap(),
        }
        .response_type()
        .is_none()
    );
}

#[test]
fn test_zoom_speed_validation() {
    // Valid speeds
    assert!(ZoomSpeed::new(0).is_ok());
    assert!(ZoomSpeed::new(7).is_ok());
    
    // Invalid speeds
    assert!(ZoomSpeed::new(8).is_err());
    assert!(ZoomSpeed::new(255).is_err());
}

#[test]
fn test_focus_speed_validation() {
    // Valid speeds
    assert!(FocusSpeed::new(0).is_ok());
    assert!(FocusSpeed::new(7).is_ok());
    
    // Invalid speeds
    assert!(FocusSpeed::new(8).is_err());
    assert!(FocusSpeed::new(255).is_err());
}

#[test]
fn test_pan_tilt_speed_validation() {
    // Valid pan speeds
    assert!(PanSpeed::new(0).is_ok());
    assert!(PanSpeed::new(0x18).is_ok());
    
    // Invalid pan speeds
    assert!(PanSpeed::new(0x19).is_err());
    assert!(PanSpeed::new(255).is_err());
    
    // Valid tilt speeds
    assert!(TiltSpeed::new(0).is_ok());
    assert!(TiltSpeed::new(0x14).is_ok());
    
    // Invalid tilt speeds
    assert!(TiltSpeed::new(0x15).is_err());
    assert!(TiltSpeed::new(255).is_err());
}

#[test]
fn test_position_validation() {
    // Valid pan positions
    assert!(PanPosition::new(-2448).is_ok());
    assert!(PanPosition::new(0).is_ok());
    assert!(PanPosition::new(2448).is_ok());
    
    // Invalid pan positions
    assert!(PanPosition::new(-2449).is_err());
    assert!(PanPosition::new(2449).is_err());
    
    // Valid tilt positions
    assert!(TiltPosition::new(-432).is_ok());
    assert!(TiltPosition::new(0).is_ok());
    assert!(TiltPosition::new(1296).is_ok());
    
    // Invalid tilt positions
    assert!(TiltPosition::new(-433).is_err());
    assert!(TiltPosition::new(1297).is_err());
}