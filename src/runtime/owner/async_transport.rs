//! Async production transport adapter for the Phase-6 owner.

use std::{future::Future, num::NonZeroUsize};

use crate::{
    command::CommandKind,
    profile::{OperationalTuning, ProfileSpec},
    protocol::framer::ProtocolFramer,
    runtime::engine::TransmissionMeta,
    transport::{envelope::FrameSequence, AsyncTransport, HasTransportConfig},
    CameraId, Error,
};

use super::{
    adapter::{
        decode_frames_with_routing, owner_policy_for_targets_with_tuning,
        validate_profile_transport, OwnerEnvelope, RoutingState, TargetRegistry,
    },
    AsyncOwnerDriver, AsyncReceive, OwnerBuffers, OwnerPolicy, WireWrite,
};

#[derive(Debug)]
struct AsyncAdapterState {
    envelope: OwnerEnvelope,
    framer: ProtocolFramer,
    routing: RoutingState,
}

/// Async owner driver over exactly one production `AsyncTransport`.
#[derive(Debug)]
pub(crate) struct AsyncTransportAdapter<T> {
    transport: T,
    state: AsyncAdapterState,
    policy: OwnerPolicy,
}

impl<T> AsyncTransportAdapter<T>
where
    T: AsyncTransport + HasTransportConfig,
{
    /// Build an owner adapter from validated profile facts and transport
    /// configuration.  Construction performs no transport I/O.
    // Test-only single-target convenience; production uses `new_with_targets`.
    #[cfg(test)]
    pub(crate) fn new(
        transport: T,
        profile: &ProfileSpec,
        target: CameraId,
    ) -> Result<Self, Error> {
        Self::new_with_tuning(transport, profile, target, OperationalTuning::new())
    }

    /// Build an owner adapter using immutable session tuning.
    // Test-only single-target convenience; production uses `new_with_targets`.
    #[cfg(test)]
    pub(crate) fn new_with_tuning(
        transport: T,
        profile: &ProfileSpec,
        target: CameraId,
        tuning: OperationalTuning,
    ) -> Result<Self, Error> {
        Self::new_with_targets(
            transport,
            &[(target, profile)],
            tuning,
            crate::DEFAULT_ADMISSION_CAPACITY,
        )
    }

    /// Build an owner adapter for several immutable target/profile pairs.
    /// Profiles must describe one compatible wire envelope; target-local
    /// socket/cancellation facts are retained in the resulting owner policy.
    pub(crate) fn new_with_targets(
        transport: T,
        profiles: &[(CameraId, &ProfileSpec)],
        tuning: OperationalTuning,
        admission_capacity: NonZeroUsize,
    ) -> Result<Self, Error> {
        // This check is deliberately before reading any startup-side transport
        // state or constructing the owner policy. Known standard transports
        // must be compatible; custom transports (which report `None`) remain
        // an explicit profile-compatibility escape hatch.
        for (_, profile) in profiles {
            validate_profile_transport(profile, transport.standard_transport_kind())?;
        }
        let standard_kind = transport.standard_transport_kind();
        // Multi-target routing must be explicitly proven by a side-effect-free
        // transport hint. This runs before the first config read, actor spawn,
        // or transport operation; custom transports default to `None` and are
        // therefore rejected unless they opt into serial addressing.
        super::adapter::validate_profile_registry_topology(
            profiles,
            standard_kind,
            transport.addressing_mode_hint(),
        )?;
        let config = *transport.transport_config();
        super::adapter::validate_profile_registry_topology(
            profiles,
            standard_kind,
            Some(config.addressing),
        )?;
        let policy = owner_policy_for_targets_with_tuning(
            profiles,
            &config,
            transport.send_semantics(),
            tuning,
            admission_capacity,
        )?;
        let targets: Vec<_> = profiles.iter().map(|(target, _)| *target).collect();
        let registry = TargetRegistry::from_targets(&targets)?;
        let profile = profiles[0].1;
        let envelope = OwnerEnvelope::from_profile(profile, config.addressing)?;
        let routing = RoutingState::new(config.addressing, registry);
        let framer = ProtocolFramer::new_with_config(config.buffer_config);
        Ok(Self {
            transport,
            state: AsyncAdapterState {
                envelope,
                framer,
                routing,
            },
            policy,
        })
    }

    /// Alias emphasizing that the profile pairs form an immutable registry.
    pub(crate) fn new_with_profile_registry(
        transport: T,
        profiles: &[(CameraId, &ProfileSpec)],
        tuning: OperationalTuning,
        admission_capacity: NonZeroUsize,
    ) -> Result<Self, Error> {
        Self::new_with_targets(transport, profiles, tuning, admission_capacity)
    }

    pub(crate) fn policy(&self) -> &OwnerPolicy {
        &self.policy
    }
}

impl<T> AsyncOwnerDriver for AsyncTransportAdapter<T>
where
    T: AsyncTransport + HasTransportConfig,
{
    fn write(
        &mut self,
        write: WireWrite<'_>,
    ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
        let datagram =
            self.policy.protocol.transport != crate::runtime::engine::TransportKind::Stream;
        let frame_meta = if write.envelope != self.state.envelope.kind() {
            Err(Error::InvalidState(
                "owner write envelope does not match transport adapter".into(),
            ))
        } else {
            let kind = if write.inquiry {
                CommandKind::Inquiry
            } else {
                CommandKind::Command
            };
            self.state.envelope.frame_into_with_sequence(
                write.bytes,
                kind,
                write.requested_sequence,
                write.frame_buffer,
            )
        };

        async move {
            let frame_meta = frame_meta?;
            self.transport
                .send(write.frame_buffer.as_ref())
                .await
                .map_err(|error| {
                    if datagram {
                        super::normalize_datagram_send_error(error)
                    } else {
                        error
                    }
                })?;
            Ok(TransmissionMeta {
                // Keep receive provenance typed on FrameMeta. TransmissionMeta
                // intentionally carries only the numeric value the engine
                // records for an outgoing write.
                sequence: frame_meta.sequence.map(FrameSequence::value),
            })
        }
    }

    // The private driver trait requires an explicitly `Send` future, so this
    // implementation keeps the `impl Future` form instead of `async fn`.
    #[allow(clippy::manual_async_fn)]
    fn receive(
        &mut self,
        buffers: &mut OwnerBuffers,
        frame_limit: usize,
    ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
        async move {
            // A failed read consumed nothing, so the framer is untouched and
            // the owner still gets to decide whether the session survives.
            // Framing/decode failures below stay on the `Err` path.
            let received = match self.transport.recv_into(buffers.receive_mut()).await {
                Ok(received) => received,
                // An expired idle read timeout means no bytes arrived, not that
                // the read failed. The blocking adapter has always normalized
                // this; doing it here too keeps a transport with an internal
                // read timeout — the shape the trait documents — from burning
                // every in-flight retry budget (#625, #637).
                Err(error) if super::receive_reported_no_data(&error) => {
                    return Ok(AsyncReceive::NoData)
                }
                Err(error) => return Ok(AsyncReceive::Fault(error)),
            };
            // Only a zero-length read means the peer closed. A short read that
            // carried bytes decodes to an empty batch when it did not finish a
            // frame, which is routine on byte-stream transports.
            if received == 0 {
                return Ok(AsyncReceive::Closed);
            }
            decode_frames_with_routing(
                &self.state.envelope,
                &mut self.state.framer,
                self.state.routing,
                buffers,
                received,
                frame_limit,
                self.policy.protocol.transport,
            )
            .map(AsyncReceive::Frames)
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{
        profile::{OperationalTuning, ProfileSpec},
        profiles::GenericVisca,
        transport::{builder::TransportConfig, SendSemantics},
    };

    #[derive(Debug)]
    struct ScriptedTransport {
        config: TransportConfig,
        sent: Vec<Vec<u8>>,
        receives: std::collections::VecDeque<Result<Vec<u8>, Error>>,
        semantics: SendSemantics,
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

        async fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
            let next = self.receives.pop_front().unwrap_or(Ok(Vec::new()))?;
            let n = next.len().min(dst.len());
            dst[..n].copy_from_slice(&next[..n]);
            Ok(n)
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
        let AsyncReceive::Frames(frames) = received else {
            panic!("a nonzero read must not report the transport as closed");
        };
        assert_eq!(frames.len(), 1);
        assert!(matches!(
            frames[0].response,
            crate::runtime::engine::DecodedResponse::Ack { .. }
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
        let AsyncReceive::Frames(frames) = first else {
            panic!("a partial frame must not be reported as a transport close");
        };
        assert!(frames.is_empty());

        let second = futures_lite::future::block_on(adapter.receive(&mut buffers, 4)).unwrap();
        let AsyncReceive::Frames(frames) = second else {
            panic!("the completing read must decode the buffered frame");
        };
        assert_eq!(frames.len(), 1);
        assert!(matches!(
            frames[0].response,
            crate::runtime::engine::DecodedResponse::Ack { .. }
        ));

        let third = futures_lite::future::block_on(adapter.receive(&mut buffers, 4)).unwrap();
        assert!(
            matches!(third, AsyncReceive::Closed),
            "only a zero-length read closes the transport"
        );
    }

    /// Issue #625/#637: a transport with an internal read timeout is the shape
    /// the public trait documents. An expired idle timeout is no data, not a
    /// receive fault, so it must never provoke a retransmission.
    #[test]
    fn an_idle_read_timeout_decodes_as_no_data() {
        for idle in [
            Error::Timeout,
            Error::Io(std::sync::Arc::new(std::io::Error::from(
                std::io::ErrorKind::WouldBlock,
            ))),
            Error::Io(std::sync::Arc::new(std::io::Error::from(
                std::io::ErrorKind::TimedOut,
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
                matches!(received, AsyncReceive::NoData),
                "an idle read timeout is not a receive fault: {received:?}"
            );
        }
    }

    /// A read failure that is not an idle timeout still reaches the owner as a
    /// fault, which is what keeps #620's transient-retry semantics working.
    #[test]
    fn a_real_read_failure_still_reaches_the_owner_as_a_fault() {
        let transport = ScriptedTransport {
            config: TransportConfig::default(),
            sent: Vec::new(),
            receives: [Err(Error::TransportError("ICMP port unreachable".into()))]
                .into_iter()
                .collect(),
            semantics: SendSemantics::Datagram,
        };
        let mut adapter =
            AsyncTransportAdapter::new(transport, &profile(), CameraId::CAMERA_1).unwrap();
        let mut buffers = OwnerBuffers::new(adapter.policy().limits).unwrap();
        let received = futures_lite::future::block_on(adapter.receive(&mut buffers, 4)).unwrap();
        assert!(matches!(received, AsyncReceive::Fault(_)));
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
