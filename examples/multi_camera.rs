//! Multi-camera session: two targets on one serialized owner, selected by
//! `camera_for`.
//!
//! Multiple targets share one transport and one owner, so a multi-target session
//! needs a serial-addressed transport (multi-target registration is rejected on
//! IP transports). This example uses a tiny in-memory serial bus so it runs with
//! no hardware. Each registered target is reached with `camera_for::<P>(target)`;
//! the two views share the owner's pacing, correlation, and cancellation.
//!
//! Requires only the default `blocking` feature.

use std::{collections::VecDeque, time::Duration};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    command::CommandKind,
    profile::ProfileSpec,
    profiles::GenericVisca,
    transport::{AddressingMode, BlockingTransport, HasTransportConfig, TransportConfig},
    CameraId, Error,
};

/// A minimal in-memory transport that reports SERIAL addressing, so multi-target
/// registration is accepted without hardware. It answers every command with a
/// per-camera ACK and completion, sourced from the addressed camera.
#[derive(Debug)]
struct SerialBus {
    config: TransportConfig,
    replies: VecDeque<Vec<u8>>,
}

impl SerialBus {
    fn new() -> Self {
        Self {
            config: TransportConfig {
                addressing: AddressingMode::Serial,
                ..TransportConfig::default()
            },
            replies: VecDeque::new(),
        }
    }
}

impl HasTransportConfig for SerialBus {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for SerialBus {
    fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
        // A daisy-chained camera replies from source 0x80 | ((addr + 8) << 4):
        // camera 1 -> 0x90, camera 2 -> 0xA0, so the owner attributes each reply
        // to the right target.
        let addr = bytes.first().copied().unwrap_or(0x81) & 0x0f;
        let source = 0x80 | (addr.saturating_add(8) << 4);
        self.replies.push_back(vec![source, 0x41, 0xff]);
        self.replies.push_back(vec![source, 0x51, 0xff]);
        Ok(())
    }

    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
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

    // Required for multi-target: the topology check rejects a `None` hint.
    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        Some(self.config.addressing)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Two targets share one raw-VISCA serial envelope. Both profiles must be
    // RawVisca; GenericVisca (and PtzOpticsG2) qualify. Sony-encapsulated
    // profiles cannot be multi-targeted.
    let mut config = SessionConfig::new(ProfileSpec::from_compile_time::<GenericVisca>()?);
    config.register_target(
        CameraId::CAMERA_2,
        ProfileSpec::from_compile_time::<GenericVisca>()?,
    )?;

    let session = Session::open(SerialBus::new(), config)?;
    let result = drive_both(&session);
    // Close is the deterministic teardown barrier; report a control error too.
    match (result, session.close()) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), _) | (Ok(()), Err(error)) => Err(error.into()),
    }
}

fn drive_both(session: &Session) -> Result<(), Error> {
    // `camera()` is only for a sole target; a multi-target session selects each
    // view with `camera_for(target)`, which also checks the compile-time profile
    // matches the registered one.
    let cam1 = session.camera_for::<GenericVisca>(CameraId::CAMERA_1)?;
    let cam2 = session.camera_for::<GenericVisca>(CameraId::CAMERA_2)?;
    println!("camera 1 target = {:?}", cam1.target());
    println!("camera 2 target = {:?}", cam2.target());

    // Applied-only stops on both cameras, each bounded by an explicit observer
    // deadline. The two views drive the one shared owner.
    cam1.zoom()
        .stop()?
        .applied_with_timeout(Duration::from_secs(2))?;
    cam2.zoom()
        .stop()?
        .applied_with_timeout(Duration::from_secs(2))?;
    println!("stopped zoom on both cameras");
    Ok(())
}
