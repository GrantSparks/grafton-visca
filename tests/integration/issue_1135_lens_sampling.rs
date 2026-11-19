//! Profile-invalid lens replies cannot establish physical stability (#1135).

#![cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]
#![allow(clippy::expect_used)]

use std::{
    sync::{
        atomic::{AtomicU16, Ordering},
        Arc,
    },
    time::Duration,
};

use grafton_visca::{
    command::{exposure, focus, zoom},
    config::SessionConfig,
    operation::Settlement,
    profiles::{GenericVisca, SonyBrcH900, SonyEviH100, SonyFr7},
    types::{AffectedAxes, FocusPosition, IdleWait, IndividualCameraId, MotionQuery, ZoomPosition},
    Error,
};
use grafton_visca_test_support::{
    facade_matrix,
    fake_camera::{frames, FakeCamera},
};

const ZOOM: [u8; 5] = [0x81, 0x09, 0x04, 0x47, 0xff];
const FOCUS: [u8; 5] = [0x81, 0x09, 0x04, 0x48, 0xff];
const IRIS: [u8; 5] = [0x81, 0x09, 0x04, 0x4b, 0xff];
const WAIT: Duration = Duration::from_secs(2);

fn sample_payload(position: u16) -> [u8; 4] {
    [12, 8, 4, 0].map(|shift| u8::try_from((position >> shift) & 0xf).expect("nibble"))
}

fn camera_reporting(position: &Arc<AtomicU16>) -> FakeCamera {
    let position = Arc::clone(position);
    FakeCamera::visca(move |request, answer| {
        if request == ZOOM || request == FOCUS || request == IRIS {
            answer.reply(frames::inquiry_reply(&sample_payload(
                position.load(Ordering::Relaxed),
            )));
        } else {
            answer.reply(frames::ack(1)).reply(frames::complete(1));
        }
    })
}

fn assert_invalid_sample(error: Error, position: u16) {
    assert!(
        matches!(error, Error::InvalidResponse { actual, .. } if actual == sample_payload(position))
    );
}

facade_matrix! {
    paused:

    fn impossible_evi_h100_samples_fail_all_observation_paths() {
        for (axis, inquiry, invalid, valid) in [
            (AffectedAxes::ZOOM, ZOOM, 0x7ac1, 0x7ac0),
            (AffectedAxes::FOCUS, FOCUS, 0x0001, 0x1000),
            (AffectedAxes::IRIS, IRIS, 0x0001, 0x0005),
        ] {
            let position = Arc::new(AtomicU16::new(invalid));
            let fake = camera_reporting(&position);
            let config = SessionConfig::from_compile_time::<SonyEviH100>().expect("EVI-H100 config");
            let session = open!(fake, config).expect("EVI-H100 session");
            let camera = session.camera::<SonyEviH100>(IndividualCameraId::CAMERA_1).expect("EVI-H100 view");

            let raw = match axis {
                AffectedAxes::ZOOM => wait!(camera.inquire(&zoom::PositionInquiry)).expect("raw zoom position").value(),
                AffectedAxes::FOCUS => wait!(camera.inquire(&focus::PositionInquiry)).expect("raw focus position").value(),
                AffectedAxes::IRIS => u16::from(wait!(camera.inquire(&exposure::IrisInquiry)).expect("raw iris position").value()),
                _ => unreachable!("lens axis"),
            };
            assert_eq!(raw, invalid);
            assert_eq!(fake.take_payloads(), [inquiry]);

            let moving = wait!(camera.motion().is_moving(MotionQuery::new(axis)));
            assert_invalid_sample(moving.expect_err("impossible position cannot show movement"), invalid);
            assert_eq!(fake.take_payloads(), [inquiry]);

            let idle = wait!(camera.motion().wait_until_idle(IdleWait::new(axis, WAIT)));
            assert_invalid_sample(idle.expect_err("impossible position cannot show idleness"), invalid);
            assert_eq!(fake.take_payloads(), [inquiry]);

            let mut operation = match axis {
                AffectedAxes::ZOOM => wait!(camera.zoom().set_position(ZoomPosition::MIN)),
                AffectedAxes::FOCUS => wait!(camera.focus().set_position(FocusPosition::new(0x1000).expect("focus position"))),
                AffectedAxes::IRIS => wait!(camera.exposure().iris_reset()),
                _ => unreachable!("lens axis"),
            }.expect("lens operation admitted");
            wait!(operation.applied()).expect("lens operation applied");
            assert_eq!(fake.take_payloads().len(), 1);
            for _ in 0..2 {
                let error = wait!(operation.settled_with_timeout(WAIT)).expect_err("impossible position cannot settle");
                match error {
                    Error::SettlementObservationFailed { source, .. } => assert_invalid_sample(*source, invalid),
                    other => panic!("expected a failed settlement observation, got {other:?}"),
                }
                assert_eq!(fake.take_payloads(), [inquiry]);
            }

            position.store(valid, Ordering::Relaxed);
            let evidence = wait!(operation.settled_with_timeout(WAIT)).expect("valid samples establish stability");
            assert!(matches!(evidence, Settlement::ObservedStable { axes, .. } if axes == axis));
            assert_eq!(fake.take_payloads(), [inquiry; 2]);
            let cached = wait!(operation.settled_with_timeout(WAIT)).expect("cached evidence");
            assert_eq!(evidence, cached);
            assert!(fake.take_payloads().is_empty());
            wait!(session.close()).expect("close EVI-H100 session");
        }
    }

    fn evi_h100_sampling_accepts_inclusive_lens_endpoints() {
        let position = Arc::new(AtomicU16::new(0));
        let fake = camera_reporting(&position);
        let config = SessionConfig::from_compile_time::<SonyEviH100>().expect("EVI-H100 config");
        let session = open!(fake, config).expect("EVI-H100 session");
        let camera = session.camera::<SonyEviH100>(IndividualCameraId::CAMERA_1).expect("EVI-H100 view");
        for (axis, inquiry, values) in [
            (AffectedAxes::ZOOM, ZOOM, [0, 0x4000, 0x7ac0]),
            (AffectedAxes::FOCUS, FOCUS, [0x1000, 0x8000, 0xf000]),
            (AffectedAxes::IRIS, IRIS, [0, 5, 0x11]),
        ] {
            for value in values {
                position.store(value, Ordering::Relaxed);
                let moving = wait!(camera.motion().is_moving(MotionQuery::new(axis)))
                    .expect("documented lens position");
                assert!(!moving, "identical {axis:?} samples at {value:#06x}");
                assert_eq!(fake.take_payloads(), [inquiry; 2]);
            }
        }
        wait!(session.close()).expect("close EVI-H100 session");
    }

    fn fr7_sampling_accepts_clear_image_zoom_and_full_focus_range() {
        let position = Arc::new(AtomicU16::new(0));
        let fake = camera_reporting(&position);
        let config = SessionConfig::from_compile_time::<SonyFr7>().expect("FR7 config");
        let session = open!(fake, config).expect("FR7 session");
        let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).expect("FR7 view");
        for (axis, inquiry, values) in [
            (AffectedAxes::ZOOM, ZOOM, [0x4000, 0x5556, 0x6000]),
            (AffectedAxes::FOCUS, FOCUS, [0, 1, 0xffff]),
        ] {
            for value in values {
                position.store(value, Ordering::Relaxed);
                let moving = wait!(camera.motion().is_moving(MotionQuery::new(axis)))
                    .expect("documented lens position");
                assert!(!moving, "identical {axis:?} samples at {value:#06x}");
                assert_eq!(fake.take_payloads(), [inquiry; 2]);
            }
        }
        position.store(0x6001, Ordering::Relaxed);
        let moving = wait!(camera.motion().is_moving(MotionQuery::new(AffectedAxes::ZOOM)));
        assert_invalid_sample(moving.expect_err("outside the Clear Image Zoom range"), 0x6001);
        assert_eq!(fake.take_payloads(), [ZOOM]);
        wait!(session.close()).expect("close FR7 session");
    }
    fn optical_endpoint_by_itself_does_not_restrict_zoom_samples() {
        let position = Arc::new(AtomicU16::new(0x7fff));
        let fake = camera_reporting(&position);
        let config = SessionConfig::from_compile_time::<GenericVisca>().expect("generic VISCA config");
        let session = open!(fake, config).expect("generic VISCA session");
        let camera = session.camera::<GenericVisca>(IndividualCameraId::CAMERA_1).expect("generic VISCA view");
        let moving = wait!(camera.motion().is_moving(MotionQuery::new(AffectedAxes::ZOOM)))
            .expect("generic zoom readback uses the full typed wire domain");
        assert!(!moving);
        assert_eq!(fake.take_payloads(), [ZOOM; 2]);
        wait!(session.close()).expect("close generic VISCA session");
    }

    fn documented_optical_only_zoom_range_bounds_brc_h900_samples() {
        let position = Arc::new(AtomicU16::new(0x4000));
        let fake = camera_reporting(&position);
        let config = SessionConfig::from_compile_time::<SonyBrcH900>().expect("BRC-H900 config");
        let session = open!(fake, config).expect("BRC-H900 session");
        let camera = session.camera::<SonyBrcH900>(IndividualCameraId::CAMERA_1).expect("BRC-H900 view");
        let moving = wait!(camera.motion().is_moving(MotionQuery::new(AffectedAxes::ZOOM)))
            .expect("optical tele endpoint");
        assert!(!moving);
        assert_eq!(fake.take_payloads(), [ZOOM; 2]);
        position.store(0x4001, Ordering::Relaxed);
        let moving = wait!(camera.motion().is_moving(MotionQuery::new(AffectedAxes::ZOOM)));
        assert_invalid_sample(moving.expect_err("outside the documented optical range"), 0x4001);
        assert_eq!(fake.take_payloads(), [ZOOM]);
        wait!(session.close()).expect("close BRC-H900 session");
    }

}
