//! Issue #565: the owner's transport-fault boundaries, on both facades.
//!
//! Each scenario pins one transport boundary, and both owners answer it the
//! same way:
//!
//! * a raw receive fault while an ACK is unconfirmed fails only that command,
//!   without replay, and keeps the session alive (#671); the strict opt-in
//!   poisons the session instead;
//! * a sequence-correlated Sony receive fault retries on the same sequence and
//!   keeps the session;
//! * a failed datagram write fails one command, while a failed stream write
//!   poisons the session and names its transport cause;
//! * socketless ACK and completion frames complete a command;
//! * a malformed datagram is discarded and a later valid reply still applies;
//! * an ACK reusing an occupied socket preserves sequence completion and targets
//!   cancellation at that named socket;
//! * replies answered from inside the write are matched on the first read pump.
//!
//! Every scenario runs on the blocking facade and on the async facade under
//! each enabled runtime; none is single-facade.
//!
//! The engine's deferred-ACK latch (#297) is *not* observable from here: these
//! owners apply a write and its result back to back, so nothing can reach the
//! engine in between. That guarantee is pinned in `runtime::engine::tests`
//! instead, where inputs can be interleaved directly (#636).

#![cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]

use grafton_visca_test_support::{facade_matrix, fake_camera, profile_fixtures};

use std::{sync::Arc, time::Duration};

use grafton_visca::{
    error::{ErrorKind, Recovery},
    operation::{AppliedOnly, CancellationOutcome},
    profiles::SonyFr7,
    transport::{AddressingMode, IoBounds, SendSemantics},
    Error,
};

use fake_camera::{frames, FakeCamera, FOCUS_STOP, ZOOM_STOP};
use profile_fixtures::{
    session_config, sony_session_config, two_camera_session_config, NonDefaultCompileTimeProfile,
};

/// A [`FakeCamera`] plus the transport facts its wire reports: the send
/// semantics, and an addressing mode used as both the transport
/// configuration's addressing and the wire's addressing hint.
///
/// `open!` builds the facade's wire through [`Rig::blocking_wire`] or
/// [`Rig::async_wire`].
struct Rig {
    camera: FakeCamera,
    semantics: SendSemantics,
    addressing: AddressingMode,
}

impl Rig {
    /// A datagram transport with the default (IP) addressing.
    fn datagram(camera: &FakeCamera) -> Self {
        Self {
            camera: camera.clone(),
            semantics: SendSemantics::Datagram,
            addressing: AddressingMode::Ip,
        }
    }

    /// A byte-stream transport with serial addressing.
    fn serial_stream(camera: &FakeCamera) -> Self {
        Self {
            camera: camera.clone(),
            semantics: SendSemantics::Stream,
            addressing: AddressingMode::Serial,
        }
    }

    fn config(&self) -> IoBounds {
        IoBounds::default()
    }

    #[cfg(all(feature = "blocking", feature = "test-utils"))]
    fn blocking_wire(&self) -> fake_camera::BlockingWire {
        self.camera
            .blocking_wire()
            .with_config(self.config())
            .with_semantics(self.semantics)
            .with_addressing(self.addressing)
    }

    #[cfg(all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    ))]
    fn async_wire(&self) -> fake_camera::AsyncWire {
        self.camera
            .async_wire()
            .with_config(self.config())
            .with_semantics(self.semantics)
            .with_addressing(self.addressing)
    }
}

fn connection_refused() -> Error {
    Error::Io {
        source: Arc::new(std::io::Error::from(std::io::ErrorKind::ConnectionRefused)),
    }
}

/// The Sony sequence number of a written frame.
/// A camera whose first write is answered only by a receive fault; every
/// later write gets ACK and completion on socket 1.
fn receive_fault_on_first_write() -> FakeCamera {
    let mut writes = 0usize;
    FakeCamera::visca(move |_, answer| {
        writes += 1;
        if writes == 1 {
            answer.fault(connection_refused());
        } else {
            answer.reply(frames::ack(1)).reply(frames::complete(1));
        }
    })
}

facade_matrix! {
    paused:

    /// Issue #671: a raw receive fault while a successfully sent command awaits
    /// its ACK leaves that one command's acceptance uncertain, but by default it
    /// does not poison the session. The command is never replayed (a raw
    /// command may already have reached the camera) and eventually fails
    /// `UnsequencedCommandUnconfirmed` — a per-request outcome that does *not*
    /// require a new session — while the session stays usable for later work.
    fn raw_receive_fault_fails_one_command_and_keeps_the_session() {
        let camera = receive_fault_on_first_write();
        let session = open!(Rig::datagram(&camera), session_config()).expect("owner session");
        let view = session
            .camera::<NonDefaultCompileTimeProfile>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("camera view");

        let error = wait!(
            wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop))
                .expect("submission")
                .applied()
        )
        .expect_err("the unconfirmed raw command fails on its own");
        assert!(matches!(error, Error::UnsequencedCommandUnconfirmed));
        assert!(
            !error.requires_new_session(),
            "a raw command that fails on a live session is a per-request outcome, not session death"
        );
        assert_eq!(
            camera.write_count(),
            1,
            "an unconfirmed raw command is never replayed after a receive fault"
        );

        // The session survived the receive fault: later ordinary work waits out
        // the inert keyed hold, then completes normally.
        wait!(
            wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::focus::Far))
                .expect("the session survives a transient receive fault")
                .applied()
        )
        .expect("later work still completes");

        session.shutdown().expect("owner shutdown");
    }

    /// Issue #671 strict opt-in, end to end: with
    /// `SessionConfig::with_strict_unconfirmed_poison(true)`, the same receive
    /// fault poisons the whole session, surfaced as `StreamPoisoned` (which
    /// requires a replacement session). This exercises the full tuning →
    /// adapter → engine plumbing of the opt-in through the real facade.
    fn strict_opt_in_raw_receive_fault_poisons_the_session() {
        let camera = receive_fault_on_first_write();
        let session = open!(
            Rig::datagram(&camera),
            session_config().with_strict_unconfirmed_poison(true)
        )
        .expect("owner session");
        let view = session
            .camera::<NonDefaultCompileTimeProfile>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("camera view");

        let error = wait!(
            wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop))
                .expect("submission")
                .applied()
        )
        .expect_err("strict mode poisons the session on the receive fault");
        assert!(
            matches!(error, Error::StreamPoisoned { .. }),
            "strict mode surfaces the poison as StreamPoisoned, got {error:?}"
        );
        assert!(
            error.requires_new_session(),
            "a poisoned session must require a replacement"
        );
    }

    /// A sequence-correlated Sony receive fault retries the same logical
    /// request and keeps the session usable. The classic trigger is UDP `recv`
    /// reporting ECONNREFUSED after an ICMP port-unreachable for an earlier
    /// datagram.
    fn sony_transient_receive_fault_retries_same_sequence_and_keeps_session() {
        let camera = receive_fault_on_first_write();
        let session = open!(Rig::datagram(&camera), sony_session_config()).expect("owner session");
        let view = session.camera::<SonyFr7>(grafton_visca::types::IndividualCameraId::CAMERA_1).expect("camera view");

        wait!(
            wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop))
                .expect("submission")
                .applied()
        )
        .expect("a Sony receive fault retries on its existing sequence");

        let writes = camera.writes();
        assert_eq!(
            writes.len(),
            2,
            "the sequence-correlated request gets one bounded retry"
        );
        assert_eq!(
            frames::sony_sequence(&writes[0]).expect("Sony write must carry its sequence header"),
            frames::sony_sequence(&writes[1]).expect("Sony write must carry its sequence header"),
            "a Sony retry must preserve the logical request sequence"
        );

        // The session survived, so later work still runs on it.
        wait!(
            wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::focus::Stop))
                .expect("the session is still usable")
                .applied()
        )
        .expect("later work still completes after the bounded retry");

        session.shutdown().expect("owner shutdown");
    }

    /// A failed datagram write fails exactly one command and the session keeps
    /// running. Sony retains the original transport cause while reporting that
    /// the attempted write may have reached the camera (#968).
    fn datagram_write_failure_fails_one_command_and_keeps_the_session() {
        let mut writes = 0usize;
        let camera = FakeCamera::visca(move |_, answer| {
            writes += 1;
            if writes == 1 {
                answer.fail_send(connection_refused());
            } else {
                answer.reply(frames::ack(1)).reply(frames::complete(1));
            }
        });
        let session = open!(Rig::datagram(&camera), sony_session_config()).expect("owner session");
        let view = session
            .camera::<SonyFr7>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("camera view");

        let mut failing = wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop))
            .expect("the submission is admitted before its write");
        let error = wait!(failing.applied()).expect_err("the first write fails on the wire");
        let Error::RequestUnconfirmed { source: terminal_cause, .. } = &error else {
            panic!("the failed datagram write retains physical uncertainty, got {error:?}");
        };
        let Error::TransportError { source, .. } = terminal_cause.as_ref() else {
            panic!("the original transport cause is retained, got {terminal_cause:?}");
        };
        let Error::Io { source: cause } = source.as_ref() else {
            panic!("the transport wrapper must preserve the original OS error, got {source:?}");
        };
        assert_eq!(cause.kind(), std::io::ErrorKind::ConnectionRefused);
        let original = std::error::Error::source(&error)
            .and_then(std::error::Error::source)
            .and_then(std::error::Error::source)
            .and_then(|cause| cause.downcast_ref::<Arc<std::io::Error>>())
            .expect("the public source chain retains the OS error");
        assert!(Arc::ptr_eq(original, cause), "the original shared cause is retained");
        assert_eq!(original.kind(), std::io::ErrorKind::ConnectionRefused);
        assert_eq!(error.kind(), ErrorKind::Unconfirmed);
        assert_eq!(
            error.failure_context(),
            Some(grafton_visca::error::FailureContext::new(
                grafton_visca::error::FailureStage::Terminal,
                grafton_visca::error::Certainty::Unconfirmed,
            )),
            "a failed write may have reached the camera"
        );
        assert_eq!(
            error.recovery(),
            Recovery::Reconcile
        );
        assert!(
            !error.requires_new_session(),
            "an isolated datagram write failure is not session death"
        );

        wait!(
            wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::focus::Stop))
                .expect("the session survives an isolated datagram write failure")
                .applied()
        )
        .expect("later work still completes");
        assert_eq!(
            camera.take_payloads(),
            vec![ZOOM_STOP.to_vec(), FOCUS_STOP.to_vec()],
            "the failed write is attempted once and never replayed; only the later command follows"
        );

        session.shutdown().expect("owner shutdown");
    }

    /// Issue #968: a failed raw datagram has no sequence with which to safely
    /// distinguish a delayed reply from the next request's response.
    fn a_failed_raw_datagram_requires_reconciliation_and_the_session_survives() {
        let mut writes = 0;
        let camera = FakeCamera::visca(move |_, answer| {
            writes += 1;
            if writes == 1 { answer.fail_send(connection_refused()); }
            else { answer.reply(frames::ack(1)).reply(frames::complete(1)); }
        });
        let session = open!(Rig::datagram(&camera), session_config()).expect("owner session");
        let view = session.camera::<NonDefaultCompileTimeProfile>(grafton_visca::types::IndividualCameraId::CAMERA_1).expect("camera view");
        let mut operation = wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop)).expect("admission");
        let error = wait!(operation.applied()).expect_err("a failed write may have applied");
        assert!(matches!(error, Error::UnsequencedCommandUnconfirmed), "{error:?}");
        assert_eq!(error.recovery(), Recovery::Reconcile);
        wait!(wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::focus::Stop)).expect("subsequent admission").applied()).expect("session remains live");
        assert_eq!(camera.take_payloads(), vec![ZOOM_STOP.to_vec(), FOCUS_STOP.to_vec()]);
        session.shutdown().expect("shutdown");
    }


    /// A failed *stream* write is a session verdict on purpose: the byte-stream
    /// position becomes unknowable, so every affected caller must learn that a
    /// replacement session is needed (#564). The exact transport cause is
    /// carried in the poison reason rather than thrown away. The two raw
    /// commands target different registered cameras so the raw same-target
    /// unacknowledged gate does not hide the session-wide stream boundary.
    fn stream_write_failure_poisons_and_names_the_transport_cause() {
        let mut writes = 0usize;
        let camera = FakeCamera::visca(move |_, answer| {
            writes += 1;
            match writes {
                1 => {
                    answer.reply(frames::ack(1));
                }
                2 => {
                    answer.fail_send(Error::Io { source: Arc::new(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        "peer went away mid-frame",
                    )) });
                }
                _ => {}
            }
        });
        let session = open!(Rig::serial_stream(&camera), two_camera_session_config())
            .expect("owner session");
        let first_camera = session
            .camera::<NonDefaultCompileTimeProfile>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("camera one view");
        let second_camera = session
            .camera::<NonDefaultCompileTimeProfile>(grafton_visca::types::IndividualCameraId::CAMERA_2)
            .expect("camera two view");

        let mut held =
            wait!(first_camera.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop)).expect("first submission");
        let mut failing = wait!(second_camera.submit::<AppliedOnly, _>(&grafton_visca::command::focus::Stop))
            .expect("the second submission is admitted before its write");

        let error = wait!(failing.applied()).expect_err("the second write fails on the wire");
        let Error::StreamPoisoned { source, .. } = &error else {
            panic!("a stream write failure is terminal for the session, got {error:?}");
        };
        assert!(matches!(source.as_ref(), Error::Io { source: io } if io.kind() == std::io::ErrorKind::WriteZero));
        assert!(std::error::Error::source(&error).is_some());
        assert!(error.requires_new_session());

        let poisoned = wait!(held.applied()).expect_err("the rest of the session is poisoned too");
        assert!(
            matches!(poisoned, Error::StreamPoisoned { .. }),
            "expected the session-wide poison, got {poisoned:?}"
        );
        assert!(poisoned.requires_new_session());
        assert_eq!(
            camera.write_count(),
            2,
            "one accepted write and the failed write; nothing is written after the poison"
        );
    }

    /// A camera that answers `90 40 FF` / `90 50 FF` sends no socket nibble.
    /// The owner assigns the command a free socket and completes it; the
    /// socketless frame is not an `InvalidResponse` and does not end the
    /// session.
    fn socketless_ack_and_completion_still_complete_a_command() {
        // Socket 0 is the socketless form: `90 40 FF` and `90 50 FF`.
        let camera = FakeCamera::visca(|_, answer| {
            answer.reply(frames::ack(0)).reply(frames::complete(0));
        });
        let session = open!(Rig::datagram(&camera), session_config()).expect("owner session");
        let view = session
            .camera::<NonDefaultCompileTimeProfile>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("camera view");

        wait!(
            wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop))
                .expect("submission")
                .applied()
        )
        .expect("a socketless ACK and completion must still complete the command");
        assert_eq!(camera.write_count(), 1, "no retry was needed");

        session.shutdown().expect("owner shutdown");
    }

    /// A malformed datagram is one bad receive, not session death. The owner
    /// must discard it and still apply the valid reply that follows on the same
    /// transport, without retrying the command or carrying the bad bytes into
    /// the next datagram.
    fn malformed_datagram_is_ignored_and_later_valid_reply_succeeds() {
        let mut writes = 0usize;
        let camera = FakeCamera::visca(move |_, answer| {
            writes += 1;
            if writes == 1 {
                answer
                    .reply([0x90, 0x61, 0xff]) // malformed error frame: missing error code
                    .reply(frames::ack(1))
                    .reply(frames::complete(1));
            }
        });
        let session = open!(Rig::datagram(&camera), session_config()).expect("owner session");
        let view = session
            .camera::<NonDefaultCompileTimeProfile>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("camera view");

        wait!(
            wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop))
                .expect("submission")
                .applied()
        )
        .expect("a later valid datagram must succeed after malformed input");
        assert_eq!(
            camera.write_count(),
            1,
            "malformed input must not trigger a retry"
        );

        session.shutdown().expect("owner shutdown");
    }

    /// A Sony ACK reusing a locally occupied socket displaces the stale owner.
    /// Its late completion still applies by sequence, while cancellation of the
    /// successor targets the socket the camera actually named (#963).
    fn ack_reusing_a_socket_preserves_late_completion_and_targets_its_cancel() {
        let mut writes = 0usize;
        let mut previous_sequence = None;
        let camera = FakeCamera::new(move |write, answer| {
            writes += 1;
            let sequence = frames::sony_sequence(write).expect("Sony write must carry its sequence header");
            match writes {
                1 => {
                    answer.reply(frames::sony_reply(sequence, &frames::ack(1)));
                }
                // The second command owns camera-named S1. Its ACK displaces
                // the first command's local socket claim, but the first's late
                // completion still applies by its sequence. Keep the second
                // live so its cancel wire proves that S1 was retained.
                2 => {
                    let previous: u32 =
                        previous_sequence.expect("the first write carried a sequence");
                    answer
                        .reply(frames::sony_reply(sequence, &frames::ack(1)))
                        .reply(frames::sony_reply(previous, &frames::complete(1)));
                }
                _ => {
                    answer.reply(frames::sony_reply(sequence, &frames::canceled(1)));
                }
            }
            previous_sequence = Some(sequence);
        });
        let session = open!(Rig::datagram(&camera), sony_session_config()).expect("owner session");
        let view = session.camera::<SonyFr7>(grafton_visca::types::IndividualCameraId::CAMERA_1).expect("camera view");

        let mut first = wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop)).expect("first submission");
        let mut second =
            wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::focus::Stop)).expect("second submission");

        wait!(first.applied()).expect("the displaced operation completes by sequence");
        assert_eq!(
            wait!(first.cancel()).expect("late completion stays cached"),
            CancellationOutcome::Completed
        );
        assert_eq!(
            wait!(second.cancel_with_timeout(Duration::from_secs(1)))
                .expect("the camera-named socket must accept cancellation"),
            CancellationOutcome::Cancelled
        );

        let writes = camera.writes();
        assert_eq!(writes.len(), 3, "two commands and one socket cancellation");
        assert_eq!(
            &writes[2][8..],
            &[0x81, 0x21, 0xff],
            "a reused S1 ACK must bind the second command to camera-named S1"
        );

        session.shutdown().expect("owner shutdown");
    }

    /// Issue #297: the camera answers from inside the write, so its ACK and
    /// completion are both already queued by the time the write returns. The
    /// owner must match them on its very first read pump — the command settles
    /// without waiting for its ACK deadline and without a second write.
    ///
    /// This is deliberately **not** a test of the engine's deferred-ACK latch.
    /// The shipped owners apply the write and its result back to back, so no
    /// frame can reach the engine between them and `Phase::Sending` is
    /// unobservable from here; a facade test claiming otherwise passes with the
    /// latch replaced by a drop. The latch itself is pinned at engine level,
    /// where an `Input` sequence can actually produce that interleaving, by
    /// `runtime::engine::tests::ack_racing_its_own_write_result_is_latched_and_applied`,
    /// `a_second_racing_ack_cannot_steal_the_latch_from_the_first` (#636).
    fn ack_answered_from_inside_the_write_is_matched_on_the_first_pump() {
        let camera = FakeCamera::acking(1);
        let session = open!(Rig::datagram(&camera), session_config()).expect("owner session");
        let view = session
            .camera::<NonDefaultCompileTimeProfile>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("camera view");

        wait!(
            wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop))
                .expect("submission")
                .applied()
        )
        .expect("an immediately answered command must not wait for its ACK deadline");
        assert_eq!(
            camera.write_count(),
            1,
            "a command answered inside its own write must never be rewritten"
        );

        session.shutdown().expect("owner shutdown");
    }
}
