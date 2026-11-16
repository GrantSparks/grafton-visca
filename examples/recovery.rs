//! Fresh-session recovery after an unusable transport.
//!
//! A poisoned or dropped session is terminal: the owner never reconnects or
//! resubmits on its own. Recovery keeps the reusable `SessionConfig`, builds a
//! *fresh* session from a re-callable transport factory, and re-queries camera
//! state before deliberately restoring the desired power-on state — the new
//! session's state cache starts `Unknown`. `Recovery::ReopenSession` is the reconnect decision.
//!
//! This is exactly the path a long-running supervisor takes when a TCP camera
//! closes or stops answering. A peer close is positive session-death evidence;
//! a silent open socket instead ends its bounded heartbeat with `Error::Timeout`
//! and a resubmission verdict rather than `ReopenSession`. The application owns the silence
//! threshold. It compares `MetricsSnapshot::received_frames` around each
//! heartbeat and deliberately rebuilds once its policy sees no positive reply.
//! The library runs no heartbeat timer of its own. See
//! `docs/observability_and_recovery.md` ("Silent peers and connection
//! liveness") for the complete policy.
//!
//! This example uses an in-memory transport, so it runs with no hardware: the
//! two runs first demonstrate a silent open peer and then a closed peer. In
//! each run the factory's next transport answers the re-query inquiries. The demo disables retries
//! (`retry_limit(0)`), so the silent probe ends at the profile's own deadline;
//! production supervisors should choose their own application silence
//! threshold. Requires only the default `blocking` feature.

use std::{collections::VecDeque, time::Duration};

use grafton_visca::{
    blocking::Session, config::OperationalTuning, config::SessionConfig, error::Recovery,
    profiles::ProfileSpec, profiles::PtzOpticsG2, transport::BlockingTransport,
    transport::HasTransportConfig, transport::ReceiveOutcome, transport::TransportConfig,
    types::CameraId, Error,
};

#[derive(Debug, Clone, Copy)]
enum Peer {
    Silent,
    Closed,
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
    fn identity(&self) -> grafton_visca::transport::TransportIdentity<'_> {
        grafton_visca::transport::TransportIdentity::Custom {
            semantics: grafton_visca::transport::SendSemantics::Stream,
            addressing: grafton_visca::transport::AddressingMode::Ip,
        }
    }

    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for FakeCamera {
    fn send_with_timeout(&mut self, bytes: &[u8], _timeout: Duration) -> Result<(), Error> {
        if matches!(self.peer, Peer::Silent | Peer::Closed) {
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
    ) -> Result<ReceiveOutcome, Error> {
        if matches!(self.peer, Peer::Closed) {
            return Err(Error::connection_closed(None));
        }
        if matches!(self.peer, Peer::Silent) {
            std::thread::sleep(timeout.min(Duration::from_millis(5)));
            return Err(Error::io_timeout());
        }
        let reply = self.replies.pop_front().ok_or(Error::io_timeout())?;
        Ok(ReceiveOutcome::copy_message(&reply, dst))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    demonstrate(Peer::Silent)?;
    demonstrate(Peer::Closed)
}

fn demonstrate(first_peer: Peer) -> Result<(), Box<dyn std::error::Error>> {
    let config = SessionConfig::new(ProfileSpec::from_compile_time::<PtzOpticsG2>()?)
        .with_tuning(OperationalTuning::new().with_retry_limit(0));

    // A RE-CALLABLE factory (`FnMut`, not `FnOnce`): a supervisor can call it on
    // every reconnect. Here the first transport is silent and later ones
    // answer, so the application-owned silence path is deterministic.
    let mut attempt = 0usize;
    let mut new_transport = move || -> Result<FakeCamera, Error> {
        attempt += 1;
        Ok(FakeCamera::new(if attempt > 1 {
            Peer::Answers
        } else {
            first_peer
        }))
    };

    // Sample positive liveness around one application-owned heartbeat.
    let session = Session::open(new_transport()?, &config)?;
    let frames_before = session.metrics()?.received_frames;
    let probe = session
        .camera::<PtzOpticsG2>(CameraId::CAMERA_1)?
        .power()
        .status();
    let frames_after = session.metrics()?.received_frames;
    let received_a_frame = frames_after > frames_before;

    match probe {
        Ok(state) if received_a_frame => {
            session.close()?;
            println!("first session healthy (power {state:?}); nothing to recover");
        }
        Err(error) if error.recovery() == Recovery::ReopenSession => {
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
    let session = Session::open(new_transport()?, config)?;
    let result = requery_and_restore(&session);
    match (result, session.close()) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), _) | (Ok(()), Err(error)) => Err(error),
    }
}

fn requery_and_restore(session: &Session) -> Result<(), Error> {
    // The fresh cache is Unknown: read the live values before restoring desired
    // state. The owner never resubmits the old session's commands.
    let camera = session.camera::<PtzOpticsG2>(CameraId::CAMERA_1)?;
    let frames_before = session.metrics()?.received_frames;
    let power = camera.power().status()?;
    let zoom = camera.zoom().position()?;
    let pan_tilt = camera.pan_tilt().position()?;
    let frames_after = session.metrics()?.received_frames;
    if frames_after <= frames_before {
        return Err(Error::InvalidState {
            reason: "successful re-query produced no positive liveness frame".into(),
        });
    }
    println!(
        "recovered: power={power:?}, zoom=0x{:04X}, pan/tilt={pan_tilt:?}, response_frames={}",
        zoom.value(),
        frames_after - frames_before
    );
    if power == grafton_visca::types::PowerStatus::Standby {
        camera.power().on()?;
        println!("restored desired power-on state after the fresh re-query");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_peer_reaches_the_reopen_session_recovery_arm(
    ) -> Result<(), Box<dyn std::error::Error>> {
        demonstrate(Peer::Closed)
    }
}
