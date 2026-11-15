//! Observer budgets follow prepared inquiries and accepted settlement tuning.

#![cfg(all(
    feature = "test-utils",
    any(
        feature = "blocking",
        feature = "runtime-tokio",
        feature = "runtime-smol"
    )
))]
#![allow(clippy::expect_used)]

use std::time::Duration;

#[cfg(any(feature = "blocking", feature = "runtime-tokio"))]
mod inquiry_budget {
    use super::*;
    use grafton_visca::{
        camera::MotionQuery, profiles::PtzOpticsG2, testing::testkit::Step, AffectedAxes,
        OperationalTuning, SessionConfig,
    };

    fn config() -> SessionConfig {
        SessionConfig::from_compile_time::<PtzOpticsG2>()
            .expect("G2 profile")
            .with_tuning(OperationalTuning::new().inquiry_timeout(Duration::from_secs(20)))
    }

    fn script() -> Vec<Step> {
        (0..2)
            .flat_map(|_| {
                [
                    Step::on_send(None, vec![]),
                    Step::after(
                        Duration::from_secs(15),
                        vec![vec![0x90, 0x50, 0, 0, 0, 0, 0xff]],
                    ),
                ]
            })
            .collect()
    }

    fn query() -> MotionQuery {
        MotionQuery::new(AffectedAxes::ZOOM).with_window(Duration::from_secs(1))
    }

    #[cfg(feature = "blocking")]
    #[test]
    fn blocking_motion_observation_covers_both_slow_inquiries() {
        use grafton_visca::testing::testkit::{ManualClock, ScriptedBlockingTransport};
        let clock = ManualClock::new();
        let participant = clock.enter();
        let session = clock
            .open_blocking_session(
                &participant,
                ScriptedBlockingTransport::new(script()).on_clock(&clock),
                config(),
            )
            .expect("virtual blocking session");
        let camera = session.camera::<PtzOpticsG2>().expect("camera");
        let start = clock.now();
        assert!(!camera
            .motion()
            .is_moving(query())
            .expect("both inquiries answer within their own deadlines"));
        let elapsed = clock.now().duration_since(start);
        assert!(
            elapsed >= Duration::from_secs(31),
            "both fifteen-second replies and the window are observed: {elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_secs(33),
            "only the scripted delays should elapse: {elapsed:?}"
        );
        session.close().expect("close");
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test(start_paused = true)]
    async fn tokio_motion_observation_covers_both_slow_inquiries() {
        use grafton_visca::{testing::testkit::ScriptedTransport, Executor, TokioRuntime};
        let runtime = TokioRuntime::from_current().expect("Tokio runtime");
        let transport = ScriptedTransport::new(script()).with_executor(runtime.clone());
        let session = grafton_visca::Session::open(transport, config(), runtime.clone())
            .await
            .expect("Tokio session");
        let camera = session.camera::<PtzOpticsG2>().expect("camera");
        let start = runtime.now();
        assert!(!camera
            .motion()
            .is_moving(query())
            .await
            .expect("both inquiries answer within their own deadlines"));
        assert_eq!(runtime.now().duration_since(start), Duration::from_secs(31));
        session.close().await.expect("close");
    }
}

mod settlement_budget {
    use super::*;
    use grafton_visca::{
        completion::Targeted, request::builtin::PanTiltHome, Error, OperationalTuning,
        SessionConfig,
    };
    use grafton_visca_test_support::{
        facade_matrix,
        fake_camera::{frames, FakeCamera},
        profile_fixtures::NonDefaultCompileTimeProfile as Raw,
    };
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    facade_matrix! {
        paused:

        fn accepted_settlement_override_bounds_default_wait_at_ninety_seconds() {
            let changing = Arc::new(AtomicBool::new(false));
            let fake = FakeCamera::new(move |request, answer| {
                if request == [0x81, 0x09, 0x06, 0x12, 0xff] {
                    let pan = u8::from(changing.fetch_xor(true, Ordering::Relaxed));
                    answer.reply(frames::inquiry_reply(&[0, pan, 0, 0, 0, 0, 0, 0]));
                } else {
                    answer.reply(frames::ack(1)).reply(frames::complete(1));
                }
            });
            let config = SessionConfig::from_compile_time::<Raw>().expect("polled profile")
                .with_tuning(OperationalTuning::new()
                    .settlement_timeout(Duration::from_secs(90))
                    .inquiry_spacing(Duration::from_secs(5)));
            let session = open!(fake, config).expect("session");
            let camera = session.camera::<Raw>().expect("camera");
            let mut operation = wait!(camera.submit::<Targeted, _>(&PanTiltHome)).expect("move");
            let started = now!();
            let error = wait!(operation.settled()).expect_err("changing positions never settle");
            assert!(matches!(error, Error::ObservationTimeout { .. }), "{error:?}");
            let elapsed = now!().duration_since(started);
            assert!(elapsed >= Duration::from_secs(90), "accepted override is honored: {elapsed:?}");
            assert!(elapsed < Duration::from_secs(91), "the wait ends at its configured bound: {elapsed:?}");
            wait!(session.close()).expect("close");
        }
    }
}
