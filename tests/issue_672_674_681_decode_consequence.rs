//! Issues #672, #674, #681: the byte-stream decode consequence, end to end.
//!
//! A stream transport (TCP, and serial) must not treat a classifier verdict as a
//! session verdict. Three defects are pinned here through the real blocking
//! facade, against the 1.x tolerance they restore:
//!
//! * #672 — a delimited-but-unclassifiable frame (padded ACK, vendor socket
//!   nibble, RS-485 echo, stray `FF`, truncation, ...) is discarded as a
//!   malformed frame and the session stays Running, exactly as 1.x logged and
//!   continued and as a datagram already discards it. Only a genuine framing
//!   loss (buffer overflow / no boundary) still poisons.
//! * #674 — a stream read that decodes more than the per-receive frame limit
//!   stops at the limit and drains the remainder, instead of poisoning the whole
//!   session over a large-but-valid burst.
//! * #681 — a single-target IP session whose camera answers with a non-default
//!   chain address (e.g. `0xA0` for VISCA address 2) still attributes the reply
//!   to the sole outstanding command (1.x's camera-blind attribution).
//!
//! The async twin of the stream-consequence rules is pinned at owner level in
//! `runtime::owner::async_actor::tests`; this file exercises the shared decode
//! through a real caller-driven session.

#![cfg(feature = "blocking")]

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    command::CommandKind,
    completion::AppliedOnly,
    profile::ProfileSpec,
    profiles::GenericVisca,
    request::builtin::ZoomStop,
    transport::{
        AddressingMode, BlockingTransport, BufferConfig, HasTransportConfig, SendSemantics,
        TransportConfig,
    },
    Error,
};

/// A caller-driven stream camera. Each queued entry is delivered as one `recv`
/// read (truncated to the caller's receive buffer, which the tests size large
/// enough that nothing is dropped); an exhausted script reports an idle timeout
/// so a pump that is waiting on a reply that will never come does not spin.
#[derive(Debug)]
struct StreamCamera {
    config: TransportConfig,
    reads: VecDeque<Vec<u8>>,
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl StreamCamera {
    fn new(addressing: AddressingMode, reads: Vec<Vec<u8>>) -> Self {
        // A generous receive buffer so a single scripted read can carry a burst
        // larger than the per-receive frame limit, reproducing the "more than 64
        // frames in one read" condition of #674 (a real raw-IP/serial session
        // uses a 256-byte buffer).
        let config = TransportConfig {
            addressing,
            buffer_config: BufferConfig {
                recv_buffer_size: 1024,
                send_buffer_size: 128,
                max_buffer_size: 8192,
            },
            ..TransportConfig::default()
        };
        Self {
            config,
            reads: reads.into(),
            sent: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl HasTransportConfig for StreamCamera {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for StreamCamera {
    fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
        self.sent.lock().expect("sent lock").push(bytes.to_vec());
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
        let Some(chunk) = self.reads.pop_front() else {
            return Err(Error::Timeout);
        };
        let n = chunk.len().min(dst.len());
        dst[..n].copy_from_slice(&chunk[..n]);
        Ok(n)
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Stream
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        Some(self.config.addressing)
    }
}

fn generic_session_config() -> SessionConfig {
    SessionConfig::new(ProfileSpec::from_compile_time::<GenericVisca>().expect("generic profile"))
}

const ACK: &[u8] = &[0x90, 0x41, 0xff];
const COMPLETION: &[u8] = &[0x90, 0x51, 0xff];

/// Run one command against a stream camera scripted with `reads`, asserting the
/// command is applied and the session stayed usable.
fn command_settles_over_stream(addressing: AddressingMode, reads: Vec<Vec<u8>>) {
    let transport = StreamCamera::new(addressing, reads);
    let session = Session::open(transport, generic_session_config()).expect("owner session");
    let camera = session.camera::<GenericVisca>().expect("camera view");

    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("submission")
        .applied()
        .expect("the command settles and the stream session stays usable");

    session.shutdown().expect("owner shutdown");
}

/// Issue #672: every quirky-but-delimited frame shape the review demonstrated
/// fatal is discarded on a stream, and the command is still settled by the
/// following well-formed ACK and completion. The session never poisons over a
/// classifier verdict. Uses the `GenericVisca` raw profile so no Sony envelope
/// is involved.
#[test]
fn stream_quirky_frames_are_discarded_and_keep_the_session() {
    // This replay is pinned to the raw `GenericVisca` profile: the malformed
    // frames below are raw VISCA, not an encapsulated envelope. Constructing the
    // profile here also records that provenance for the 1.x behavioral oracle.
    ProfileSpec::from_compile_time::<GenericVisca>().expect("raw GenericVisca profile");
    // The sixteen-plus shapes the protocol reviewer probed, each a frame the
    // framer delimits (or absorbs) but the strict classifier will not accept.
    let quirks: &[&[u8]] = &[
        &[0x90, 0x41, 0x00, 0xff],             // padded ACK
        &[0x90, 0x43, 0xff],                   // vendor ACK socket nibble
        &[0x90, 0x51, 0x00, 0xff],             // padded completion
        &[0x90, 0x53, 0xff],                   // vendor completion socket nibble
        &[0x90, 0x61, 0xff],                   // error frame missing its code
        &[0x90, 0x63, 0x02, 0xff],             // vendor error socket nibble
        &[0x90, 0x38, 0x00, 0xff],             // padded network-change notice
        &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff], // RS-485 echo of a controller frame
        &[0x88, 0x30, 0x02, 0xff],             // late address-set reply
        &[0x90, 0xff],                         // truncated frame
        &[0x00, 0x90, 0x41, 0xff],             // noise-prefixed (whole frame is malformed)
        &[0xff],                               // one stray line-noise terminator byte
        &[0x80, 0x41, 0xff],                   // controller address, never a reply source
        &[0x8f, 0x41, 0xff],                   // broadcast-range address
        &[0x90, 0x00, 0xff],                   // reply with an impossible type byte-pair
        &[0x90, 0x4f, 0xff],                   // ACK naming an out-of-range socket nibble
    ];

    for quirk in quirks {
        // A fresh single-target IP stream session per shape: the quirky frame
        // first, then a clean ACK and completion that must still settle the
        // command because the quirk did not tear the session down.
        command_settles_over_stream(
            AddressingMode::Ip,
            vec![quirk.to_vec(), ACK.to_vec(), COMPLETION.to_vec()],
        );
    }
}

/// Issue #672 on serial: the RS-485 echo and other delimited quirks on a serial
/// stream are also discarded rather than poisoning the daisy chain. Every shape
/// carries its own `FF`, so discarding it does not disturb the frames around it.
#[test]
fn serial_stream_echo_and_quirks_are_discarded_and_keep_the_session() {
    let quirks: &[&[u8]] = &[
        &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff], // the controller's own command, echoed
        &[0x90, 0x41, 0x00, 0xff],             // padded ACK
        &[0x90, 0xff],                         // truncated frame (with terminator)
    ];
    for quirk in quirks {
        command_settles_over_stream(
            AddressingMode::Serial,
            vec![quirk.to_vec(), ACK.to_vec(), COMPLETION.to_vec()],
        );
    }
}

/// Issue #672 observability: a discarded malformed stream frame is counted in
/// the published `MetricsSnapshot::ignored_malformed_frames`, so the tolerance
/// is durable beyond the lossy diagnostics stream.
#[test]
fn a_discarded_malformed_stream_frame_increments_the_published_metric() {
    let transport = StreamCamera::new(
        AddressingMode::Ip,
        vec![
            vec![0x90, 0x41, 0x00, 0xff], // padded ACK: delimited but unclassifiable
            ACK.to_vec(),
            COMPLETION.to_vec(),
        ],
    );
    let session = Session::open(transport, generic_session_config()).expect("owner session");
    let camera = session.camera::<GenericVisca>().expect("camera view");

    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("submission")
        .applied()
        .expect("the command settles despite the malformed frame");

    let metrics = session.metrics().expect("metrics snapshot");
    assert!(
        metrics.ignored_malformed_frames >= 1,
        "a discarded malformed frame must be counted, got {}",
        metrics.ignored_malformed_frames
    );

    session.shutdown().expect("owner shutdown");
}

/// Builds one read carrying `count` frames: `count - 2` harmless network-change
/// notices, then the ACK and completion that settle the command.
fn burst_read(count: usize) -> Vec<u8> {
    let mut read = Vec::new();
    for _ in 0..count.saturating_sub(2) {
        read.extend_from_slice(&[0x90, 0x38, 0xff]); // network-change notice
    }
    read.extend_from_slice(ACK);
    read.extend_from_slice(COMPLETION);
    read
}

/// Issue #674: a single stream read that decodes more than the per-receive frame
/// limit (default 64) must not poison the session. Sixty-five frames in one read
/// are all decoded across the pump and the command settles.
#[test]
fn stream_over_limit_burst_keeps_the_session_and_settles() {
    command_settles_over_stream(AddressingMode::Ip, vec![burst_read(65)]);
}

/// Issue #674 boundary: a stream read of exactly the per-receive frame limit is
/// decoded fully and is not itself an error.
#[test]
fn stream_exactly_frame_limit_burst_keeps_the_session_and_settles() {
    command_settles_over_stream(AddressingMode::Ip, vec![burst_read(64)]);
}

/// Issue #674, large burst: well past the limit (a 256-byte buffer holds ~85
/// three-byte frames) is still only a large-but-valid burst, not a framing loss.
#[test]
fn stream_large_burst_keeps_the_session_and_settles() {
    command_settles_over_stream(AddressingMode::Ip, vec![burst_read(85)]);
}

/// Issue #681: a single-target IP session whose camera is configured with VISCA
/// address 2 answers `A0 ...`. That reply must attribute to the sole outstanding
/// command (1.x's camera-blind attribution) rather than poisoning the session.
#[test]
fn single_target_ip_chain_address_reply_settles_the_command() {
    // The camera answers with its chain address 2 for both the ACK and the
    // completion; a mixed `0x90`/`0xA0` session must also settle.
    command_settles_over_stream(
        AddressingMode::Ip,
        vec![
            vec![0xa0, 0x41, 0xff], // chain-address ACK
            vec![0x90, 0x51, 0xff], // default-address completion
        ],
    );
    command_settles_over_stream(
        AddressingMode::Ip,
        vec![
            vec![0xa0, 0x41, 0xff], // chain-address ACK
            vec![0xa0, 0x51, 0xff], // chain-address completion
        ],
    );
}

/// The genuine framing loss #672 is careful to preserve: a stream buffer that
/// grows past its maximum without ever finding a frame boundary is a real loss
/// of the stream position and still poisons. This keeps the decouple from
/// weakening the one case that must remain terminal.
#[test]
fn stream_unbounded_garbage_without_a_boundary_still_poisons() {
    // Enough boundary-free bytes to grow the framer past its 8192-byte maximum
    // across reads: the framer cannot frame them and cannot recover its
    // position, which is a real loss the session must not survive silently.
    let garbage: Vec<Vec<u8>> = (0..10).map(|_| vec![0x00_u8; 1024]).collect();
    let transport = StreamCamera::new(AddressingMode::Ip, garbage);
    let session = Session::open(transport, generic_session_config()).expect("owner session");
    let camera = session.camera::<GenericVisca>().expect("camera view");

    let error = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .expect("submission")
        .applied()
        .expect_err("a stream that lost its framing position must poison");
    assert!(
        error.requires_new_session(),
        "a genuine framing loss still demands a replacement session, got {error:?}"
    );
}
