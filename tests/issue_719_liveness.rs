//! Issue #719: silence is distinct from session death and valid replies expose
//! a positive, facade-level liveness signal.

#![cfg(feature = "blocking")]

use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use grafton_visca::{
    blocking::Session,
    camera::profiles::PtzOpticsG2,
    command::CommandKind,
    profile::ProfileSpec,
    transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
    Error, OperationalTuning, SessionConfig,
};

#[derive(Debug, Clone, Copy)]
enum Peer {
    Silent,
    Answers,
    KeepaliveExpired,
}

#[derive(Debug)]
struct LivenessTransport {
    config: TransportConfig,
    peer: Peer,
    reply_pending: bool,
}

impl LivenessTransport {
    fn new(peer: Peer) -> Self {
        Self {
            config: TransportConfig::default(),
            peer,
            reply_pending: false,
        }
    }
}

impl HasTransportConfig for LivenessTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for LivenessTransport {
    fn send_with_timeout(
        &mut self,
        _bytes: &[u8],
        _kind: CommandKind,
        _timeout: Duration,
    ) -> Result<(), Error> {
        self.reply_pending = true;
        Ok(())
    }

    fn recv_into_with_timeout(
        &mut self,
        destination: &mut [u8],
        timeout: Duration,
    ) -> Result<usize, Error> {
        match self.peer {
            Peer::Answers if self.reply_pending => {
                self.reply_pending = false;
                let reply = [0x90, 0x50, 0x02, 0xff];
                destination[..reply.len()].copy_from_slice(&reply);
                Ok(reply.len())
            }
            Peer::KeepaliveExpired => Err(Error::Io(Arc::new(std::io::Error::from(
                std::io::ErrorKind::TimedOut,
            )))),
            Peer::Silent | Peer::Answers => {
                thread::sleep(timeout.min(Duration::from_millis(5)));
                Err(Error::Timeout)
            }
        }
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Stream
    }
}

fn config() -> SessionConfig {
    SessionConfig::new(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("built-in profile"))
        .with_tuning(OperationalTuning::new().retry_limit(0).retry_timing(
            Duration::from_millis(1),
            Duration::from_millis(1),
            Duration::from_millis(40),
        ))
        .expect("bounded liveness-probe tuning")
}

#[test]
fn response_counter_distinguishes_silence_from_positive_liveness() {
    let silent = Session::open(LivenessTransport::new(Peer::Silent), config()).expect("session");
    let before = silent.metrics().expect("metrics before probe");
    let started = Instant::now();
    let error = silent
        .camera::<PtzOpticsG2>()
        .expect("camera")
        .power()
        .state()
        .expect_err("silent peer must exhaust the bounded probe");
    assert!(matches!(error, Error::Timeout));
    assert!(error.is_retryable());
    assert!(!error.requires_new_session());
    assert!(started.elapsed() < Duration::from_secs(1));
    let after = silent.metrics().expect("metrics after probe");
    assert_eq!(before.received_frames, 0);
    assert_eq!(after.received_frames, before.received_frames);
    silent
        .close()
        .expect("a silent session is still locally live");

    let answering =
        Session::open(LivenessTransport::new(Peer::Answers), config()).expect("session");
    let before = answering.metrics().expect("metrics before probe");
    assert!(answering
        .camera::<PtzOpticsG2>()
        .expect("camera")
        .power()
        .state()
        .expect("power reply"));
    let after = answering.metrics().expect("metrics after probe");
    assert_eq!(after.received_frames, before.received_frames + 1);
    answering.close().expect("close answering session");
}

#[test]
fn os_keepalive_timeout_is_session_death_not_idle_no_data() {
    let session =
        Session::open(LivenessTransport::new(Peer::KeepaliveExpired), config()).expect("session");
    let error = session
        .camera::<PtzOpticsG2>()
        .expect("camera")
        .power()
        .state()
        .expect_err("OS timeout must terminate the connection");
    assert!(matches!(error, Error::ConnectionClosed { .. }));
    assert!(error.requires_new_session());
    assert!(matches!(
        session.close(),
        Err(Error::ConnectionClosed { .. })
    ));
}
