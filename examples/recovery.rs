//! Fresh-session recovery after an unusable transport.
//!
//! A poisoned or dropped session is terminal: the owner never reconnects or
//! resubmits on its own. Recovery keeps the reusable `SessionConfig`, builds a
//! *fresh* session from a re-callable transport factory, and re-queries camera
//! state before doing anything else — the new session's state cache starts
//! `Unknown`. `Error::requires_new_session()` is the reconnect decision.
//!
//! This example uses an in-memory transport, so it runs with no hardware: the
//! first transport reports a peer close (`requires_new_session()` is `true`) and
//! the factory's next transport answers the re-query inquiries. Requires only the
//! default `blocking` feature.

use std::{collections::VecDeque, time::Duration};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    camera::profiles::PtzOpticsG2,
    command::CommandKind,
    profile::ProfileSpec,
    transport::{BlockingTransport, HasTransportConfig, TransportConfig},
    Error,
};

/// A single-camera IP transport. When `healthy` is false every read reports a
/// zero-byte peer close, which the owner surfaces as `Error::ConnectionClosed`
/// (`requires_new_session()` is `true`). When healthy it answers the power,
/// zoom, and pan/tilt position inquiries with valid VISCA data.
#[derive(Debug)]
struct FakeCamera {
    config: TransportConfig,
    healthy: bool,
    replies: VecDeque<Vec<u8>>,
}

impl FakeCamera {
    fn new(healthy: bool) -> Self {
        Self {
            config: TransportConfig::default(),
            healthy,
            replies: VecDeque::new(),
        }
    }
}

impl HasTransportConfig for FakeCamera {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for FakeCamera {
    fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
        if !self.healthy {
            return Ok(()); // the following read reports the peer close
        }
        let reply = match (bytes.get(1), bytes.get(2), bytes.get(3)) {
            (Some(0x09), Some(0x04), Some(0x00)) => vec![0x90, 0x50, 0x02, 0xff], // power ON
            (Some(0x09), Some(0x04), Some(0x47)) => {
                vec![0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0xff] // zoom = 0x1234
            }
            (Some(0x09), Some(0x06), Some(0x12)) => {
                vec![
                    0x90, 0x50, 0x00, 0x01, 0x02, 0x03, 0x00, 0x04, 0x05, 0x06, 0xff,
                ] // pan/tilt
            }
            _ => {
                self.replies.push_back(vec![0x90, 0x41, 0xff]); // ACK
                vec![0x90, 0x51, 0xff] // completion
            }
        };
        self.replies.push_back(reply);
        Ok(())
    }

    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        if !self.healthy {
            return Ok(0); // zero-byte read == peer closed == ConnectionClosed
        }
        let reply = self.replies.pop_front().ok_or(Error::Timeout)?;
        dst[..reply.len()].copy_from_slice(&reply);
        Ok(reply.len())
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        _timeout: Duration,
    ) -> Result<usize, Error> {
        self.recv_into(dst)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = SessionConfig::new(ProfileSpec::from_compile_time::<PtzOpticsG2>()?);

    // A RE-CALLABLE factory (`FnMut`, not `FnOnce`): a supervisor can call it on
    // every reconnect. Here the first transport is dead and later ones are
    // healthy, so the recovery path is exercised deterministically.
    let mut attempt = 0usize;
    let mut new_transport = move || -> Result<FakeCamera, Error> {
        attempt += 1;
        Ok(FakeCamera::new(attempt > 1))
    };

    // Open the first (doomed) session and probe it.
    let session = Session::open(new_transport()?, config.clone())?;
    let probe = session.camera::<PtzOpticsG2>()?.power().state();
    // The session is already dead on the error path, so its close reports the
    // same terminal error; ignore it and let `probe` drive the decision.
    let _ = session.close();

    match probe {
        Ok(state) => {
            println!("first session healthy (power {state}); nothing to recover");
        }
        Err(error) if error.requires_new_session() => {
            println!("session unusable ({error}); rebuilding from the retained config");
            recover(&config, &mut new_transport)?;
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

/// Builds a fresh session from the reusable config and re-queries state before
/// restoring anything. The factory is borrowed `&mut` so a supervisor loop can
/// keep calling it.
fn recover(
    config: &SessionConfig,
    new_transport: &mut impl FnMut() -> Result<FakeCamera, Error>,
) -> Result<(), Error> {
    let session = Session::open(new_transport()?, config.clone())?;
    let result = requery(&session);
    match (result, session.close()) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), _) | (Ok(()), Err(error)) => Err(error),
    }
}

fn requery(session: &Session) -> Result<(), Error> {
    // The fresh cache is Unknown: read the live values before restoring desired
    // state. The owner never resubmits the old session's commands.
    let camera = session.camera::<PtzOpticsG2>()?;
    let power = camera.power().state()?;
    let zoom = camera.zoom().position()?;
    let pan_tilt = camera.pan_tilt().position()?;
    println!(
        "recovered: power={power}, zoom=0x{:04X}, pan/tilt={pan_tilt:?}",
        zoom.value()
    );
    // A real supervisor would now re-apply its intended state deliberately.
    Ok(())
}
