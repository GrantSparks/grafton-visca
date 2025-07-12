//! Tests to verify that command enums produce the expected bytes.
//!
//! These tests ensure that the high-level command enums generate the correct
//! VISCA protocol bytes according to the specification.

use grafton_visca::{
    command::{
        focus::{Focus, FocusSpeed},
        pan_tilt::{PanTilt, PanTiltDirection},
        zoom::ZoomSpeed,
        EncodeVisca, Zoom,
    },
    types::{PanPosition, PanSpeed, TiltPosition, TiltSpeed},
};

#[test]
fn test_zoom_command_consistency() {
    // Zoom Stop
    let zoom_stop = Zoom::Stop;
    assert_eq!(
        zoom_stop.try_into_vec().unwrap(),
        vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xFF]
    );

    // Zoom Tele Standard
    let zoom_tele_std = Zoom::TeleStd;
    assert_eq!(
        zoom_tele_std.try_into_vec().unwrap(),
        vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]
    );

    // Zoom Wide Standard
    let zoom_wide_std = Zoom::WideStd;
    assert_eq!(
        zoom_wide_std.try_into_vec().unwrap(),
        vec![0x81, 0x01, 0x04, 0x07, 0x03, 0xFF]
    );

    // Zoom Tele Variable
    let zoom_tele = Zoom::TeleVariable(ZoomSpeed::new(5).unwrap());
    assert_eq!(
        zoom_tele.try_into_vec().unwrap(),
        vec![0x81, 0x01, 0x04, 0x07, 0x25, 0xFF]
    );

    // Zoom Wide Variable
    let zoom_wide = Zoom::WideVariable(ZoomSpeed::new(3).unwrap());
    assert_eq!(
        zoom_wide.try_into_vec().unwrap(),
        vec![0x81, 0x01, 0x04, 0x07, 0x33, 0xFF]
    );
}

#[test]
fn test_focus_command_consistency() {
    // Focus Auto
    let focus_auto = Focus::Auto;
    assert_eq!(
        focus_auto.try_into_vec().unwrap(),
        vec![0x81, 0x01, 0x04, 0x38, 0x02, 0xFF]
    );

    // Focus Manual
    let focus_manual = Focus::Manual;
    assert_eq!(
        focus_manual.try_into_vec().unwrap(),
        vec![0x81, 0x01, 0x04, 0x38, 0x03, 0xFF]
    );

    // Focus Stop
    let focus_stop = Focus::Stop;
    assert_eq!(
        focus_stop.try_into_vec().unwrap(),
        vec![0x81, 0x01, 0x04, 0x08, 0x00, 0xFF]
    );

    // Focus Near Standard
    let focus_near_std = Focus::Near;
    assert_eq!(
        focus_near_std.try_into_vec().unwrap(),
        vec![0x81, 0x01, 0x04, 0x08, 0x03, 0xFF]
    );

    // Focus Far Standard
    let focus_far_std = Focus::Far;
    assert_eq!(
        focus_far_std.try_into_vec().unwrap(),
        vec![0x81, 0x01, 0x04, 0x08, 0x02, 0xFF]
    );

    // Focus Near Variable
    let focus_near = Focus::NearWithSpeed(FocusSpeed::new(4).unwrap());
    assert_eq!(
        focus_near.try_into_vec().unwrap(),
        vec![0x81, 0x01, 0x04, 0x08, 0x34, 0xFF]
    );

    // Focus Far Variable
    let focus_far = Focus::FarWithSpeed(FocusSpeed::new(6).unwrap());
    assert_eq!(
        focus_far.try_into_vec().unwrap(),
        vec![0x81, 0x01, 0x04, 0x08, 0x26, 0xFF]
    );

    // Focus One Push Trigger
    let focus_one_push = Focus::OnePushTrigger;
    assert_eq!(
        focus_one_push.try_into_vec().unwrap(),
        vec![0x81, 0x01, 0x04, 0x18, 0x01, 0xFF]
    );

    // Focus Infinity
    let focus_infinity = Focus::Infinity;
    assert_eq!(
        focus_infinity.try_into_vec().unwrap(),
        vec![0x81, 0x01, 0x04, 0x18, 0x02, 0xFF]
    );
}

#[test]
fn test_pan_tilt_command_consistency() {
    // Pan/Tilt Home
    let pt_home = PanTilt::Home;
    assert_eq!(
        pt_home.try_into_vec().unwrap(),
        vec![0x81, 0x01, 0x06, 0x04, 0xFF]
    );

    // Pan/Tilt Reset
    let pt_reset = PanTilt::Reset;
    assert_eq!(
        pt_reset.try_into_vec().unwrap(),
        vec![0x81, 0x01, 0x06, 0x05, 0xFF]
    );

    // Pan/Tilt Stop
    let pt_stop = PanTilt::Move {
        direction: PanTiltDirection::Stop,
        pan_speed: PanSpeed::new(0).unwrap(),
        tilt_speed: TiltSpeed::new(0).unwrap(),
    };
    assert_eq!(
        pt_stop.try_into_vec().unwrap(),
        vec![0x81, 0x01, 0x06, 0x01, 0x00, 0x00, 0x03, 0x03, 0xFF]
    );

    // Pan/Tilt Up
    let pt_up = PanTilt::Move {
        direction: PanTiltDirection::Up,
        pan_speed: PanSpeed::new(0x10).unwrap(),
        tilt_speed: TiltSpeed::new(0x10).unwrap(),
    };
    assert_eq!(
        pt_up.try_into_vec().unwrap(),
        vec![0x81, 0x01, 0x06, 0x01, 0x10, 0x10, 0x03, 0x01, 0xFF]
    );

    // Pan/Tilt DownRight
    let pt_downright = PanTilt::Move {
        direction: PanTiltDirection::DownRight,
        pan_speed: PanSpeed::new(0x08).unwrap(),
        tilt_speed: TiltSpeed::new(0x08).unwrap(),
    };
    assert_eq!(
        pt_downright.try_into_vec().unwrap(),
        vec![0x81, 0x01, 0x06, 0x01, 0x08, 0x08, 0x02, 0x02, 0xFF]
    );

    // Pan/Tilt Absolute Position
    let pt_absolute = PanTilt::AbsolutePosition {
        pan: PanPosition::new(0).unwrap(),
        tilt: TiltPosition::new(0).unwrap(),
        pan_speed: PanSpeed::new(0x10).unwrap(),
        tilt_speed: TiltSpeed::new(0x10).unwrap(),
    };
    assert_eq!(
        pt_absolute.try_into_vec().unwrap(),
        vec![
            0x81, 0x01, 0x06, 0x02, 0x10, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0xFF
        ]
    );

    // Pan/Tilt Relative Position
    let pt_relative = PanTilt::RelativePosition {
        pan: PanPosition::new(100).unwrap(),
        tilt: TiltPosition::new(50).unwrap(),
        pan_speed: PanSpeed::new(0x08).unwrap(),
        tilt_speed: TiltSpeed::new(0x08).unwrap(),
    };
    let bytes = pt_relative.try_into_vec().unwrap();
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
    assert!(Zoom::Stop.response_type().is_none());
    // TeleStd and WideStd have response types
    assert!(Zoom::TeleStd.response_type().is_some());
    assert!(Zoom::WideStd.response_type().is_some());
    // Variable speed zoom commands don't have response types
    assert!(Zoom::TeleVariable(ZoomSpeed::new(5).unwrap())
        .response_type()
        .is_none());
    assert!(Zoom::WideVariable(ZoomSpeed::new(5).unwrap())
        .response_type()
        .is_none());

    assert!(Focus::Auto.response_type().is_none());
    assert!(Focus::Stop.response_type().is_none());
    assert!(PanTilt::Home.response_type().is_none());
    assert!(PanTilt::Reset.response_type().is_none());
    assert!(PanTilt::Move {
        direction: PanTiltDirection::Stop,
        pan_speed: PanSpeed::new(0).unwrap(),
        tilt_speed: TiltSpeed::new(0).unwrap(),
    }
    .response_type()
    .is_none());
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
