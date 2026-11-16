//! Golden wire-byte pins for the public movement request encoders.
//!
//! Every assertion in this file compares the encoder output against an
//! absolute byte vector that was derived from the VISCA specification by hand
//! (see `test-support/src/patterns.rs`). Nothing here recomputes an expectation
//! with the encoder, the direction table, or the profile conversion under
//! test, so a transposed field or a swapped table entry fails these tests even
//! though it leaves every differential test green.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use crate::common::profile_encodings;

use grafton_visca_test_support::patterns;

use grafton_visca::{
    profiles::ProfileSpec,
    request::Request,
    types::{
        CameraId, FocusPosition, FocusSpeed, PanSpeed, PanTiltDirection, PanTiltLimitCorner,
        PresetNumber, TiltSpeed, ZoomPosition, ZoomSpeed,
    },
    Error,
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

fn drive(direction: PanTiltDirection) -> grafton_visca::command::pan_tilt::Drive {
    grafton_visca::command::pan_tilt::Drive::for_target(
        &grafton_visca::config::TargetRegistration::new(
            grafton_visca::types::CameraId::CAMERA_1,
            grafton_visca::profiles::ProfileSpec::from_compile_time::<
                grafton_visca::profiles::PtzOpticsG2,
            >()
            .expect("source profile"),
        )
        .expect("target registration"),
        direction,
        grafton_visca::types::PanTiltSpeed {
            pan: pan_speed(PAN_SPEED_MAX),
            tilt: tilt_speed(TILT_SPEED_MAX),
        },
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
        &grafton_visca::command::pan_tilt::Stop::for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                grafton_visca::profiles::ProfileSpec::from_compile_time::<
                    grafton_visca::profiles::PtzOpticsG2,
                >()
                .expect("source profile"),
            )
            .expect("target registration"),
        )
        .expect("profile-safe stop"),
        patterns::pan_tilt::STOP,
    );
    assert_golden(
        "pan/tilt home",
        &grafton_visca::command::pan_tilt::Home,
        patterns::pan_tilt::HOME,
    );
    assert_golden(
        "pan/tilt reset",
        &grafton_visca::command::pan_tilt::Reset,
        patterns::pan_tilt::RESET,
    );
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
    use grafton_visca::types::Degrees;

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

    let absolute = grafton_visca::command::pan_tilt::Absolute::for_target(
        &grafton_visca::config::TargetRegistration::new(
            grafton_visca::types::CameraId::CAMERA_1,
            profile.clone(),
        )
        .expect("target registration"),
        grafton_visca::types::PanTiltPosition {
            pan: Degrees::new(10.0).expect("valid degrees"),
            tilt: Degrees::new(-5.0).expect("valid degrees"),
        },
        grafton_visca::types::PanTiltSpeed {
            pan: pan_speed(0x0A),
            tilt: tilt_speed(0x05),
        },
    )
    .expect("absolute pan/tilt target inside the profile range");
    assert_golden(
        "absolute pan/tilt",
        &absolute,
        patterns::pan_tilt::ABSOLUTE_PAN_144_TILT_NEG_72,
    );

    let relative = grafton_visca::command::pan_tilt::Relative::for_target(
        &grafton_visca::config::TargetRegistration::new(
            grafton_visca::types::CameraId::CAMERA_1,
            profile.clone(),
        )
        .expect("target registration"),
        grafton_visca::types::PanTiltPosition {
            pan: Degrees::new(10.0).expect("valid degrees"),
            tilt: Degrees::new(-5.0).expect("valid degrees"),
        },
        grafton_visca::types::PanTiltSpeed {
            pan: pan_speed(0x0A),
            tilt: tilt_speed(0x05),
        },
    )
    .expect("relative pan/tilt offset inside the profile range");
    assert_golden(
        "relative pan/tilt",
        &relative,
        patterns::pan_tilt::RELATIVE_PAN_144_TILT_NEG_72,
    );
}

/// A relative displacement can exceed the profile's absolute tilt bounds.
#[test]
fn g2_relative_negative_45_degrees_matches_the_signed_offset_literal() {
    use grafton_visca::types::Degrees;

    let target =
        grafton_visca::config::TargetRegistration::new(CameraId::CAMERA_1, ptz_optics_profile())
            .expect("G2 registration");
    let relative = grafton_visca::command::pan_tilt::Relative::for_target(
        &target,
        grafton_visca::types::PanTiltPosition {
            pan: Degrees::new(0.0).expect("valid degrees"),
            tilt: Degrees::new(-45.0).expect("valid degrees"),
        },
        grafton_visca::types::PanTiltSpeed {
            pan: pan_speed(9),
            tilt: tilt_speed(7),
        },
    )
    .expect("reachable G2 displacement");
    assert_golden(
        "G2 relative (0, -45)",
        &relative,
        &[0x81, 1, 6, 3, 9, 7, 0, 0, 0, 0, 15, 13, 7, 8, 0xff],
    );
    assert!(matches!(
        grafton_visca::command::pan_tilt::Absolute::for_target(
            &target,
            grafton_visca::types::PanTiltPosition {
                pan: Degrees::new(0.0).expect("valid degrees"),
                tilt: Degrees::new(-45.0).expect("valid degrees")
            },
            grafton_visca::types::PanTiltSpeed {
                pan: pan_speed(9),
                tilt: tilt_speed(7)
            }
        ),
        Err(Error::InvalidParameter { .. })
    ));
}

/// Standard VISCA assigns `0x00` to the down-left limit corner and `0x01` to
/// the up-right corner.  These public request-path vectors are independently
/// hand-derived, pinning the discriminator in both set and clear frames.
#[test]
fn standard_visca_pan_tilt_limit_corners_match_golden_frames() {
    use grafton_visca::{capabilities::PanTiltWireCodec, types::Degrees};

    let profile = ptz_optics_profile();
    let conversion = profile
        .pan_tilt_coordinates()
        .expect("PtzOptics G2 declares a pan/tilt coordinate conversion");
    assert_eq!(conversion.wire_codec(), PanTiltWireCodec::StandardVisca);

    for (corner, label, set_golden, clear_golden) in [
        (
            PanTiltLimitCorner::DownLeft,
            "Standard VISCA down-left limit",
            &[
                0x81, 0x01, 0x06, 0x07, 0x00, 0x00, 0x00, 0x00, 0x09, 0x00, 0x0F, 0x0F, 0x0B, 0x08,
                0xFF,
            ][..],
            &[
                0x81, 0x01, 0x06, 0x07, 0x01, 0x00, 0x07, 0x0F, 0x0F, 0x0F, 0x07, 0x0F, 0x0F, 0x0F,
                0xFF,
            ][..],
        ),
        (
            PanTiltLimitCorner::UpRight,
            "Standard VISCA up-right limit",
            &[
                0x81, 0x01, 0x06, 0x07, 0x00, 0x01, 0x00, 0x00, 0x09, 0x00, 0x0F, 0x0F, 0x0B, 0x08,
                0xFF,
            ][..],
            &[
                0x81, 0x01, 0x06, 0x07, 0x01, 0x01, 0x07, 0x0F, 0x0F, 0x0F, 0x07, 0x0F, 0x0F, 0x0F,
                0xFF,
            ][..],
        ),
    ] {
        let set = grafton_visca::command::pan_tilt::SetLimit::for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                profile.clone(),
            )
            .expect("target registration"),
            corner,
            grafton_visca::types::PanTiltPosition {
                pan: Degrees::new(10.0).expect("valid degrees"),
                tilt: Degrees::new(-5.0).expect("valid degrees"),
            },
        )
        .expect("standard VISCA limit position");
        assert_golden(&format!("{label} set"), &set, set_golden);

        let clear = grafton_visca::command::pan_tilt::ClearLimit::for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                profile.clone(),
            )
            .expect("target registration"),
            corner,
        )
        .expect("standard VISCA limit clear");
        assert_golden(&format!("{label} clear"), &clear, clear_golden);
    }
}

/// Sony BRC-300 uses one speed byte, a fixed `00`, then five pan and four
/// tilt nibbles. The manual (pp. 12 and 22) gives the endpoint polarity and
/// approximates one degree as `0xD0`: positive raw pan is left and positive
/// raw tilt is up. These profile-scale vectors therefore encode +45° right as
/// negative pan and +15° up as positive tilt; exact endpoint bytes are pinned
/// separately.
#[test]
fn sony_brc300_profile_uses_documented_position_and_limit_frames() {
    use grafton_visca::{capabilities::PanTiltWireCodec, profiles::SonyBrc300, types::Degrees};

    let profile = ProfileSpec::from_compile_time::<SonyBrc300>().expect("Sony BRC-300 profile");
    let conversion = profile
        .pan_tilt_coordinates()
        .expect("Sony BRC-300 pan/tilt conversion");
    assert_eq!(conversion.wire_codec(), PanTiltWireCodec::SonyBrc300);
    assert_eq!(conversion.pan_degrees_to_units(), -208.0);
    assert_eq!(conversion.tilt_degrees_to_units(), 208.0);

    let absolute_error = grafton_visca::command::pan_tilt::Absolute::for_target(
        &grafton_visca::config::TargetRegistration::new(
            grafton_visca::types::CameraId::CAMERA_1,
            profile.clone(),
        )
        .expect("target registration"),
        grafton_visca::types::PanTiltPosition {
            pan: Degrees::new(45.0).expect("valid degrees"),
            tilt: Degrees::new(15.0).expect("valid degrees"),
        },
        grafton_visca::types::PanTiltSpeed {
            pan: pan_speed(0x18),
            tilt: tilt_speed(0x14),
        },
    )
    .expect_err("BRC-300 absolute position must reject unequal paired speeds");
    assert!(matches!(
        absolute_error,
        Error::InvalidRequest { reason: message } if message.contains("pan and tilt speeds must match")
    ));

    let relative_error = grafton_visca::command::pan_tilt::Relative::for_target(
        &grafton_visca::config::TargetRegistration::new(
            grafton_visca::types::CameraId::CAMERA_1,
            profile.clone(),
        )
        .expect("target registration"),
        grafton_visca::types::PanTiltPosition {
            pan: Degrees::new(45.0).expect("valid degrees"),
            tilt: Degrees::new(15.0).expect("valid degrees"),
        },
        grafton_visca::types::PanTiltSpeed {
            pan: pan_speed(0x18),
            tilt: tilt_speed(0x14),
        },
    )
    .expect_err("BRC-300 relative position must reject unequal paired speeds");
    assert!(matches!(
        relative_error,
        Error::InvalidRequest { reason: message } if message.contains("pan and tilt speeds must match")
    ));

    let absolute = grafton_visca::command::pan_tilt::Absolute::for_target(
        &grafton_visca::config::TargetRegistration::new(
            grafton_visca::types::CameraId::CAMERA_1,
            profile.clone(),
        )
        .expect("target registration"),
        grafton_visca::types::PanTiltPosition {
            pan: Degrees::new(45.0).expect("valid degrees"),
            tilt: Degrees::new(15.0).expect("valid degrees"),
        },
        grafton_visca::types::PanTiltSpeed {
            pan: pan_speed(0x09),
            tilt: tilt_speed(0x09),
        },
    )
    .expect("BRC-300 absolute position");
    assert_golden(
        "Sony BRC-300 absolute position",
        &absolute,
        &[
            0x81, 0x01, 0x06, 0x02, 0x09, 0x00, 0x0F, 0x0D, 0x0B, 0x07, 0x00, 0x00, 0x0C, 0x03,
            0x00, 0xFF,
        ],
    );

    let relative = grafton_visca::command::pan_tilt::Relative::for_target(
        &grafton_visca::config::TargetRegistration::new(
            grafton_visca::types::CameraId::CAMERA_1,
            profile.clone(),
        )
        .expect("target registration"),
        grafton_visca::types::PanTiltPosition {
            pan: Degrees::new(45.0).expect("valid degrees"),
            tilt: Degrees::new(15.0).expect("valid degrees"),
        },
        grafton_visca::types::PanTiltSpeed {
            pan: pan_speed(0x09),
            tilt: tilt_speed(0x09),
        },
    )
    .expect("BRC-300 relative position");
    assert_golden(
        "Sony BRC-300 relative position",
        &relative,
        &[
            0x81, 0x01, 0x06, 0x03, 0x09, 0x00, 0x0F, 0x0D, 0x0B, 0x07, 0x00, 0x00, 0x0C, 0x03,
            0x00, 0xFF,
        ],
    );

    let fastest = grafton_visca::command::pan_tilt::Absolute::for_target(
        &grafton_visca::config::TargetRegistration::new(
            grafton_visca::types::CameraId::CAMERA_1,
            profile.clone(),
        )
        .expect("target registration"),
        grafton_visca::types::PanTiltPosition {
            pan: Degrees::new(45.0).expect("valid degrees"),
            tilt: Degrees::new(15.0).expect("valid degrees"),
        },
        grafton_visca::types::PanTiltSpeed {
            pan: pan_speed(0x18),
            tilt: tilt_speed(0x18),
        },
    )
    .expect("BRC-300 highest documented paired speed");
    assert_golden(
        "Sony BRC-300 absolute position at speed 0x18",
        &fastest,
        &[
            0x81, 0x01, 0x06, 0x02, 0x18, 0x00, 0x0F, 0x0D, 0x0B, 0x07, 0x00, 0x00, 0x0C, 0x03,
            0x00, 0xFF,
        ],
    );

    let limit = grafton_visca::command::pan_tilt::SetLimit::for_target(
        &grafton_visca::config::TargetRegistration::new(
            grafton_visca::types::CameraId::CAMERA_1,
            profile.clone(),
        )
        .expect("target registration"),
        PanTiltLimitCorner::UpRight,
        grafton_visca::types::PanTiltPosition {
            pan: Degrees::new(45.0).expect("valid degrees"),
            tilt: Degrees::new(15.0).expect("valid degrees"),
        },
    )
    .expect("BRC-300 limit position");
    assert_golden(
        "Sony BRC-300 limit set",
        &limit,
        &[
            0x81, 0x01, 0x06, 0x07, 0x00, 0x01, 0x0F, 0x0D, 0x0B, 0x07, 0x00, 0x00, 0x0C, 0x03,
            0x00, 0xFF,
        ],
    );

    let clear = grafton_visca::command::pan_tilt::ClearLimit::for_target(
        &grafton_visca::config::TargetRegistration::new(
            grafton_visca::types::CameraId::CAMERA_1,
            profile.clone(),
        )
        .expect("target registration"),
        PanTiltLimitCorner::UpRight,
    )
    .expect("BRC-300 limit clear");
    assert_golden(
        "Sony BRC-300 limit clear",
        &clear,
        &[
            0x81, 0x01, 0x06, 0x07, 0x01, 0x01, 0x07, 0x0F, 0x0F, 0x0F, 0x0F, 0x07, 0x0F, 0x0F,
            0x0F, 0xFF,
        ],
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
    assert_golden(
        "zoom stop",
        &grafton_visca::command::zoom::Stop,
        patterns::zoom::STOP,
    );
    assert_golden(
        "zoom tele",
        &grafton_visca::command::zoom::Tele,
        patterns::zoom::TELE_STD,
    );
    assert_golden(
        "zoom wide",
        &grafton_visca::command::zoom::Wide,
        patterns::zoom::WIDE_STD,
    );

    let speed = ZoomSpeed::new(5).expect("valid zoom speed");
    assert_golden(
        "zoom tele variable",
        &grafton_visca::command::zoom::TeleVariable::for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                grafton_visca::profiles::ProfileSpec::from_compile_time::<
                    grafton_visca::profiles::PtzOpticsG2,
                >()
                .expect("source profile"),
            )
            .expect("target registration"),
            speed,
        )
        .expect("profile-aware request"),
        patterns::zoom::TELE_VARIABLE_5,
    );
    assert_golden(
        "zoom wide variable",
        &grafton_visca::command::zoom::WideVariable::for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                grafton_visca::profiles::ProfileSpec::from_compile_time::<
                    grafton_visca::profiles::PtzOpticsG2,
                >()
                .expect("source profile"),
            )
            .expect("target registration"),
            speed,
        )
        .expect("profile-aware request"),
        patterns::zoom::WIDE_VARIABLE_5,
    );

    assert_golden(
        "zoom direct",
        &grafton_visca::command::zoom::SetPosition::new(
            ZoomPosition::new(0x1234).expect("valid zoom position"),
        ),
        patterns::zoom::DIRECT_0X1234,
    );
}

#[test]
fn focus_drive_and_direct_target_match_golden_frames() {
    assert_golden(
        "focus stop",
        &grafton_visca::command::focus::Stop,
        patterns::focus::STOP,
    );
    assert_golden(
        "focus far",
        &grafton_visca::command::focus::Far,
        patterns::focus::FAR,
    );
    assert_golden(
        "focus near",
        &grafton_visca::command::focus::Near,
        patterns::focus::NEAR,
    );

    let speed = FocusSpeed::new(5).expect("valid focus speed");
    assert_golden(
        "focus far variable",
        &grafton_visca::command::focus::FarVariable::for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                grafton_visca::profiles::ProfileSpec::from_compile_time::<
                    grafton_visca::profiles::PtzOpticsG2,
                >()
                .expect("source profile"),
            )
            .expect("target registration"),
            speed,
        )
        .expect("profile-aware request"),
        patterns::focus::FAR_VARIABLE_5,
    );
    assert_golden(
        "focus near variable",
        &grafton_visca::command::focus::NearVariable::for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                grafton_visca::profiles::ProfileSpec::from_compile_time::<
                    grafton_visca::profiles::PtzOpticsG2,
                >()
                .expect("source profile"),
            )
            .expect("target registration"),
            speed,
        )
        .expect("profile-aware request"),
        patterns::focus::NEAR_VARIABLE_5,
    );

    assert_golden(
        "focus infinity",
        &grafton_visca::command::focus::Infinity,
        patterns::focus::INFINITY,
    );
    assert_golden(
        "focus auto",
        &grafton_visca::command::focus::SetMode::new(grafton_visca::types::FocusMode::Auto),
        patterns::focus::AUTO,
    );
    assert_golden(
        "focus manual",
        &grafton_visca::command::focus::SetMode::new(grafton_visca::types::FocusMode::Manual),
        patterns::focus::MANUAL,
    );

    assert_golden(
        "focus direct",
        &grafton_visca::command::focus::SetPosition::new(
            FocusPosition::new(0x2345).expect("valid value"),
        ),
        patterns::focus::DIRECT_0X2345,
    );
}

#[test]
fn preset_and_power_commands_match_golden_frames() {
    let profile = ptz_optics_profile();
    let slot = PresetNumber::new(1).expect("valid preset slot");

    assert_golden(
        "preset recall",
        &grafton_visca::command::presets::Recall::for_target(
            &grafton_visca::config::TargetRegistration::new(CameraId::CAMERA_1, profile.clone())
                .expect("target registration"),
            slot,
        )
        .expect("profile declares preset recall"),
        patterns::preset::RECALL_1,
    );
    assert_golden(
        "preset set",
        &grafton_visca::command::presets::Store::new(slot),
        patterns::preset::SET_1,
    );
    assert_golden(
        "preset reset",
        &grafton_visca::command::presets::Clear::new(slot),
        patterns::preset::RESET_1,
    );

    assert_golden(
        "power on",
        &grafton_visca::command::power::On,
        patterns::power::ON,
    );
    assert_golden(
        "power standby",
        &grafton_visca::command::power::Standby,
        patterns::power::STANDBY,
    );
}

/// Regression pin for #683: preset slot 255 has a trailing `0xFF` data byte and
/// must still be terminated. The old const builder mistook that data byte for
/// the terminator and wrote the frame one byte short, so these goldens (which
/// are `MAX_SIZE` long) fail before the fix and pass after.
#[test]
fn preset_slot_255_stays_terminated() {
    // This encoder boundary fixture declares a synthetic 256-slot inventory;
    // the built-in G2 profile correctly limits its physical inventory to 128.
    let source = ptz_optics_profile();
    let mut capabilities = source.capabilities().clone();
    capabilities.profile_id = None;
    capabilities.model_name = "Synthetic 256-slot preset camera".into();
    capabilities
        .presets
        .as_mut()
        .expect("preset inventory")
        .highest = 255;
    let profile = profile_encodings::with_source_encodings(
        ProfileSpec::builder(capabilities)
            .transports(source.transports())
            .envelope(source.envelope())
            .timing(source.timing())
            .maximum_command_sockets(source.maximum_command_sockets())
            .supports_operation_complete(source.supports_operation_complete())
            .supports_command_cancel(source.supports_command_cancel())
            .position_inquiries(source.position_inquiries()),
        &source,
    )
    .build()
    .expect("synthetic profile declares all wire preset numbers");
    let slot = PresetNumber::new(255).expect("255 is a valid preset slot");

    assert_golden(
        "preset recall 255",
        &grafton_visca::command::presets::Recall::for_target(
            &grafton_visca::config::TargetRegistration::new(CameraId::CAMERA_1, profile.clone())
                .expect("target registration"),
            slot,
        )
        .expect("profile declares preset recall"),
        patterns::preset::RECALL_255,
    );
    assert_golden(
        "preset set 255",
        &grafton_visca::command::presets::Store::new(slot),
        patterns::preset::SET_255,
    );
    assert_golden(
        "preset reset 255",
        &grafton_visca::command::presets::Clear::new(slot),
        patterns::preset::RESET_255,
    );
}
