//! Fresh-session recovery after an unusable transport.
//!
//! A poisoned or dropped session is terminal: the owner never reconnects or
//! resubmits on its own. Recovery keeps the reusable `SessionConfig`, builds a
//! *fresh* session from a re-callable transport factory, and re-queries camera
//! state before deliberately restoring the desired power-on state — the new
//! session's state cache starts `Unknown`. `Error::requires_new_session()` is
//! the reconnect decision.
//!
//! This is exactly the path a long-running supervisor takes when a TCP camera
//! closes or stops answering. A peer close is positive session-death evidence;
//! a silent open socket instead ends its bounded heartbeat with `Error::Timeout`
//! and `requires_new_session() == false`. The application owns the silence
//! threshold. It compares `MetricsSnapshot::received_frames` around each
//! heartbeat and deliberately rebuilds once its policy sees no positive reply.
//! The library runs no heartbeat timer of its own. See
//! `docs/observability_and_recovery.md` ("Silent peers and connection
//! liveness") for the complete policy.
//!
//! This example uses an in-memory transport, so it runs with no hardware: the
//! first transport stays open but never answers, and the factory's next
//! transport answers the re-query inquiries. The demo uses an intentionally
//! short retry budget; production supervisors should choose their own
//! application silence threshold. Requires only the default `blocking` feature.

use std::{collections::VecDeque, time::Duration};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    camera::profiles::PtzOpticsG2,
    command::CommandKind,
    profile::ProfileSpec,
    transport::{BlockingTransport, HasTransportConfig, TransportConfig},
    Error, OperationalTuning,
};

#[derive(Debug, Clone, Copy)]
enum Peer {
    Silent,
    Answers,
}

/// A single-camera IP transport. A silent peer accepts writes and keeps its
/// socket open but never produces a VISCA frame. An answering peer responds to
/// the power, zoom, and pan/tilt position inquiries with valid VISCA data.
#[derive(Debug)]
struct FakeCamera {
    config: TransportConfig,
    peer: Peer,
    replies: VecDeque<Vec<u8>>,
}

impl FakeCamera {
    fn new(peer: Peer) -> Self {
        Self {
            config: TransportConfig::default(),
            peer,
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
    fn send_with_timeout(
        &mut self,
        bytes: &[u8],
        _kind: CommandKind,
        _timeout: Duration,
    ) -> Result<(), Error> {
        if matches!(self.peer, Peer::Silent) {
            return Ok(()); // accepted locally; the peer never answers
        }
        let reply = match (bytes.get(1), bytes.get(2), bytes.get(3)) {
            (Some(0x09), Some(0x04), Some(0x00)) => vec![0x90, 0x50, 0x03, 0xff], // power OFF
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

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        timeout: Duration,
    ) -> Result<usize, Error> {
        if matches!(self.peer, Peer::Silent) {
            std::thread::sleep(timeout.min(Duration::from_millis(5)));
            return Err(Error::Timeout);
        }
        let reply = self.replies.pop_front().ok_or(Error::Timeout)?;
        dst[..reply.len()].copy_from_slice(&reply);
        Ok(reply.len())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = SessionConfig::new(ProfileSpec::from_compile_time::<PtzOpticsG2>()?).with_tuning(
        OperationalTuning::new().retry_limit(0).retry_timing(
            Duration::from_millis(1),
            Duration::from_millis(1),
            Duration::from_millis(500),
        ),
    )?;

    // A RE-CALLABLE factory (`FnMut`, not `FnOnce`): a supervisor can call it on
    // every reconnect. Here the first transport is silent and later ones
    // answer, so the application-owned silence path is deterministic.
    let mut attempt = 0usize;
    let mut new_transport = move || -> Result<FakeCamera, Error> {
        attempt += 1;
        Ok(FakeCamera::new(if attempt > 1 {
            Peer::Answers
        } else {
            Peer::Silent
        }))
    };

    // Sample positive liveness around one application-owned heartbeat.
    let session = Session::open(new_transport()?, config.clone())?;
    let frames_before = session.metrics()?.received_frames;
    let probe = session.camera::<PtzOpticsG2>()?.power().state();
    let frames_after = session.metrics()?.received_frames;
    let received_a_frame = frames_after > frames_before;

    match probe {
        Ok(state) if received_a_frame => {
            session.close()?;
            println!("first session healthy (power {state}); nothing to recover");
        }
        Err(error) if error.requires_new_session() => {
            let _ = session.close();
            println!("session unusable ({error}); rebuilding from the retained config");
            recover(&config, &mut new_transport)?;
        }
        Err(error) if !received_a_frame => {
            // `Timeout` alone does not prove the owner is dead. This demo's
            // application policy rebuilds after one unanswered heartbeat; a
            // production supervisor will usually require a longer run.
            session.close()?;
            println!("no response frame arrived ({error}); rebuilding by silence policy");
            recover(&config, &mut new_transport)?;
        }
        Err(error) => {
            let _ = session.close();
            return Err(error.into());
        }
        Ok(_) => {
            session.close()?;
            return Err("heartbeat completed without a decoded response frame".into());
        }
    }
    Ok(())
}

/// Builds a fresh session from the reusable config, re-queries state, and then
/// restores the application's desired power-on state. The factory is borrowed
/// `&mut` so a supervisor loop can keep calling it.
fn recover(
    config: &SessionConfig,
    new_transport: &mut impl FnMut() -> Result<FakeCamera, Error>,
) -> Result<(), Error> {
    let session = Session::open(new_transport()?, config.clone())?;
    let result = requery_and_restore(&session);
    match (result, session.close()) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), _) | (Ok(()), Err(error)) => Err(error),
    }
}

fn requery_and_restore(session: &Session) -> Result<(), Error> {
    // The fresh cache is Unknown: read the live values before restoring desired
    // state. The owner never resubmits the old session's commands.
    let camera = session.camera::<PtzOpticsG2>()?;
    let frames_before = session.metrics()?.received_frames;
    let power = camera.power().state()?;
    let zoom = camera.zoom().position()?;
    let pan_tilt = camera.pan_tilt().position()?;
    let frames_after = session.metrics()?.received_frames;
    if frames_after <= frames_before {
        return Err(Error::InvalidState(
            "successful re-query produced no positive liveness frame".into(),
        ));
    }
    println!(
        "recovered: power={power}, zoom=0x{:04X}, pan/tilt={pan_tilt:?}, response_frames={}",
        zoom.value(),
        frames_after - frames_before
    );
    if !power {
        camera.power().on()?;
        println!("restored desired power-on state after the fresh re-query");
    }
    Ok(())
}
