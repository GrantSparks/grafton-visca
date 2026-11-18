//! Sony socket reuse follows the camera's assignment through both owner facades.
#![cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]

use grafton_visca::{
    config::SessionConfig,
    observability::{DiagnosticCapacity, DiagnosticEvent, DiagnosticPhase},
    operation::{AppliedOnly, CancellationOutcome, OperationId},
    profiles::SonyFr7,
    Error,
};
use grafton_visca_test_support::{
    facade_matrix,
    fake_camera::{frames, FakeCamera, ZOOM_TELE},
};

fn transitions_for(
    id: OperationId,
    mut next_event: impl FnMut() -> Option<DiagnosticEvent>,
) -> Vec<(DiagnosticPhase, DiagnosticPhase)> {
    std::iter::from_fn(&mut next_event)
        .filter_map(|event| match event {
            DiagnosticEvent::Transition {
                id: operation,
                from,
                to,
                ..
            } if operation == id => Some((from, to)),
            _ => None,
        })
        .collect()
}

facade_matrix! {
    fn a_sony_successor_applies_on_the_socket_reused_after_a_lost_completion() {
        let mut commands = 0;
        let fake = FakeCamera::visca(move |_, answer| {
            commands += 1;
            answer.reply(frames::ack(1));
            if commands != 1 { answer.reply(frames::complete(1)); }
        });
        let session = open!(fake, SessionConfig::from_compile_time::<SonyFr7>().unwrap()).unwrap();
        let diagnostics = wait!(session.subscribe_diagnostics(DiagnosticCapacity::MAX)).unwrap();
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
        let transitions = transitions_for(first.id(), || diagnostics.try_recv());
        assert!(transitions.contains(&(DiagnosticPhase::Executing, DiagnosticPhase::Displaced)));
        assert!(!transitions.contains(&(DiagnosticPhase::Executing, DiagnosticPhase::Executing)));
    }

    fn a_sony_ack_displaces_a_command_resolving_cancellation() {
        let mut successor_sequence = None;
        let fake = FakeCamera::new(move |write, answer| {
            let (sequence, bytes) = frames::sony_split(write).unwrap();
            match bytes {
                ZOOM_TELE => {
                    answer.reply(frames::sony_reply(sequence, &frames::ack(1)));
                }
                [0x81, 0x21, 0xff] => {
                    let successor = successor_sequence.expect("successor written before cancel");
                    // The cancellation receives no answer of its own. Its
                    // write releases the successor's ACK on the reused socket.
                    answer
                        .reply(frames::sony_reply(successor, &frames::ack(1)))
                        .reply(frames::sony_reply(successor, &frames::complete(1)));
                }
                _ => successor_sequence = Some(sequence),
            }
        });
        let session = open!(fake, SessionConfig::from_compile_time::<SonyFr7>().unwrap()).unwrap();
        let diagnostics = wait!(session.subscribe_diagnostics(DiagnosticCapacity::MAX)).unwrap();
        let camera = session.camera::<SonyFr7>(grafton_visca::types::IndividualCameraId::CAMERA_1).unwrap();
        let mut first = wait!(camera.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Tele)).unwrap();
        let mut second = wait!(camera.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Wide)).unwrap();
        wait_for_writes!(fake, 2);
        assert!(matches!(wait!(first.cancel()), Err(Error::CancellationUnconfirmed)));
        wait!(second.applied()).unwrap();

        let sequence = frames::sony_sequence(&fake.writes()[0]).unwrap();
        fake.push(frames::sony_reply(sequence, &frames::complete(1)));
        wait!(first.applied()).unwrap();
        wait!(session.close()).unwrap();
        let transitions = transitions_for(first.id(), || diagnostics.try_recv());
        assert!(transitions.contains(&(
            DiagnosticPhase::AwaitingCancellationResolution,
            DiagnosticPhase::Displaced,
        )));
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
