//! Golden wire-byte pins for the public movement request encoders.
//!
//! Every assertion in this file compares the encoder output against an
//! absolute byte vector that was derived from the VISCA specification by hand
//! (see `tests/common/patterns.rs`). Nothing here recomputes an expectation
//! with the encoder, the direction table, or the profile conversion under
//! test, so a transposed field or a swapped table entry fails these tests even
//! though it leaves every differential test green.

#![allow(clippy::expect_used, clippy::unwrap_used)]

#[path = "common/patterns.rs"]
mod patterns;

use grafton_visca::{
    command::{FocusSpeed, PowerOn, PowerStandby},
    request::builtin::{
        FocusDrive, FocusInfinity, FocusModeCommand, FocusStop, FocusTarget, PanTiltAbsolute,
        PanTiltDrive, PanTiltHome, PanTiltRelative, PanTiltReset, PanTiltStop, PresetRecall,
        PresetReset, PresetSet, ZoomDrive, ZoomStop, ZoomTarget,
    },
    types::{FocusPosition, PanSpeed, TiltSpeed, ZoomPosition, ZoomSpeed},
    CameraId, PanTiltDirection, PresetNumber, ProfileSpec, Request,
};

use patterns::{PAN_SPEED_MAX, TILT_SPEED_MAX};

/// Encodes one request for camera 1 and returns the exact frame it wrote.
fn wire<R: Request + ?Sized>(request: &R) -> Vec<u8> {
    let mut buffer = vec![0_u8; R::MAX_SIZE];
    let written = request
        .write_into(CameraId::CAMERA_1, &mut buffer)
        .expect("golden fixture requests must encode");
    buffer.truncate(written);
    buffer
}

/// Asserts an encoded frame equals a golden byte vector, hex-formatted on
/// failure so a single wrong nibble is readable.
#[track_caller]
fn assert_golden<R: Request + ?Sized>(label: &str, request: &R, golden: &[u8]) {
    let actual = wire(request);
    assert_eq!(
        actual, golden,
        "{label} wire bytes drifted\n  expected: {golden:02X?}\n  actual:   {actual:02X?}"
    );
}

fn ptz_optics_profile() -> ProfileSpec {
    ProfileSpec::from_compile_time::<grafton_visca::profiles::PtzOpticsG2>()
        .expect("PtzOptics G2 profile")
}

fn pan_speed(value: u8) -> PanSpeed {
    PanSpeed::new(value).expect("valid pan speed")
}

fn tilt_speed(value: u8) -> TiltSpeed {
    TiltSpeed::new(value).expect("valid tilt speed")
}

fn drive(direction: PanTiltDirection) -> PanTiltDrive {
    PanTiltDrive::new(
        direction,
        pan_speed(PAN_SPEED_MAX),
        tilt_speed(TILT_SPEED_MAX),
    )
    .expect("valid directional drive")
}

/// The four cardinal directions, pinned to absolute frames.
///
/// A swap of any two rows of `PanTiltDirection::to_bytes` fails here: the
/// golden frames name the pan and tilt direction bytes independently of the
/// table that produces them.
#[test]
fn cardinal_pan_tilt_directions_match_golden_frames() {
    assert_golden(
        "pan/tilt up",
        &drive(PanTiltDirection::Up),
        patterns::pan_tilt::UP,
    );
    assert_golden(
        "pan/tilt down",
        &drive(PanTiltDirection::Down),
        patterns::pan_tilt::DOWN,
    );
    assert_golden(
        "pan/tilt left",
        &drive(PanTiltDirection::Left),
        patterns::pan_tilt::LEFT,
    );
    assert_golden(
        "pan/tilt right",
        &drive(PanTiltDirection::Right),
        patterns::pan_tilt::RIGHT,
    );
}

/// The four diagonals, pinned to absolute frames.
#[test]
fn diagonal_pan_tilt_directions_match_golden_frames() {
    assert_golden(
        "pan/tilt up-left",
        &drive(PanTiltDirection::UpLeft),
        patterns::pan_tilt::UP_LEFT,
    );
    assert_golden(
        "pan/tilt up-right",
        &drive(PanTiltDirection::UpRight),
        patterns::pan_tilt::UP_RIGHT,
    );
    assert_golden(
        "pan/tilt down-left",
        &drive(PanTiltDirection::DownLeft),
        patterns::pan_tilt::DOWN_LEFT,
    );
    assert_golden(
        "pan/tilt down-right",
        &drive(PanTiltDirection::DownRight),
        patterns::pan_tilt::DOWN_RIGHT,
    );
}

/// Every direction produces a distinct frame.
///
/// Golden frames pin each direction to the right bytes; this additionally
/// rules out a table that collapses two directions onto one encoding.
#[test]
fn every_pan_tilt_direction_encodes_to_a_distinct_frame() {
    let frames: Vec<Vec<u8>> = [
        PanTiltDirection::Up,
        PanTiltDirection::Down,
        PanTiltDirection::Left,
        PanTiltDirection::Right,
        PanTiltDirection::UpLeft,
        PanTiltDirection::UpRight,
        PanTiltDirection::DownLeft,
        PanTiltDirection::DownRight,
    ]
    .into_iter()
    .map(|direction| wire(&drive(direction)))
    .collect();

    for (index, frame) in frames.iter().enumerate() {
        for (other_index, other) in frames.iter().enumerate().skip(index + 1) {
            assert_ne!(
                frame, other,
                "directions {index} and {other_index} encode to the same frame {frame:02X?}"
            );
        }
    }
}

#[test]
fn pan_tilt_stop_home_and_reset_match_golden_frames() {
    assert_golden(
        "pan/tilt stop",
        &PanTiltStop::new(pan_speed(0x0C), tilt_speed(0x0A)),
        patterns::pan_tilt::STOP,
    );
    assert_golden("pan/tilt home", &PanTiltHome, patterns::pan_tilt::HOME);
    assert_golden("pan/tilt reset", &PanTiltReset, patterns::pan_tilt::RESET);
}

/// Absolute and relative pan/tilt positioning, pinned to absolute frames.
///
/// The fixture assumptions below fix the profile conversion so the golden
/// frames stay hand-derivable: 10 deg pan and -5 deg tilt at 14.4 units per
/// degree are +144 and -72 camera units, which a signed-centered profile puts
/// on the wire as `0x0090` and `0xFFB8`. Transposing the pan and tilt u16
/// pushes in the encoder, or the pan and tilt speed pushes, fails this test.
#[test]
fn absolute_and_relative_pan_tilt_positions_match_golden_frames() {
    use grafton_visca::units::Degrees;

    let profile = ptz_optics_profile();
    let conversion = profile
        .pan_tilt_coordinates()
        .expect("PtzOptics G2 declares a pan/tilt coordinate conversion");
    assert!(
        (conversion.pan_degrees_to_units() - 14.4).abs() < f32::EPSILON,
        "fixture assumption: 14.4 pan units per degree"
    );
    assert!(
        (conversion.tilt_degrees_to_units() - 14.4).abs() < f32::EPSILON,
        "fixture assumption: 14.4 tilt units per degree"
    );
    assert_eq!(
        conversion.coordinate_system(),
        grafton_visca::capabilities::CoordinateSystem::SignedCentered,
        "fixture assumption: signed-centered wire coordinates"
    );

    let absolute = PanTiltAbsolute::for_profile(
        Degrees(10.0),
        Degrees(-5.0),
        pan_speed(0x0A),
        tilt_speed(0x05),
        &profile,
    )
    .expect("absolute pan/tilt target inside the profile range");
    assert_golden(
        "absolute pan/tilt",
        &absolute,
        patterns::pan_tilt::ABSOLUTE_PAN_144_TILT_NEG_72,
    );

    let relative = PanTiltRelative::for_profile(
        Degrees(10.0),
        Degrees(-5.0),
        pan_speed(0x0A),
        tilt_speed(0x05),
        &profile,
    )
    .expect("relative pan/tilt offset inside the profile range");
    assert_golden(
        "relative pan/tilt",
        &relative,
        patterns::pan_tilt::RELATIVE_PAN_144_TILT_NEG_72,
    );
}

/// Absolute pan/tilt is asymmetric in the pan and tilt fields.
///
/// The golden frame above already pins the byte order; this states the
/// property it depends on, so a future fixture that accidentally chose a
/// palindromic position pair cannot silently weaken the pin.
#[test]
fn absolute_pan_tilt_golden_frame_distinguishes_the_two_axes() {
    let frame = patterns::pan_tilt::ABSOLUTE_PAN_144_TILT_NEG_72;
    let pan_nibbles = &frame[6..10];
    let tilt_nibbles = &frame[10..14];
    assert_ne!(
        pan_nibbles, tilt_nibbles,
        "the golden absolute frame must not be symmetric across pan and tilt"
    );
    assert_ne!(
        frame[4], frame[5],
        "the golden absolute frame must not be symmetric across the speed fields"
    );
}

#[test]
fn zoom_drive_and_direct_target_match_golden_frames() {
    assert_golden("zoom stop", &ZoomStop, patterns::zoom::STOP);
    assert_golden("zoom tele", &ZoomDrive::Tele, patterns::zoom::TELE_STD);
    assert_golden("zoom wide", &ZoomDrive::Wide, patterns::zoom::WIDE_STD);

    let speed = ZoomSpeed::new(5).expect("valid zoom speed");
    assert_golden(
        "zoom tele variable",
        &ZoomDrive::TeleVariable(speed),
        patterns::zoom::TELE_VARIABLE_5,
    );
    assert_golden(
        "zoom wide variable",
        &ZoomDrive::WideVariable(speed),
        patterns::zoom::WIDE_VARIABLE_5,
    );

    assert_golden(
        "zoom direct",
        &ZoomTarget::new(ZoomPosition::new(0x1234).expect("valid zoom position")),
        patterns::zoom::DIRECT_0X1234,
    );
}

#[test]
fn focus_drive_and_direct_target_match_golden_frames() {
    assert_golden("focus stop", &FocusStop, patterns::focus::STOP);
    assert_golden("focus far", &FocusDrive::Far, patterns::focus::FAR);
    assert_golden("focus near", &FocusDrive::Near, patterns::focus::NEAR);

    let speed = FocusSpeed::new(5).expect("valid focus speed");
    assert_golden(
        "focus far variable",
        &FocusDrive::FarVariable(speed),
        patterns::focus::FAR_VARIABLE_5,
    );
    assert_golden(
        "focus near variable",
        &FocusDrive::NearVariable(speed),
        patterns::focus::NEAR_VARIABLE_5,
    );

    assert_golden("focus infinity", &FocusInfinity, patterns::focus::INFINITY);
    assert_golden("focus auto", &FocusModeCommand::Auto, patterns::focus::AUTO);
    assert_golden(
        "focus manual",
        &FocusModeCommand::Manual,
        patterns::focus::MANUAL,
    );

    assert_golden(
        "focus direct",
        &FocusTarget::new(FocusPosition::new(0x2345)),
        patterns::focus::DIRECT_0X2345,
    );
}

#[test]
fn preset_and_power_commands_match_golden_frames() {
    let profile = ptz_optics_profile();
    let slot = PresetNumber::new(1).expect("valid preset slot");

    assert_golden(
        "preset recall",
        &PresetRecall::for_profile(slot, &profile).expect("profile declares preset recall"),
        patterns::preset::RECALL_1,
    );
    assert_golden("preset set", &PresetSet::new(slot), patterns::preset::SET_1);
    assert_golden(
        "preset reset",
        &PresetReset::new(slot),
        patterns::preset::RESET_1,
    );

    assert_golden("power on", &PowerOn, patterns::power::ON);
    assert_golden("power standby", &PowerStandby, patterns::power::STANDBY);
}
