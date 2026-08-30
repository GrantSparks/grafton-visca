//! Issue #700: a no-reply raw operation is terminal exactly when its first
//! blocking transport write succeeds.

#![cfg(feature = "blocking")]

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    command::CommandKind,
    completion,
    raw::{self, RawReplyShape},
    transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
    AffectedAxes, ControlClass, Error, ProfileSpec, RetryClass, TimeoutClass,
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
    fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
        self.writes.lock().expect("write lock").push(bytes.to_vec());
        Ok(())
    }

    fn recv_into(&mut self, _dst: &mut [u8]) -> Result<usize, Error> {
        *self.receives.lock().expect("receive lock") += 1;
        Err(Error::Timeout)
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
fn no_reply_operation_returns_an_applied_handle_after_one_write() {
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
    let command = raw::AppliedOnly::with_policy(wire, AffectedAxes::ZOOM, policy)
        .expect("no-reply raw operation");

    camera
        .submit::<completion::AppliedOnly, _>(&command)
        .expect("a successful first write returns a blocking operation handle")
        .applied()
        .expect("the immediate no-reply terminal remains available to the handle");

    assert_eq!(
        *writes.lock().expect("write lock"),
        vec![wire.to_vec()],
        "the no-reply operation is written exactly once"
    );
    assert_eq!(
        *receives.lock().expect("receive lock"),
        0,
        "no-reply application does not pump for a response"
    );

    session.shutdown().expect("owner shutdown");
}
