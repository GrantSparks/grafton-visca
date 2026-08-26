//! Async production transport adapter for the Phase-6 owner.

use std::future::Future;

use crate::{
    command::CommandKind,
    profile::{OperationalTuning, ProfileSpec},
    protocol::framer::ProtocolFramer,
    runtime::engine::{DecodedFrame, TransmissionMeta},
    transport::{AsyncTransport, HasTransportConfig},
    CameraId, Error,
};

use super::{
    adapter::{
        decode_frames_with_routing, owner_policy_for_targets_with_tuning,
        validate_profile_transport, OwnerEnvelope, RoutingState, TargetRegistry,
    },
    AsyncOwnerDriver, OwnerBuffers, OwnerPolicy, WireWrite,
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

/// Short alias used by owner construction code.
pub(crate) type AsyncOwnerAdapter<T> = AsyncTransportAdapter<T>;

impl<T> AsyncTransportAdapter<T>
where
    T: AsyncTransport + HasTransportConfig,
{
    /// Build an owner adapter from validated profile facts and transport
    /// configuration.  Construction performs no transport I/O.
    pub(crate) fn new(
        transport: T,
        profile: &ProfileSpec,
        target: CameraId,
    ) -> Result<Self, Error> {
        Self::new_with_tuning(transport, profile, target, OperationalTuning::new())
    }

    /// Build an owner adapter using immutable session tuning.
    pub(crate) fn new_with_tuning(
        transport: T,
        profile: &ProfileSpec,
        target: CameraId,
        tuning: OperationalTuning,
    ) -> Result<Self, Error> {
        Self::new_with_targets(transport, &[(target, profile)], tuning)
    }

    /// Build an owner adapter for several immutable target/profile pairs.
    /// Profiles must describe one compatible wire envelope; target-local
    /// socket/cancellation facts are retained in the resulting owner policy.
    pub(crate) fn new_with_targets(
        transport: T,
        profiles: &[(CameraId, &ProfileSpec)],
        tuning: OperationalTuning,
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
    ) -> Result<Self, Error> {
        Self::new_with_targets(transport, profiles, tuning)
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
            Ok(self
                .state
                .envelope
                .frame_into(write.bytes, kind, write.frame_buffer))
        };

        async move {
            let frame_meta = frame_meta?;
            self.transport.send(write.frame_buffer.as_ref()).await?;
            Ok(TransmissionMeta {
                sequence: frame_meta.sequence,
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
    ) -> impl Future<Output = Result<Vec<DecodedFrame>, Error>> + Send {
        async move {
            let received = self.transport.recv_into(buffers.receive_mut()).await?;
            decode_frames_with_routing(
                &self.state.envelope,
                &mut self.state.framer,
                self.state.routing,
                buffers,
                received,
                frame_limit,
            )
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
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
            SendSemantics::Datagram
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
        };
        let mut adapter =
            AsyncTransportAdapter::new(transport, &profile(), CameraId::CAMERA_1).unwrap();
        assert_eq!(
            adapter.policy().protocol.transport,
            crate::runtime::engine::TransportKind::Datagram
        );

        let mut buffers = OwnerBuffers::new(adapter.policy().limits).unwrap();
        let frames = futures_lite::future::block_on(adapter.receive(&mut buffers, 4)).unwrap();
        assert_eq!(frames.len(), 1);
        assert!(matches!(
            frames[0].response,
            crate::runtime::engine::DecodedResponse::Ack { .. }
        ));
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
