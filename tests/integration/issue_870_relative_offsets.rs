//! Public facade coverage for degree-based pan/tilt moves and readbacks.

#![allow(clippy::expect_used)]

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
        profiles::{PtzOpticsG2, SonyEviH100},
        types::{Degrees, IndividualCameraId, PanTiltLimitCorner, PanTiltPosition, SpeedLevel},
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

        fn evi_h100_endpoint_readbacks_reproduce_the_absolute_and_limit_words() {
            // Signed 16-bit tilt -907 is 0xFC75 at the nominal -20° endpoint.
            const WORDS: [u8; 8] = [0, 0, 0, 0, 0xF, 0xC, 7, 5];
            let fake = FakeCamera::visca(|payload, answer| {
                if payload == [0x81, 9, 6, 0x12, 0xff] {
                    answer.reply(fake_camera::frames::inquiry_reply(&WORDS));
                } else {
                    answer.reply(fake_camera::frames::ack(1)).reply(fake_camera::frames::complete(1));
                }
            });
            let config = SessionConfig::from_compile_time::<SonyEviH100>().expect("EVI-H100 config");
            let session = open!(fake, config).expect("session");
            let camera = session.camera::<SonyEviH100>(IndividualCameraId::CAMERA_1).expect("camera");
            let endpoint = PanTiltPosition::from_degrees(0.0, -20.0).expect("endpoint");
            let measured = wait!(camera.pan_tilt().position()).expect("position");
            assert_eq!(fake.take_only_payload(), [0x81, 9, 6, 0x12, 0xff]);
            assert_eq!(measured, endpoint);
            let mut operation = wait!(camera.pan_tilt().absolute(measured, SpeedLevel::Slowest)).expect("reuse inquiry");
            wait!(operation.applied()).expect("absolute applied");
            assert_eq!(&fake.take_only_payload()[6..14], &WORDS);

            let corner = PanTiltLimitCorner::DownLeft;
            wait!(camera.pan_tilt().set_limit(corner, endpoint)).expect("set limit");
            let first_limit = fake.take_only_payload();
            assert_eq!(&first_limit[6..14], &WORDS);
            let update = camera.state_cache().pan_tilt().limit().expect("confirmed limit");
            let cached = update.position().expect("confirmed position");
            assert_eq!(cached, endpoint);
            wait!(camera.pan_tilt().set_limit(update.corner(), cached)).expect("reuse cached endpoint");
            assert_eq!(fake.take_only_payload(), first_limit);
            wait!(session.close()).expect("close");
        }

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
