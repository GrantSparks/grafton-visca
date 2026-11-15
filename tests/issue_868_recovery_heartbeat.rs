//! A bounded heartbeat keeps observing after a lost raw-stream reply.
#![cfg(all(feature = "blocking", feature = "test-utils"))]
#![allow(clippy::expect_used)]
use grafton_visca::{
    profiles::PtzOpticsG2, testing::testkit::ManualClock, transport::SendSemantics, Error,
    ProfileSpec, Recovery, SessionConfig,
};
use grafton_visca_test_support::fake_camera::{frames, FakeCamera};

#[test]
fn heartbeat_policy_counts_every_nonfatal_failure_and_recognizes_new_activity() {
    let clock = ManualClock::new();
    let _participant = clock.enter();
    let mut answers = false;
    let fake = FakeCamera::new(move |bytes, reply| {
        if bytes == [0x81, 0x09, 0x04, 0x00, 0xff] {
            if answers {
                reply.reply(frames::inquiry_reply(&[0x02]));
            }
        } else {
            answers = true;
            reply.reply(frames::ack(1)).reply(frames::complete(1));
        }
    });
    let session = clock
        .open_blocking_session(
            fake.blocking_wire()
                .with_semantics(SendSemantics::Stream)
                .on_clock(&clock),
            SessionConfig::new(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("profile")),
        )
        .expect("session");
    let camera = session.camera::<PtzOpticsG2>().expect("camera");
    let mut unanswered = 0;
    for attempt in 0..2 {
        let before = session.metrics().expect("metrics").received_frames;
        let failure = camera.power().state().expect_err("first reply is missing");
        assert_ne!(failure.recovery(), Recovery::ReopenSession);
        if session.metrics().expect("metrics").received_frames == before {
            unanswered += 1;
        }
        if attempt == 0 {
            assert!(matches!(failure, Error::Timeout { .. }));
        } else {
            assert!(matches!(failure, Error::InquiryCorrelationLost { .. }));
        }
    }
    assert_eq!(unanswered, 2);
    assert_eq!(
        fake.write_count(),
        1,
        "the second inquiry is correctly latched"
    );
    // An answered ACK-bearing STOP reaches the camera and reopens the inquiry
    // lane even while the earlier inquiry's answer is still owed.
    camera
        .zoom()
        .stop()
        .expect("STOP submission")
        .applied()
        .expect("STOP applied");
    let before = session.metrics().expect("metrics").received_frames;
    assert!(camera.power().state().expect("new power reply"));
    assert!(session.metrics().expect("metrics").received_frames > before);
    assert_eq!(fake.write_count(), 3);
    session.close().expect("close");
}
