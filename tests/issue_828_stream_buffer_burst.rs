//! #828: a valid stream burst must never overflow the receive limits.
//!
//! `BufferConfig { recv_buffer_size: 64, max_buffer_size: 64 }` passes
//! validation. The owner reads up to `recv_buffer_size` bytes at a time, so a
//! reply split across reads followed by one full read of valid replies used to
//! exceed the framer's 64-byte bound and poison a perfectly healthy raw stream.
//! The framer now always has room for one full read on top of the retention
//! bound.

#![cfg(feature = "blocking")]

use std::{collections::VecDeque, time::Duration};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    command::CommandKind,
    profile::ProfileSpec,
    profiles::PtzOpticsG2,
    transport::{
        AddressingMode, BlockingTransport, HasTransportConfig, ReceiveOutcome, SendSemantics,
        TransportConfig,
    },
    Error,
};

/// A raw TCP camera that answers the first write with a reply split across a
/// short read and one full `recv_buffer_size` read.
#[derive(Debug)]
struct BurstTransport {
    config: TransportConfig,
    pending: VecDeque<Vec<u8>>,
    answered: bool,
}

impl HasTransportConfig for BurstTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for BurstTransport {
    fn send_with_timeout(
        &mut self,
        _bytes: &[u8],
        _kind: CommandKind,
        _timeout: Duration,
    ) -> Result<(), Error> {
        if !self.answered {
            self.answered = true;
            // The ACK arrives split: two bytes now ...
            self.pending.push_back(vec![0x90, 0x41]);
            // ... and its terminator leads one full 64-byte read that also
            // carries the completion and twenty Network Change notices.
            let mut burst = vec![0xff, 0x90, 0x51, 0xff];
            for _ in 0..20 {
                burst.extend_from_slice(&[0x90, 0x38, 0xff]);
            }
            assert_eq!(burst.len(), self.config.buffer_config.recv_buffer_size);
            self.pending.push_back(burst);
        }
        Ok(())
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        timeout: Duration,
    ) -> Result<ReceiveOutcome, Error> {
        let Some(chunk) = self.pending.pop_front() else {
            std::thread::sleep(timeout.min(Duration::from_millis(5)));
            return Err(Error::io_timeout());
        };
        Ok(ReceiveOutcome::copy_message(&chunk, dst))
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Stream
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        Some(AddressingMode::Ip)
    }
}

#[test]
fn a_split_reply_followed_by_a_full_read_keeps_the_stream_running() {
    let mut config = TransportConfig::for_tcp();
    config.buffer_config.recv_buffer_size = 64;
    config.buffer_config.max_buffer_size = 64;
    let transport = BurstTransport {
        config,
        pending: VecDeque::new(),
        answered: false,
    };

    let session = Session::open(
        transport,
        SessionConfig::new(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("profile")),
    )
    .expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("camera view");

    camera
        .power()
        .on()
        .expect("the burst completes the command instead of poisoning the stream");
    session.close().expect("clean close");
}
