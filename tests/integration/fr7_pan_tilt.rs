//! R7 position frames and polling through the real owner facades.

#![cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]
#![allow(clippy::expect_used)]

use std::time::Duration;

use grafton_visca::{
    config::{CameraConfig, MountPosition, SessionConfig},
    operation::Settlement,
    profiles::SonyFr7,
    types::{AffectedAxes, Degrees, IdleWait, MotionQuery, PanTiltLimitCorner, SpeedLevel},
    Error,
};
use grafton_visca_test_support::{
    facade_matrix,
    fake_camera::{self, FakeCamera},
};

const POSITION: [u8; 13] = [0x90, 0x50, 0, 9, 12, 10, 7, 0, 11, 3, 11, 0, 0xff];
const POSITION_INQUIRY: [u8; 5] = [0x81, 9, 6, 0x12, 0xff];

fn camera() -> FakeCamera {
    position_camera(|| POSITION)
}

fn position_camera(mut reply: impl FnMut() -> [u8; 13] + Send + 'static) -> FakeCamera {
    FakeCamera::visca(move |request, answer| {
        if request == POSITION_INQUIRY {
            answer.reply(reply());
        } else {
            answer
                .reply(fake_camera::frames::ack(1))
                .reply(fake_camera::frames::complete(1));
        }
    })
}

fn position_reply((pan, tilt): (i32, i32)) -> [u8; 13] {
    let nibbles = |word| {
        [16, 12, 8, 4, 0].map(|shift| u8::try_from((word >> shift) & 0x0f).expect("masked nibble"))
    };
    let mut reply = POSITION;
    reply[2..7].copy_from_slice(&nibbles(pan));
    reply[7..12].copy_from_slice(&nibbles(tilt));
    reply
}

fn assert_invalid_position(error: &Error, reply: [u8; 13]) {
    assert!(
        matches!(error, Error::InvalidResponse { actual, .. } if actual == &reply[2..12]),
        "expected the offending pan/tilt words, got {error:?}",
    );
}

facade_matrix! {
    paused:

    fn fr7_positions_limits_and_motion_observation_use_r7_words() {
        let fake = camera();
        let config = SessionConfig::from_compile_time::<SonyFr7>().expect("FR7 config");
        let session = open!(fake, config).expect("FR7 session");
        let camera = session.camera::<SonyFr7>(grafton_visca::types::IndividualCameraId::CAMERA_1).expect("FR7 view");
        let mut operation = wait!(camera.pan_tilt().absolute(grafton_visca::types::PanTiltPosition { pan: Degrees::new(-170.0).expect("valid degrees"), tilt: Degrees::new(195.0).expect("valid degrees") }, SpeedLevel::Slowest)).expect("FR7 absolute");
        wait!(operation.settled()).expect("settles using ten position nibbles");
        let position = wait!(camera.pan_tilt().position()).expect("FR7 position");
        assert!((position.pan.value() + 170.0).abs() < 0.005);
        assert!((position.tilt.value() - 195.0).abs() < 0.005);
        let raw = grafton_visca::request::Inquiry::decoder_for_profile(
            &grafton_visca::command::pan_tilt::PositionInquiry,
            camera.profile(),
        ).expect("FR7 decoder").decode(&POSITION[2..12]).expect("R7 raw position words");
        assert_eq!((raw.pan, raw.tilt), (40103, 46000));
        assert!(!wait!(camera.motion().is_moving(MotionQuery::new(AffectedAxes::PAN_TILT))).expect("FR7 motion observation"));
        wait!(camera.motion().wait_until_idle(IdleWait::new(AffectedAxes::PAN_TILT, Duration::from_secs(5)).with_interval(Duration::from_millis(25)))).expect("FR7 idle polling");
        wait!(camera.pan_tilt().set_limit(PanTiltLimitCorner::UpRight, grafton_visca::types::PanTiltPosition { pan: Degrees::new(-170.0).expect("valid degrees"), tilt: Degrees::new(195.0).expect("valid degrees") })).expect("FR7 limit set");
        wait!(camera.pan_tilt().clear_limit(PanTiltLimitCorner::UpRight)).expect("FR7 limit clear");
        let mut operation = wait!(camera.pan_tilt().relative(grafton_visca::types::PanTiltPosition { pan: Degrees::new(200.0).expect("valid degrees"), tilt: Degrees::new(-35.0).expect("valid degrees") }, SpeedLevel::Slowest)).expect("full-span relative move");
        wait!(operation.applied()).expect("relative application");
        let payloads = fake.take_payloads();
        assert_eq!(payloads[0], [0x81, 1, 6, 2, 1, 1, 0, 9, 12, 10, 7, 0, 11, 3, 11, 0, 0xff]);
        assert!(payloads.contains(&vec![0x81, 1, 6, 7, 0, 1, 0, 9, 12, 10, 7, 0, 11, 3, 11, 0, 0xff]));
        assert!(payloads.contains(&vec![0x81, 1, 6, 7, 1, 1, 7, 15, 15, 15, 15, 7, 15, 15, 15, 15, 0xff]));
        assert_eq!(payloads.last().expect("relative frame").len(), 17);
        wait!(session.close()).expect("close FR7 session");
    }

    fn fr7_single_camera_ceiling_mount_controls_preparation_before_io() {
        let fake = camera();
        let config = CameraConfig::<SonyFr7>::udp("unused.invalid")
            .with_mount(MountPosition::Ceiling);
        let session = open_camera!(fake, &config).expect("single-camera ceiling session");
        let camera = session.camera();
        assert_eq!(camera.registration().tilt_range_degrees(), Some(-210.0..=15.0));
        assert!(matches!(
            wait!(camera.pan_tilt().absolute(grafton_visca::types::PanTiltPosition {
                pan: Degrees::new(0.0).expect("valid degrees"),
                tilt: Degrees::new(80.0).expect("valid degrees"),
            }, SpeedLevel::Slowest)),
            Err(grafton_visca::Error::InvalidParameter { parameter: "tilt position", .. })
        ));
        assert!(wait!(camera.pan_tilt().set_limit(PanTiltLimitCorner::UpRight,
            grafton_visca::types::PanTiltPosition {
                pan: Degrees::new(0.0).expect("valid degrees"),
                tilt: Degrees::new(80.0).expect("valid degrees"),
            })).is_err());
        assert_eq!(fake.write_count(), 0);
        let mut operation = wait!(camera.pan_tilt().absolute(grafton_visca::types::PanTiltPosition {
            pan: Degrees::new(0.0).expect("valid degrees"),
            tilt: Degrees::new(-210.0).expect("valid degrees"),
        }, SpeedLevel::Slowest)).expect("ceiling lower endpoint");
        wait!(operation.applied()).expect("ceiling application");
        assert_eq!(&fake.take_only_payload()[11..16], [15, 3, 14, 7, 13]);

        // The fixture returns a desktop-only tilt of +46000 units.
        let error = wait!(camera.pan_tilt().position()).expect_err("outside this mount's tilt range");
        assert!(matches!(error, grafton_visca::Error::InvalidResponse { actual, .. } if actual == POSITION[2..12]));
        let raw = wait!(camera.inquire(&grafton_visca::command::pan_tilt::PositionInquiry)).expect("raw position");
        assert_eq!(raw.raw_values(), (40103, 46000));
        wait!(session.close()).expect("close single-camera ceiling session");
    }

    fn fr7_single_camera_default_mount_keeps_desktop_limits() {
        let fake = camera();
        let config = CameraConfig::<SonyFr7>::udp("unused.invalid");
        assert_eq!(config.mount(), None);
        let session = open_camera!(fake, &config).expect("single-camera default session");
        let camera = session.camera();
        assert_eq!(camera.registration().mount(), Some(MountPosition::Desktop));
        assert_eq!(camera.registration().tilt_range_degrees(), Some(-30.0..=195.0));
        assert!(wait!(camera.pan_tilt().absolute(grafton_visca::types::PanTiltPosition {
            pan: Degrees::new(0.0).expect("valid degrees"),
            tilt: Degrees::new(-100.0).expect("valid degrees"),
        }, SpeedLevel::Slowest)).is_err());
        assert_eq!(fake.write_count(), 0);
        let mut operation = wait!(camera.pan_tilt().absolute(grafton_visca::types::PanTiltPosition {
            pan: Degrees::new(0.0).expect("valid degrees"),
            tilt: Degrees::new(80.0).expect("valid degrees"),
        }, SpeedLevel::Slowest)).expect("desktop position");
        wait!(operation.applied()).expect("desktop application");
        assert_eq!(fake.write_count(), 1);
        wait!(session.close()).expect("close single-camera default session");
    }

    fn fr7_motion_verdicts_require_in_range_replies() {
        let axes = AffectedAxes::PAN_TILT;
        let motion = MotionQuery::new(axes);
        let idle = IdleWait::new(axes, Duration::from_secs(5)).with_interval(Duration::from_millis(25));
        for (mount, min_tilt, max_tilt) in [
            (MountPosition::Desktop, -0x1ba5, 0xb3b0),
            (MountPosition::Ceiling, -0xc183, 0x0dd2),
        ] {
            let min_pan = -0x09ca7;
            let max_pan = 0x09ca7;
            for (invalid_units, valid_units) in [
                ((min_pan - 1, min_tilt), (min_pan, min_tilt)),
                ((max_pan + 1, max_tilt), (max_pan, max_tilt)),
                ((min_pan, min_tilt - 1), (min_pan, min_tilt)),
                ((max_pan, max_tilt + 1), (max_pan, max_tilt)),
            ] {
                let invalid = position_reply(invalid_units);
                let valid = position_reply(valid_units);
                let mut replies = [
                    invalid, invalid, invalid, valid, invalid, valid, invalid,
                    valid, valid, valid, valid, valid, valid, valid,
                ].into_iter();
                let fake = position_camera(move || replies.next().expect("scripted position reply"));
                let config = CameraConfig::<SonyFr7>::udp("unused.invalid").with_mount(mount);
                let session = open_camera!(fake, &config).expect("FR7 session");
                let camera = session.camera();

                let error = wait!(camera.pan_tilt().position()).expect_err("out-of-range degree readback");
                assert_invalid_position(&error, invalid);
                let raw = wait!(camera.inquire(&grafton_visca::command::pan_tilt::PositionInquiry)).expect("raw reply");
                assert_eq!(raw.raw_values(), invalid_units);

                let error = wait!(camera.motion().is_moving(motion)).expect_err("invalid baseline");
                assert_invalid_position(&error, invalid);
                assert_eq!(fake.take_payloads(), [POSITION_INQUIRY; 3]);
                let error = wait!(camera.motion().wait_until_idle(idle)).expect_err("invalid second sample");
                assert_invalid_position(&error, invalid);
                assert_eq!(fake.take_payloads(), [POSITION_INQUIRY; 2]);

                let mut home = wait!(camera.pan_tilt().home()).expect("home operation");
                wait!(home.applied()).expect("home applied");
                assert_eq!(fake.take_only_payload(), [0x81, 1, 6, 4, 0xff]);
                let error = wait!(home.settled()).expect_err("invalid settlement sample");
                let Error::SettlementObservationFailed { operation, source, .. } = error else {
                    panic!("expected a settlement observation error, got {error:?}");
                };
                assert_eq!(operation, home.id());
                assert_invalid_position(&source, invalid);
                assert_eq!(fake.take_payloads(), [POSITION_INQUIRY; 2]);

                wait!(camera.pan_tilt().position()).expect("legal endpoint readback");
                assert!(!wait!(camera.motion().is_moving(motion)).expect("legal stationary endpoints"));
                assert_eq!(fake.take_payloads(), [POSITION_INQUIRY; 3]);
                wait!(camera.motion().wait_until_idle(idle)).expect("fresh idle wait");
                assert_eq!(fake.take_payloads(), [POSITION_INQUIRY; 2]);
                assert!(matches!(wait!(home.settled()).expect("fresh settlement wait"),
                    Settlement::ObservedStable { axes: observed, .. } if observed == axes));
                assert_eq!(fake.take_payloads(), [POSITION_INQUIRY; 2]);
                wait!(session.close()).expect("close FR7 session");
            }
        }
    }

    fn fr7_ceiling_registration_controls_noun_preparation_before_io() {
        let fake = camera();
        let mut config = SessionConfig::from_compile_time::<SonyFr7>().expect("FR7 config");
        config = config.with_mount(grafton_visca::types::IndividualCameraId::CAMERA_1, MountPosition::Ceiling).expect("ceiling config");
        let session = open!(fake, config).expect("ceiling session");
        let camera = session.camera::<SonyFr7>(grafton_visca::types::IndividualCameraId::CAMERA_1).expect("FR7 view");
        assert!(wait!(camera.pan_tilt().absolute(grafton_visca::types::PanTiltPosition { pan: Degrees::new(0.0).expect("valid degrees"), tilt: Degrees::new(80.0).expect("valid degrees") }, SpeedLevel::Slowest)).is_err());
        assert_eq!(fake.write_count(), 0);
        let mut operation = wait!(camera.pan_tilt().absolute(grafton_visca::types::PanTiltPosition { pan: Degrees::new(0.0).expect("valid degrees"), tilt: Degrees::new(-210.0).expect("valid degrees") }, SpeedLevel::Slowest)).expect("ceiling lower endpoint");
        wait!(operation.applied()).expect("ceiling application");
        assert_eq!(&fake.take_only_payload()[11..16], [15, 3, 14, 7, 13]);
        wait!(session.close()).expect("close ceiling session");
    }
}

#[cfg(all(
    any(feature = "async", feature = "blocking"),
    feature = "runtime-tokio"
))]
#[tokio::test(start_paused = true)]
async fn dynamic_fr7_noun_uses_the_same_mount_and_frame() {
    use grafton_visca::{r#async::Session, runtime::TokioExecutor};
    let fake = camera();
    let session = Session::open(
        fake.async_wire(),
        &SessionConfig::from_compile_time::<SonyFr7>().expect("config"),
        TokioExecutor::from_current().expect("Tokio executor"),
    )
    .await
    .expect("session");
    let camera = session
        .camera::<grafton_visca::profiles::AnyProfile>(
            grafton_visca::types::IndividualCameraId::CAMERA_1,
        )
        .expect("dynamic FR7");
    let mut operation = camera
        .pan_tilt()
        .absolute(
            grafton_visca::types::PanTiltPosition {
                pan: Degrees::new(-170.0).expect("valid degrees"),
                tilt: Degrees::new(195.0).expect("valid degrees"),
            },
            SpeedLevel::Slowest,
        )
        .await
        .expect("dynamic absolute");
    operation.applied().await.expect("dynamic application");
    assert_eq!(
        fake.take_only_payload(),
        [0x81, 1, 6, 2, 1, 1, 0, 9, 12, 10, 7, 0, 11, 3, 11, 0, 0xff]
    );
    session.close().await.expect("close session");
}
