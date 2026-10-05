//! Async production transport driver for the owner actor. Only the native
//! send and read calls live here; everything else is [`TransportAdapter`]'s.

use std::future::Future;

use crate::{
    runtime::engine::TransmissionMeta,
    transport::{AsyncTransport, HasTransportConfig},
    Error,
};

use super::{
    adapter::{AsyncIo, TransportAdapter},
    AsyncOwnerDriver, OwnerBuffers, OwnerReceive, WireWrite,
};

impl<T> TransportAdapter<T, AsyncIo>
where
    T: AsyncTransport + HasTransportConfig,
{
    /// Send Sony's sequence-number RESET before the owner actor starts.
    pub(crate) async fn send_sony_sequence_reset(&mut self) -> Result<(), Error> {
        let mut frame = bytes::BytesMut::new();
        self.framing.frame_sony_sequence_reset(&mut frame)?;
        let sent = self.transport.send(frame.as_ref()).await;
        sent.map_err(|error| self.framing.send_error(error))
    }
}

impl<T> AsyncOwnerDriver for TransportAdapter<T, AsyncIo>
where
    T: AsyncTransport + HasTransportConfig,
{
    fn write(
        &mut self,
        mut write: WireWrite<'_>,
    ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
        let framed = self.framing.frame_write(&mut write);
        async move {
            let meta = framed?;
            let sent = self.transport.send(write.frame_buffer.as_ref()).await;
            sent.map_err(|error| self.framing.send_error(error))?;
            Ok(meta)
        }
    }

    // The private driver trait requires an explicitly `Send` future, so this
    // implementation keeps the `impl Future` form instead of `async fn`.
    #[allow(clippy::manual_async_fn)]
    fn receive(
        &mut self,
        buffers: &mut OwnerBuffers,
        frame_limit: usize,
    ) -> impl Future<Output = Result<OwnerReceive, Error>> + Send {
        async move {
            if let Some(buffered) = self.buffered(buffers, frame_limit)? {
                return Ok(buffered);
            }
            let read = self.transport.recv_into(buffers.receive_mut()).await;
            self.classify_read(buffers, frame_limit, read)
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{
        profile::{OperationalTuning, ProfileSpec},
        profiles::{GenericVisca, SonyFR7},
        protocol::framer::RawIncompletePrefix,
        runtime::{
            engine::RawPrefixEvidence,
            owner::{AsyncTransportAdapter, RetainedStreamInput},
        },
        transport::{builder::TransportConfig, ReceiveOutcome, SendSemantics},
        CameraId, ViscaSocket,
    };

    #[derive(Debug)]
    struct ScriptedTransport {
        config: TransportConfig,
        sent: Vec<Vec<u8>>,
        receives: std::collections::VecDeque<Result<Vec<u8>, Error>>,
        semantics: SendSemantics,
    }

    /// A custom datagram transport that cannot observe truncation: it copies
    /// what fits of each datagram and reports a buffer-filling read as
    /// possibly truncated, as the receive contract requires.
    #[derive(Debug)]
    struct UnobservedTruncation {
        config: TransportConfig,
        datagrams: std::collections::VecDeque<Vec<u8>>,
    }

    impl HasTransportConfig for UnobservedTruncation {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl AsyncTransport for UnobservedTruncation {
        async fn send(&mut self, _bytes: &[u8]) -> Result<(), Error> {
            Ok(())
        }

        async fn recv_into(&mut self, dst: &mut [u8]) -> Result<ReceiveOutcome, Error> {
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

    impl HasTransportConfig for ScriptedTransport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl AsyncTransport for ScriptedTransport {
        async fn send(&mut self, bytes: &[u8]) -> Result<(), Error> {
            self.sent.push(bytes.to_vec());
            Ok(())
        }

        async fn recv_into(&mut self, dst: &mut [u8]) -> Result<ReceiveOutcome, Error> {
            let next = self.receives.pop_front().unwrap_or(Ok(Vec::new()))?;
            Ok(ReceiveOutcome::copy_message(&next, dst))
        }

        fn send_semantics(&self) -> SendSemantics {
            self.semantics
        }
    }

    fn profile() -> ProfileSpec {
        ProfileSpec::from_compile_time::<GenericVisca>().unwrap()
    }

    #[test]
    fn policy_maps_async_transport_semantics_and_decodes_frames() {
        let transport = ScriptedTransport {
            config: TransportConfig::default(),
            sent: Vec::new(),
            receives: [Ok(vec![0x90, 0x41, 0xff])].into_iter().collect(),
            semantics: SendSemantics::Datagram,
        };
        let mut adapter =
            AsyncTransportAdapter::new(transport, &profile(), CameraId::CAMERA_1).unwrap();
        assert_eq!(
            adapter.policy().protocol.transport,
            crate::runtime::engine::TransportKind::Datagram
        );

        let mut buffers = OwnerBuffers::new(adapter.policy().limits).unwrap();
        let received = futures_lite::future::block_on(adapter.receive(&mut buffers, 4)).unwrap();
        let OwnerReceive::Frames(frames) = received else {
            panic!("a nonzero read must not report the transport as closed");
        };
        assert_eq!(frames.len(), 1);
        assert!(matches!(
            frames[0].response,
            crate::runtime::engine::DecodedResponse::Ack { .. }
        ));
    }

    #[test]
    fn startup_sony_sequence_reset_writes_the_control_frame() {
        let transport = ScriptedTransport {
            config: TransportConfig::default(),
            sent: Vec::new(),
            receives: std::collections::VecDeque::new(),
            semantics: SendSemantics::Datagram,
        };
        let profile = ProfileSpec::from_compile_time::<SonyFR7>().unwrap();
        let mut adapter =
            AsyncTransportAdapter::new(transport, &profile, CameraId::CAMERA_1).unwrap();

        futures_lite::future::block_on(adapter.send_sony_sequence_reset()).unwrap();

        assert_eq!(
            adapter.transport.sent,
            [vec![0x02, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0x01]]
        );
    }

    /// A consumed over-size datagram reports a truncated outcome; its valid
    /// ACK prefix must never be decoded.
    #[test]
    fn truncated_datagram_prefix_is_rejected_before_visca_decode() {
        let config = TransportConfig::default();
        let mut oversized = vec![0x90, 0x41, 0xff];
        oversized.resize(config.buffer_config.recv_buffer_size + 1, 0);
        let transport = ScriptedTransport {
            config,
            sent: Vec::new(),
            receives: [Ok(oversized)].into_iter().collect(),
            semantics: SendSemantics::Datagram,
        };
        let mut adapter =
            AsyncTransportAdapter::new(transport, &profile(), CameraId::CAMERA_1).unwrap();
        let mut buffers = OwnerBuffers::new(adapter.policy().limits).unwrap();

        let received = futures_lite::future::block_on(adapter.receive(&mut buffers, 4));

        assert!(matches!(
            received,
            Err(Error::ResponseTooLarge { max_size })
                if max_size == config.buffer_config.recv_buffer_size
        ));
    }

    /// Issue #804: a custom datagram transport whose read exactly fills the
    /// owner buffer cannot prove the datagram fitted. Both owners discard it
    /// through one classification (the blocking owner used to decode its
    /// prefix), and the next datagram decodes.
    #[test]
    fn an_unobservable_exact_fill_is_discarded_like_the_blocking_owner() {
        let config = TransportConfig::default();
        let capacity = config.buffer_config.recv_buffer_size;
        let mut oversized = vec![0x90, 0x41, 0xff];
        oversized.resize(capacity + 1, 0);
        let mut adapter = AsyncTransportAdapter::new(
            UnobservedTruncation {
                config,
                datagrams: [oversized, vec![0x90, 0x41, 0xff]].into_iter().collect(),
            },
            &profile(),
            CameraId::CAMERA_1,
        )
        .unwrap();
        let mut buffers = OwnerBuffers::new(adapter.policy().limits).unwrap();

        let discarded = futures_lite::future::block_on(adapter.receive(&mut buffers, 4));
        assert!(matches!(
            discarded,
            Err(Error::ResponseTooLarge { max_size }) if max_size == capacity
        ));
        let next = futures_lite::future::block_on(adapter.receive(&mut buffers, 4)).unwrap();
        let OwnerReceive::Frames(frames) = next else {
            panic!("the next datagram must decode, got {next:?}");
        };
        assert!(matches!(
            frames[..],
            [ref ack] if matches!(ack.response, crate::runtime::engine::DecodedResponse::Ack { .. })
        ));
    }

    /// #675: the transport's advertised read/write timeouts must reach the owner
    /// policy so the async owner can enforce them. They were previously dropped
    /// here, leaving both builder knobs inert on every async transport.
    #[test]
    fn read_and_write_timeouts_are_lowered_from_transport_config() {
        let transport = ScriptedTransport {
            config: TransportConfig {
                read_timeout: std::time::Duration::from_millis(111),
                write_timeout: std::time::Duration::from_millis(222),
                ..TransportConfig::default()
            },
            sent: Vec::new(),
            receives: std::collections::VecDeque::new(),
            semantics: SendSemantics::Datagram,
        };
        let adapter =
            AsyncTransportAdapter::new(transport, &profile(), CameraId::CAMERA_1).unwrap();
        assert_eq!(
            adapter.policy().read_timeout,
            std::time::Duration::from_millis(111)
        );
        assert_eq!(
            adapter.policy().write_timeout,
            std::time::Duration::from_millis(222)
        );
    }

    #[test]
    fn partial_stream_frame_is_reported_as_an_empty_batch_not_a_close() {
        let transport = ScriptedTransport {
            config: TransportConfig::default(),
            sent: Vec::new(),
            // One reply split across two reads, then a genuine end of stream.
            receives: [Ok(vec![0x90, 0x41]), Ok(vec![0xff]), Ok(Vec::new())]
                .into_iter()
                .collect(),
            semantics: SendSemantics::Stream,
        };
        let mut adapter =
            AsyncTransportAdapter::new(transport, &profile(), CameraId::CAMERA_1).unwrap();
        let mut buffers = OwnerBuffers::new(adapter.policy().limits).unwrap();

        let first = futures_lite::future::block_on(adapter.receive(&mut buffers, 4)).unwrap();
        let OwnerReceive::Frames(frames) = first else {
            panic!("a partial frame must not be reported as a transport close");
        };
        assert!(frames.is_empty());

        let second = futures_lite::future::block_on(adapter.receive(&mut buffers, 4)).unwrap();
        let OwnerReceive::Frames(frames) = second else {
            panic!("the completing read must decode the buffered frame");
        };
        assert_eq!(frames.len(), 1);
        assert!(matches!(
            frames[0].response,
            crate::runtime::engine::DecodedResponse::Ack { .. }
        ));

        let third = futures_lite::future::block_on(adapter.receive(&mut buffers, 4)).unwrap();
        assert!(
            matches!(third, OwnerReceive::Closed),
            "only a zero-length read closes the transport"
        );
    }

    /// The production adapter must expose only protocol facts from an
    /// incomplete raw frame.  In particular, the low nibble of an ACK is not
    /// socket ownership: the engine alone decides whether a due correlation
    /// release may progress.  These literal vectors exercise the real
    /// `ProtocolFramer` seam used by the async actor.
    #[test]
    fn buffered_raw_prefix_evidence_is_socket_aware_without_guessing_ack_ownership() {
        let cases = [
            (
                vec![0x90],
                RawIncompletePrefix::SourceOnly,
                "source-only input",
            ),
            (vec![0x90, 0x41], RawIncompletePrefix::Ack, "ACK"),
            (
                vec![0x90, 0x50],
                RawIncompletePrefix::SocketlessCompletion,
                "socketless completion",
            ),
            (
                vec![0x90, 0x60],
                RawIncompletePrefix::SocketlessError,
                "socketless error",
            ),
            (
                vec![0x90, 0x51],
                RawIncompletePrefix::NamedCompletionOrError(ViscaSocket::S1),
                "named S1 completion",
            ),
            (
                vec![0x90, 0x62],
                RawIncompletePrefix::NamedCompletionOrError(ViscaSocket::S2),
                "named S2 error",
            ),
            (
                vec![0x90, 0x53],
                RawIncompletePrefix::Noncorrelating,
                "invalid named socket",
            ),
        ];

        for (prefix, expected_kind, case) in cases {
            let transport = ScriptedTransport {
                config: TransportConfig::default(),
                sent: Vec::new(),
                receives: [Ok(prefix)].into_iter().collect(),
                semantics: SendSemantics::Stream,
            };
            let mut adapter =
                AsyncTransportAdapter::new(transport, &profile(), CameraId::CAMERA_1).unwrap();
            let mut buffers = OwnerBuffers::new(adapter.policy().limits).unwrap();
            let received = futures_lite::future::block_on(adapter.receive(&mut buffers, 4))
                .expect("a partial prefix is ordinary stream input");
            assert!(matches!(received, OwnerReceive::Frames(ref frames) if frames.is_empty()));
            assert_eq!(
                adapter.buffered_raw_prefix_evidence().unwrap(),
                Some(RawPrefixEvidence::Incomplete {
                    target: CameraId::CAMERA_1,
                    kind: expected_kind,
                }),
                "{case}",
            );
        }

        // #745: untrusted incomplete input must never escape as a fallible
        // source-routing operation. The async owner sees a discard verdict for
        // the malformed source, including a passed-through address-set and a
        // single noise byte.
        for prefix in [vec![0x80, 0x50, 0xdd], vec![0x88, 0x30, 0x02], vec![0x00]] {
            let transport = ScriptedTransport {
                config: TransportConfig::default(),
                sent: Vec::new(),
                receives: [Ok(prefix.clone())].into_iter().collect(),
                semantics: SendSemantics::Stream,
            };
            let mut adapter =
                AsyncTransportAdapter::new(transport, &profile(), CameraId::CAMERA_1).unwrap();
            let mut buffers = OwnerBuffers::new(adapter.policy().limits).unwrap();
            let _ = futures_lite::future::block_on(adapter.receive(&mut buffers, 4)).unwrap();
            assert_eq!(
                adapter.buffered_raw_prefix_evidence().unwrap(),
                Some(RawPrefixEvidence::Malformed),
                "prefix {prefix:02x?}",
            );
        }
    }

    /// Issue #625/#637: a transport with an internal read timeout is the shape
    /// the public trait documents. An expired idle timeout is no data, not a
    /// receive fault, so it must never provoke a retransmission.
    #[test]
    fn an_idle_read_timeout_decodes_as_no_data() {
        for idle in [
            Error::io_timeout(),
            Error::Io(std::sync::Arc::new(std::io::Error::from(
                std::io::ErrorKind::WouldBlock,
            ))),
            Error::Io(std::sync::Arc::new(std::io::Error::from(
                std::io::ErrorKind::Interrupted,
            ))),
        ] {
            let transport = ScriptedTransport {
                config: TransportConfig::default(),
                sent: Vec::new(),
                receives: [Err(idle)].into_iter().collect(),
                semantics: SendSemantics::Datagram,
            };
            let mut adapter =
                AsyncTransportAdapter::new(transport, &profile(), CameraId::CAMERA_1).unwrap();
            let mut buffers = OwnerBuffers::new(adapter.policy().limits).unwrap();
            let received =
                futures_lite::future::block_on(adapter.receive(&mut buffers, 4)).unwrap();
            assert!(
                matches!(received, OwnerReceive::NoData),
                "an idle read timeout is not a receive fault: {received:?}"
            );
        }
    }

    /// A read failure that is not an idle timeout still reaches the owner as a
    /// fault, which is what keeps #620's transient-retry semantics working.
    #[test]
    fn real_read_failures_still_reach_the_owner_as_faults() {
        for fault in [
            Error::TransportError("ICMP port unreachable".into()),
            Error::Io(std::sync::Arc::new(std::io::Error::from(
                std::io::ErrorKind::TimedOut,
            ))),
        ] {
            let transport = ScriptedTransport {
                config: TransportConfig::default(),
                sent: Vec::new(),
                receives: [Err(fault)].into_iter().collect(),
                semantics: SendSemantics::Datagram,
            };
            let mut adapter =
                AsyncTransportAdapter::new(transport, &profile(), CameraId::CAMERA_1).unwrap();
            let mut buffers = OwnerBuffers::new(adapter.policy().limits).unwrap();
            let received =
                futures_lite::future::block_on(adapter.receive(&mut buffers, 4)).unwrap();
            assert!(matches!(received, OwnerReceive::Fault(_)));
        }
    }

    #[test]
    fn immutable_tuning_is_lowered_into_owner_policy() {
        let profile = profile();
        let tuning = OperationalTuning::new()
            .command_spacing(
                profile.timing().minimum_command_spacing() + std::time::Duration::from_millis(1),
            )
            .inquiry_spacing(
                profile.timing().minimum_inquiry_spacing() + std::time::Duration::from_millis(1),
            )
            .maximum_command_sockets(1);
        let transport = ScriptedTransport {
            config: TransportConfig::default(),
            sent: Vec::new(),
            receives: std::collections::VecDeque::new(),
            semantics: SendSemantics::Datagram,
        };
        let adapter =
            AsyncTransportAdapter::new_with_tuning(transport, &profile, CameraId::CAMERA_1, tuning)
                .unwrap();
        assert_eq!(
            adapter.policy().protocol.command_spacing,
            tuning.command_spacing_override().unwrap()
        );
        assert_eq!(
            adapter.policy().protocol.inquiry_spacing,
            tuning.inquiry_spacing_override().unwrap()
        );
        assert_eq!(adapter.policy().targets[1].unwrap().command_sockets, 1);
    }
}
