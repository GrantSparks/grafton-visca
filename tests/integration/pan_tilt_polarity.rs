//! Pan/tilt sign convention, pinned per built-in profile.
//!
//! Degrees are camera-independent: positive pan is right and positive tilt
//! is up. Each profile's signed degree-to-unit scales map them onto its own
//! raw axes, whose direction its source documents. This test encodes an
//! absolute +45° move for every built-in profile and checks the raw words
//! point the documented "right" and "up" way.

#![allow(clippy::expect_used)]

use std::collections::BTreeSet;

use grafton_visca::{
    capabilities::PanTiltWireCodec,
    profiles::{
        CompileTimeProfile, NearusBrc300, ProfileId, ProfileSpec, PtzOptics30x, PtzOpticsG2,
        PtzOpticsG3, SonyBrc300, SonyBrcH900, SonyEviH100, SonyFr7,
    },
    request::Request,
    types::{Degrees, PanSpeed, TiltSpeed},
};

/// Which way an increasing raw word moves the camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Raw {
    Increasing,
    Decreasing,
}

/// One profile's documented raw directions for "right" and "up", and where
/// they come from.
struct Polarity {
    id: ProfileId,
    spec: ProfileSpec,
    right: Raw,
    up: Raw,
    basis: &'static str,
}

fn polarity<P: CompileTimeProfile>(right: Raw, up: Raw, basis: &'static str) -> Polarity {
    Polarity {
        id: P::ID.expect("built-in profile identity"),
        spec: ProfileSpec::from_compile_time::<P>().expect("built-in profile"),
        right,
        up,
        basis,
    }
}

fn table() -> Vec<Polarity> {
    use Raw::{Decreasing, Increasing};

    // The Sony-standard convention (R8): increasing raw pan is right and
    // increasing raw tilt is up. The PTZOptics sources give degree ranges
    // but no raw direction, so those rows follow it unverified
    // (see "Known unverified facts" in docs/camera_profile_support.md).
    const STANDARD: &str = "Sony-standard raw axes; model source gives no direction (unverified)";
    vec![
        polarity::<PtzOpticsG2>(Increasing, Increasing, STANDARD),
        polarity::<PtzOpticsG3>(Increasing, Increasing, STANDARD),
        polarity::<PtzOptics30x>(Increasing, Increasing, STANDARD),
        polarity::<SonyFr7>(
            Decreasing,
            Increasing,
            "R7 p.12: 09CA7 is +170° CCW/left; 0B3B0 is +195° up",
        ),
        polarity::<SonyBrcH900>(
            Decreasing,
            Increasing,
            "R11 v2.20 p.37: positive pan 09BDE is left; tilt 52F8 is up",
        ),
        polarity::<SonyEviH100>(
            Increasing,
            Increasing,
            "R8 limit table: UpRight pan 0001..1E1B, tilt 0001..0FF0 (+90°)",
        ),
        polarity::<SonyBrc300>(
            Decreasing,
            Increasing,
            "R12 value table: 08A58 left end, 493D up end",
        ),
        polarity::<NearusBrc300>(
            Decreasing,
            Increasing,
            "R21 limit table: 08A58 left end, 493D up end",
        ),
    ]
}

/// Decodes the signed pan and tilt words of an absolute-position frame.
fn raw_words(frame: &[u8], codec: PanTiltWireCodec) -> (i32, i32) {
    let nibbles = |bytes: &[u8]| {
        bytes
            .iter()
            .fold(0_u32, |word, byte| (word << 4) | u32::from(byte & 0x0F))
    };
    let sign_extend = |word: u32, bits: u32| ((word << (32 - bits)) as i32) >> (32 - bits);
    match codec {
        // 81 01 06 02 VV WW 0p 0p 0p 0p 0t 0t 0t 0t FF
        PanTiltWireCodec::StandardVisca => (
            sign_extend(nibbles(&frame[6..10]), 16),
            sign_extend(nibbles(&frame[10..14]), 16),
        ),
        // 81 01 06 02 VV 00 0p 0p 0p 0p 0p 0t 0t 0t 0t FF
        PanTiltWireCodec::SonyBrc300 => (
            sign_extend(nibbles(&frame[6..11]), 20),
            sign_extend(nibbles(&frame[11..15]), 16),
        ),
        // 81 01 06 02 VV WW 0p 0p 0p 0p 0p 0t 0t 0t 0t 0t FF
        PanTiltWireCodec::SonyFr7 => (
            sign_extend(nibbles(&frame[6..11]), 20),
            sign_extend(nibbles(&frame[11..16]), 20),
        ),
        other => panic!("no polarity decoder for {other:?}"),
    }
}

fn direction(word: i32) -> Raw {
    assert_ne!(word, 0, "+45° must move off centre");
    if word > 0 {
        Raw::Increasing
    } else {
        Raw::Decreasing
    }
}

#[test]
fn positive_degrees_are_right_and_up_for_every_built_in_profile() {
    let table = table();
    let covered: BTreeSet<_> = table.iter().map(|row| format!("{:?}", row.id)).collect();
    let all: BTreeSet<_> = ProfileId::all()
        .iter()
        .filter(|id| id.spec().expect("profile").pan_tilt_coordinates().is_some())
        .map(|id| format!("{id:?}"))
        .collect();
    assert_eq!(covered, all, "every built-in profile has a polarity row");

    for row in table {
        let codec = row
            .spec
            .pan_tilt_coordinates()
            .expect("absolute pan/tilt conversion")
            .wire_codec();
        let request = grafton_visca::command::pan_tilt::Absolute::new(
            grafton_visca::types::PanTiltPosition {
                pan: Degrees::new(45.0).expect("valid degrees"),
                tilt: Degrees::new(45.0).expect("valid degrees"),
            },
            grafton_visca::types::PanTiltSpeed {
                pan: PanSpeed::new(1).expect("pan speed"),
                tilt: TiltSpeed::new(1).expect("tilt speed"),
            },
        );
        let mut frame = vec![0_u8; grafton_visca::command::pan_tilt::Absolute::MAX_SIZE];
        let target = grafton_visca::config::TargetRegistration::new(
            grafton_visca::types::IndividualCameraId::CAMERA_1,
            row.spec.clone(),
        );
        let written = target
            .encode(&request, &mut frame)
            .unwrap_or_else(|error| panic!("{:?}: +45°/+45° must encode: {error}", row.id));
        frame.truncate(written);

        let (pan, tilt) = raw_words(&frame, codec);
        assert_eq!(
            direction(pan),
            row.right,
            "{:?}: +45° pan must move right ({})",
            row.id,
            row.basis
        );
        assert_eq!(
            direction(tilt),
            row.up,
            "{:?}: +45° tilt must move up ({})",
            row.id,
            row.basis
        );
    }
}
