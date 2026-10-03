//! Blocking operation handles observe through borrowing waits (#777).
//!
//! The blocking twin of `issue_777_operation_observation.rs`: every wait
//! takes `&mut self`, the handle caches what it observes, a timed-out wait
//! releases only itself, and cancellation is one idempotent intent per handle.

#![cfg(feature = "blocking")]
#![allow(clippy::expect_used)]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use grafton_visca::{
    blocking::{Operation, Session, SessionConfig},
    command::CommandKind,
    completion::{AppliedOnly, Targeted},
    profile::ProfileSpec,
    request::builtin::{PanTiltHome, ZoomDrive, ZoomStop},
    transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
    CancellationOutcome, Error,
};

use profile_fixtures::NonDefaultCompileTimeProfile as Raw;

const ZOOM_TELE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x02, 0xff];
const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff];
const PAN_TILT_HOME: &[u8] = &[0x81, 0x01, 0x06, 0x04, 0xff];
const CANCEL_SOCKET_ONE: &[u8] = &[0x81, 0x21, 0xff];
const ACK_SOCKET_ONE: &[u8] = &[0x90, 0x41, 0xff];
const COMPLETE_SOCKET_ONE: &[u8] = &[0x90, 0x51, 0xff];
const CANCELLED_SOCKET_ONE: &[u8] = &[0x90, 0x61, 0x04, 0xff];
const SYNTAX_ERROR_SOCKET_ONE: &[u8] = &[0x90, 0x61, 0x02, 0xff];
const PAN_TILT_POSITION_INQUIRY: &[u8] = &[0x81, 0x09, 0x06, 0x12, 0xff];
const PAN_TILT_POSITION: &[u8] = &[
    0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff,
];

/// A short wait that the scripted camera never answers in time.
const SHORT: Duration = Duration::from_millis(20);

/// Replays exactly the frames a test queues and records every write, except
/// pan-tilt position inquiries, which it answers with a fixed position once a
/// test enables that.
#[derive(Debug)]
struct ScriptedTransport {
    config: TransportConfig,
    probe: Probe,
}

#[derive(Clone, Debug, Default)]
struct Probe {
    responses: Arc<Mutex<VecDeque<Vec<u8>>>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    positions: Arc<Mutex<PositionReplies>>,
}

/// Whether position inquiries are answered, and how many written while they
/// were not are still owed a reply.
#[derive(Debug, Default)]
struct PositionReplies {
    answering: bool,
    owed: usize,
}

impl Probe {
    fn push(&self, bytes: &[u8]) {
        self.responses
            .lock()
            .expect("responses lock")
            .push_back(bytes.to_vec());
    }

    /// Answers every position inquiry from now on, including those already
    /// written and unanswered.
    fn answer_positions(&self) {
        let mut positions = self.positions.lock().expect("positions lock");
        positions.answering = true;
        for _ in 0..std::mem::take(&mut positions.owed) {
            self.push(PAN_TILT_POSITION);
        }
    }

    fn record_write(&self, bytes: &[u8]) {
        self.writes
            .lock()
            .expect("writes lock")
            .push(bytes.to_vec());
        if bytes == PAN_TILT_POSITION_INQUIRY {
            let mut positions = self.positions.lock().expect("positions lock");
            if positions.answering {
                self.push(PAN_TILT_POSITION);
            } else {
                positions.owed += 1;
            }
        }
    }

    fn writes(&self) -> Vec<Vec<u8>> {
        self.writes.lock().expect("writes lock").clone()
    }

    /// Writes other than position inquiries.
    fn commands(&self) -> Vec<Vec<u8>> {
        self.writes()
            .into_iter()
            .filter(|bytes| bytes != PAN_TILT_POSITION_INQUIRY)
            .collect()
    }
}

impl HasTransportConfig for ScriptedTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for ScriptedTransport {
    fn send_with_timeout(
        &mut self,
        bytes: &[u8],
        _kind: CommandKind,
        _timeout: Duration,
    ) -> Result<(), Error> {
        self.probe.record_write(bytes);
        Ok(())
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        timeout: Duration,
    ) -> Result<usize, Error> {
        let next = self
            .probe
            .responses
            .lock()
            .expect("responses lock")
            .pop_front();
        let Some(bytes) = next else {
            // Model a silent camera: the read waits out its bound.
            std::thread::sleep(timeout.min(Duration::from_millis(1)));
            return Err(Error::Timeout);
        };
        dst[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

fn open() -> (Session, Probe) {
    let probe = Probe::default();
    let transport = ScriptedTransport {
        config: TransportConfig::default(),
        probe: probe.clone(),
    };
    let profile = ProfileSpec::from_compile_time::<Raw>().expect("raw two-socket profile");
    let session = Session::open(transport, SessionConfig::new(profile)).expect("owner session");
    (session, probe)
}

/// Admits a continuous zoom and queues its acknowledgement on socket one.
fn running_zoom<'session>(
    session: &'session Session,
    probe: &Probe,
) -> Operation<'session, AppliedOnly> {
    let camera = session.camera::<Raw>().expect("raw camera");
    let operation = camera
        .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
        .expect("continuous zoom admitted");
    probe.push(ACK_SOCKET_ONE);
    operation
}

fn debug<T: std::fmt::Debug>(value: &T) -> String {
    format!("{value:?}")
}

#[test]
fn a_timed_out_wait_keeps_the_handle_observing() {
    let (session, probe) = open();
    let mut moving = running_zoom(&session, &probe);

    assert!(matches!(
        moving.applied_with_timeout(SHORT),
        Err(Error::Timeout)
    ));
    probe.push(COMPLETE_SOCKET_ONE);
    moving
        .applied()
        .expect("the handle observes the completion");
    moving
        .applied_with_timeout(Duration::ZERO)
        .expect("a cached outcome needs no wait");
    assert_eq!(probe.writes(), vec![ZOOM_TELE.to_vec()]);
    drop(moving);
    session.shutdown().expect("owner shutdown");
}

#[test]
fn application_then_settlement_restarts_an_abandoned_proof() {
    let (session, probe) = open();
    let camera = session.camera::<Raw>().expect("raw camera");
    let mut home = camera
        .submit::<Targeted, _>(&PanTiltHome)
        .expect("home admitted");
    probe.push(ACK_SOCKET_ONE);

    // Abandoned while awaiting application.
    assert!(matches!(
        home.settled_with_timeout(SHORT),
        Err(Error::Timeout)
    ));
    probe.push(COMPLETE_SOCKET_ONE);
    home.applied().expect("home applied");

    // Abandoned while polling: the camera does not answer position inquiries.
    assert!(matches!(
        home.settled_with_timeout(SHORT),
        Err(Error::Timeout)
    ));
    assert!(
        probe.writes().len() > 1,
        "settlement polls the pan-tilt position"
    );

    probe.answer_positions();
    home.settled().expect("two equal samples settle home");
    let polled = probe.writes().len();
    home.settled_with_timeout(Duration::ZERO)
        .expect("settlement is cached");
    home.applied().expect("application is cached");
    assert_eq!(probe.writes().len(), polled, "cached waits poll nothing");
    assert_eq!(probe.commands(), vec![PAN_TILT_HOME.to_vec()]);
    drop(home);
    session.shutdown().expect("owner shutdown");
}

#[test]
fn a_failed_outcome_is_cached_and_answers_cancel() {
    let (session, probe) = open();
    let mut moving = running_zoom(&session, &probe);
    probe.push(SYNTAX_ERROR_SOCKET_ONE);

    let first = moving.applied().expect_err("the camera rejects it");
    let again = moving.applied().expect_err("the failure is cached");
    assert_eq!(debug(&first), debug(&again));
    let cancelled = moving.cancel().expect_err("the failure decides");
    assert_eq!(debug(&first), debug(&cancelled));
    assert_eq!(probe.writes(), vec![ZOOM_TELE.to_vec()]);
    drop(moving);
    session.shutdown().expect("owner shutdown");
}

#[test]
fn cancellation_is_one_idempotent_intent() {
    let (session, probe) = open();
    let mut moving = running_zoom(&session, &probe);

    assert!(matches!(
        moving.cancel_with_timeout(SHORT),
        Err(Error::Timeout)
    ));
    assert!(matches!(
        moving.cancel_with_timeout(SHORT),
        Err(Error::Timeout)
    ));
    assert_eq!(
        probe.writes(),
        vec![ZOOM_TELE.to_vec(), CANCEL_SOCKET_ONE.to_vec()],
        "the repeat observed the one intent"
    );

    probe.push(CANCELLED_SOCKET_ONE);
    assert_eq!(
        moving.cancel().expect("cancellation won"),
        CancellationOutcome::Cancelled
    );
    assert!(matches!(moving.applied(), Err(Error::CommandCanceled)));
    assert_eq!(
        moving.cancel().expect("cached"),
        CancellationOutcome::Cancelled
    );
    assert_eq!(probe.writes().len(), 2);
    drop(moving);
    session.shutdown().expect("owner shutdown");
}

#[test]
fn cancel_after_the_outcome_sends_nothing() {
    let (session, probe) = open();
    let mut moving = running_zoom(&session, &probe);
    probe.push(COMPLETE_SOCKET_ONE);
    moving.applied().expect("completed");

    assert_eq!(
        moving.cancel().expect("completed first"),
        CancellationOutcome::Completed
    );
    assert_eq!(probe.writes(), vec![ZOOM_TELE.to_vec()]);
    drop(moving);
    session.shutdown().expect("owner shutdown");
}

#[test]
fn a_failed_cancellation_leaves_the_outcome_observable() {
    let (session, probe) = open();
    let mut moving = running_zoom(&session, &probe);

    // The camera never answers the socket cancel, so the owner's own
    // cancellation observation deadline ends the intent.
    let failed = moving.cancel().expect_err("unanswered cancel fails");
    assert!(matches!(failed, Error::Timeout), "{failed:?}");
    let again = moving.cancel().expect_err("the failure is cached");
    assert!(matches!(again, Error::Timeout), "{again:?}");
    assert_eq!(
        probe.writes(),
        vec![ZOOM_TELE.to_vec(), CANCEL_SOCKET_ONE.to_vec()],
        "a failed intent is not retried"
    );

    probe.push(COMPLETE_SOCKET_ONE);
    moving
        .applied()
        .expect("the operation's own outcome is still observed");
    assert_eq!(
        moving.cancel().expect("the outcome decides"),
        CancellationOutcome::Completed
    );
    drop(moving);
    session.shutdown().expect("owner shutdown");
}

#[test]
fn dropping_a_handle_mid_cancellation_keeps_the_owner_serving() {
    let (session, probe) = open();
    let mut moving = running_zoom(&session, &probe);
    assert!(matches!(
        moving.cancel_with_timeout(SHORT),
        Err(Error::Timeout)
    ));
    drop(moving);
    probe.push(CANCELLED_SOCKET_ONE);

    let camera = session.camera::<Raw>().expect("raw camera");
    let mut stop = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("stop admitted");
    probe.push(ACK_SOCKET_ONE);
    probe.push(COMPLETE_SOCKET_ONE);
    stop.applied().expect("the owner keeps serving");
    assert_eq!(
        probe.writes(),
        vec![
            ZOOM_TELE.to_vec(),
            CANCEL_SOCKET_ONE.to_vec(),
            ZOOM_STOP.to_vec()
        ]
    );
    drop(stop);
    session.shutdown().expect("owner shutdown");
}
