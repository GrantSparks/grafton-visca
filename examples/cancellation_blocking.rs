//! Blocking cancellation on a cancel-capable profile, using an in-memory Sony
//! VISCA-over-IP transport so the supported path runs without camera hardware.
//!
//! The transport returns an ACK for a continuous zoom command, then a
//! socket-specific cancellation terminal for the owner's cancel frame. The
//! example observes `CancellationOutcome::Cancelled` with a bounded deadline
//! and still sends an explicit STOP: cancellation is protocol evidence, not a
//! physical-motion guarantee.

use std::{collections::VecDeque, time::Duration};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    command::CommandKind,
    profile::ProfileSpec,
    profiles::SonyFR7,
    transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
    CancellationOutcome, Error,
};

#[derive(Debug)]
struct CancelCamera {
    config: TransportConfig,
    replies: VecDeque<Vec<u8>>,
}

impl CancelCamera {
    fn new() -> Self {
        Self {
            config: TransportConfig::default(),
            replies: VecDeque::new(),
        }
    }

    fn queue_reply(&mut self, request: &[u8], payload: &[u8]) -> Result<(), Error> {
        let sequence = request.get(4..8).ok_or_else(|| {
            Error::InvalidRequest("Sony request did not contain a sequence header".into())
        })?;
        let payload_len = u16::try_from(payload.len())
            .map_err(|_| Error::InvalidRequest("scripted reply is too large".into()))?;
        let mut framed = Vec::with_capacity(8 + payload.len());
        framed.extend_from_slice(&[0x01, 0x11]);
        framed.extend_from_slice(&payload_len.to_be_bytes());
        framed.extend_from_slice(sequence);
        framed.extend_from_slice(payload);
        self.replies.push_back(framed);
        Ok(())
    }
}

impl HasTransportConfig for CancelCamera {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for CancelCamera {
    fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
        match bytes.get(8..) {
            Some([0x81, 0x01, 0x04, 0x07, 0x02, 0xff]) => {
                self.queue_reply(bytes, &[0x90, 0x41, 0xff])
            }
            Some([0x81, 0x21, 0xff]) => self.queue_reply(bytes, &[0x90, 0x61, 0x04, 0xff]),
            Some([0x81, 0x01, 0x04, 0x07, 0x00, 0xff]) => {
                self.queue_reply(bytes, &[0x90, 0x41, 0xff])?;
                self.queue_reply(bytes, &[0x90, 0x51, 0xff])
            }
            _ => Err(Error::InvalidRequest(
                "scripted camera received an unexpected command".into(),
            )),
        }
    }

    fn recv_into(&mut self, destination: &mut [u8]) -> Result<usize, Error> {
        self.recv_into_with_timeout(destination, self.config.read_timeout)
    }

    fn recv_into_with_timeout(
        &mut self,
        destination: &mut [u8],
        _timeout: Duration,
    ) -> Result<usize, Error> {
        let reply = self.replies.pop_front().ok_or(Error::Timeout)?;
        destination[..reply.len()].copy_from_slice(&reply);
        Ok(reply.len())
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = SessionConfig::new(ProfileSpec::from_compile_time::<SonyFR7>()?);
    let session = Session::open(CancelCamera::new(), config)?;
    let camera = session.camera::<SonyFR7>()?;

    let drive = camera.zoom().tele()?;
    let cancellation = drive.cancel().map_err(|rejected| rejected.into_error())?;
    let outcome = cancellation.outcome(Duration::from_secs(1))?;
    if outcome != CancellationOutcome::Cancelled {
        return Err(format!("cancellation lost to unexpected outcome {outcome:?}").into());
    }
    println!("blocking cancellation confirmed on a cancel-capable profile");

    camera
        .zoom()
        .stop()?
        .applied_with_timeout(Duration::from_secs(1))?;
    session.close()?;
    Ok(())
}
