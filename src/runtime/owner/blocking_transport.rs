//! Blocking production transport driver for the native owner worker.
//!
//! The worker thread owns exactly one adapter, so its transport, envelope and
//! protocol framer need no lock: every write, read and decode runs on that
//! one thread, outside any engine borrow (D24). Only the native send and read
//! calls live here; everything else is [`TransportAdapter`]'s.

use std::time::Duration;

use crate::{
    runtime::engine::TransmissionMeta,
    transport::{BlockingTransport, HasTransportConfig},
    Error,
};

use super::{
    adapter::{BlockingIo, TransportAdapter},
    BlockingOwnerDriver, OwnerBuffers, OwnerReceive, WireWrite,
};

impl<T> TransportAdapter<T, BlockingIo>
where
    T: BlockingTransport + HasTransportConfig,
{
    /// Send Sony's sequence-number RESET before the owner worker starts.
    pub(crate) fn send_sony_sequence_reset(&mut self) -> Result<(), Error> {
        let mut frame = bytes::BytesMut::new();
        self.framing.frame_sony_sequence_reset(&mut frame)?;
        let write_timeout = self.transport.transport_config().write_timeout;
        let sent = self.transport.send_with_timeout(
            frame.as_ref(),
            crate::command::CommandKind::Command,
            write_timeout,
        );
        sent.map_err(|error| self.framing.send_error(error))
    }
}

impl<T> BlockingOwnerDriver for TransportAdapter<T, BlockingIo>
where
    T: BlockingTransport + HasTransportConfig,
{
    fn write(&mut self, mut write: WireWrite<'_>) -> Result<TransmissionMeta, Error> {
        let meta = self.framing.frame_write(&mut write)?;
        let write_timeout = self.transport.transport_config().write_timeout;
        let sent = self.transport.send_with_timeout(
            write.frame_buffer.as_ref(),
            write.command_kind(),
            write_timeout,
        );
        sent.map_err(|error| self.framing.send_error(error))?;
        Ok(meta)
    }

    fn receive(
        &mut self,
        buffers: &mut OwnerBuffers,
        frame_limit: usize,
        timeout: Duration,
    ) -> Result<OwnerReceive, Error> {
        if let Some(buffered) = self.buffered(buffers, frame_limit)? {
            return Ok(buffered);
        }
        let read = self
            .transport
            .recv_into_with_timeout(buffers.receive_mut(), timeout);
        self.classify_read(buffers, frame_limit, read)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };

    use super::*;
    use crate::{
        command::CommandKind,
        prepared::{prepare_command, ClassSelection},
        profiles::{GenericVisca, SonyFR7},
        protocol::framer::RawIncompletePrefix,
        runtime::{
            engine::{
                DecodedResponse, EnvelopeSequence, RawPrefixEvidence, SequenceWidth, TransportKind,
            },
            owner::{BlockingOwnerHandle, BlockingTransportAdapter, Clock, RetainedStreamInput},
        },
        transport::{
            builder::{AddressingMode, TransportConfig},
            ReceiveOutcome, SendSemantics,
        },
        CameraId, OperationalTuning, ProfileSpec,
    };

    /// Reads replay a script; an exhausted script times out rather than
    /// reporting end of stream.
    #[derive(Debug)]
    struct ScriptedTransport {
        config: TransportConfig,
        sent: Arc<Mutex<Vec<Vec<u8>>>>,
        write_timeouts: Arc<Mutex<Vec<Duration>>>,
        receives: VecDeque<Result<Vec<u8>, Error>>,
        semantics: SendSemantics,
    }

    impl ScriptedTransport {
        fn new(receives: impl IntoIterator<Item = Result<Vec<u8>, Error>>) -> Self {
            Self::with_config(config(), receives)
        }

        fn with_config(
            config: TransportConfig,
            receives: impl IntoIterator<Item = Result<Vec<u8>, Error>>,
        ) -> Self {
            Self {
                config,
                sent: Arc::new(Mutex::new(Vec::new())),
                write_timeouts: Arc::new(Mutex::new(Vec::new())),
                receives: receives.into_iter().collect(),
                semantics: SendSemantics::Datagram,
            }
        }

        fn stream(mut self) -> Self {
            self.semantics = SendSemantics::Stream;
            self
        }
    }

    impl HasTransportConfig for ScriptedTransport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl BlockingTransport for ScriptedTransport {
        fn send_with_timeout(
            &mut self,
            bytes: &[u8],
            _kind: CommandKind,
            timeout: Duration,
        ) -> Result<(), Error> {
            self.sent.lock().unwrap().push(bytes.to_vec());
            self.write_timeouts.lock().unwrap().push(timeout);
            Ok(())
        }

        fn recv_into_with_timeout(
            &mut self,
            dst: &mut [u8],
            timeout: Duration,
        ) -> Result<ReceiveOutcome, Error> {
            let Some(next) = self.receives.pop_front() else {
                std::thread::sleep(timeout);
                return Err(Error::io_timeout());
            };
            Ok(ReceiveOutcome::copy_message(&next?, dst))
        }

        fn send_semantics(&self) -> SendSemantics {
            self.semantics
        }
    }

    fn config() -> TransportConfig {
        TransportConfig {
            addressing: AddressingMode::Ip,
            ..TransportConfig::default()
        }
    }

    fn adapter(transport: ScriptedTransport) -> BlockingTransportAdapter<ScriptedTransport> {
        let profile = ProfileSpec::from_compile_time::<GenericVisca>().unwrap();
        BlockingTransportAdapter::new(transport, &profile, CameraId::CAMERA_1).unwrap()
    }

    fn sony_adapter(transport: ScriptedTransport) -> BlockingTransportAdapter<ScriptedTransport> {
        let profile = ProfileSpec::from_compile_time::<SonyFR7>().unwrap();
        BlockingTransportAdapter::new(transport, &profile, CameraId::CAMERA_1).unwrap()
    }

    fn buffers(adapter: &BlockingTransportAdapter<ScriptedTransport>) -> OwnerBuffers {
        OwnerBuffers::new(adapter.policy().limits).unwrap()
    }

    /// One receive with a short timeout and the default frame limit.
    fn receive(
        adapter: &mut BlockingTransportAdapter<ScriptedTransport>,
        buffers: &mut OwnerBuffers,
    ) -> Result<OwnerReceive, Error> {
        adapter.receive(buffers, 4, Duration::from_millis(1))
    }

    fn frames(result: Result<OwnerReceive, Error>) -> Vec<crate::runtime::engine::DecodedFrame> {
        match result {
            Ok(OwnerReceive::Frames(frames)) => frames,
            other => panic!("expected decoded frames, got {other:?}"),
        }
    }

    fn sony_reply(sequence: u32) -> Vec<u8> {
        let payload = [0x90, 0x41, 0xff];
        let mut framed = crate::protocol::sony::SonyHeader::new_reply(payload.len(), sequence)
            .encode()
            .to_vec();
        framed.extend_from_slice(&payload);
        framed
    }

    #[test]
    fn policy_and_raw_write_use_profile_and_transport_facts() {
        let mut transport_config = config();
        transport_config.write_timeout = Duration::from_millis(37);
        let transport = ScriptedTransport::with_config(transport_config, []);
        let write_timeouts = Arc::clone(&transport.write_timeouts);
        let adapter = adapter(transport);
        assert_eq!(
            adapter.policy().protocol.envelope,
            crate::runtime::engine::EnvelopeKind::Raw
        );
        assert_eq!(adapter.policy().protocol.transport, TransportKind::Datagram);
        assert_eq!(adapter.policy().protocol.inquiry_capacity, 1);

        let owner =
            BlockingOwnerHandle::spawn(adapter.policy().clone(), adapter, Clock::System).unwrap();
        let profile = ProfileSpec::from_compile_time::<GenericVisca>().unwrap();
        let prepared = prepare_command(
            &crate::request::builtin::FocusModeCommand::Manual,
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .unwrap();
        let receipt = owner.submit_command(prepared).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while write_timeouts.lock().unwrap().is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            *write_timeouts.lock().unwrap(),
            [Duration::from_millis(37)],
            "the owner forwards the configured bound to every blocking write"
        );
        drop(receipt);
        owner.close().unwrap();
    }

    #[test]
    fn startup_sony_sequence_reset_writes_the_control_frame() {
        let transport = ScriptedTransport::new([]);
        let sent = Arc::clone(&transport.sent);
        let mut adapter = sony_adapter(transport);

        adapter.send_sony_sequence_reset().unwrap();

        assert_eq!(
            *sent.lock().unwrap(),
            [vec![0x02, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0x01]]
        );
    }

    #[test]
    fn a_read_timeout_is_no_data_and_a_failed_read_is_a_fault() {
        let mut adapter = adapter(ScriptedTransport::new([
            Err(Error::io_timeout()),
            Err(Error::TransportError("boom".into())),
            Ok(Vec::new()),
        ]));
        let mut buffers = buffers(&adapter);
        assert!(matches!(
            receive(&mut adapter, &mut buffers),
            Ok(OwnerReceive::NoData)
        ));
        assert!(matches!(
            receive(&mut adapter, &mut buffers),
            Ok(OwnerReceive::Fault(Error::TransportError(_)))
        ));
        assert!(
            matches!(
                receive(&mut adapter, &mut buffers),
                Ok(OwnerReceive::Closed)
            ),
            "only a zero-length read reports end of stream"
        );
    }

    /// Issue #637/#719: raw `WouldBlock` and `Interrupted` mean an idle read;
    /// raw `TimedOut` remains a fault because TCP keepalive exhaustion can use
    /// that spelling. Custom idle timers use `Error::io_timeout()`.
    #[test]
    fn raw_idle_io_kinds_map_to_the_same_no_data_answer() {
        let io = |kind| Err(Error::Io(Arc::new(std::io::Error::from(kind))));
        let mut adapter = adapter(ScriptedTransport::new([
            io(std::io::ErrorKind::WouldBlock),
            io(std::io::ErrorKind::Interrupted),
            io(std::io::ErrorKind::TimedOut),
            io(std::io::ErrorKind::ConnectionRefused),
        ]));
        let mut buffers = buffers(&adapter);
        for _ in 0..2 {
            assert!(matches!(
                receive(&mut adapter, &mut buffers),
                Ok(OwnerReceive::NoData)
            ));
        }
        assert!(
            matches!(
                receive(&mut adapter, &mut buffers),
                Ok(OwnerReceive::Fault(Error::Io(_)))
            ),
            "an OS timeout remains a fault the owner classifies as terminal"
        );
        assert!(
            matches!(
                receive(&mut adapter, &mut buffers),
                Ok(OwnerReceive::Fault(Error::Io(_)))
            ),
            "another real read failure still reaches the owner"
        );
    }

    /// A consumed over-size datagram reports a truncated outcome. Its copied
    /// prefix must not enter the raw decoder; it takes the decode-error path
    /// the owner discards, and later datagrams still decode.
    #[test]
    fn an_oversized_datagram_is_a_decode_error_and_later_frames_still_decode() {
        let mut oversized = vec![0x90, 0x41, 0xff];
        oversized.resize(config().buffer_config.recv_buffer_size + 1, 0);
        let mut adapter = adapter(ScriptedTransport::new([
            Ok(oversized),
            Ok(vec![0x90, 0x41, 0xff]),
        ]));
        let mut buffers = buffers(&adapter);
        assert!(matches!(
            receive(&mut adapter, &mut buffers),
            Err(Error::ResponseTooLarge { .. })
        ));
        let frames = frames(receive(&mut adapter, &mut buffers));
        assert!(
            matches!(frames[..], [ref ack] if matches!(ack.response, DecodedResponse::Ack { .. }))
        );
    }

    /// A custom datagram transport that cannot observe truncation: it copies
    /// what fits of each datagram and reports a buffer-filling read as
    /// possibly truncated, as the receive contract requires.
    #[derive(Debug)]
    struct UnobservedTruncation {
        config: TransportConfig,
        datagrams: VecDeque<Vec<u8>>,
    }

    impl HasTransportConfig for UnobservedTruncation {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl BlockingTransport for UnobservedTruncation {
        fn send_with_timeout(
            &mut self,
            _bytes: &[u8],
            _kind: CommandKind,
            _timeout: Duration,
        ) -> Result<(), Error> {
            Ok(())
        }

        fn recv_into_with_timeout(
            &mut self,
            dst: &mut [u8],
            _timeout: Duration,
        ) -> Result<ReceiveOutcome, Error> {
            let datagram = self.datagrams.pop_front().ok_or_else(Error::io_timeout)?;
            let copied = datagram.len().min(dst.len());
            dst[..copied].copy_from_slice(&datagram[..copied]);
            Ok(if copied == dst.len() {
                ReceiveOutcome::possibly_truncated(copied)
            } else {
                ReceiveOutcome::complete(copied)
            })
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    /// Issue #804: a custom datagram transport whose read exactly fills the
    /// owner buffer cannot prove the datagram fitted. The blocking owner used
    /// to decode that prefix while the async owner discarded it; both now
    /// discard it through one classification, and the next datagram decodes.
    #[test]
    fn an_unobservable_exact_fill_is_discarded_like_the_async_owner() {
        let capacity = config().buffer_config.recv_buffer_size;
        // A valid ACK prefix of a datagram larger than the buffer.
        let mut oversized = vec![0x90, 0x41, 0xff];
        oversized.resize(capacity + 1, 0);
        let profile = ProfileSpec::from_compile_time::<GenericVisca>().unwrap();
        let mut adapter = BlockingTransportAdapter::new(
            UnobservedTruncation {
                config: config(),
                datagrams: [oversized, vec![0x90, 0x41, 0xff]].into_iter().collect(),
            },
            &profile,
            CameraId::CAMERA_1,
        )
        .unwrap();
        let mut buffers = OwnerBuffers::new(adapter.policy().limits).unwrap();
        let receive = |adapter: &mut BlockingTransportAdapter<UnobservedTruncation>,
                       buffers: &mut OwnerBuffers| {
            adapter.receive(buffers, 4, Duration::from_millis(1))
        };
        assert!(matches!(
            receive(&mut adapter, &mut buffers),
            Err(Error::ResponseTooLarge { max_size }) if max_size == capacity
        ));
        let frames = frames(receive(&mut adapter, &mut buffers));
        assert!(
            matches!(frames[..], [ref ack] if matches!(ack.response, DecodedResponse::Ack { .. }))
        );
    }

    /// The smallest receive buffer a session accepts holds the largest valid
    /// reply: a 16-byte VISCA message behind Sony's 8-byte header.
    #[test]
    fn the_minimum_receive_buffer_decodes_the_largest_sony_reply() {
        let payload = [
            0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c,
            0x0d, 0xff,
        ];
        let mut framed = crate::protocol::sony::SonyHeader::new_reply(payload.len(), 7)
            .encode()
            .to_vec();
        framed.extend_from_slice(&payload);
        assert_eq!(
            framed.len(),
            crate::transport::BufferConfig::MIN_RECV_BUFFER_SIZE
        );
        let mut config = config();
        config.buffer_config.recv_buffer_size =
            crate::transport::BufferConfig::MIN_RECV_BUFFER_SIZE;
        let mut adapter = sony_adapter(ScriptedTransport::with_config(config, [Ok(framed)]));
        let mut buffers = buffers(&adapter);
        let frames = frames(receive(&mut adapter, &mut buffers));
        assert!(matches!(
            frames[..],
            [ref reply] if matches!(
                reply.response,
                DecodedResponse::InquiryReply { ref payload, .. } if payload.len() == 13
            )
        ));
    }

    #[test]
    fn one_read_decodes_every_basic_frame_it_carries() {
        let mut adapter = adapter(ScriptedTransport::new([Ok(vec![
            0x90, 0x41, 0xff, 0x90, 0x51, 0xff,
        ])]));
        let mut buffers = buffers(&adapter);
        let frames = frames(receive(&mut adapter, &mut buffers));
        assert_eq!(frames.len(), 2);
        assert!(matches!(frames[0].response, DecodedResponse::Ack { .. }));
        assert!(matches!(
            frames[1].response,
            DecodedResponse::Completion { .. }
        ));
    }

    /// Frames beyond the per-receive limit stay in the stream framer and are
    /// returned by the next receive without another read (#674).
    #[test]
    fn frames_beyond_the_receive_limit_are_drained_before_the_next_read() {
        let mut adapter = adapter(
            ScriptedTransport::new([Ok(vec![0x90, 0x41, 0xff, 0x90, 0x51, 0xff])]).stream(),
        );
        let mut buffers = buffers(&adapter);
        let first = match adapter.receive(&mut buffers, 1, Duration::from_millis(1)) {
            Ok(OwnerReceive::Frames(frames)) => frames,
            other => panic!("expected one frame, got {other:?}"),
        };
        assert!(
            matches!(first[..], [ref ack] if matches!(ack.response, DecodedResponse::Ack { .. }))
        );
        assert!(adapter.has_buffered_stream_input().unwrap());
        let second = match adapter.receive(&mut buffers, 1, Duration::from_millis(1)) {
            Ok(OwnerReceive::Frames(frames)) => frames,
            other => panic!("expected the buffered frame, got {other:?}"),
        };
        assert!(matches!(
            second[..],
            [ref completion] if matches!(completion.response, DecodedResponse::Completion { .. })
        ));
    }

    /// A raw owner already knows its envelope from the selected profile. A
    /// malformed/noise prefix can resemble a Sony header, but its `FF` still
    /// delimits one discarded raw frame before the replies that follow it.
    #[test]
    fn raw_owner_recovers_sony_looking_noise_before_following_replies() {
        for payload_type in [[0x01, 0x11], [0x02, 0x00]] {
            let mut bytes = vec![
                payload_type[0],
                payload_type[1],
                0x00,
                0x05,
                0x12,
                0x34,
                0x56,
                0x78,
                0x55,
                0xff,
            ];
            bytes.extend_from_slice(&[0x90, 0x41, 0xff]);
            bytes.extend_from_slice(&[0x90, 0x51, 0xff]);

            let mut adapter = adapter(ScriptedTransport::new([Ok(bytes)]).stream());
            let mut buffers = buffers(&adapter);
            let frames = frames(receive(&mut adapter, &mut buffers));
            assert_eq!(frames.len(), 2, "{payload_type:02x?}");
            assert!(matches!(frames[0].response, DecodedResponse::Ack { .. }));
            assert!(matches!(
                frames[1].response,
                DecodedResponse::Completion { .. }
            ));
            assert_eq!(buffers.take_discarded_malformed(), 1, "{payload_type:02x?}");
        }
    }

    #[test]
    fn stream_framer_discards_an_orphaned_prefix_before_a_later_tail() {
        let mut adapter = adapter(
            ScriptedTransport::new([
                Ok(vec![0x90, 0x50]),
                Ok(vec![0x02, 0xff]),
                Ok(vec![0x90, 0x50, 0x03, 0xff]),
            ])
            .stream(),
        );
        let mut buffers = buffers(&adapter);

        assert!(frames(receive(&mut adapter, &mut buffers)).is_empty());
        assert!(adapter.has_buffered_stream_input().unwrap());
        assert_eq!(
            adapter.buffered_raw_prefix_evidence().unwrap(),
            Some(RawPrefixEvidence::Incomplete {
                target: CameraId::CAMERA_1,
                kind: RawIncompletePrefix::SocketlessCompletion,
            })
        );

        adapter.discard_buffered_stream_input().unwrap();
        assert!(!adapter.has_buffered_stream_input().unwrap());

        // This would have completed `[90 50 02 FF]` if the stale prefix had
        // survived. Alone it is one delimited malformed frame and is ignored.
        assert!(frames(receive(&mut adapter, &mut buffers)).is_empty());
        assert_eq!(buffers.take_discarded_malformed(), 1);
        assert!(!adapter.has_buffered_stream_input().unwrap());

        let frames = frames(receive(&mut adapter, &mut buffers));
        assert_eq!(frames.len(), 1, "a later complete reply remains decodable");
        assert!(matches!(
            &frames[0].response,
            DecodedResponse::InquiryReply { payload, .. } if payload.as_slice() == [0x03]
        ));
    }

    /// Issue #745: invalid incomplete raw prefixes are explicit malformed
    /// evidence, never an error that can poison the owner before it decides
    /// to discard the input.
    #[test]
    fn invalid_incomplete_raw_prefixes_are_malformed_evidence() {
        for (prefix, addressing) in [
            (vec![0x80, 0x50, 0xdd], AddressingMode::Ip),
            // A passed-through address-set broadcast on a serial chain is
            // never a camera response source.
            (vec![0x88, 0x30, 0x02], AddressingMode::Serial),
            (vec![0x00], AddressingMode::Ip),
        ] {
            let mut stream_config = config();
            stream_config.addressing = addressing;
            let mut adapter = adapter(
                ScriptedTransport::with_config(stream_config, [Ok(prefix.clone())]).stream(),
            );
            let mut buffers = buffers(&adapter);
            assert!(frames(receive(&mut adapter, &mut buffers)).is_empty());
            assert_eq!(
                adapter.buffered_raw_prefix_evidence().unwrap(),
                Some(RawPrefixEvidence::Malformed),
                "prefix {prefix:02x?}",
            );
        }
    }

    #[test]
    fn sony_decoder_preserves_full_sequence_metadata() {
        let mut adapter = sony_adapter(ScriptedTransport::new([Ok(sony_reply(0x1234_5678))]));
        let mut buffers = buffers(&adapter);
        let frames = frames(receive(&mut adapter, &mut buffers));
        assert_eq!(
            frames[0].sequence,
            Some(EnvelopeSequence {
                value: 0x1234_5678,
                width: SequenceWidth::Full32,
            })
        );
        assert!(matches!(frames[0].response, DecodedResponse::Ack { .. }));
    }

    #[test]
    fn sony_framer_buffers_a_fragmented_header_and_payload_by_declared_length() {
        let framed = sony_reply(0xff00_ff00);
        let mut adapter = sony_adapter(
            ScriptedTransport::new([Ok(framed[..6].to_vec()), Ok(framed[6..].to_vec())]).stream(),
        );
        let mut buffers = buffers(&adapter);
        assert!(frames(receive(&mut adapter, &mut buffers)).is_empty());
        let frames = frames(receive(&mut adapter, &mut buffers));
        assert_eq!(frames.len(), 1);
        assert_eq!(
            frames[0].sequence,
            Some(EnvelopeSequence {
                value: 0xff00_ff00,
                width: SequenceWidth::Full32,
            })
        );
        assert!(matches!(frames[0].response, DecodedResponse::Ack { .. }));
    }

    #[test]
    fn sony_decoder_preserves_potentially_truncated_lower16_metadata() {
        // A zero upper half is deliberately classified as potentially
        // truncated by the production envelope parser, even though 42 also
        // fits in a genuine full-width Sony sequence.
        let mut adapter = sony_adapter(ScriptedTransport::new([Ok(sony_reply(42))]));
        let mut buffers = buffers(&adapter);
        let frames = frames(receive(&mut adapter, &mut buffers));
        assert_eq!(
            frames[0].sequence,
            Some(EnvelopeSequence {
                value: 42,
                width: SequenceWidth::Lower16,
            })
        );
    }

    #[test]
    fn malformed_visca_is_an_invalid_response() {
        let mut adapter = adapter(ScriptedTransport::new([Ok(vec![0x80, 0x41, 0xff])]));
        let mut buffers = buffers(&adapter);
        assert!(matches!(
            receive(&mut adapter, &mut buffers),
            Err(Error::InvalidResponse { .. })
        ));
    }
}
