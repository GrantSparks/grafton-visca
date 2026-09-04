//! A no-reply raw plain command succeeds exactly when its first blocking
//! transport write succeeds. It deliberately does not create an operation
//! handle or claim protocol application.

#![cfg(feature = "blocking")]

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    command::CommandKind,
    raw::{self, RawReplyShape},
    transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
    ControlClass, DiagnosticEvent, DiagnosticOutcome, Error, ProfileSpec, RetryClass, TimeoutClass,
};

#[derive(Debug)]
struct NoReplyTransport {
    config: TransportConfig,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    receives: Arc<Mutex<usize>>,
}

impl NoReplyTransport {
    fn new() -> Self {
        Self {
            config: TransportConfig::default(),
            writes: Arc::new(Mutex::new(Vec::new())),
            receives: Arc::new(Mutex::new(0)),
        }
    }
}

impl HasTransportConfig for NoReplyTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for NoReplyTransport {
    fn send_with_timeout(
        &mut self,
        bytes: &[u8],
        _kind: CommandKind,
        _timeout: Duration,
    ) -> Result<(), Error> {
        self.writes.lock().expect("write lock").push(bytes.to_vec());
        Ok(())
    }

    fn recv_into_with_timeout(
        &mut self,
        _dst: &mut [u8],
        _timeout: Duration,
    ) -> Result<usize, Error> {
        *self.receives.lock().expect("receive lock") += 1;
        Err(Error::Timeout)
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

#[test]
fn no_reply_plain_command_returns_after_one_write() {
    let transport = NoReplyTransport::new();
    let writes = Arc::clone(&transport.writes);
    let receives = Arc::clone(&transport.receives);
    let profile = ProfileSpec::from_compile_time::<grafton_visca::profiles::PtzOpticsG2>()
        .expect("runtime profile");
    let session = Session::open(transport, SessionConfig::new(profile)).expect("blocking session");
    let camera = session
        .camera::<grafton_visca::profiles::PtzOpticsG2>()
        .expect("camera view");

    let wire = [0x81, 0x01, 0x04, 0x07, 0xff];
    let policy = raw::Policy::new(TimeoutClass::Quick, RetryClass::Never, ControlClass::Normal)
        .expect("raw policy")
        .with_reply_shape(RawReplyShape::NoReply);
    let command = raw::Plain::with_policy(wire, policy).expect("no-reply raw command");

    camera
        .execute(&command)
        .expect("a successful first write returns plain fire-and-forget success");

    assert_eq!(
        *writes.lock().expect("write lock"),
        vec![wire.to_vec()],
        "the no-reply command is locally written exactly once"
    );
    assert_eq!(
        *receives.lock().expect("receive lock"),
        0,
        "a successful local no-reply write does not pump for a response"
    );
    assert!(
        session
            .drain_diagnostics()
            .expect("diagnostics")
            .iter()
            .any(|event| matches!(
                event,
                DiagnosticEvent::Terminal {
                    outcome: DiagnosticOutcome::Written,
                    ..
                }
            )),
        "the public diagnostic reports a local write, never Applied"
    );

    session.shutdown().expect("owner shutdown");
}
