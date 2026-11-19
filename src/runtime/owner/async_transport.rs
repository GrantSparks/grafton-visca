//! Async production transport driver for the owner actor. Only the native
//! send and read calls live here; everything else is [`TransportAdapter`]'s.

use std::future::Future;

use crate::{
    executor::Executor,
    runtime::engine::WriteOutcome,
    timeout::{within, Deadline},
    transport::{AsyncTransport, HasTransportConfig},
    Error,
};

use super::{
    adapter::{AsyncIo, SonyResetProgress, TransportAdapter},
    turn::TRANSIENT_RECEIVE_PAUSE,
    AsyncOwnerDriver, OwnerBuffers, OwnerPolicy, OwnerReceive, WireWrite,
};

impl<T> TransportAdapter<T, AsyncIo>
where
    T: AsyncTransport + HasTransportConfig,
{
    /// Confirm Sony's sequence-number RESET before the owner actor starts.
    pub(crate) async fn confirm_sony_sequence_reset(
        &mut self,
        policy: &OwnerPolicy,
        executor: &impl Executor,
    ) -> Result<(), Error> {
        let mut frame = bytes::BytesMut::new();
        if !self.framing.frame_sony_sequence_reset(&mut frame) {
            return Ok(());
        }
        let mut buffers = OwnerBuffers::new(policy.limits);
        let deadline =
            Deadline::after(executor.now(), policy.write_timeout.get(), "write_timeout")?;
        within(
            async {
                self.transport
                    .send(frame.as_ref())
                    .await
                    .map_err(|error| self.framing.send_error(error))?;

                let mut acknowledged = false;
                loop {
                    deadline.remaining_or(executor.now(), Error::io_timeout)?;
                    let received = self.receive(&mut buffers).await;
                    let progress =
                        self.classify_sony_reset_read(received, &mut buffers, &mut acknowledged)?;
                    let received_at = executor.now();
                    match progress {
                        SonyResetProgress::Confirmed if received_at <= deadline.instant() => {
                            return Ok(())
                        }
                        SonyResetProgress::Confirmed => return Err(Error::io_timeout()),
                        SonyResetProgress::Waiting => futures_lite::future::yield_now().await,
                        SonyResetProgress::Idle => {
                            let pause = deadline
                                .remaining_at(received_at)
                                .min(TRANSIENT_RECEIVE_PAUSE);
                            executor.sleep(pause).await;
                        }
                    }
                }
            },
            executor.timer(deadline.instant()),
        )
        .await
        .ok_or_else(Error::io_timeout)?
    }
}

impl<T> AsyncOwnerDriver for TransportAdapter<T, AsyncIo>
where
    T: AsyncTransport + HasTransportConfig,
{
    fn write(&mut self, mut write: WireWrite<'_>) -> impl Future<Output = WriteOutcome> + Send {
        let framed = write.frame();
        async move {
            let meta = match framed {
                Ok(meta) => meta,
                Err(error) => return WriteOutcome::NotSent(error),
            };
            match self.transport.send(write.frame_buffer.as_ref()).await {
                Ok(()) => WriteOutcome::Sent(meta),
                Err(error) => WriteOutcome::Failed(self.framing.send_error(error)),
            }
        }
    }

    // The private driver trait requires an explicitly `Send` future, so this
    // implementation keeps the `impl Future` form instead of `async fn`.
    #[allow(clippy::manual_async_fn)]
    fn receive(&mut self, buffers: &mut OwnerBuffers) -> impl Future<Output = OwnerReceive> + Send {
        async move {
            if let Some(buffered) = self.buffered(buffers) {
                return buffered;
            }
            let read = self.transport.recv_into(buffers.receive_mut()).await;
            self.classify_read(buffers, read)
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{
        config::OperationalTuning,
        profiles::{GenericVisca, ProfileSpec},
        protocol::framer::RawIncompletePrefix,
        runtime::{
            engine::RawPrefixEvidence,
            owner::{AsyncTransportAdapter, RetainedStreamInput},
        },
        transport::{builder::IoBounds, ReceiveOutcome, SendSemantics},
        ViscaSocket,
    };

    #[derive(Debug)]
    struct ScriptedTransport {
        config: IoBounds,
        sent: Vec<Vec<u8>>,
        receives: std::collections::VecDeque<Result<Vec<u8>, Error>>,
        semantics: SendSemantics,
    }

    /// A custom datagram transport that cannot observe truncation: it copies
    /// what fits of each datagram and reports a buffer-filling read as
    /// possibly truncated, as the receive contract requires.
    #[derive(Debug)]
    struct UnobservedTruncation {
        config: IoBounds,
        datagrams: std::collections::VecDeque<Vec<u8>>,
    }

    impl HasTransportConfig for UnobservedTruncation {
        fn identity(&self) -> crate::transport::TransportIdentity<'_> {
            crate::transport::TransportIdentity::Custom {
                semantics: SendSemantics::Datagram,
                addressing: crate::transport::AddressingMode::Ip,
            }
        }

        fn io_bounds(&self) -> &IoBounds {
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
    }

    impl HasTransportConfig for ScriptedTransport {
        fn identity(&self) -> crate::transport::TransportIdentity<'_> {
            crate::transport::TransportIdentity::Custom {
                semantics: self.semantics,
                addressing: crate::transport::AddressingMode::Ip,
            }
        }

        fn io_bounds(&self) -> &IoBounds {
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
    }

    fn profile() -> ProfileSpec {
        ProfileSpec::from_compile_time::<GenericVisca>().unwrap()
    }

    #[test]
    fn policy_maps_async_transport_semantics_and_decodes_frames() {
        let transport = ScriptedTransport {
            config: Default::default(),
            sent: Vec::new(),
            receives: [Ok(vec![0x90, 0x41, 0xff])].into_iter().collect(),
            semantics: SendSemantics::Datagram,
        };
        let (mut adapter, adapter_policy) =
            AsyncTransportAdapter::new(transport, &profile(), crate::target::TargetSlot::CAMERA_1)
                .unwrap();
        assert_eq!(
            adapter_policy.protocol.transport,
            crate::runtime::engine::TransportKind::Datagram
        );

        let mut buffers = OwnerBuffers::new(adapter_policy.limits);
        let received = futures_lite::future::block_on(adapter.receive(&mut buffers));
        let OwnerReceive::Frames(frames) = received else {
            panic!("a nonzero read must not report the transport as closed");
        };
        assert_eq!(frames.len(), 1);
        assert!(matches!(
            frames[0].response,
            crate::runtime::engine::DecodedResponse::Ack { .. }
        ));
    }

    /// A consumed over-size datagram reports a truncated outcome; its valid
    /// ACK prefix must never be decoded.
    #[test]
    fn truncated_datagram_prefix_is_rejected_before_visca_decode() {
        let config = IoBounds::default();
        let mut oversized = vec![0x90, 0x41, 0xff];
        oversized.resize(config.buffer_config().recv_buffer_size() + 1, 0);
        let transport = ScriptedTransport {
            config,
            sent: Vec::new(),
            receives: [Ok(oversized)].into_iter().collect(),
            semantics: SendSemantics::Datagram,
        };
        let (mut adapter, adapter_policy) =
            AsyncTransportAdapter::new(transport, &profile(), crate::target::TargetSlot::CAMERA_1)
                .unwrap();
        let mut buffers = OwnerBuffers::new(adapter_policy.limits);

        let received = futures_lite::future::block_on(adapter.receive(&mut buffers));

        assert!(matches!(
            received,
            OwnerReceive::NoProgress(crate::runtime::owner::NoProgressReason::MalformedDatagram)
        ));
    }

    /// Issue #804: a custom datagram transport whose read exactly fills the
    /// owner buffer cannot prove the datagram fitted. Both owners discard it
    /// through one classification (the blocking owner used to decode its
    /// prefix), and the next datagram decodes.
    #[test]
    fn an_unobservable_exact_fill_is_discarded_like_the_blocking_owner() {
        let config = IoBounds::default();
        let capacity = config.buffer_config().recv_buffer_size();
        let mut oversized = vec![0x90, 0x41, 0xff];
        oversized.resize(capacity + 1, 0);
        let (mut adapter, adapter_policy) = AsyncTransportAdapter::new(
            UnobservedTruncation {
                config,
                datagrams: [oversized, vec![0x90, 0x41, 0xff]].into_iter().collect(),
            },
            &profile(),
            crate::target::TargetSlot::CAMERA_1,
        )
        .unwrap();
        let mut buffers = OwnerBuffers::new(adapter_policy.limits);

        let discarded = futures_lite::future::block_on(adapter.receive(&mut buffers));
        assert!(matches!(
            discarded,
            OwnerReceive::NoProgress(crate::runtime::owner::NoProgressReason::MalformedDatagram)
        ));
        let next = futures_lite::future::block_on(adapter.receive(&mut buffers));
        let OwnerReceive::Frames(frames) = next else {
            panic!("the next datagram must decode, got {next:?}");
        };
        assert!(matches!(
            frames.iter().collect::<Vec<_>>().as_slice(),
            [ack] if matches!(ack.response, crate::runtime::engine::DecodedResponse::Ack { .. })
        ));
    }

    /// #675: the transport's advertised read/write timeouts must reach the owner
    /// policy so the async owner can enforce them. They were previously dropped
    /// here, leaving both builder knobs inert on every async transport.
    #[test]
    fn read_and_write_timeouts_are_lowered_from_transport_config() {
        let transport = ScriptedTransport {
            config: IoBounds::default()
                .with_read_timeout(std::time::Duration::from_millis(111))
                .unwrap()
                .with_write_timeout(std::time::Duration::from_millis(222))
                .unwrap(),
            sent: Vec::new(),
            receives: std::collections::VecDeque::new(),
            semantics: SendSemantics::Datagram,
        };
        let (_adapter, adapter_policy) =
            AsyncTransportAdapter::new(transport, &profile(), crate::target::TargetSlot::CAMERA_1)
                .unwrap();
        assert_eq!(
            adapter_policy.read_timeout.get(),
            std::time::Duration::from_millis(111)
        );
        assert_eq!(
            adapter_policy.write_timeout.get(),
            std::time::Duration::from_millis(222)
        );
    }

    #[test]
    fn partial_stream_frame_is_reported_as_an_empty_batch_not_a_close() {
        let transport = ScriptedTransport {
            config: Default::default(),
            sent: Vec::new(),
            // One reply split across two reads, then a genuine end of stream.
            receives: [Ok(vec![0x90, 0x41]), Ok(vec![0xff]), Ok(Vec::new())]
                .into_iter()
                .collect(),
            semantics: SendSemantics::Stream,
        };
        let (mut adapter, adapter_policy) =
            AsyncTransportAdapter::new(transport, &profile(), crate::target::TargetSlot::CAMERA_1)
                .unwrap();
        let mut buffers = OwnerBuffers::new(adapter_policy.limits);

        let first = futures_lite::future::block_on(adapter.receive(&mut buffers));
        let OwnerReceive::Frames(frames) = first else {
            panic!("a partial frame must not be reported as a transport close");
        };
        assert!(frames.is_empty());

        let second = futures_lite::future::block_on(adapter.receive(&mut buffers));
        let OwnerReceive::Frames(frames) = second else {
            panic!("the completing read must decode the buffered frame");
        };
        assert_eq!(frames.len(), 1);
        assert!(matches!(
            frames[0].response,
            crate::runtime::engine::DecodedResponse::Ack { .. }
        ));

        let third = futures_lite::future::block_on(adapter.receive(&mut buffers));
        assert!(
            matches!(
                third,
                OwnerReceive::Fatal(crate::runtime::owner::ReceiveFatal::TransportClosed {
                    cause: None
                })
            ),
            "only a zero-length read closes the transport"
        );
    }

    /// The production adapter must expose only protocol facts from an
    /// incomplete raw frame.  In particular, the low nibble of an ACK is not
    /// socket ownership: the engine alone decides whether a due correlation
    /// release may progress.  These literal vectors exercise the real
    /// `RawFramer` seam used by the async actor.
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
                config: Default::default(),
                sent: Vec::new(),
                receives: [Ok(prefix)].into_iter().collect(),
                semantics: SendSemantics::Stream,
            };
            let (mut adapter, adapter_policy) = AsyncTransportAdapter::new(
                transport,
                &profile(),
                crate::target::TargetSlot::CAMERA_1,
            )
            .unwrap();
            let mut buffers = OwnerBuffers::new(adapter_policy.limits);
            let received = futures_lite::future::block_on(adapter.receive(&mut buffers));
            assert!(matches!(received, OwnerReceive::Frames(ref frames) if frames.is_empty()));
            assert_eq!(
                adapter.retained().and_then(|input| input.evidence()),
                Some(RawPrefixEvidence::Incomplete {
                    target: crate::target::TargetSlot::CAMERA_1,
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
                config: Default::default(),
                sent: Vec::new(),
                receives: [Ok(prefix.clone())].into_iter().collect(),
                semantics: SendSemantics::Stream,
            };
            let (mut adapter, adapter_policy) = AsyncTransportAdapter::new(
                transport,
                &profile(),
                crate::target::TargetSlot::CAMERA_1,
            )
            .unwrap();
            let mut buffers = OwnerBuffers::new(adapter_policy.limits);
            let _ = futures_lite::future::block_on(adapter.receive(&mut buffers));
            assert_eq!(
                adapter.retained().and_then(|input| input.evidence()),
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
            Error::Io {
                source: std::sync::Arc::new(std::io::Error::from(std::io::ErrorKind::WouldBlock)),
            },
            Error::Io {
                source: std::sync::Arc::new(std::io::Error::from(std::io::ErrorKind::Interrupted)),
            },
        ] {
            let transport = ScriptedTransport {
                config: Default::default(),
                sent: Vec::new(),
                receives: [Err(idle)].into_iter().collect(),
                semantics: SendSemantics::Datagram,
            };
            let (mut adapter, adapter_policy) = AsyncTransportAdapter::new(
                transport,
                &profile(),
                crate::target::TargetSlot::CAMERA_1,
            )
            .unwrap();
            let mut buffers = OwnerBuffers::new(adapter_policy.limits);
            let received = futures_lite::future::block_on(adapter.receive(&mut buffers));
            assert!(
                matches!(
                    received,
                    OwnerReceive::NoProgress(crate::runtime::owner::NoProgressReason::EagerIdle)
                ),
                "an idle read timeout is not a receive fault: {received:?}"
            );
        }
    }

    /// A read failure that is not an idle timeout still reaches the owner as a
    /// fault, which is what keeps #620's transient-retry semantics working.
    #[test]
    fn real_read_failures_still_reach_the_owner_as_faults() {
        for fault in [
            Error::transport_error(std::io::Error::other("ICMP port unreachable")),
            Error::Io {
                source: std::sync::Arc::new(std::io::Error::from(std::io::ErrorKind::TimedOut)),
            },
        ] {
            let transport = ScriptedTransport {
                config: Default::default(),
                sent: Vec::new(),
                receives: [Err(fault)].into_iter().collect(),
                semantics: SendSemantics::Datagram,
            };
            let (mut adapter, adapter_policy) = AsyncTransportAdapter::new(
                transport,
                &profile(),
                crate::target::TargetSlot::CAMERA_1,
            )
            .unwrap();
            let mut buffers = OwnerBuffers::new(adapter_policy.limits);
            let received = futures_lite::future::block_on(adapter.receive(&mut buffers));
            assert!(matches!(
                received,
                OwnerReceive::Transient(_) | OwnerReceive::Fatal(_)
            ));
        }
    }

    #[test]
    fn immutable_tuning_is_lowered_into_owner_policy() {
        let profile = profile();
        let tuning = OperationalTuning::new()
            .with_command_spacing(
                profile.timing().command_spacing() + std::time::Duration::from_millis(1),
            )
            .with_inquiry_spacing(
                profile.timing().inquiry_spacing() + std::time::Duration::from_millis(1),
            )
            .with_maximum_command_sockets(crate::types::CommandSocketCount::ONE);
        let transport = ScriptedTransport {
            config: Default::default(),
            sent: Vec::new(),
            receives: std::collections::VecDeque::new(),
            semantics: SendSemantics::Datagram,
        };
        let (_adapter, adapter_policy) = AsyncTransportAdapter::new_with_tuning(
            transport,
            &profile,
            crate::target::TargetSlot::CAMERA_1,
            tuning,
        )
        .unwrap();
        assert_eq!(
            adapter_policy.protocol.command_spacing,
            tuning.command_spacing().unwrap()
        );
        assert_eq!(
            adapter_policy.protocol.inquiry_spacing,
            tuning.inquiry_spacing().unwrap()
        );
        assert_eq!(
            adapter_policy.targets[crate::target::TargetSlot::CAMERA_1]
                .unwrap()
                .command_sockets,
            crate::CommandSocketCount::ONE
        );
    }
}
