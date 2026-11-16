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
    config::{MountPosition, SessionConfig},
    profiles::SonyFr7,
    types::{AffectedAxes, Degrees, IdleWait, MotionQuery, PanTiltLimitCorner, SpeedLevel},
};
use grafton_visca_test_support::{
    facade_matrix,
    fake_camera::{self, FakeCamera},
};

const POSITION: [u8; 13] = [0x90, 0x50, 0, 9, 12, 10, 7, 0, 11, 3, 11, 0, 0xff];

fn camera() -> FakeCamera {
    FakeCamera::visca(|request, answer| {
        if request == [0x81, 9, 6, 0x12, 0xff] {
            answer.reply(POSITION);
        } else {
            answer
                .reply(fake_camera::frames::ack(1))
                .reply(fake_camera::frames::complete(1));
        }
    })
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
        ).decode(&POSITION[2..12]).expect("R7 raw position words");
        assert_eq!((raw.pan, raw.tilt), (40103, 46000));
        assert!(!wait!(camera.motion().is_moving(MotionQuery::new(AffectedAxes::PAN_TILT))).expect("FR7 motion observation"));
        wait!(camera.motion().wait_until_idle(IdleWait::new(AffectedAxes::PAN_TILT, Duration::from_secs(5)).with_interval(Duration::ZERO))).expect("FR7 idle polling");
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
