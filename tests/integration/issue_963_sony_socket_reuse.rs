//! Socket reassignment and projected state ordering through both owner facades.
#![cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]

use std::time::Duration;

use grafton_visca::{
    config::SessionConfig,
    observability::{DiagnosticCapacity, DiagnosticEvent, DiagnosticPhase},
    operation::{AppliedOnly, CancellationOutcome, OperationId},
    profiles::{ProfileSpec, PtzOpticsG2, SonyFr7},
    transport::{AddressingMode, SendSemantics},
    types::{IndividualCameraId, NdFilterMode},
    Error,
};
use grafton_visca_test_support::{
    facade_matrix,
    fake_camera::{frames, FakeCamera, ZOOM_TELE},
};

const ND_AUTO_OFF: &[u8] = &[0x81, 0x01, 0x7e, 0x04, 0x53, 0x03, 0xff];
const ND_AUTO_ON: &[u8] = &[0x81, 0x01, 0x7e, 0x04, 0x53, 0x02, 0xff];
const ND_VARIABLE_MODE: &[u8] = &[0x81, 0x01, 0x7e, 0x04, 0x52, 0x01, 0xff];
const ZOOM_WIDE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x03, 0xff];
const CANCEL_S1: &[u8] = &[0x81, 0x21, 0xff];

macro_rules! wait_for_execution {
    ($diagnostics:ident, $id:expr, $wait:ident) => {
        loop {
            if matches!(
                $wait!($diagnostics.recv_with_timeout(Duration::from_secs(1)))
                    .unwrap()
                    .expect("expected execution transition"),
                DiagnosticEvent::Transition {
                    id,
                    to: DiagnosticPhase::Executing,
                    ..
                } if id == $id
            ) {
                break;
            }
        }
    };
}

#[derive(Clone, Copy)]
enum AckTrace {
    OrderedReuse,
    ReorderedAcks,
    SuccessorAckMissing,
}

fn sony_nd_camera() -> FakeCamera {
    FakeCamera::visca(|write, answer| match write {
        ND_AUTO_OFF | ND_AUTO_ON => {
            answer.reply(frames::ack(1));
        }
        ND_VARIABLE_MODE => {
            answer.reply(frames::ack(1)).reply(frames::complete(1));
        }
        _ => panic!("unexpected write: {write:02x?}"),
    })
}

struct RoutedFakeCamera(FakeCamera);

impl RoutedFakeCamera {
    #[cfg(feature = "blocking")]
    fn blocking_wire(&self) -> grafton_visca_test_support::fake_camera::BlockingWire {
        self.0
            .blocking_wire()
            .with_addressing(AddressingMode::Serial)
            .with_semantics(SendSemantics::Stream)
    }

    #[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
    fn async_wire(&self) -> grafton_visca_test_support::fake_camera::AsyncWire {
        self.0
            .async_wire()
            .with_addressing(AddressingMode::Serial)
            .with_semantics(SendSemantics::Stream)
    }
}

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
        #[allow(unused_mut)]
        let mut diagnostics = wait!(session.subscribe_diagnostics(DiagnosticCapacity::MAX)).unwrap();
        let camera = session.camera::<SonyFr7>(grafton_visca::types::IndividualCameraId::CAMERA_1).unwrap();
        let mut first = wait!(camera.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Tele)).unwrap();
        wait_for_execution!(diagnostics, first.id(), wait);
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

    fn sony_udp_cancel_requires_proven_socket_ownership() {
        for trace in [AckTrace::OrderedReuse, AckTrace::ReorderedAcks, AckTrace::SuccessorAckMissing] {
            let fake = FakeCamera::visca(|bytes, answer| {
                if bytes == CANCEL_S1 {
                    answer.reply(frames::canceled(1));
                }
            });
            let session = open!(fake, SessionConfig::from_compile_time::<SonyFr7>().unwrap()).unwrap();
            #[allow(unused_mut)]
            let mut diagnostics = wait!(session.subscribe_diagnostics(DiagnosticCapacity::MAX)).unwrap();
            let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).unwrap();
            let mut first = wait!(camera.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Tele)).unwrap();
            let writes = wait_for_writes!(fake, 1);
            let (first_sequence, payload) = frames::sony_split(&writes[0]).unwrap();
            assert_eq!(payload, ZOOM_TELE);

            if !matches!(trace, AckTrace::ReorderedAcks) {
                fake.push(frames::sony_reply(first_sequence, &frames::ack(1)));
                wait_for_execution!(diagnostics, first.id(), wait);
            }

            let mut second = wait!(camera.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Wide)).unwrap();
            let writes = wait_for_writes!(fake, 2);
            let (second_sequence, payload) = frames::sony_split(&writes[1]).unwrap();
            assert_eq!(payload, ZOOM_WIDE);

            if !matches!(trace, AckTrace::SuccessorAckMissing) {
                fake.push(frames::sony_reply(second_sequence, &frames::ack(1)));
                wait_for_execution!(diagnostics, second.id(), wait);
            }
            if matches!(trace, AckTrace::ReorderedAcks) {
                fake.push(frames::sony_reply(first_sequence, &frames::ack(1)));
                wait_for_execution!(diagnostics, first.id(), wait);
            }

            let refused = wait!(first.cancel());
            assert!(matches!(refused, Err(Error::CancellationUnconfirmed)), "{refused:?}");
            assert_eq!(fake.write_count(), 2);
            fake.push(frames::sony_reply(first_sequence, &frames::complete(1)));
            wait!(first.applied()).unwrap();

            if matches!(trace, AckTrace::OrderedReuse) {
                assert_eq!(wait!(second.cancel()).unwrap(), CancellationOutcome::Cancelled);
                let writes = fake.writes();
                assert_eq!(writes.len(), 3);
                assert_eq!(frames::sony_split(&writes[2]).unwrap().1, CANCEL_S1);
            } else {
                fake.push(frames::sony_reply(second_sequence, &frames::complete(1)));
                wait!(second.applied()).unwrap();
                assert_eq!(fake.write_count(), 2);
            }
            wait!(session.close()).unwrap();
        }
    }
}

facade_matrix! {
    fn sony_setters_wait_for_their_key_across_socket_reuse() {
        let fake = sony_nd_camera();
        let session = open!(fake, SessionConfig::from_compile_time::<SonyFr7>().unwrap()).unwrap();
        #[allow(unused_mut)]
        let mut diagnostics = wait!(session.subscribe_diagnostics(DiagnosticCapacity::MAX)).unwrap();
        let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).unwrap();
        let cache = camera.state_cache();

        let (first, (second, first_id)) = together!({
            wait!(camera.nd_filter().set_auto(false))
        }, {
            wait_for_reads!(fake, 1);
            let first_id = loop {
                if let DiagnosticEvent::Admitted { id, .. } =
                    wait!(diagnostics.recv_with_timeout(Duration::from_secs(1)))
                        .unwrap()
                        .expect("expected admission diagnostic")
                {
                    break id;
                }
            };
            let (second, ()) = together!({
                wait!(camera.nd_filter().set_auto(true))
            }, {
                while !matches!(
                    wait!(diagnostics.recv_with_timeout(Duration::from_secs(1)))
                        .unwrap()
                        .expect("expected admission diagnostic"),
                    DiagnosticEvent::Admitted { .. }
                ) {}
                // A different key can use the socket and even displace the first
                // setter. Its later sequence completion still settles that setter.
                wait!(camera.nd_filter().set_mode(NdFilterMode::Variable)).unwrap();
                let writes = fake.writes();
                assert_eq!(writes.len(), 2);
                assert_eq!(frames::sony_split(&writes[0]).unwrap().1, ND_AUTO_OFF);
                assert_eq!(frames::sony_split(&writes[1]).unwrap().1, ND_VARIABLE_MODE);
                assert_eq!(cache.nd_filter().mode(), Some(NdFilterMode::Variable));
                assert_eq!(cache.nd_filter().auto(), None);

                let sequence = frames::sony_sequence(&writes[0]).unwrap();
                fake.push(frames::sony_reply(sequence, &frames::complete(1)));

                let writes = wait_for_writes!(fake, 3);
                assert_eq!(cache.nd_filter().auto(), Some(false));
                let (sequence, payload) = frames::sony_split(&writes[2]).unwrap();
                assert_eq!(payload, ND_AUTO_ON);
                fake.push(frames::sony_reply(sequence, &frames::complete(1)));
            });
            (second, first_id)
        });
        first.unwrap();
        second.unwrap();
        assert_eq!(cache.nd_filter().auto(), Some(true));
        assert_eq!(fake.write_count(), 3);
        wait!(session.close()).unwrap();
        assert!(transitions_for(first_id, || diagnostics.try_recv())
            .contains(&(DiagnosticPhase::Executing, DiagnosticPhase::Displaced)));
    }

    fn same_key_setters_to_distinct_targets_progress_independently() {
        let fake = RoutedFakeCamera(FakeCamera::new(|write, answer| {
            match write[0] {
                0x81 => { answer.reply(frames::ack(1)); }
                0x82 => {
                    answer.reply([0xa0, 0x41, 0xff]).reply([0xa0, 0x51, 0xff]);
                }
                _ => panic!("unexpected write: {write:02x?}"),
            }
        }));
        let profile = ProfileSpec::from_compile_time::<PtzOpticsG2>().unwrap();
        let config = SessionConfig::new(profile.clone())
            .with_target(IndividualCameraId::CAMERA_2, profile)
            .unwrap();
        let session = open!(fake, config).unwrap();
        let first = session.camera::<PtzOpticsG2>(IndividualCameraId::CAMERA_1).unwrap();
        let other = session.camera::<PtzOpticsG2>(IndividualCameraId::CAMERA_2).unwrap();

        let (first_result, ()) = together!({
            wait!(first.streaming().set_multicast(false))
        }, {
            wait_for_reads!(fake.0, 1);
            wait!(other.streaming().set_multicast(true)).unwrap();

            let writes = fake.0.writes();
            assert_eq!(writes.len(), 2);
            assert_eq!(writes[0][0], 0x81);
            assert_eq!(writes[1][0], 0x82);
            assert_eq!(first.state_cache().streaming().multicast(), None);
            assert_eq!(other.state_cache().streaming().multicast(), Some(true));

            fake.0.push(frames::complete(1));
        });
        first_result.unwrap();
        assert_eq!(first.state_cache().streaming().multicast(), Some(false));
        assert_eq!(other.state_cache().streaming().multicast(), Some(true));
        wait!(session.close()).unwrap();
    }
}

// Only the async facade can detach a plain setter by dropping its future.
#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
mod detached {
    use super::*;
    use futures_lite::future::poll_once;
    use grafton_visca::{r#async::Session, runtime::Executor};
    use grafton_visca_test_support::runtime_matrix;

    async fn detached_setter_keeps_its_place<E: Executor>(executor: E) {
        let fake = sony_nd_camera();
        let session = Session::open(
            fake.async_wire(),
            &SessionConfig::from_compile_time::<SonyFr7>().unwrap(),
            executor.clone(),
        )
        .await
        .unwrap();
        let mut diagnostics = session
            .subscribe_diagnostics(DiagnosticCapacity::MAX)
            .await
            .unwrap();
        let camera = session
            .camera::<SonyFr7>(IndividualCameraId::CAMERA_1)
            .unwrap();
        let nd = camera.nd_filter();

        let mut first = Box::pin(nd.set_auto(false));
        assert!(poll_once(first.as_mut()).await.is_none());
        fake.wait_for_reads_async(&executor, 1).await;
        drop(first);
        let mut second = Box::pin(nd.set_auto(true));
        assert!(poll_once(second.as_mut()).await.is_none());

        let mut admitted = 0;
        while admitted < 2 {
            let event = diagnostics
                .recv_with_timeout(Duration::from_secs(1))
                .await
                .unwrap()
                .expect("expected admission diagnostic");
            admitted += usize::from(matches!(event, DiagnosticEvent::Admitted { .. }));
        }
        nd.set_mode(NdFilterMode::Variable).await.unwrap();
        let writes = fake.writes();
        assert_eq!(writes.len(), 2);
        let sequence = frames::sony_sequence(&writes[0]).unwrap();
        fake.push(frames::sony_reply(sequence, &frames::complete(1)));

        let writes = fake.wait_for_writes_async(&executor, 3).await;
        let (sequence, payload) = frames::sony_split(&writes[2]).unwrap();
        assert_eq!(payload, ND_AUTO_ON);
        assert_eq!(camera.state_cache().nd_filter().auto(), Some(false));
        fake.push(frames::sony_reply(sequence, &frames::complete(1)));
        second.await.unwrap();
        assert_eq!(camera.state_cache().nd_filter().auto(), Some(true));
        session.close().await.unwrap();
    }

    runtime_matrix!(detached_setter_keeps_its_place);
}
