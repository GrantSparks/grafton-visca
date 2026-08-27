//! Issue #565: 1.x transport fault tolerance, through the blocking facade.
//!
//! Each test pins one behavior the 2.0 rewrite dropped: a transient receive
//! error must not destroy the session, a failed stream write must still report
//! its own transport error to the caller that owned it, a socketless ACK or
//! completion must still work, an ACK naming an occupied socket must be
//! reassigned instead of dropped, and an ACK that is already queued by the time
//! the write returns must still be matched (#297).

#![cfg(feature = "blocking")]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    command::CommandKind,
    completion::AppliedOnly,
    profile::ProfileSpec,
    request::builtin::{FocusStop, ZoomStop},
    transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
    Error,
};

use profile_fixtures::NonDefaultCompileTimeProfile;

const ACK_SOCKET_ONE: &[u8] = &[0x90, 0x41, 0xff];
const COMPLETE_SOCKET_ONE: &[u8] = &[0x90, 0x51, 0xff];
const COMPLETE_SOCKET_TWO: &[u8] = &[0x90, 0x52, 0xff];
const ACK_NO_SOCKET: &[u8] = &[0x90, 0x40, 0xff];
const COMPLETE_NO_SOCKET: &[u8] = &[0x90, 0x50, 0xff];

/// What the camera does when the n-th frame is written.
#[derive(Debug, Clone)]
enum OnSend {
    /// Accept the write and queue these reads, in order.
    Reply(Vec<Vec<u8>>),
    /// Fail the write itself.
    Fail(Error),
}

/// A scripted blocking camera whose reads may fail.
#[derive(Debug)]
struct FaultTransport {
    config: TransportConfig,
    semantics: SendSemantics,
    script: VecDeque<OnSend>,
    trailing: Vec<Vec<u8>>,
    reads: VecDeque<Result<Vec<u8>, Error>>,
    faults: VecDeque<(usize, Error)>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    sends: usize,
}

impl FaultTransport {
    fn new(semantics: SendSemantics, script: Vec<OnSend>) -> Self {
        Self {
            config: TransportConfig::default(),
            semantics,
            script: script.into(),
            trailing: Vec::new(),
            reads: VecDeque::new(),
            faults: VecDeque::new(),
            writes: Arc::new(Mutex::new(Vec::new())),
            sends: 0,
        }
    }

    fn probe(&self) -> Probe {
        Probe {
            writes: Arc::clone(&self.writes),
        }
    }

    /// Replies used once the explicit script is exhausted.
    fn with_trailing_reply(mut self, replies: Vec<Vec<u8>>) -> Self {
        self.trailing = replies;
        self
    }

    /// Inject a read failure before the reply queued by send `send_number`.
    fn with_read_fault(mut self, send_number: usize, error: Error) -> Self {
        self.faults.push_back((send_number, error));
        self
    }
}

#[derive(Clone, Debug)]
struct Probe {
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl Probe {
    fn writes(&self) -> Vec<Vec<u8>> {
        self.writes.lock().expect("writes lock").clone()
    }
}

impl HasTransportConfig for FaultTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for FaultTransport {
    fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
        self.sends = self.sends.saturating_add(1);
        let step = self
            .script
            .pop_front()
            .unwrap_or_else(|| OnSend::Reply(self.trailing.clone()));
        if let OnSend::Fail(error) = step {
            return Err(error);
        }
        self.writes
            .lock()
            .expect("writes lock")
            .push(bytes.to_vec());
        while self
            .faults
            .front()
            .is_some_and(|(number, _)| *number == self.sends)
        {
            let (_, error) = self.faults.pop_front().expect("checked above");
            self.reads.push_back(Err(error));
        }
        if let OnSend::Reply(replies) = step {
            for reply in replies {
                self.reads.push_back(Ok(reply));
            }
        }
        Ok(())
    }

    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        self.recv_into_with_timeout(dst, Duration::from_millis(1))
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        _timeout: Duration,
    ) -> Result<usize, Error> {
        let bytes = self.reads.pop_front().ok_or(Error::Timeout)??;
        dst[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }

    fn send_semantics(&self) -> SendSemantics {
        self.semantics
    }
}

fn session_config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
            .expect("two-socket runtime profile"),
    )
}

fn standard_reply() -> Vec<Vec<u8>> {
    vec![ACK_SOCKET_ONE.to_vec(), COMPLETE_SOCKET_ONE.to_vec()]
}

/// A transient receive error retries the in-flight command and keeps the
/// session usable. The 1.x runtime called this `SchedulerEvent::NetworkError`;
/// the classic trigger is a UDP `recv` reporting ECONNREFUSED after an ICMP
/// port-unreachable for an earlier datagram.
#[test]
fn transient_receive_error_retries_instead_of_destroying_the_session() {
    let transport = FaultTransport::new(
        SendSemantics::Datagram,
        vec![OnSend::Reply(Vec::new()), OnSend::Reply(standard_reply())],
    )
    .with_trailing_reply(standard_reply())
    .with_read_fault(
        1,
        Error::Io(Arc::new(std::io::Error::from(
            std::io::ErrorKind::ConnectionRefused,
        ))),
    );
    let probe = transport.probe();
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("submission")
        .applied()
        .expect("a transient receive error must retry, not fail the session");

    assert_eq!(
        probe.writes().len(),
        2,
        "the in-flight command is written again after the transient fault"
    );

    // The session survived, so later work still runs on it.
    camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("the session is still usable")
        .applied()
        .expect("later work still completes");

    session.shutdown().expect("owner shutdown");
}

/// A failed datagram write fails exactly one command and the session keeps
/// running: a datagram is framed by its own boundary, so nothing else is
/// affected. This is the per-command write behavior 1.x had.
#[test]
fn datagram_write_failure_fails_one_command_and_keeps_the_session() {
    let transport = FaultTransport::new(
        SendSemantics::Datagram,
        vec![
            OnSend::Fail(Error::Io(Arc::new(std::io::Error::from(
                std::io::ErrorKind::ConnectionRefused,
            )))),
            OnSend::Reply(standard_reply()),
        ],
    )
    .with_trailing_reply(standard_reply());
    let probe = transport.probe();
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let error = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect_err("the first write fails on the wire");
    assert!(
        matches!(error, Error::Io(_)),
        "the caller whose datagram write failed sees its own transport error, got {error:?}"
    );
    assert!(
        !error.requires_new_session(),
        "an isolated datagram write failure is not session death"
    );

    camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("the session survives an isolated datagram write failure")
        .applied()
        .expect("later work still completes");
    assert_eq!(probe.writes().len(), 1, "a failed write is not recorded");

    session.shutdown().expect("owner shutdown");
}

/// A failed *stream* write is a session verdict on purpose: the byte-stream
/// position becomes unknowable, so every affected caller must learn that a
/// replacement session is needed (#564). The exact transport cause is carried
/// in the poison reason rather than thrown away.
#[test]
fn stream_write_failure_poisons_and_names_the_transport_cause() {
    let transport = FaultTransport::new(
        SendSemantics::Stream,
        vec![
            OnSend::Reply(vec![ACK_SOCKET_ONE.to_vec()]),
            OnSend::Fail(Error::Io(Arc::new(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "peer went away mid-frame",
            )))),
        ],
    );
    let probe = transport.probe();
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let held = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("first submission");

    let error = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect_err("the second write fails on the wire");
    let Error::StreamPoisoned { reason } = &error else {
        panic!("a stream write failure is terminal for the session, got {error:?}");
    };
    assert!(
        reason.contains("peer went away mid-frame"),
        "the transport cause must survive in the poison reason: {reason}"
    );
    assert!(error.requires_new_session());

    let poisoned = held
        .applied()
        .expect_err("the rest of the session is poisoned too");
    assert!(
        matches!(poisoned, Error::StreamPoisoned { .. }),
        "expected the session-wide poison, got {poisoned:?}"
    );
    assert!(poisoned.requires_new_session());
    assert_eq!(probe.writes().len(), 1, "a failed write is not recorded");
}

/// A camera that answers `90 40 FF` / `90 50 FF` sends no socket nibble. 1.x
/// assigned a free socket; 2.0 turned the frame into a hard `InvalidResponse`
/// that killed the session.
#[test]
fn socketless_ack_and_completion_still_complete_a_command() {
    let transport = FaultTransport::new(
        SendSemantics::Datagram,
        vec![OnSend::Reply(vec![
            ACK_NO_SOCKET.to_vec(),
            COMPLETE_NO_SOCKET.to_vec(),
        ])],
    )
    .with_trailing_reply(vec![ACK_NO_SOCKET.to_vec(), COMPLETE_NO_SOCKET.to_vec()]);
    let probe = transport.probe();
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("submission")
        .applied()
        .expect("a socketless ACK and completion must still complete the command");
    assert_eq!(probe.writes().len(), 1, "no retry was needed");

    session.shutdown().expect("owner shutdown");
}

/// A camera that repeats a socket nibble it has already handed out must not
/// cost the second command its ACK deadline: 1.x reassigned it to the other
/// socket.
#[test]
fn ack_naming_an_occupied_socket_is_reassigned() {
    let transport = FaultTransport::new(
        SendSemantics::Datagram,
        vec![
            OnSend::Reply(vec![ACK_SOCKET_ONE.to_vec()]),
            // The camera answers the second command with socket one again.
            OnSend::Reply(vec![
                ACK_SOCKET_ONE.to_vec(),
                COMPLETE_SOCKET_ONE.to_vec(),
                COMPLETE_SOCKET_TWO.to_vec(),
            ]),
        ],
    );
    let probe = transport.probe();
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    let first = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("first submission");
    let second = camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .expect("second submission");

    first.applied().expect("first operation applied");
    second
        .applied()
        .expect("the reassigned ACK must complete the second operation");
    assert_eq!(
        probe.writes().len(),
        2,
        "neither command needed a retry after reassignment"
    );

    session.shutdown().expect("owner shutdown");
}

/// Issue #297: the camera's ACK is already queued by the time the write
/// returns. It must still be matched to the command that produced it.
#[test]
fn ack_queued_during_the_write_is_still_matched() {
    let transport = FaultTransport::new(
        SendSemantics::Datagram,
        vec![OnSend::Reply(standard_reply())],
    )
    .with_trailing_reply(standard_reply());
    let probe = transport.probe();
    let session = Session::open(transport, session_config()).expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");

    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("submission")
        .applied()
        .expect("an immediately answered command must not wait for its ACK deadline");
    assert_eq!(probe.writes().len(), 1);

    session.shutdown().expect("owner shutdown");
}
