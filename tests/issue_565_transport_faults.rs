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
//! * an ACK naming an occupied socket falls back to the other free socket;
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
    feature = "blocking",
    all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    )
))]

#[path = "common/fake_camera.rs"]
mod fake_camera;
#[macro_use]
#[path = "common/matrix.rs"]
mod matrix;
#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{sync::Arc, time::Duration};

use grafton_visca::{
    completion::AppliedOnly,
    profile::ProfileSpec,
    profiles::SonyFR7,
    request::builtin::{FocusDrive, FocusStop, ZoomStop},
    transport::{AddressingMode, SendSemantics, TransportConfig},
    CameraId, CancellationOutcome, Error, SessionConfig,
};

use fake_camera::{frames, AsyncWire, BlockingWire, FakeCamera, FOCUS_STOP, ZOOM_STOP};
use profile_fixtures::NonDefaultCompileTimeProfile;

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
            addressing: TransportConfig::default().addressing,
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

    fn config(&self) -> TransportConfig {
        let mut config = TransportConfig::default();
        config.addressing = self.addressing;
        config
    }

    #[allow(dead_code)]
    fn blocking_wire(&self) -> BlockingWire {
        self.camera
            .blocking_wire()
            .with_config(self.config())
            .with_semantics(self.semantics)
            .with_addressing(self.addressing)
    }

    #[allow(dead_code)]
    fn async_wire(&self) -> AsyncWire {
        self.camera
            .async_wire()
            .with_config(self.config())
            .with_semantics(self.semantics)
            .with_addressing(self.addressing)
    }
}

fn connection_refused() -> Error {
    Error::Io(Arc::new(std::io::Error::from(
        std::io::ErrorKind::ConnectionRefused,
    )))
}

/// The Sony sequence number of a written frame.
fn session_config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
            .expect("two-socket runtime profile"),
    )
}

fn sony_session_config() -> SessionConfig {
    SessionConfig::new(ProfileSpec::from_compile_time::<SonyFR7>().expect("Sony FR7 profile"))
}

fn multi_target_session_config() -> SessionConfig {
    let mut config = session_config();
    config
        .register_target(
            CameraId::CAMERA_2,
            ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
                .expect("two-socket runtime profile"),
        )
        .expect("second raw target");
    config
}

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
            .camera::<NonDefaultCompileTimeProfile>()
            .expect("camera view");

        let error = wait!(
            wait!(view.submit::<AppliedOnly, _>(&ZoomStop))
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
            wait!(view.submit::<AppliedOnly, _>(&FocusDrive::Far))
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
            .camera::<NonDefaultCompileTimeProfile>()
            .expect("camera view");

        let error = wait!(
            wait!(view.submit::<AppliedOnly, _>(&ZoomStop))
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
        let view = session.camera::<SonyFR7>().expect("camera view");

        wait!(
            wait!(view.submit::<AppliedOnly, _>(&ZoomStop))
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
            wait!(view.submit::<AppliedOnly, _>(&FocusStop))
                .expect("the session is still usable")
                .applied()
        )
        .expect("later work still completes after the bounded retry");

        session.shutdown().expect("owner shutdown");
    }

    /// A failed datagram write fails exactly one command and the session keeps
    /// running: a datagram is framed by its own boundary, so the owner treats
    /// the failure as that command's outcome and nothing else is affected.
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
        let session = open!(Rig::datagram(&camera), session_config()).expect("owner session");
        let view = session
            .camera::<NonDefaultCompileTimeProfile>()
            .expect("camera view");

        let mut failing = wait!(view.submit::<AppliedOnly, _>(&ZoomStop))
            .expect("the submission is admitted before its write");
        let error = wait!(failing.applied()).expect_err("the first write fails on the wire");
        assert!(
            matches!(error, Error::Io(_)),
            "the caller whose datagram write failed sees its own transport error, got {error:?}"
        );
        assert!(
            !error.requires_new_session(),
            "an isolated datagram write failure is not session death"
        );

        wait!(
            wait!(view.submit::<AppliedOnly, _>(&FocusStop))
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
                    answer.fail_send(Error::Io(Arc::new(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        "peer went away mid-frame",
                    ))));
                }
                _ => {}
            }
        });
        let session = open!(Rig::serial_stream(&camera), multi_target_session_config())
            .expect("owner session");
        let first_camera = session
            .camera_for::<NonDefaultCompileTimeProfile>(CameraId::CAMERA_1)
            .expect("camera one view");
        let second_camera = session
            .camera_for::<NonDefaultCompileTimeProfile>(CameraId::CAMERA_2)
            .expect("camera two view");

        let mut held =
            wait!(first_camera.submit::<AppliedOnly, _>(&ZoomStop)).expect("first submission");
        let mut failing = wait!(second_camera.submit::<AppliedOnly, _>(&FocusStop))
            .expect("the second submission is admitted before its write");

        let error = wait!(failing.applied()).expect_err("the second write fails on the wire");
        let Error::StreamPoisoned { reason, .. } = &error else {
            panic!("a stream write failure is terminal for the session, got {error:?}");
        };
        assert!(
            reason.contains("peer went away mid-frame"),
            "the transport cause must survive in the poison reason: {reason}"
        );
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
            .camera::<NonDefaultCompileTimeProfile>()
            .expect("camera view");

        wait!(
            wait!(view.submit::<AppliedOnly, _>(&ZoomStop))
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
            .camera::<NonDefaultCompileTimeProfile>()
            .expect("camera view");

        wait!(
            wait!(view.submit::<AppliedOnly, _>(&ZoomStop))
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

    /// A sequence-correlated Sony camera may name a socket another request
    /// still owns. Because the second command is already uniquely identified by
    /// its sequence, the owner falls back to the target's other free socket
    /// instead of dropping the ACK. The cancellation wire below proves that the
    /// fallback assigned S2 while the first command still owns S1.
    fn ack_naming_an_occupied_socket_falls_back_to_the_other_free_socket() {
        let mut writes = 0usize;
        let mut previous_sequence = None;
        let camera = FakeCamera::new(move |write, answer| {
            writes += 1;
            let sequence = frames::sony_sequence(write).expect("Sony write must carry its sequence header");
            match writes {
                1 => {
                    answer.reply(frames::sony_reply(sequence, &frames::ack(1)));
                }
                // The camera answers the second command with occupied S1. The
                // uniquely identified request must fall back to free S2. The
                // previous-sequence completion then settles only the first
                // command; leave the second live so its cancel wire exposes the
                // assignment.
                2 => {
                    let previous: u32 =
                        previous_sequence.expect("the first write carried a sequence");
                    answer
                        .reply(frames::sony_reply(sequence, &frames::ack(1)))
                        .reply(frames::sony_reply(previous, &frames::complete(1)));
                }
                _ => {
                    answer.reply(frames::sony_reply(sequence, &frames::canceled(2)));
                }
            }
            previous_sequence = Some(sequence);
        });
        let session = open!(Rig::datagram(&camera), sony_session_config()).expect("owner session");
        let view = session.camera::<SonyFR7>().expect("camera view");

        let mut first = wait!(view.submit::<AppliedOnly, _>(&ZoomStop)).expect("first submission");
        let mut second =
            wait!(view.submit::<AppliedOnly, _>(&FocusStop)).expect("second submission");

        wait!(first.applied()).expect("first operation applied");
        assert_eq!(
            wait!(second.cancel_with_timeout(Duration::from_secs(1)))
                .expect("the fallback-assigned socket must accept cancellation"),
            CancellationOutcome::Cancelled
        );

        let writes = camera.writes();
        assert_eq!(writes.len(), 3, "two commands and one socket cancellation");
        assert_eq!(
            &writes[2][8..],
            &[0x81, 0x22, 0xff],
            "an occupied S1 ACK must assign the second command to free S2"
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
            .camera::<NonDefaultCompileTimeProfile>()
            .expect("camera view");

        wait!(
            wait!(view.submit::<AppliedOnly, _>(&ZoomStop))
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
