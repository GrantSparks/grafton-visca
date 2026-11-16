//! Sony socket reuse follows the camera's assignment through both owner facades.
#![cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]

use grafton_visca::{
    config::SessionConfig,
    operation::{AppliedOnly, CancellationOutcome},
    profiles::SonyFr7,
    Error,
};
use grafton_visca_test_support::{
    facade_matrix,
    fake_camera::{frames, FakeCamera},
};

facade_matrix! {
    fn a_sony_successor_applies_on_the_socket_reused_after_a_lost_completion() {
        let mut commands = 0;
        let fake = FakeCamera::visca(move |_, answer| {
            commands += 1;
            answer.reply(frames::ack(1));
            if commands != 1 { answer.reply(frames::complete(1)); }
        });
        let session = open!(fake, SessionConfig::from_compile_time::<SonyFr7>().unwrap()).unwrap();
        let camera = session.camera::<SonyFr7>(grafton_visca::types::IndividualCameraId::CAMERA_1).unwrap();
        let mut first = wait!(camera.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Tele)).unwrap();
        let mut second = wait!(camera.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Wide)).unwrap();
        wait!(second.applied()).unwrap();
        let writes = fake.writes();
        assert_eq!(writes.len(), 2);
        let sequence = frames::sony_sequence(&writes[0]).unwrap();
        fake.push(frames::sony_reply(sequence, &frames::complete(1)));
        wait!(first.applied()).unwrap();
        assert_eq!(fake.write_count(), 2, "neither command was replayed");
        wait!(session.close()).unwrap();
    }

    fn a_sony_successor_cancel_targets_the_reused_socket_and_refuses_the_displaced_command() {
        let fake = FakeCamera::visca(|bytes, answer| {
            if matches!(bytes, [0x81, 0x21, 0xff]) {
                answer.reply(frames::canceled(1));
            } else {
                answer.reply(frames::ack(1));
            }
        });
        let session = open!(fake, SessionConfig::from_compile_time::<SonyFr7>().unwrap()).unwrap();
        let camera = session.camera::<SonyFr7>(grafton_visca::types::IndividualCameraId::CAMERA_1).unwrap();
        let mut first = wait!(camera.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Tele)).unwrap();
        let mut second = wait!(camera.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Wide)).unwrap();
        wait_for_reads!(fake, 2);
        assert_eq!(wait!(second.cancel()).unwrap(), CancellationOutcome::Cancelled);
        let refused = wait!(first.cancel());
        assert!(matches!(refused, Err(Error::CancellationUnconfirmed)), "{refused:?}");
        let writes = fake.writes();
        assert_eq!(writes.len(), 3);
        assert_eq!(frames::sony_split(&writes[2]).unwrap().1, &[0x81, 0x21, 0xff]);
        let sequence = frames::sony_sequence(&writes[0]).unwrap();
        fake.push(frames::sony_reply(sequence, &frames::complete(1)));
        wait!(first.applied()).unwrap();
        wait!(session.close()).unwrap();
    }
}
