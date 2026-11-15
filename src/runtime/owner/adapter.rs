//! Shared production transport-adapter pieces for the Phase-6 owner.
//!
//! The owner deliberately keeps transport I/O, envelope handling, framing, and
//! response classification at its boundary.  This module contains the small
//! amount of mode-independent work needed by the blocking and async adapters;
//! it does not own scheduling, settlement, or observer deadlines.

#[cfg(any(test, feature = "test-utils"))]
use crate::OperationalTuning;

use std::{borrow::Cow, num::NonZeroUsize};

#[cfg(test)]
use bytes::Bytes;
use smallvec::SmallVec;

use crate::{
    camera::TransportKind as CameraTransportKind,
    profile::{ProfileEnvelope, ProfileSpec},
    protocol::{
        framer::{Framer, FramingError, RawFramer, SonyFramer},
        response::{decode_basic_for_source, BasicKind},
    },
    raw::INLINE_BYTES,
    runtime::engine::{
        CancellationPolicy, DecodedFrame, DecodedResponse, EnvelopeKind, EnvelopeSequence,
        ProtocolPolicy, SequenceWidth, TargetPolicy, TransportKind,
    },
    target::{PerTarget, TargetSlot},
    transport::{
        builder::{AddressingMode, TransportConfig},
        envelope::{Envelope, FrameMeta, FrameSequence, RawVisca, SonyEncapsulated, SonyResponse},
        SendSemantics,
    },
    CameraId, Error,
};

use super::{FrameBatch, OwnerBuffers, OwnerPolicy};
#[cfg(any(feature = "async", feature = "blocking"))]
use super::{OwnerReceive, ReceiveFatal, RetainedEvidence, RetainedInput, RetainedStreamInput};
#[cfg(feature = "async")]
use crate::transport::AsyncTransport;
#[cfg(feature = "blocking")]
use crate::transport::BlockingTransport;
#[cfg(any(feature = "async", feature = "blocking"))]
use crate::{
    protocol::framer::RawBufferedInput,
    transport::{HasTransportConfig, ReceiveOutcome},
};

pub(crate) enum OwnerResponse<'a> {
    Visca { payload: &'a [u8], meta: FrameMeta },
    SonyControl { code: u16 },
}

/// Associate each sealed envelope with its only valid frame grammar.
pub(crate) trait AdapterEnvelope: Envelope {
    type Framer: Framer;
    fn extract_owner<'a>(&self, framed: &'a [u8]) -> Result<OwnerResponse<'a>, Error>;
}

impl AdapterEnvelope for RawVisca {
    type Framer = RawFramer;
    fn extract_owner<'a>(&self, framed: &'a [u8]) -> Result<OwnerResponse<'a>, Error> {
        Ok(OwnerResponse::Visca {
            payload: framed,
            meta: FrameMeta::Raw,
        })
    }
}

impl AdapterEnvelope for SonyEncapsulated {
    type Framer = SonyFramer;
    fn extract_owner<'a>(&self, framed: &'a [u8]) -> Result<OwnerResponse<'a>, Error> {
        match self.extract_owner_response(framed)? {
            SonyResponse::Visca { payload, sequence } => Ok(OwnerResponse::Visca {
                payload,
                meta: FrameMeta::Sony { sequence },
            }),
            SonyResponse::Control { code } => Ok(OwnerResponse::SonyControl { code }),
        }
    }
}

impl From<ProfileEnvelope> for EnvelopeKind {
    fn from(envelope: ProfileEnvelope) -> Self {
        match envelope {
            ProfileEnvelope::RawVisca => Self::Raw,
            ProfileEnvelope::SonyEncapsulated => Self::Sony,
        }
    }
}

/// Immutable set of camera targets which may be attributed by one transport
/// adapter.
///
/// A serial response carries its source camera in the high nibble of the
/// first byte. IP VISCA responses do not carry a routable camera source, so an
/// IP response is attributable only when this set contains exactly one
/// target. Keeping the set in the adapter (rather than consulting request
/// state while decoding) prevents an unsolicited response from inheriting a
/// configured default target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TargetRegistry {
    registered: PerTarget<bool>,
    count: u8,
}

impl TargetRegistry {
    /// Creates a registry from individual camera targets.
    pub(crate) fn from_targets(targets: &[TargetSlot]) -> Result<Self, Error> {
        let mut registered = PerTarget::default();
        let mut count = 0_u8;
        if targets.is_empty() {
            return Err(Error::InvalidRequest(
                "owner target registry must contain at least one camera".into(),
            ));
        }
        for target in targets {
            let slot = &mut registered[*target];
            if *slot {
                return Err(Error::InvalidRequest(
                    "owner target registry contains a duplicate camera".into(),
                ));
            }
            *slot = true;
            count = count.saturating_add(1);
        }
        Ok(Self { registered, count })
    }

    /// Creates a registry for one target, preserving the existing constructor
    /// behavior.
    // Single-target sessions are the common case, but only this module's own
    // routing tests build a registry that way today; production construction
    // goes through `from_targets` with the session's registered set (#636).
    #[cfg(test)]
    pub(crate) fn single(target: TargetSlot) -> Result<Self, Error> {
        Self::from_targets(&[target])
    }

    pub(crate) fn contains(self, target: TargetSlot) -> bool {
        self.registered[target]
    }

    pub(crate) fn sole_target(self) -> Option<TargetSlot> {
        if self.count != 1 {
            return None;
        }
        self.registered
            .iter()
            .find_map(|(target, registered)| registered.then_some(target))
    }
}

/// Addressing facts retained by an adapter for strict response routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RoutingState {
    addressing: AddressingMode,
    targets: TargetRegistry,
}

impl RoutingState {
    pub(crate) const fn new(addressing: AddressingMode, targets: TargetRegistry) -> Self {
        Self {
            addressing,
            targets,
        }
    }

    pub(crate) const fn addressing(self) -> AddressingMode {
        self.addressing
    }

    pub(crate) const fn targets(self) -> TargetRegistry {
        self.targets
    }
}

/// Lower one immutable owner policy for a bounded set of target/profile
/// registrations. All profiles must use the same envelope because one
/// physical adapter owns one framer and one wire mode. Per-target socket and
/// cancellation facts remain local, while shared pacing uses the strictest
/// compatible profile requirement.
// Untuned convenience over `owner_policy_for_targets_with_tuning`, exercised by
// this module's own tests; session construction always supplies tuning (#636).
#[cfg(test)]
pub(crate) fn owner_policy_for_targets(
    profiles: &[(TargetSlot, &ProfileSpec)],
    config: &TransportConfig,
    semantics: SendSemantics,
) -> Result<OwnerPolicy, Error> {
    owner_policy_for_targets_with_tuning(
        profiles,
        config,
        semantics,
        crate::session_config::ValidatedTuning::validate(
            OperationalTuning::new(),
            profiles.iter().map(|(_, profile)| *profile),
        )?,
        crate::DEFAULT_ADMISSION_CAPACITY,
        false,
    )
}

/// Tuning-aware multi-target owner policy lowering. The target registry is
/// validated before any owner state is created, so registration is immutable
/// for the lifetime of the resulting owner.
pub(crate) fn owner_policy_for_targets_with_tuning(
    profiles: &[(TargetSlot, &ProfileSpec)],
    config: &TransportConfig,
    semantics: SendSemantics,
    tuning: crate::session_config::ValidatedTuning,
    admission_capacity: NonZeroUsize,
    strict_unconfirmed_poison: bool,
) -> Result<OwnerPolicy, Error> {
    if profiles.is_empty() {
        return Err(Error::InvalidRequest(
            "owner target registry must contain at least one camera".into(),
        ));
    }
    if profiles.len() > 7 {
        return Err(Error::InvalidRequest(
            "owner target registry cannot contain more than seven cameras".into(),
        ));
    }

    let validated_tuning = tuning;
    let tuning = tuning.get();
    let first_profile = profiles[0].1;
    let envelope = EnvelopeKind::from(first_profile.envelope());
    if config.addressing == AddressingMode::Ip && profiles.len() > 1 {
        return Err(Error::NotSupported);
    }
    let mut target_policies = PerTarget::default();
    let mut command_spacing = std::time::Duration::ZERO;
    let mut inquiry_spacing = std::time::Duration::ZERO;
    let mut inquiry_cooldown = std::time::Duration::ZERO;
    let mut raw_inquiry_release_hold = std::time::Duration::ZERO;
    let mut seen = PerTarget::default();
    // The profile-only facts, kept alongside the tuned ones so a later runtime
    // reconfiguration can re-derive from the profile rather than ratcheting off
    // the value a previous override installed (#631).
    let mut baseline = super::TuningBaseline {
        command_spacing: std::time::Duration::ZERO,
        inquiry_spacing: std::time::Duration::ZERO,
        command_sockets: PerTarget::default(),
    };

    for (target, profile) in profiles {
        if seen[*target] {
            return Err(Error::InvalidRequest(
                "owner target registry contains a duplicate camera".into(),
            ));
        }
        seen[*target] = true;
        if profile.envelope() != first_profile.envelope() {
            return Err(Error::InvalidRequest(
                "registered session profiles require one wire envelope".into(),
            ));
        }

        let timing = profile.timing();
        baseline.command_spacing = baseline
            .command_spacing
            .max(timing.minimum_command_spacing());
        baseline.inquiry_spacing = baseline
            .inquiry_spacing
            .max(timing.minimum_inquiry_spacing());
        baseline.command_sockets[*target] = Some(profile.maximum_command_sockets());
        command_spacing = command_spacing.max(
            tuning
                .command_spacing_override()
                .unwrap_or_else(|| timing.minimum_command_spacing()),
        );
        inquiry_spacing = inquiry_spacing.max(
            tuning
                .inquiry_spacing_override()
                .unwrap_or_else(|| timing.minimum_inquiry_spacing()),
        );
        inquiry_cooldown = inquiry_cooldown.max(timing.busy_timeout());
        raw_inquiry_release_hold = raw_inquiry_release_hold.max(timing.raw_inquiry_reply_skew());
        target_policies[*target] = Some(TargetPolicy {
            command_sockets: tuning
                .maximum_command_sockets_override()
                .unwrap_or_else(|| profile.maximum_command_sockets()),
            cancellation: if profile.supports_command_cancel() {
                CancellationPolicy::Supported
            } else {
                CancellationPolicy::Unsupported
            },
            control_reserve: profile.typed_stop_paths(),
        });
    }

    config.validate()?;

    let transport = match semantics {
        SendSemantics::Datagram => TransportKind::Datagram,
        SendSemantics::Stream => TransportKind::Stream,
    };

    let inquiry_capacity = if envelope == EnvelopeKind::Sony {
        admission_capacity.get()
    } else {
        1
    };
    let wire = crate::runtime::engine::WireProtocol::new(envelope, config.addressing);
    let mut policy = OwnerPolicy::with_targets(
        ProtocolPolicy {
            capacity: admission_capacity.get(),
            wire,
            transport,
            inquiry_capacity,
            command_spacing,
            inquiry_spacing,
            // A syntax-error inquiry retry needs to respect the strictest
            // profile busy/cooldown fact. Ordinary inquiries do not incur this
            // wait.
            inquiry_cooldown,
            // A raw inquiry that times out or retries may still emit one late,
            // unkeyed reply. Bound that narrow inquiry-only hold by the
            // profile's dedicated reply-skew fact rather than the unrelated
            // one-second cancellation ambiguity window (#712).
            raw_inquiry_release_hold,
            raw_release_grace: config
                .read_timeout
                .min(std::time::Duration::from_millis(100)),
            // Off by default: a raw command that can no longer be confirmed
            // fails on its own and quarantines its correlation, rather than
            // poisoning the whole session. The strict opt-in restores the
            // whole-session poison for deployments that prefer it.
            strict_unconfirmed_poison,
        },
        target_policies,
        validated_tuning,
    )?;
    policy.limits.receive_bytes = config.buffer_config.recv_buffer_size;
    // Lower the caller's advertised read/write timeouts onto the owner policy so
    // both owners enforce them (#675): the async owner around each read,
    // because the runtime-agnostic async transports hold no timer of their own,
    // and the blocking worker as the idle span its sliced reads accumulate
    // before reporting no data (#780).
    policy.read_timeout = config.read_timeout;
    policy.write_timeout = config.write_timeout;
    policy.baseline = baseline;
    Ok(policy)
}

/// Marks a [`TransportAdapter`] driven by the blocking owner worker.
#[cfg(feature = "blocking")]
#[derive(Debug)]
pub(crate) enum BlockingIo {}

/// Marks a [`TransportAdapter`] driven by the async owner actor.
#[cfg(feature = "async")]
#[derive(Debug)]
pub(crate) enum AsyncIo {}

/// The facts a transport declares through its facade's transport trait `F`
/// ([`BlockingIo`] or [`AsyncIo`]). Both traits declare them identically; this
/// seam lets one adapter constructor read them from either.
#[cfg(any(feature = "async", feature = "blocking"))]
pub(crate) trait DeclaredTransport<F>: HasTransportConfig {
    fn declared_addressing_hint(&self) -> Option<AddressingMode>;
    fn declared_semantics(&self) -> SendSemantics;
}

#[cfg(feature = "blocking")]
impl<T: BlockingTransport + HasTransportConfig> DeclaredTransport<BlockingIo> for T {
    fn declared_addressing_hint(&self) -> Option<AddressingMode> {
        self.addressing_mode_hint()
    }

    fn declared_semantics(&self) -> SendSemantics {
        self.send_semantics()
    }
}

#[cfg(feature = "async")]
impl<T: AsyncTransport + HasTransportConfig> DeclaredTransport<AsyncIo> for T {
    fn declared_addressing_hint(&self) -> Option<AddressingMode> {
        self.addressing_mode_hint()
    }

    fn declared_semantics(&self) -> SendSemantics {
        self.send_semantics()
    }
}

/// One production transport plus its owner-side framing and policy.
///
/// The facade marker `F` selects which owner drives it: the blocking worker
/// implements its driver for `TransportAdapter<T, BlockingIo>` and the async
/// actor for `TransportAdapter<T, AsyncIo>`. Everything but the native send
/// and read call is written here once, including the classification of what
/// a read returned (#804).
#[cfg(any(feature = "async", feature = "blocking"))]
#[derive(Debug)]
pub(crate) struct TransportAdapter<T, F> {
    pub(super) transport: T,
    pub(super) framing: AdapterFraming,
    policy: OwnerPolicy,
    facade: std::marker::PhantomData<fn() -> F>,
}

/// The blocking owner's adapter over one blocking transport.
#[cfg(feature = "blocking")]
pub(crate) type BlockingTransportAdapter<T> = TransportAdapter<T, BlockingIo>;

/// The async owner's adapter over one async transport.
#[cfg(feature = "async")]
pub(crate) type AsyncTransportAdapter<T> = TransportAdapter<T, AsyncIo>;

#[cfg(any(feature = "async", feature = "blocking"))]
impl<T, F> TransportAdapter<T, F>
where
    T: DeclaredTransport<F>,
{
    /// Build an adapter for several immutable target/profile pairs on one
    /// physical transport; see [`AdapterFraming::for_targets`]. No transport
    /// operation occurs here.
    pub(crate) fn new_with_targets(
        transport: T,
        profiles: &[(TargetSlot, &ProfileSpec)],
        tuning: crate::session_config::ValidatedTuning,
        admission_capacity: NonZeroUsize,
        strict_unconfirmed_poison: bool,
    ) -> Result<Self, Error> {
        let (framing, policy) = AdapterFraming::for_targets(
            &transport,
            transport.declared_addressing_hint(),
            transport.declared_semantics(),
            profiles,
            tuning,
            admission_capacity,
            strict_unconfirmed_poison,
        )?;
        Ok(Self {
            transport,
            framing,
            policy,
            facade: std::marker::PhantomData,
        })
    }

    /// Test-only single-target convenience with default tuning.
    #[cfg(test)]
    pub(crate) fn new(
        transport: T,
        profile: &ProfileSpec,
        target: TargetSlot,
    ) -> Result<Self, Error> {
        Self::new_with_tuning(transport, profile, target, OperationalTuning::new())
    }

    /// Test-only single-target convenience.
    #[cfg(test)]
    pub(crate) fn new_with_tuning(
        transport: T,
        profile: &ProfileSpec,
        target: TargetSlot,
        tuning: OperationalTuning,
    ) -> Result<Self, Error> {
        Self::new_with_targets(
            transport,
            &[(target, profile)],
            crate::session_config::ValidatedTuning::validate(tuning, [profile])?,
            crate::DEFAULT_ADMISSION_CAPACITY,
            false,
        )
    }
}

#[cfg(any(feature = "async", feature = "blocking"))]
impl<T, F> TransportAdapter<T, F> {
    pub(crate) fn policy(&self) -> &OwnerPolicy {
        &self.policy
    }

    /// Complete frames a prior receive left buffered, delivered before the
    /// adapter reads again; see [`AdapterFraming::drain_buffered`].
    pub(super) fn buffered(
        &mut self,
        buffers: &mut OwnerBuffers,
        frame_limit: usize,
    ) -> Option<OwnerReceive> {
        match self.framing.drain_buffered(buffers, frame_limit) {
            Ok(frames) => frames.map(OwnerReceive::Frames),
            Err(error) => Some(OwnerReceive::consumed_error(
                error,
                self.policy.protocol.transport,
            )),
        }
    }

    /// Classify what one native read into `buffers` returned. Both owners
    /// share this one rule:
    ///
    /// - A read whose [`ReceiveOutcome`] is not complete consumed input whose
    ///   tail is gone, so its prefix is never decoded. It is a decode error,
    ///   not a transport fault: on a datagram transport the owner discards
    ///   that one datagram and continues (a fault could retry work against an
    ///   already-consumed response); on a byte stream the consumed bytes are
    ///   lost, the stream position is unknowable, and the session ends.
    /// - Zero bytes is end of stream; a short read that carried bytes decodes
    ///   to an empty batch when it did not finish a frame.
    /// - An idle read timeout is no data, not a failed read. Custom
    ///   transports use `Error::io_timeout()`; raw `WouldBlock` and
    ///   `Interrupted` spellings mean the same. A raw `TimedOut` can be TCP
    ///   keepalive exhaustion and stays a fault (#719).
    /// - Any other failed read consumed nothing, so the framer is untouched
    ///   and the owner decides whether the session survives.
    pub(super) fn classify_read(
        &mut self,
        buffers: &mut OwnerBuffers,
        frame_limit: usize,
        read: Result<ReceiveOutcome, Error>,
    ) -> OwnerReceive {
        let decoded = match read {
            Ok(outcome) if !outcome.is_complete() => Err(Error::ResponseTooLarge {
                max_size: self.policy.limits.receive_bytes,
            }),
            Ok(outcome) if outcome.copied_len() == 0 => {
                return OwnerReceive::Fatal(ReceiveFatal::TransportClosed { cause: None })
            }
            Ok(outcome) => self
                .framing
                .decode(buffers, outcome.copied_len(), frame_limit),
            Err(error) => return OwnerReceive::read_error(error),
        };
        match decoded {
            Ok(frames) => OwnerReceive::Frames(frames),
            Err(error) => OwnerReceive::consumed_error(error, self.policy.protocol.transport),
        }
    }
}

#[cfg(any(feature = "async", feature = "blocking"))]
impl<T, F> RetainedStreamInput for TransportAdapter<T, F> {
    fn retained(&self) -> Option<RetainedInput> {
        self.framing.retained()
    }
    fn discard_first(&mut self) -> Result<(), Error> {
        self.framing.discard_first()
    }
}

/// The owner-side framing of one physical transport: its envelope, its
/// protocol framer and its response routing.
///
/// Every [`TransportAdapter`] holds exactly one of these beside its
/// transport, so validation, write framing, decoding and the retained
/// stream-input seam are written once for both owners.
#[cfg(any(feature = "async", feature = "blocking"))]
#[derive(Debug)]
pub(crate) struct AdapterFraming {
    framing: EnvelopeFraming,
    routing: RoutingState,
    transport: TransportKind,
}

#[cfg(any(feature = "async", feature = "blocking"))]
#[derive(Debug)]
enum EnvelopeFraming {
    Raw {
        envelope: RawVisca,
        framer: RawFramer,
    },
    Sony {
        envelope: SonyEncapsulated,
        framer: SonyFramer,
    },
}

#[cfg(any(feature = "async", feature = "blocking"))]
impl AdapterFraming {
    /// Validate a target/profile registry against `transport` and build its
    /// framing and owner policy. Profiles must describe one compatible wire
    /// envelope; target-local socket/cancellation facts are retained in the
    /// resulting owner policy. No transport I/O happens here.
    ///
    /// `addressing_hint` and `semantics` are the transport's own answers,
    /// which the async and blocking transport traits each declare.
    pub(crate) fn for_targets<T>(
        transport: &T,
        addressing_hint: Option<AddressingMode>,
        semantics: SendSemantics,
        profiles: &[(TargetSlot, &ProfileSpec)],
        tuning: crate::session_config::ValidatedTuning,
        admission_capacity: NonZeroUsize,
        strict_unconfirmed_poison: bool,
    ) -> Result<(Self, OwnerPolicy), Error>
    where
        T: HasTransportConfig,
    {
        // This check is deliberately before reading any startup-side transport
        // state or constructing the owner policy. Known standard transports
        // must be compatible; custom transports (which report `None`) remain
        // an explicit profile-compatibility escape hatch.
        let standard_kind = transport.standard_transport_kind();
        for (_, profile) in profiles {
            validate_profile_transport(profile, standard_kind)?;
        }
        // Multi-target routing must be explicitly proven by a side-effect-free
        // transport hint. This runs before the first config read, owner start,
        // or transport operation; custom transports default to `None` and are
        // therefore rejected unless they opt into serial addressing.
        validate_profile_registry_topology(profiles, standard_kind, addressing_hint)?;
        let config = *transport.transport_config();
        validate_profile_registry_topology(profiles, standard_kind, Some(config.addressing))?;
        let policy = owner_policy_for_targets_with_tuning(
            profiles,
            &config,
            semantics,
            tuning,
            admission_capacity,
            strict_unconfirmed_poison,
        )?;
        let targets: Vec<_> = profiles.iter().map(|(target, _)| *target).collect();
        let registry = TargetRegistry::from_targets(&targets)?;
        let framing = match policy.protocol.wire.encoder() {
            crate::runtime::engine::ProtocolEncoder::Raw(encoder) => EnvelopeFraming::Raw {
                envelope: encoder.envelope(),
                framer: RawFramer::new(config.buffer_config),
            },
            crate::runtime::engine::ProtocolEncoder::Sony(encoder) => EnvelopeFraming::Sony {
                envelope: encoder.envelope(),
                framer: SonyFramer::new(config.buffer_config),
            },
        };
        let framing = Self {
            framing,
            routing: RoutingState::new(config.addressing, registry),
            transport: policy.protocol.transport,
        };
        Ok((framing, policy))
    }

    const fn is_stream(&self) -> bool {
        matches!(self.transport, TransportKind::Stream)
    }

    /// Frame Sony's sequence-number RESET, sent before the owner starts.
    pub(crate) fn frame_sony_sequence_reset(&self, out: &mut bytes::BytesMut) -> bool {
        match &self.framing {
            EnvelopeFraming::Raw { .. } => false,
            EnvelopeFraming::Sony { envelope, .. } => {
                envelope.frame_sequence_reset_into(out);
                true
            }
        }
    }

    /// The error a failed send reports. A datagram send failure leaves the
    /// session running, so it must not claim the session is over.
    pub(crate) fn send_error(&self, error: Error) -> Error {
        if self.is_stream() {
            error
        } else {
            super::normalize_datagram_send_error(error)
        }
    }

    /// Decode `received` newly read bytes, at most `frame_limit` frames.
    pub(crate) fn decode(
        &mut self,
        buffers: &mut OwnerBuffers,
        received: usize,
        frame_limit: usize,
    ) -> Result<FrameBatch, Error> {
        match &mut self.framing {
            EnvelopeFraming::Raw { envelope, framer } => decode_frames_with_routing(
                envelope,
                framer,
                self.routing,
                buffers,
                received,
                frame_limit,
                self.transport,
            ),
            EnvelopeFraming::Sony { envelope, framer } => decode_frames_with_routing(
                envelope,
                framer,
                self.routing,
                buffers,
                received,
                frame_limit,
                self.transport,
            ),
        }
    }

    /// Attribute complete frames a prior receive left buffered at the
    /// per-receive frame limit, without reading (#674, #542
    /// protocol-input-first). Returns `None` when nothing was buffered, so the
    /// shell reads; a datagram framer is always cleared and drains nothing.
    /// A genuine framing failure on the buffered bytes still surfaces as `Err`
    /// and poisons.
    pub(crate) fn drain_buffered(
        &mut self,
        buffers: &mut OwnerBuffers,
        frame_limit: usize,
    ) -> Result<Option<FrameBatch>, Error> {
        let buffered = self.decode(buffers, 0, frame_limit)?;
        if !buffered.is_empty() || buffers.discarded_malformed() > 0 {
            Ok(Some(buffered))
        } else {
            buffers.recycle_frames(buffered);
            Ok(None)
        }
    }
}

#[cfg(any(feature = "async", feature = "blocking"))]
impl RetainedStreamInput for AdapterFraming {
    fn retained(&self) -> Option<RetainedInput> {
        if !self.is_stream() {
            return None;
        }
        let (length, evidence) = match &self.framing {
            EnvelopeFraming::Raw { framer, .. } => {
                let input = framer.buffered_raw_incomplete_prefix(|source| {
                    response_target_for_raw_prefix(self.routing, source)
                })?;
                let evidence = match input {
                    RawBufferedInput::Complete => RetainedEvidence::Complete,
                    RawBufferedInput::Malformed => RetainedEvidence::Malformed,
                    RawBufferedInput::Incomplete { target, kind } => {
                        RetainedEvidence::Incomplete { target, kind }
                    }
                };
                (framer.buffered_len(), evidence)
            }
            EnvelopeFraming::Sony { framer, .. } => {
                (framer.buffered_len(), RetainedEvidence::NotRaw)
            }
        };
        Some(RetainedInput {
            len: NonZeroUsize::new(length)?,
            evidence,
        })
    }

    fn discard_first(&mut self) -> Result<(), Error> {
        if let EnvelopeFraming::Raw { framer, .. } = &mut self.framing {
            framer.discard_first_input();
        }
        Ok(())
    }
}

/// Decode one received chunk using immutable multi-target routing state.
pub(crate) fn decode_frames_with_routing<E: AdapterEnvelope>(
    envelope: &E,
    framer: &mut E::Framer,
    routing: RoutingState,
    buffers: &mut OwnerBuffers,
    received: usize,
    frame_limit: usize,
    transport: TransportKind,
) -> Result<FrameBatch, Error> {
    let datagram = transport == TransportKind::Datagram;
    buffers.set_discarded_malformed(0);
    if datagram {
        framer.clear();
    }
    if received > buffers.receive_mut().len() {
        return Err(Error::InvalidResponse {
            expected: Cow::Borrowed("transport read fitting the owner receive buffer"),
            actual: received.to_le_bytes().to_vec(),
        });
    }
    if received == 0 && (datagram || !framer.has_buffered_data()) {
        return Ok(FrameBatch::new());
    }
    let mut frames = buffers.take_frames();
    let mut discarded = 0_usize;
    let result = (|| {
        framer.push_slice(&buffers.receive_mut()[..received])?;
        loop {
            if !datagram && frames.len() >= frame_limit {
                break;
            }
            let Some(next) =
                framer.visit_next_frame(|frame| decode_frame(envelope, routing, frame))
            else {
                break;
            };
            if datagram && frames.len() >= frame_limit {
                return Err(Error::invalid_response(
                    format!("a datagram within the per-receive limit of {frame_limit} frames"),
                    Vec::new(),
                ));
            }
            let decoded = match next {
                Ok(decoded) => decoded,
                Err(FramingError::OversizedRawFrame { .. }) if !datagram => {
                    discarded = discarded.saturating_add(1);
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            match decoded {
                Ok(Some(frame)) => frames.push_back(frame),
                Ok(None) => {}
                Err(error) if datagram => return Err(error),
                Err(_) => {
                    discarded = discarded.saturating_add(1);
                }
            }
        }
        if datagram && framer.has_buffered_data() {
            return Err(Error::invalid_response(
                "complete VISCA frame at datagram boundary",
                Vec::new(),
            ));
        }
        Ok(())
    })();
    if datagram {
        framer.clear();
    }
    match result {
        Ok(()) => {
            buffers.set_discarded_malformed(discarded);
            Ok(frames)
        }
        Err(error) => {
            buffers.recycle_frames(frames);
            Err(error)
        }
    }
}

fn decode_frame<E: AdapterEnvelope>(
    envelope: &E,
    routing: RoutingState,
    framed: impl AsRef<[u8]>,
) -> Result<Option<DecodedFrame>, Error> {
    let framed = framed.as_ref();
    let (payload, meta) = match envelope.extract_owner(framed)? {
        OwnerResponse::Visca { payload, meta } => (payload, meta),
        OwnerResponse::SonyControl { code } => {
            let Some(target) = routing.targets().sole_target() else {
                return Ok(None);
            };
            return Ok(Some(DecodedFrame {
                target,
                sequence: None,
                response: DecodedResponse::SonyControl { code },
            }));
        }
    };
    let Some(target) = decode_response_target(routing, payload)? else {
        // An IP source is ambiguous when more than one target is registered;
        // drop it without falling back to any configured target.
        return Ok(None);
    };
    let sequence = meta.sequence().map(|sequence| match sequence {
        FrameSequence::Full32(value) => EnvelopeSequence {
            value,
            width: SequenceWidth::Full32,
        },
        FrameSequence::MaybeTruncated(value) => EnvelopeSequence {
            value: u32::from(value),
            width: SequenceWidth::Lower16,
        },
    });
    if !routing.targets().contains(target) {
        // Preserve a source-valid but unregistered serial frame as an inert
        // frame. This keeps async receive alive while ensuring the engine
        // cannot attribute it to any registered target or request.
        return Ok(Some(DecodedFrame {
            target,
            sequence,
            response: DecodedResponse::Unknown,
        }));
    }
    let response = decode_response(target, payload)?;
    Ok(Some(DecodedFrame {
        target,
        sequence,
        response,
    }))
}

/// Classify a VISCA response after the owner boundary has established its
/// target. The shared parser borrows the original frame; no normalization copy
/// or second source-address validation is required.
fn decode_response(target: TargetSlot, payload: &[u8]) -> Result<DecodedResponse, Error> {
    let Some(basic) = decode_basic_for_source(payload, target.camera()) else {
        return Err(Error::InvalidResponse {
            expected: Cow::Borrowed("valid VISCA response"),
            actual: payload.to_vec(),
        });
    };
    Ok(match basic.kind {
        // The socket nibble is optional on the wire. A camera that answers
        // `90 40 FF` / `90 50 FF` is well formed, and deciding what an absent
        // socket means is the scheduler's job, not the transport adapter's:
        // an ACK takes the first free socket, and a completion is attributed
        // by sequence or by sole socket ownership. Rejecting the frame here
        // would kill an otherwise healthy session over real hardware.
        BasicKind::Ack { socket } => DecodedResponse::Ack { socket },
        BasicKind::Completion { socket } => DecodedResponse::Completion { socket },
        BasicKind::DataReply(payload) => {
            let mut owned = SmallVec::<[u8; INLINE_BYTES]>::new();
            owned.extend_from_slice(payload.as_slice());
            DecodedResponse::InquiryReply { payload: owned }
        }
        BasicKind::Error { socket, code } => DecodedResponse::Error { socket, code },
        BasicKind::NetworkChange => DecodedResponse::NetworkChange,
        BasicKind::Unknown(_) => DecodedResponse::Unknown,
    })
}

/// Strictly decode the source byte of a VISCA response.
///
/// `decode_basic` intentionally retains its historical broad `0x9x` header
/// behavior for protocol compatibility. The owner boundary is stricter: serial
/// replies use exactly `0x90`, `0xa0`, ..., `0xf0` for camera 1 through 7, while IP and
/// Sony replies use exactly `0x90` and can be attributed only to one registered
/// target.
pub(crate) fn decode_response_target(
    routing: RoutingState,
    payload: &[u8],
) -> Result<Option<TargetSlot>, Error> {
    let Some(&source) = payload.first() else {
        return Err(invalid_source(payload));
    };
    match classify_response_source(routing, source) {
        ResponseSource::Target(target) => Ok(Some(target)),
        ResponseSource::Ambiguous => Ok(None),
        ResponseSource::Invalid => Err(invalid_source(payload)),
    }
}

/// Resolve a raw-prefix source without turning malformed input into an error.
///
/// An incomplete frame has no decoder context, so it must be discarded when
/// its start cannot use the exact strict source rule that complete responses
/// use. Returning `None` keeps both owner shells structurally unable to route
/// an invalid prefix into `ShutdownReason::FramingFailure` (#745).
pub(crate) fn response_target_for_raw_prefix(
    routing: RoutingState,
    source: u8,
) -> Option<TargetSlot> {
    match classify_response_source(routing, source) {
        ResponseSource::Target(target) => Some(target),
        ResponseSource::Ambiguous | ResponseSource::Invalid => None,
    }
}

/// A source-byte classification shared by complete-frame decoding and the
/// incomplete raw-prefix path.  Complete decoding preserves its established
/// error/ambiguous behavior; prefixes convert the latter two outcomes into a
/// discard verdict before they escape the framer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResponseSource {
    Target(TargetSlot),
    Ambiguous,
    Invalid,
}

fn classify_response_source(routing: RoutingState, source: u8) -> ResponseSource {
    match routing.addressing() {
        AddressingMode::Serial => {
            // The low nibble is reserved in a serial source byte. Accept only
            // camera sources 0x90..=0xf0 on a 0x10 boundary; 0x80/0x88 are
            // controller/broadcast addresses and never response sources.
            if source & 0x0f != 0 || !(0x90..=0xf0).contains(&source) {
                return ResponseSource::Invalid;
            }
            let id = (source >> 4).saturating_sub(8);
            CameraId::new(id)
                .and_then(TargetSlot::try_from)
                .map_or(ResponseSource::Invalid, ResponseSource::Target)
        }
        AddressingMode::Ip => {
            // #590/#598/#681: an IP VISCA reply carries no routable camera source, so it is
            // attributable only when exactly one target is registered. A camera configured with a
            // non-default chain address answers with *that* address (e.g. `0xA0` for VISCA address
            // 2) even on a single-target IP session. Attribution there is camera-blind: with one
            // target there is nothing to disambiguate, so for exactly one registered target the
            // engine accepts any VISCA reply source (high nibble `0x9..=0xF`) and binds it to the
            // sole target. `decode_basic` already accepts the whole `0x9y..=0xFy` range, so the
            // classifier that follows still reads the frame.
            match routing.targets().sole_target() {
                Some(target) => {
                    if source < 0x90 {
                        // Below `0x90` is the controller/broadcast range
                        // (`0x80`-`0x8F`), never a reply source.
                        return ResponseSource::Invalid;
                    }
                    ResponseSource::Target(target)
                }
                None => {
                    // More than one target registered: an IP source cannot
                    // disambiguate between them, so keep the strict `0x90` check
                    // and drop anything ambiguous without guessing a target.
                    if source != 0x90 {
                        return ResponseSource::Invalid;
                    }
                    ResponseSource::Ambiguous
                }
            }
        }
    }
}

fn invalid_source(payload: &[u8]) -> Error {
    Error::InvalidResponse {
        expected: Cow::Borrowed("VISCA response with a strict camera source"),
        actual: payload.to_vec(),
    }
}

/// Convert profile envelope facts to the camera-level transport kind for
/// callers that need to validate a standard transport before opening it.
pub(crate) fn profile_supports_transport(
    profile: &ProfileSpec,
    kind: Option<CameraTransportKind>,
) -> bool {
    let Some(kind) = kind else {
        // Custom transports intentionally remain an explicit escape hatch.
        return true;
    };
    match kind {
        CameraTransportKind::Tcp => profile.transports().tcp_port().is_some(),
        CameraTransportKind::Udp => profile.transports().udp_port().is_some(),
        CameraTransportKind::Serial => profile.transports().supports_serial(),
        CameraTransportKind::Custom => true,
    }
}

/// Validate a standard transport kind against a runtime profile before any
/// owner or transport startup work occurs.  A transport that does not expose
/// a standard kind is an explicit custom escape hatch and remains allowed.
pub(crate) fn validate_profile_transport(
    profile: &ProfileSpec,
    kind: Option<CameraTransportKind>,
) -> Result<(), Error> {
    if profile_supports_transport(profile, kind) {
        return Ok(());
    }

    let Some(kind) = kind else {
        // `profile_supports_transport` currently returns true for this case;
        // keep the branch defensive if that policy ever changes.
        return Ok(());
    };
    if let Some(profile_id) = profile.capabilities().profile_id {
        return Err(Error::UnsupportedTransport {
            profile: profile_id,
            transport: kind,
        });
    }
    Err(Error::InvalidRequest(
        format!("profile does not support {kind} transport").into(),
    ))
}

/// Validate the immutable topology of a profile registry against one
/// transport's addressing facts.
///
/// Multi-target response routing is defined only for raw VISCA over serial:
/// serial replies carry a source camera ID, while IP replies do not.  Keeping
/// this check after per-profile standard-transport validation but before owner
/// policy construction gives callers deterministic preflight errors without
/// spawning an actor or touching transport I/O. A `None` addressing value is
/// the conservative answer for an unknown/custom transport and therefore
/// cannot authorize a multi-target registry.
pub(crate) fn validate_profile_registry_topology(
    profiles: &[(TargetSlot, &ProfileSpec)],
    kind: Option<CameraTransportKind>,
    addressing: Option<AddressingMode>,
) -> Result<(), Error> {
    if profiles.len() <= 1 {
        return Ok(());
    }
    if matches!(
        kind,
        Some(CameraTransportKind::Tcp | CameraTransportKind::Udp)
    ) {
        return Err(Error::NotSupported);
    }
    if profiles
        .iter()
        .any(|(_, profile)| profile.envelope() != ProfileEnvelope::RawVisca)
    {
        return Err(Error::NotSupported);
    }
    if addressing != Some(AddressingMode::Serial) {
        return Err(Error::NotSupported);
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{
        profile::ProfileSpec,
        profiles::{GenericVisca, PtzOpticsG2, SonyFR7},
        runtime::engine::DecodedResponse,
        runtime::owner::OwnerLimits,
        transport::envelope::{RawVisca, SonyEncapsulated},
    };

    fn all_targets() -> TargetRegistry {
        TargetRegistry::from_targets(&[
            TargetSlot::CAMERA_1,
            TargetSlot::CAMERA_2,
            TargetSlot::CAMERA_3,
            TargetSlot::CAMERA_4,
            TargetSlot::CAMERA_5,
            TargetSlot::CAMERA_6,
            TargetSlot::CAMERA_7,
        ])
        .unwrap()
    }

    #[test]
    fn serial_source_bytes_map_exactly_to_all_seven_targets() {
        let routing = RoutingState::new(AddressingMode::Serial, all_targets());
        for (source, expected) in [
            (0x90, TargetSlot::CAMERA_1),
            (0xa0, TargetSlot::CAMERA_2),
            (0xb0, TargetSlot::CAMERA_3),
            (0xc0, TargetSlot::CAMERA_4),
            (0xd0, TargetSlot::CAMERA_5),
            (0xe0, TargetSlot::CAMERA_6),
            (0xf0, TargetSlot::CAMERA_7),
        ] {
            assert!(matches!(
                decode_response_target(routing, &[source]),
                Ok(Some(actual)) if actual == expected
            ));
        }
    }

    #[test]
    fn sony_control_reply_is_inert_but_preserves_its_diagnostic_code() {
        let envelope = SonyEncapsulated::new(AddressingMode::Ip);
        let routing = RoutingState::new(
            AddressingMode::Ip,
            TargetRegistry::single(TargetSlot::CAMERA_1).unwrap(),
        );
        let framed = Bytes::from_static(&[
            0x02, 0x01, // control reply
            0x00, 0x02, // two-byte payload
            0x12, 0x34, 0x56, 0x78, // ignored control sequence
            0x0F, 0x01, // sequence-number error
        ]);

        let decoded = decode_frame(&envelope, routing, framed)
            .expect("valid Sony control reply")
            .expect("sole-target Sony reply");
        assert_eq!(decoded.target, TargetSlot::CAMERA_1);
        assert_eq!(decoded.sequence, None);
        assert_eq!(
            decoded.response,
            DecodedResponse::SonyControl { code: 0x0F01 }
        );
    }

    #[test]
    fn serial_source_rejects_low_nibble_non_camera_and_broadcast_bytes() {
        let routing = RoutingState::new(AddressingMode::Serial, all_targets());
        for source in [0x00, 0x70, 0x80, 0x88, 0x91, 0xa1, 0xff] {
            assert!(
                decode_response_target(routing, &[source]).is_err(),
                "{source:#x}"
            );
        }
    }

    #[test]
    fn camera_two_ack_and_error_retain_source_and_socket() {
        let envelope = RawVisca::new(AddressingMode::Serial);
        let routing = RoutingState::new(AddressingMode::Serial, all_targets());
        let ack = decode_frame(&envelope, routing, Bytes::from_static(&[0xa0, 0x42, 0xff]))
            .unwrap()
            .unwrap();
        assert_eq!(ack.target, TargetSlot::CAMERA_2);
        assert!(matches!(
            ack.response,
            DecodedResponse::Ack {
                socket: Some(crate::ViscaSocket::S2)
            }
        ));

        let error = decode_frame(
            &envelope,
            routing,
            Bytes::from_static(&[0xa0, 0x61, 0x04, 0xff]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(error.target, TargetSlot::CAMERA_2);
        assert!(matches!(
            error.response,
            DecodedResponse::Error {
                socket: Some(crate::ViscaSocket::S1),
                code: 0x04
            }
        ));
    }

    /// Issue #590/#598/#681: an IP session has exactly one registered target, so
    /// any VISCA reply source is attributed to that sole target
    /// (camera-blind attribution). A camera configured with a non-default chain
    /// address answers with *that* address (e.g. `0xA0` for VISCA address 2), and
    /// that reply must still settle the sole outstanding command rather than
    /// poisoning the session. Only lead bytes below `0x90` (controller/broadcast)
    /// are rejected.
    #[test]
    fn ip_single_target_attributes_any_reply_source_to_the_sole_target() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let routing = RoutingState::new(
            AddressingMode::Ip,
            TargetRegistry::single(TargetSlot::CAMERA_2).unwrap(),
        );
        // The default `0x90` reply attributes to the sole target.
        let default_source =
            decode_frame(&envelope, routing, Bytes::from_static(&[0x90, 0x42, 0xff]))
                .unwrap()
                .unwrap();
        assert_eq!(default_source.target, TargetSlot::CAMERA_2);
        // A camera answering with its chain address `0xA0` still attributes to
        // the sole target, and its ACK still classifies (socket S2).
        let chain_source =
            decode_frame(&envelope, routing, Bytes::from_static(&[0xa0, 0x42, 0xff]))
                .expect(
                    "a chain-address reply on a single-target IP session is not a decode failure",
                )
                .expect("a single-target IP session attributes any reply source");
        assert_eq!(chain_source.target, TargetSlot::CAMERA_2);
        assert!(matches!(
            chain_source.response,
            DecodedResponse::Ack {
                socket: Some(crate::ViscaSocket::S2)
            }
        ));
        // A controller/broadcast lead byte is never a reply source.
        assert!(decode_frame(&envelope, routing, Bytes::from_static(&[0x80, 0x42, 0xff])).is_err());
    }

    /// The strict `0x90` check is retained when more than one target is
    /// registered: an IP source cannot disambiguate between targets there, so a
    /// non-`0x90` source is rejected and a bare `0x90` is dropped without
    /// guessing a target (this configuration is rejected at session construction,
    /// so the branch is defensive).
    #[test]
    fn ip_multi_target_keeps_the_strict_source_check() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let routing = RoutingState::new(
            AddressingMode::Ip,
            TargetRegistry::from_targets(&[TargetSlot::CAMERA_1, TargetSlot::CAMERA_2]).unwrap(),
        );
        // A bare `0x90` reply is ambiguous across the registered targets: it is
        // dropped (Ok(None)) rather than attributed to a guessed target.
        assert!(
            decode_frame(&envelope, routing, Bytes::from_static(&[0x90, 0x41, 0xff]))
                .unwrap()
                .is_none()
        );
        // A non-`0x90` source stays a hard error with more than one target.
        assert!(decode_frame(&envelope, routing, Bytes::from_static(&[0xa0, 0x41, 0xff])).is_err());
    }

    #[test]
    fn valid_unregistered_serial_source_becomes_inert_unknown_frame() {
        let envelope = RawVisca::new(AddressingMode::Serial);
        let routing = RoutingState::new(
            AddressingMode::Serial,
            TargetRegistry::single(TargetSlot::CAMERA_1).unwrap(),
        );
        let frame = decode_frame(&envelope, routing, Bytes::from_static(&[0xa0, 0x42, 0xff]))
            .unwrap()
            .unwrap();
        assert_eq!(frame.target, TargetSlot::CAMERA_2);
        assert!(matches!(frame.response, DecodedResponse::Unknown));
    }

    /// Issue #565: `90 40 FF` / `90 50 FF` carry no socket nibble. They are
    /// well-formed VISCA and must reach the scheduler with `socket: None`
    /// rather than failing the whole session with `InvalidResponse`.
    #[test]
    fn socketless_ack_and_completion_decode_without_a_socket() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let routing = RoutingState::new(
            AddressingMode::Ip,
            TargetRegistry::single(TargetSlot::CAMERA_1).unwrap(),
        );
        let ack = decode_frame(&envelope, routing, Bytes::from_static(&[0x90, 0x40, 0xff]))
            .expect("a socketless ACK is not a decode failure")
            .expect("a socketless ACK is attributable");
        assert_eq!(ack.target, TargetSlot::CAMERA_1);
        assert!(matches!(
            ack.response,
            DecodedResponse::Ack { socket: None }
        ));

        let completion = decode_frame(&envelope, routing, Bytes::from_static(&[0x90, 0x50, 0xff]))
            .expect("a socketless completion is not a decode failure")
            .expect("a socketless completion is attributable");
        assert!(matches!(
            completion.response,
            DecodedResponse::Completion { socket: None }
        ));

        // The socket nibble is still carried through when the camera sends one.
        let acked = decode_frame(&envelope, routing, Bytes::from_static(&[0x90, 0x42, 0xff]))
            .unwrap()
            .unwrap();
        assert!(matches!(
            acked.response,
            DecodedResponse::Ack {
                socket: Some(crate::ViscaSocket::S2)
            }
        ));
    }

    /// The other direction of the same contract: tolerating an absent socket
    /// does not weaken frame validation.
    #[test]
    fn malformed_frames_are_still_rejected() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let routing = RoutingState::new(
            AddressingMode::Ip,
            TargetRegistry::single(TargetSlot::CAMERA_1).unwrap(),
        );
        for frame in [
            // Too short to be any VISCA response.
            &[0x90, 0xff][..],
            // An error frame with no error code.
            &[0x90, 0x61, 0xff][..],
        ] {
            assert!(
                decode_frame(&envelope, routing, Bytes::copy_from_slice(frame)).is_err(),
                "{frame:02x?} must remain a decode failure"
            );
        }
        // A controller address is never a response source.
        assert!(decode_frame(&envelope, routing, Bytes::from_static(&[0x80, 0x40, 0xff])).is_err());
    }

    #[test]
    fn datagram_partial_bytes_are_discarded_before_the_next_datagram() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let routing = RoutingState::new(
            AddressingMode::Ip,
            TargetRegistry::single(TargetSlot::CAMERA_1).unwrap(),
        );
        let mut framer = RawFramer::new_with_limits(32, 32, 32);
        let mut buffers = OwnerBuffers::new(OwnerLimits::default()).unwrap();

        buffers.receive_mut()[..2].copy_from_slice(&[0x90, 0x41]);
        assert!(matches!(
            decode_frames_with_routing(
                &envelope,
                &mut framer,
                routing,
                &mut buffers,
                2,
                4,
                TransportKind::Datagram,
            ),
            Err(Error::InvalidResponse { .. })
        ));
        assert!(!framer.has_buffered_data());

        buffers.receive_mut()[..3].copy_from_slice(&[0x90, 0x41, 0xff]);
        let frames = decode_frames_with_routing(
            &envelope,
            &mut framer,
            routing,
            &mut buffers,
            3,
            4,
            TransportKind::Datagram,
        )
        .expect("a valid later datagram must not inherit the partial prefix");
        assert_eq!(frames.len(), 1);
        assert!(!framer.has_buffered_data());
    }

    #[test]
    fn datagram_frame_batch_overflow_discards_the_whole_datagram() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let routing = RoutingState::new(
            AddressingMode::Ip,
            TargetRegistry::single(TargetSlot::CAMERA_1).unwrap(),
        );
        let mut framer = RawFramer::new_with_limits(32, 32, 32);
        let mut buffers = OwnerBuffers::new(OwnerLimits::default()).unwrap();
        buffers.receive_mut()[..6].copy_from_slice(&[0x90, 0x41, 0xff, 0x90, 0x51, 0xff]);
        assert!(matches!(
            decode_frames_with_routing(
                &envelope,
                &mut framer,
                routing,
                &mut buffers,
                6,
                1,
                TransportKind::Datagram,
            ),
            // A frame-count limit is reported as an invalid (over-limit) batch,
            // not the byte-labeled `ResponseTooLarge` (#674).
            Err(Error::InvalidResponse { .. })
        ));
        assert!(!framer.has_buffered_data());

        buffers.receive_mut()[..3].copy_from_slice(&[0x90, 0x51, 0xff]);
        assert_eq!(
            decode_frames_with_routing(
                &envelope,
                &mut framer,
                routing,
                &mut buffers,
                3,
                1,
                TransportKind::Datagram,
            )
            .unwrap()
            .len(),
            1
        );
    }

    #[test]
    fn stream_cumulative_framer_overflow_is_reported_without_resync() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let routing = RoutingState::new(
            AddressingMode::Ip,
            TargetRegistry::single(TargetSlot::CAMERA_1).unwrap(),
        );
        let mut framer = RawFramer::new_with_limits(8, 32, 5);
        let mut buffers = OwnerBuffers::new(OwnerLimits::default()).unwrap();

        buffers.receive_mut()[..4].copy_from_slice(&[0x90, 0x41, 0x00, 0x00]);
        assert!(decode_frames_with_routing(
            &envelope,
            &mut framer,
            routing,
            &mut buffers,
            4,
            4,
            TransportKind::Stream,
        )
        .unwrap()
        .is_empty());
        buffers.receive_mut()[..2].copy_from_slice(&[0x00, 0x00]);
        assert!(matches!(
            decode_frames_with_routing(
                &envelope,
                &mut framer,
                routing,
                &mut buffers,
                2,
                4,
                TransportKind::Stream,
            ),
            Err(Error::ResponseTooLarge { max_size: 5 })
        ));
        assert!(framer.has_buffered_data());
    }

    /// Issue #672: a delimited-but-unclassifiable frame on a stream is discarded
    /// as malformed and counted, not turned into a framing failure. Decoding
    /// continues with the following well-formed frame, and the framer keeps its
    /// place, so the session (which reads this result) is never poisoned.
    #[test]
    fn stream_malformed_frame_is_discarded_and_counted_not_poisoned() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let routing = RoutingState::new(
            AddressingMode::Ip,
            TargetRegistry::single(TargetSlot::CAMERA_1).unwrap(),
        );
        let mut framer = RawFramer::new_with_limits(64, 64, 64);
        let mut buffers = OwnerBuffers::new(OwnerLimits::default()).unwrap();

        // A padded ACK (four bytes where an ACK is exactly three) followed by a
        // well-formed completion, in one stream read.
        let bytes = [0x90, 0x41, 0x00, 0xff, 0x90, 0x51, 0xff];
        buffers.receive_mut()[..bytes.len()].copy_from_slice(&bytes);
        let frames = decode_frames_with_routing(
            &envelope,
            &mut framer,
            routing,
            &mut buffers,
            bytes.len(),
            8,
            TransportKind::Stream,
        )
        .expect("a malformed stream frame is discarded, not a framing failure");
        assert_eq!(frames.len(), 1, "only the well-formed completion survives");
        assert!(matches!(
            frames[0].response,
            DecodedResponse::Completion { .. }
        ));
        assert_eq!(
            buffers.take_discarded_malformed(),
            1,
            "the padded ACK is reported as one discarded malformed frame"
        );
        assert!(!framer.has_buffered_data());
    }

    /// Issue #672 on a datagram is unchanged: a datagram is atomic, so a single
    /// malformed frame in it discards the whole datagram as an `Err` the owner
    /// turns into `Ignored(MalformedFrame)` — it never decodes the good frames
    /// around it.
    #[test]
    fn datagram_malformed_frame_still_discards_the_whole_datagram() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let routing = RoutingState::new(
            AddressingMode::Ip,
            TargetRegistry::single(TargetSlot::CAMERA_1).unwrap(),
        );
        let mut framer = RawFramer::new_with_limits(64, 64, 64);
        let mut buffers = OwnerBuffers::new(OwnerLimits::default()).unwrap();
        let bytes = [0x90, 0x41, 0x00, 0xff, 0x90, 0x51, 0xff];
        buffers.receive_mut()[..bytes.len()].copy_from_slice(&bytes);
        assert!(matches!(
            decode_frames_with_routing(
                &envelope,
                &mut framer,
                routing,
                &mut buffers,
                bytes.len(),
                8,
                TransportKind::Datagram,
            ),
            Err(Error::InvalidResponse { .. })
        ));
        assert!(!framer.has_buffered_data());
    }

    /// Issue #674: a stream read that decodes more than the per-receive frame
    /// limit stops at the limit and leaves the remaining complete frames
    /// buffered for the next receive (drained here with a zero-length read),
    /// rather than failing with `ResponseTooLarge` and poisoning the session.
    #[test]
    fn stream_over_limit_stops_at_limit_and_buffers_remainder() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let routing = RoutingState::new(
            AddressingMode::Ip,
            TargetRegistry::single(TargetSlot::CAMERA_1).unwrap(),
        );
        let mut framer = RawFramer::new_with_limits(64, 64, 64);
        let mut buffers = OwnerBuffers::new(OwnerLimits::default()).unwrap();

        // Three well-formed frames in one read, with a per-receive limit of two.
        let bytes = [0x90, 0x41, 0xff, 0x90, 0x51, 0xff, 0x90, 0x38, 0xff];
        buffers.receive_mut()[..bytes.len()].copy_from_slice(&bytes);
        let first = decode_frames_with_routing(
            &envelope,
            &mut framer,
            routing,
            &mut buffers,
            bytes.len(),
            2,
            TransportKind::Stream,
        )
        .expect("an over-limit stream read is not a framing failure");
        assert_eq!(first.len(), 2, "the read stops at the frame limit");
        assert_eq!(buffers.take_discarded_malformed(), 0);
        assert!(
            framer.has_buffered_data(),
            "the third frame is left buffered for the next receive"
        );

        // A zero-length read drains the buffered remainder without new bytes.
        let second = decode_frames_with_routing(
            &envelope,
            &mut framer,
            routing,
            &mut buffers,
            0,
            2,
            TransportKind::Stream,
        )
        .expect("draining the remainder is not a framing failure");
        assert_eq!(second.len(), 1, "the buffered remainder is drained next");
        assert!(matches!(second[0].response, DecodedResponse::NetworkChange));
        assert!(!framer.has_buffered_data());
    }

    /// Issue #674 boundary: a stream read of exactly the per-receive frame limit
    /// decodes every frame and leaves nothing buffered — the limit itself is not
    /// an error.
    #[test]
    fn stream_exactly_frame_limit_decodes_all_and_buffers_nothing() {
        let envelope = RawVisca::new(AddressingMode::Ip);
        let routing = RoutingState::new(
            AddressingMode::Ip,
            TargetRegistry::single(TargetSlot::CAMERA_1).unwrap(),
        );
        let mut framer = RawFramer::new_with_limits(64, 64, 64);
        let mut buffers = OwnerBuffers::new(OwnerLimits::default()).unwrap();

        let bytes = [0x90, 0x41, 0xff, 0x90, 0x51, 0xff];
        buffers.receive_mut()[..bytes.len()].copy_from_slice(&bytes);
        let frames = decode_frames_with_routing(
            &envelope,
            &mut framer,
            routing,
            &mut buffers,
            bytes.len(),
            2,
            TransportKind::Stream,
        )
        .expect("exactly the frame limit is not a framing failure");
        assert_eq!(frames.len(), 2);
        assert!(!framer.has_buffered_data());
    }

    #[test]
    fn multi_target_policy_retains_target_local_facts_and_shared_bounds() {
        let profile = ProfileSpec::from_compile_time::<GenericVisca>().unwrap();
        let profiles = [
            (TargetSlot::CAMERA_1, &profile),
            (TargetSlot::CAMERA_2, &profile),
        ];
        let config = TransportConfig {
            addressing: AddressingMode::Serial,
            ..TransportConfig::default()
        };
        let policy = owner_policy_for_targets(&profiles, &config, SendSemantics::Stream).unwrap();
        assert!(policy.targets[TargetSlot::CAMERA_1].is_some());
        assert!(policy.targets[TargetSlot::CAMERA_2].is_some());
        assert_eq!(policy.protocol.inquiry_capacity, 1);
        assert_eq!(
            policy.protocol.command_spacing,
            profile.timing().minimum_command_spacing()
        );
    }

    /// Production raw owners always enter the engine with one inquiry flight,
    /// even when their admission queue is wider. Raw replies carry no request
    /// identity, so the engine's bounded released-inquiry quarantine relies on
    /// this topology; sequenced Sony sessions alone may use the wider capacity.
    #[test]
    fn production_raw_policy_is_single_flight_while_sony_uses_admission_capacity() {
        let raw = ProfileSpec::from_compile_time::<GenericVisca>().unwrap();
        let sony = ProfileSpec::from_compile_time::<SonyFR7>().unwrap();
        let raw_profiles = [(TargetSlot::CAMERA_1, &raw)];
        let sony_profiles = [(TargetSlot::CAMERA_1, &sony)];
        let capacity = NonZeroUsize::new(3).unwrap();
        let config = TransportConfig::default();

        let raw_policy = owner_policy_for_targets_with_tuning(
            &raw_profiles,
            &config,
            SendSemantics::Datagram,
            crate::session_config::ValidatedTuning::validate(OperationalTuning::new(), [&raw])
                .unwrap(),
            capacity,
            false,
        )
        .unwrap();
        assert_eq!(raw_policy.protocol.wire.kind(), EnvelopeKind::Raw);
        assert_eq!(raw_policy.protocol.inquiry_capacity, 1);
        assert_eq!(
            raw_policy.protocol.raw_inquiry_release_hold,
            raw.timing().raw_inquiry_reply_skew()
        );

        let sony_policy = owner_policy_for_targets_with_tuning(
            &sony_profiles,
            &config,
            SendSemantics::Datagram,
            crate::session_config::ValidatedTuning::validate(OperationalTuning::new(), [&sony])
                .unwrap(),
            capacity,
            false,
        )
        .unwrap();
        assert_eq!(sony_policy.protocol.wire.kind(), EnvelopeKind::Sony);
        assert_eq!(sony_policy.protocol.inquiry_capacity, capacity.get());
    }

    #[test]
    fn multi_target_policy_uses_strictest_compatible_profile_pacing() {
        let relaxed = ProfileSpec::from_compile_time::<GenericVisca>().unwrap();
        let strict = ProfileSpec::from_compile_time::<PtzOpticsG2>().unwrap();
        let profiles = [
            (TargetSlot::CAMERA_1, &relaxed),
            (TargetSlot::CAMERA_2, &strict),
        ];
        let config = TransportConfig {
            addressing: AddressingMode::Serial,
            ..TransportConfig::default()
        };
        let policy = owner_policy_for_targets(&profiles, &config, SendSemantics::Stream).unwrap();
        assert_eq!(
            policy.protocol.command_spacing,
            strict.timing().minimum_command_spacing()
        );
        assert_eq!(
            policy.protocol.inquiry_spacing,
            strict.timing().minimum_inquiry_spacing()
        );
    }

    #[test]
    fn buffer_policy_accepts_equal_receive_and_maximum_sizes() {
        let profile = ProfileSpec::from_compile_time::<GenericVisca>().unwrap();
        let profiles = [(TargetSlot::CAMERA_1, &profile)];
        let config = TransportConfig {
            buffer_config: crate::transport::BufferConfig {
                recv_buffer_size: 64,
                max_buffer_size: 64,
            },
            ..TransportConfig::default()
        };

        for semantics in [SendSemantics::Datagram, SendSemantics::Stream] {
            let policy = owner_policy_for_targets(&profiles, &config, semantics)
                .expect("equal receive and maximum sizes are valid");
            assert_eq!(policy.limits.receive_bytes, 64);
        }
    }

    #[test]
    fn buffer_policy_rejects_receive_size_larger_than_maximum() {
        let profile = ProfileSpec::from_compile_time::<GenericVisca>().unwrap();
        let profiles = [(TargetSlot::CAMERA_1, &profile)];
        let config = TransportConfig {
            buffer_config: crate::transport::BufferConfig {
                recv_buffer_size: 65,
                max_buffer_size: 64,
            },
            ..TransportConfig::default()
        };

        for semantics in [SendSemantics::Datagram, SendSemantics::Stream] {
            let error = owner_policy_for_targets(&profiles, &config, semantics)
                .expect_err("receive size larger than maximum must be rejected");
            assert!(matches!(
                error,
                Error::InvalidRequest(message)
                    if message.as_ref() == "transport receive buffer cannot exceed maximum buffer"
            ));
        }
    }
}

/// Production read lowering with transport I/O replaced by a supplied chunk.
#[cfg(feature = "test-utils")]
#[derive(Debug)]
pub(crate) struct ReceiveProbe {
    adapter: TransportAdapter<ProbeConfig, ()>,
    buffers: OwnerBuffers,
}

#[cfg(feature = "test-utils")]
#[derive(Debug)]
struct ProbeConfig(TransportConfig);

#[cfg(feature = "test-utils")]
impl HasTransportConfig for ProbeConfig {
    fn transport_config(&self) -> &TransportConfig {
        &self.0
    }
}

#[cfg(feature = "test-utils")]
impl ReceiveProbe {
    pub(crate) fn new(
        profile: &ProfileSpec,
        config: TransportConfig,
        semantics: SendSemantics,
    ) -> Result<Self, Error> {
        let transport = ProbeConfig(config);
        let target = TargetSlot::try_from(CameraId::CAMERA_1)?;
        let (framing, policy) = AdapterFraming::for_targets(
            &transport,
            Some(config.addressing),
            semantics,
            &[(target, profile)],
            crate::session_config::ValidatedTuning::validate(OperationalTuning::new(), [profile])?,
            crate::DEFAULT_ADMISSION_CAPACITY,
            false,
        )?;
        let buffers = OwnerBuffers::new(policy.limits)?;
        Ok(Self {
            adapter: TransportAdapter {
                transport,
                framing,
                policy,
                facade: std::marker::PhantomData,
            },
            buffers,
        })
    }

    pub(crate) fn receive(&mut self, chunk: &[u8]) -> Result<usize, Error> {
        let outcome = ReceiveOutcome::copy_message(chunk, self.buffers.receive_mut());
        let limit = self.adapter.policy.limits.frames_per_receive;
        match self
            .adapter
            .classify_read(&mut self.buffers, limit, Ok(outcome))
        {
            OwnerReceive::Frames(mut frames) => {
                let count = frames.len();
                while let Some(frame) = frames.pop_front() {
                    std::hint::black_box(frame);
                }
                self.buffers.recycle_frames(frames);
                Ok(count)
            }
            OwnerReceive::NoProgress(_) => Ok(0),
            OwnerReceive::Transient(error) => Err(error),
            OwnerReceive::Fatal(ReceiveFatal::TransportClosed { cause }) => {
                Err(cause.unwrap_or_else(|| Error::connection_closed(None)))
            }
            OwnerReceive::Fatal(ReceiveFatal::FramingFailure { cause }) => Err(cause),
        }
    }
}
