//! Issue #690: `set_tuning` must observe the session's terminal boundary.
//!
//! `set_tuning` documents that it "returns the session's terminal error if the
//! owner is gone," but the blocking `reconfigure` path mutated owner state
//! directly and never consulted the boundary gate, so it returned `Ok(())` on a
//! poisoned or closed session — a silent no-op contradicting its contract. It
//! now takes the same `enter` turn every submission does: a live session still
//! reconfigures, a re-entrant call is `TransportBusy`, and a terminated session
//! yields its terminal error. Profile validation failures travel through that
//! turn too, so an invalid proposal cannot mask an existing terminal cause.

#![cfg(feature = "blocking")]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{collections::VecDeque, sync::Arc, time::Duration};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    command::CommandKind,
    completion::AppliedOnly,
    profile::ProfileSpec,
    request::builtin::ZoomStop,
    transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
    Error, OperationalTuning,
};

use profile_fixtures::NonDefaultCompileTimeProfile;

/// A scripted raw datagram camera. Each send optionally queues reads, and a send
/// may instead fail the following read pump (to poison the strict session).
#[derive(Debug)]
struct ScriptedTransport {
    config: TransportConfig,
    sends: usize,
    fault_after_send: Option<usize>,
    reads: VecDeque<Result<Vec<u8>, Error>>,
    replies_each_send: Vec<Vec<u8>>,
}

impl ScriptedTransport {
    fn faulting() -> Self {
        Self {
            config: TransportConfig::default(),
            sends: 0,
            fault_after_send: Some(1),
            reads: VecDeque::new(),
            replies_each_send: Vec::new(),
        }
    }

    fn healthy() -> Self {
        Self {
            config: TransportConfig::default(),
            sends: 0,
            fault_after_send: None,
            reads: VecDeque::new(),
            replies_each_send: vec![vec![0x90, 0x41, 0xff], vec![0x90, 0x51, 0xff]],
        }
    }
}

impl HasTransportConfig for ScriptedTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for ScriptedTransport {
    fn send_with_kind(&mut self, _bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
        self.sends = self.sends.saturating_add(1);
        if self.fault_after_send == Some(self.sends) {
            self.reads
                .push_back(Err(Error::Io(Arc::new(std::io::Error::from(
                    std::io::ErrorKind::ConnectionRefused,
                )))));
        } else {
            for reply in &self.replies_each_send {
                self.reads.push_back(Ok(reply.clone()));
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
        SendSemantics::Datagram
    }
}

fn raw_config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
            .expect("raw runtime profile"),
    )
}

/// A live session still accepts a valid retune — the fix must not break the
/// documented happy path.
#[test]
fn set_tuning_on_a_live_session_still_succeeds() {
    let session = Session::open(ScriptedTransport::healthy(), raw_config()).expect("owner session");
    let tuning = OperationalTuning::new().command_spacing(Duration::from_millis(40));
    session
        .set_tuning(tuning)
        .expect("a live session accepts a valid retune");
    assert_eq!(
        session.tuning(),
        tuning,
        "the retune took effect on the live session"
    );
    session.shutdown().expect("owner shutdown");
}

#[test]
fn set_tuning_on_a_live_session_rejects_invalid_tuning_without_changing_it() {
    let session = Session::open(ScriptedTransport::healthy(), raw_config()).expect("owner session");
    let installed = OperationalTuning::new().command_spacing(Duration::from_millis(40));
    session
        .set_tuning(installed)
        .expect("a live session accepts valid tuning");

    let error = session
        .set_tuning(OperationalTuning::new().ack_timeout(Duration::ZERO))
        .expect_err("a live session still rejects a zero protocol timeout");
    assert!(matches!(error, Error::InvalidRequest(_)), "got {error:?}");
    assert_eq!(
        session.tuning(),
        installed,
        "a rejected live update leaves the installed tuning unchanged"
    );
    session.shutdown().expect("owner shutdown");
}

#[test]
fn set_tuning_on_a_poisoned_session_returns_the_terminal_error() {
    let config = raw_config().with_strict_unconfirmed_poison(true);
    let session = Session::open(ScriptedTransport::faulting(), config).expect("owner session");
    {
        let camera = session
            .camera::<NonDefaultCompileTimeProfile>()
            .expect("camera view");
        let error = camera
            .submit::<AppliedOnly, _>(&ZoomStop)
            .expect("submission")
            .applied()
            .expect_err("the strict opt-in poisons on the receive fault");
        assert!(matches!(error, Error::StreamPoisoned { .. }));
    }

    // Even an otherwise-valid update must report the retained terminal cause
    // once the owner is poisoned.
    let error = session
        .set_tuning(OperationalTuning::new())
        .expect_err("set_tuning must not silently succeed on a poisoned session");
    assert!(
        matches!(error, Error::StreamPoisoned { .. }),
        "set_tuning surfaces the session's terminal error, got {error:?}"
    );
    assert!(error.requires_new_session());

    let error = session
        .set_tuning(OperationalTuning::new().ack_timeout(Duration::ZERO))
        .expect_err("profile validation cannot mask an existing poison");
    assert!(
        matches!(error, Error::StreamPoisoned { .. }),
        "set_tuning gives the poison precedence over InvalidRequest, got {error:?}"
    );
}

#[test]
fn set_tuning_on_a_closed_session_returns_the_terminal_error() {
    let session = Session::open(ScriptedTransport::healthy(), raw_config()).expect("owner session");
    session.shutdown().expect("clean shutdown");

    // The session is Shutdown; even an otherwise-valid update must report that
    // retained terminal cause.
    let error = session
        .set_tuning(OperationalTuning::new())
        .expect_err("set_tuning must not silently succeed on a closed session");
    assert!(
        matches!(error, Error::RuntimeShutdown),
        "set_tuning surfaces the shutdown boundary, got {error:?}"
    );

    let error = session
        .set_tuning(OperationalTuning::new().ack_timeout(Duration::ZERO))
        .expect_err("profile validation cannot mask a completed shutdown");
    assert!(
        matches!(error, Error::RuntimeShutdown),
        "set_tuning gives shutdown precedence over InvalidRequest, got {error:?}"
    );
}
