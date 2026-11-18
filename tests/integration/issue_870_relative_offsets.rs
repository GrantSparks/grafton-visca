//! Reachable relative offsets are independent of absolute position bounds.

#![allow(clippy::expect_used)]

use grafton_visca::{
    capabilities::{CapabilityRange, PanTiltWireCodec},
    config::{MountPosition, TargetRegistration},
    profiles::{
        NearusBrc300, ProfileId, ProfileSpec, PtzOptics30x, PtzOpticsG2, PtzOpticsG3, SonyBrc300,
        SonyBrcH900, SonyEviH100, SonyFr7,
    },
    request::Request,
    types::{CameraId, Degrees, PanSpeed, TiltSpeed},
};
use proptest::prelude::*;

fn targets() -> Vec<TargetRegistration> {
    let profiles = [
        ProfileSpec::from_compile_time::<PtzOpticsG2>(),
        ProfileSpec::from_compile_time::<PtzOpticsG3>(),
        ProfileSpec::from_compile_time::<PtzOptics30x>(),
        ProfileSpec::from_compile_time::<SonyFr7>(),
        ProfileSpec::from_compile_time::<SonyBrcH900>(),
        ProfileSpec::from_compile_time::<SonyEviH100>(),
        ProfileSpec::from_compile_time::<SonyBrc300>(),
        ProfileSpec::from_compile_time::<NearusBrc300>(),
    ];
    assert_eq!(
        profiles.len(),
        ProfileId::all()
            .iter()
            .filter(|id| id.spec().expect("profile").pan_tilt_coordinates().is_some())
            .count(),
        "cover every built-in with sourced geometry"
    );
    let mut targets = profiles
        .into_iter()
        .map(|profile| {
            TargetRegistration::new(
                grafton_visca::types::IndividualCameraId::CAMERA_1,
                profile.expect("profile"),
            )
        })
        .collect::<Vec<_>>();
    targets.push(
        TargetRegistration::new(
            grafton_visca::types::IndividualCameraId::CAMERA_1,
            ProfileSpec::from_compile_time::<SonyFr7>().expect("FR7"),
        )
        .with_mount(MountPosition::Ceiling)
        .expect("ceiling"),
    );
    targets
}

fn words<R: Request>(request: &R, target: &TargetRegistration) -> (i32, i32) {
    let codec = target
        .profile()
        .pan_tilt_coordinates()
        .expect("coordinates")
        .wire_codec();
    let (pan_nibbles, tilt_nibbles) = match codec {
        PanTiltWireCodec::StandardVisca => (4, 4),
        PanTiltWireCodec::SonyBrc300 => (5, 4),
        PanTiltWireCodec::SonyFr7 => (5, 5),
        _ => panic!("add the new codec to the independent word reader"),
    };
    let mut frame = [0_u8; 17];
    let len = request
        .write_into(CameraId::CAMERA_1, &mut frame)
        .expect("frame");
    assert_eq!(len, 7 + pan_nibbles + tilt_nibbles);
    // Read signed words independently of the production framing decoder.
    let signed = |nibbles: &[u8]| {
        let word = nibbles.iter().fold(0_i32, |word, nibble| {
            assert!(*nibble < 16);
            (word << 4) | i32::from(*nibble)
        });
        let bits = nibbles.len() * 4;
        if word & (1 << (bits - 1)) == 0 {
            word
        } else {
            word - (1 << bits)
        }
    };
    (
        signed(&frame[6..6 + pan_nibbles]),
        signed(&frame[6 + pan_nibbles..len - 1]),
    )
}

fn relative(target: &TargetRegistration, pan: f32, tilt: f32) -> (i32, i32) {
    let request = grafton_visca::command::pan_tilt::Relative::for_target(
        target,
        grafton_visca::types::PanTiltPosition {
            pan: Degrees::new(pan).expect("valid degrees"),
            tilt: Degrees::new(tilt).expect("valid degrees"),
        },
        grafton_visca::types::PanTiltSpeed {
            pan: PanSpeed::new(1).expect("speed"),
            tilt: TiltSpeed::new(1).expect("speed"),
        },
    )
    .expect("reachable offset");
    words(&request, target)
}

fn absolute(target: &TargetRegistration, pan: f32, tilt: f32) -> (i32, i32) {
    let request = grafton_visca::command::pan_tilt::Absolute::for_target(
        target,
        grafton_visca::types::PanTiltPosition {
            pan: Degrees::new(pan).expect("valid degrees"),
            tilt: Degrees::new(tilt).expect("valid degrees"),
        },
        grafton_visca::types::PanTiltSpeed {
            pan: PanSpeed::new(1).expect("speed"),
            tilt: TiltSpeed::new(1).expect("speed"),
        },
    )
    .expect("reachable absolute point");
    words(&request, target)
}

fn choose(range: CapabilityRange<i32>, seed: u32) -> i32 {
    range.min()
        + i32::try_from(seed % u32::try_from(range.max() - range.min() + 1).expect("span"))
            .expect("offset")
}

fn check_quantized_difference(
    target: &TargetRegistration,
    from: (i32, i32),
    to: (i32, i32),
) -> Result<(), proptest::test_runner::TestCaseError> {
    let geometry = target
        .profile()
        .capabilities()
        .pan_tilt
        .and_then(|domain| domain.geometry)
        .expect("geometry");
    let conversion = geometry.coordinates;
    let pan_degrees = geometry.pan_range_degrees();
    let tilt_degrees = target.tilt_range_degrees().expect("tilt degrees");
    // A sourced wire endpoint can inverse-convert just outside its nominal
    // degree range (EVI-H100's -907 is one example). Keep the exact inverse
    // for interior values and use the authored limit for that endpoint.
    let point = |(pan, tilt)| {
        (
            conversion
                .pan_degrees(pan)
                .clamp(*pan_degrees.start(), *pan_degrees.end()),
            conversion
                .tilt_degrees(tilt)
                .clamp(*tilt_degrees.start(), *tilt_degrees.end()),
        )
    };
    let start = point(from);
    let end = point(to);
    prop_assert_eq!(absolute(target, start.0, start.1), from);
    prop_assert_eq!(absolute(target, end.0, end.1), to);
    prop_assert_eq!(
        relative(target, end.0 - start.0, end.1 - start.1),
        (to.0 - from.0, to.1 - from.1)
    );
    Ok(())
}

#[test]
fn every_builtin_span_boundary_encodes_and_one_more_unit_is_refused() {
    for target in targets() {
        let caps = target
            .profile()
            .capabilities()
            .pan_tilt
            .as_ref()
            .expect("pan/tilt");
        let geometry = caps.geometry.expect("geometry");
        let conversion = geometry.coordinates;
        let pan_span = geometry.pan_range.max() - geometry.pan_range.min();
        let nominal = |degrees: f32| match geometry.endpoint_quantization {
            grafton_visca::capabilities::EndpointQuantization::WholeDegrees => degrees.round(),
            _ => degrees,
        };
        let tilt = target.tilt_range().expect("tilt");
        let tilt_span = tilt.max() - tilt.min();
        let lower_corner = (geometry.pan_range.min(), tilt.min());
        let upper_corner = (geometry.pan_range.max(), tilt.max());
        check_quantized_difference(&target, lower_corner, upper_corner)
            .expect("lower to upper endpoint difference");
        check_quantized_difference(&target, upper_corner, lower_corner)
            .expect("upper to lower endpoint difference");
        for (pan, tilt) in [
            (pan_span, 0),
            (-pan_span, 0),
            (0, tilt_span),
            (0, -tilt_span),
        ] {
            assert_eq!(
                relative(
                    &target,
                    nominal(conversion.pan_degrees(pan)),
                    nominal(conversion.tilt_degrees(tilt))
                ),
                (pan, tilt)
            );
            let (outside_pan, outside_tilt) = if pan == 0 {
                (0, tilt + tilt.signum())
            } else {
                (pan + pan.signum(), 0)
            };
            let result = grafton_visca::command::pan_tilt::Relative::for_target(
                &target,
                grafton_visca::types::PanTiltPosition {
                    pan: Degrees::new(conversion.pan_degrees(outside_pan)).expect("valid degrees"),
                    tilt: Degrees::new(conversion.tilt_degrees(outside_tilt))
                        .expect("valid degrees"),
                },
                grafton_visca::types::PanTiltSpeed {
                    pan: PanSpeed::new(1).expect("speed"),
                    tilt: TiltSpeed::new(1).expect("speed"),
                },
            );
            assert!(
                matches!(result, Err(grafton_visca::Error::InvalidParameter { .. })),
                "{} {:?}: one unit past ({pan}, {tilt})",
                target.profile().name(),
                target.mount()
            );
        }
    }
}

#[test]
fn nonfinite_relative_offsets_are_rejected_before_request_construction() {
    for nonfinite in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(matches!(
            Degrees::new(nonfinite),
            Err(grafton_visca::Error::InvalidParameter {
                parameter: "Degrees",
                ..
            })
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn quantized_endpoint_differences_encode_exact_signed_offsets(seeds in any::<[u32; 4]>()) {
        for target in targets() {
            let pan = target.profile().capabilities().pan_tilt
                .and_then(|domain| domain.geometry).expect("geometry").pan_range;
            let tilt = target.tilt_range().expect("tilt");
            check_quantized_difference(
                &target,
                (choose(pan, seeds[0]), choose(tilt, seeds[2])),
                (choose(pan, seeds[1]), choose(tilt, seeds[3])),
            )?;
        }
    }

    #[test]
    fn independently_rounded_continuous_points_differ_by_at_most_one_unit(fractions in prop::array::uniform4(0.01_f32..0.99)) {
        for target in targets() {
            let caps = target.profile().capabilities().pan_tilt.as_ref().expect("pan/tilt");
            let pan = caps.geometry.expect("geometry").pan_range_degrees();
            let tilt = target.tilt_range_degrees().expect("tilt degrees");
            let interpolate = |range: &std::ops::RangeInclusive<f32>, fraction: f32| range.start() + (range.end() - range.start()) * fraction;
            let start = (interpolate(&pan, fractions[0]), interpolate(&tilt, fractions[1]));
            let end = (interpolate(&pan, fractions[2]), interpolate(&tilt, fractions[3]));
            let from = absolute(&target, start.0, start.1);
            let to = absolute(&target, end.0, end.1);
            let delta = relative(&target, end.0 - start.0, end.1 - start.1);
            // round(y-x) is not generally round(y)-round(x). Independent
            // nearest rounding has at most one unit of discrepancy. Exact
            // equality for quantized endpoint inputs is tested above.
            prop_assert!((delta.0 - (to.0 - from.0)).abs() <= 1);
            prop_assert!((delta.1 - (to.1 - from.1)).abs() <= 1);
        }
    }
}

#[cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]
mod facades {
    use grafton_visca::{
        config::SessionConfig,
        profiles::PtzOpticsG2,
        types::{Degrees, SpeedLevel},
        Error,
    };
    use grafton_visca_test_support::{
        facade_matrix,
        fake_camera::{self, FakeCamera},
    };

    const RELATIVE: [u8; 15] = [0x81, 1, 6, 3, 1, 1, 0, 0, 0, 0, 15, 13, 7, 8, 0xff];

    fn camera() -> FakeCamera {
        FakeCamera::visca(|_, answer| {
            answer
                .reply(fake_camera::frames::ack(1))
                .reply(fake_camera::frames::complete(1));
        })
    }

    facade_matrix! {
        paused:

        fn g2_out_of_absolute_range_relative_move_reaches_the_registered_target() {
            let fake = camera();
            let session = open!(fake, SessionConfig::from_compile_time::<PtzOpticsG2>().expect("G2 config")).expect("session");
            let view = session.camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1).expect("G2 view");
            let error = wait!(view.pan_tilt().absolute(grafton_visca::types::PanTiltPosition { pan: Degrees::new(0.0).expect("valid degrees"), tilt: Degrees::new(-45.0).expect("valid degrees") }, SpeedLevel::Slowest)).expect_err("absolute refusal");
            assert!(matches!(error, Error::InvalidParameter { .. }));
            assert_eq!(fake.write_count(), 0);
            let mut operation = wait!(view.pan_tilt().relative(grafton_visca::types::PanTiltPosition { pan: Degrees::new(0.0).expect("valid degrees"), tilt: Degrees::new(-45.0).expect("valid degrees") }, SpeedLevel::Slowest)).expect("relative admission");
            wait!(operation.applied()).expect("relative applied");
            assert_eq!(fake.take_only_payload(), RELATIVE);
            wait!(session.close()).expect("close");
        }
    }

    #[cfg(all(
        any(feature = "async", feature = "blocking"),
        feature = "runtime-tokio"
    ))]
    #[tokio::test(start_paused = true)]
    async fn dynamic_g2_uses_the_same_relative_span_and_signed_words() {
        use grafton_visca::{r#async::Session, runtime::TokioExecutor};
        let fake = camera();
        let session = Session::open(
            fake.async_wire(),
            &SessionConfig::from_compile_time::<PtzOpticsG2>().expect("G2 config"),
            TokioExecutor::from_current().expect("Tokio executor"),
        )
        .await
        .expect("session");
        let view = session
            .camera::<grafton_visca::profiles::AnyProfile>(
                grafton_visca::types::IndividualCameraId::CAMERA_1,
            )
            .expect("dynamic view");
        let error = view
            .pan_tilt()
            .absolute(
                grafton_visca::types::PanTiltPosition {
                    pan: Degrees::new(0.0).expect("valid degrees"),
                    tilt: Degrees::new(-45.0).expect("valid degrees"),
                },
                SpeedLevel::Slowest,
            )
            .await
            .expect_err("absolute refusal");
        assert!(matches!(error, Error::InvalidParameter { .. }));
        assert_eq!(fake.write_count(), 0);
        let mut operation = view
            .pan_tilt()
            .relative(
                grafton_visca::types::PanTiltPosition {
                    pan: Degrees::new(0.0).expect("valid degrees"),
                    tilt: Degrees::new(-45.0).expect("valid degrees"),
                },
                SpeedLevel::Slowest,
            )
            .await
            .expect("dynamic relative");
        operation.applied().await.expect("applied");
        assert_eq!(fake.take_only_payload(), RELATIVE);
        session.close().await.expect("close");
    }
}
