//! A Sony ACK arriving after its deadline is acceptance evidence on both
//! owners, including when the retry is already waiting in backoff.

#![cfg(any(
    feature = "blocking",
    all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    )
))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::time::Duration;

use grafton_visca::{
    completion::AppliedOnly, profiles::SonyFR7, request::builtin::ZoomStop, OperationalTuning,
    ProfileSpec, SessionConfig,
};
use grafton_visca_test_support::{
    facade_matrix,
    fake_camera::{frames, FakeCamera},
};

fn config() -> SessionConfig {
    SessionConfig::new(ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile"))
        .with_tuning(OperationalTuning::new().retry_timing(
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(10),
        ))
}

fn ack_timeout() -> Duration {
    ProfileSpec::from_compile_time::<SonyFR7>()
        .expect("FR7 profile")
        .timing()
        .ack_timeout()
}

facade_matrix! {
    paused:

    fn a_sony_ack_ten_milliseconds_after_its_deadline_prevents_a_second_write() {
        let camera = FakeCamera::new(|_, _| {});
        let session = open!(camera, config()).expect("session");
        let view = session.camera::<SonyFR7>().expect("FR7 view");
        let replies = camera.clone();
        after!(ack_timeout() + Duration::from_millis(10), move || {
            let writes = replies.writes();
            let sequence = frames::sony_sequence(&writes[0]).expect("Sony sequence");
            replies.push(frames::sony_reply(sequence, &frames::ack(1)));
            replies.push(frames::sony_reply(sequence, &frames::complete(1)));
        });
        let mut operation = wait!(view.submit::<AppliedOnly, _>(&ZoomStop)).expect("admission");
        wait!(operation.applied_with_timeout(Duration::from_secs(5))).expect("late acceptance still applies the original command");
        pause!(Duration::from_secs(1));
        assert_eq!(camera.write_count(), 1, "a late exact-sequence ACK must cancel the pending retry");
        session.shutdown().expect("shutdown");
    }

    fn a_sony_completion_during_retry_backoff_finishes_without_rewriting() {
        let camera = FakeCamera::new(|_, _| {});
        let session = open!(camera, config()).expect("session");
        let view = session.camera::<SonyFR7>().expect("FR7 view");
        let replies = camera.clone();
        after!(ack_timeout() + Duration::from_millis(250), move || {
            let writes = replies.writes();
            let sequence = frames::sony_sequence(&writes[0]).expect("Sony sequence");
            replies.push(frames::sony_reply(sequence, &frames::complete(1)));
        });
        let mut operation = wait!(view.submit::<AppliedOnly, _>(&ZoomStop)).expect("admission");
        wait!(operation.applied_with_timeout(Duration::from_secs(5))).expect("an exact completion applies the retained attempt");
        pause!(Duration::from_secs(1));
        assert_eq!(camera.write_count(), 1, "the first execution completed during backoff");
        session.shutdown().expect("shutdown");
    }
}
