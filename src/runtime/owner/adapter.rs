//! Shared production transport-adapter pieces for the Phase-6 owner.
//!
//! The owner deliberately keeps transport I/O, envelope handling, framing, and
//! response classification at its boundary.  This module contains the small
//! amount of mode-independent work needed by the blocking and async adapters;
//! it does not own scheduling, settlement, or observer deadlines.

use std::borrow::Cow;

use bytes::Bytes;
use smallvec::SmallVec;

use crate::{
    camera::TransportKind as CameraTransportKind,
    command::CommandKind,
    profile::{OperationalTuning, ProfileEnvelope, ProfileSpec},
    protocol::{
        framer::ProtocolFramer,
        response::{decode_basic, BasicKind},
    },
    raw::INLINE_BYTES,
    runtime::engine::{
        CancellationPolicy, DecodedFrame, DecodedResponse, EnvelopeKind, EnvelopeSequence,
        ProtocolPolicy, SequenceWidth, TargetPolicy,
    },
    transport::{
        builder::{AddressingMode, TransportConfig},
        envelope::{Envelope, FrameMeta, RawVisca, SonyEncapsulated},
        SendSemantics,
    },
    CameraId, Error,
};

use super::{OwnerBuffers, OwnerPolicy};

/// Concrete envelope selected by a validated profile.
///
/// The envelope is kept in the adapter, rather than reconstructed for every
/// transmission, because Sony's sequence counter is session state.
#[derive(Debug, Clone)]
pub(crate) enum OwnerEnvelope {
    Raw(RawVisca),
    Sony(SonyEncapsulated),
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
    registered: [bool; 9],
    count: u8,
}

impl TargetRegistry {
    /// Creates a registry from individual camera targets.
    pub(crate) fn from_targets(targets: &[CameraId]) -> Result<Self, Error> {
        let mut registered = [false; 9];
        let mut count = 0_u8;
        if targets.is_empty() {
            return Err(Error::InvalidRequest(
                "owner target registry must contain at least one camera".into(),
            ));
        }
        for target in targets {
            if !(1..=7).contains(&target.id()) {
                return Err(Error::InvalidRequest(
                    "owner target registry requires individual cameras (1 through 7)".into(),
                ));
            }
            let slot = &mut registered[usize::from(target.id())];
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
    pub(crate) fn single(target: CameraId) -> Result<Self, Error> {
        Self::from_targets(&[target])
    }

    pub(crate) fn contains(self, target: CameraId) -> bool {
        self.registered[target.id() as usize]
    }

    pub(crate) const fn len(self) -> usize {
        self.count as usize
    }

    pub(crate) fn sole_target(self) -> Option<CameraId> {
        if self.count != 1 {
            return None;
        }
        let mut id = 1_u8;
        while id <= 7 {
            if self.registered[id as usize] {
                // The registry is built only through `CameraId`, so this
                // conversion is guaranteed to succeed.
                return CameraId::new(id).ok();
            }
            id += 1;
        }
        None
    }

    pub(crate) fn targets(self) -> [Option<CameraId>; 7] {
        [
            self.target_at(1),
            self.target_at(2),
            self.target_at(3),
            self.target_at(4),
            self.target_at(5),
            self.target_at(6),
            self.target_at(7),
        ]
    }

    fn target_at(self, id: u8) -> Option<CameraId> {
        if self.registered[id as usize] {
            CameraId::new(id).ok()
        } else {
            None
        }
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

impl OwnerEnvelope {
    pub(crate) fn from_profile(
        profile: &ProfileSpec,
        addressing: AddressingMode,
    ) -> Result<Self, Error> {
        match profile.envelope() {
            ProfileEnvelope::RawVisca => Ok(Self::Raw(RawVisca::new(addressing))),
            ProfileEnvelope::SonyEncapsulated => Ok(Self::Sony(SonyEncapsulated::new(addressing))),
        }
    }

    pub(crate) const fn kind(&self) -> EnvelopeKind {
        match self {
            Self::Raw(_) => EnvelopeKind::Raw,
            Self::Sony(_) => EnvelopeKind::Sony,
        }
    }

    pub(crate) const fn addressing(&self) -> AddressingMode {
        match self {
            Self::Raw(envelope) => envelope.addressing(),
            Self::Sony(envelope) => envelope.addressing(),
        }
    }

    pub(crate) const fn supports_sequence_correlation(&self) -> bool {
        match self {
            Self::Raw(_) => RawVisca::SUPPORTS_SEQUENCE_CORRELATION,
            Self::Sony(_) => SonyEncapsulated::SUPPORTS_SEQUENCE_CORRELATION,
        }
    }

    pub(crate) fn frame_into(
        &self,
        visca_bytes: &[u8],
        kind: CommandKind,
        out: &mut bytes::BytesMut,
    ) -> FrameMeta {
        match self {
            Self::Raw(envelope) => envelope.frame_into(visca_bytes, kind, out),
            Self::Sony(envelope) => envelope.frame_into(visca_bytes, kind, out),
        }
    }

    pub(crate) fn extract_with_meta(&self, framed: Bytes) -> Result<(Bytes, FrameMeta), Error> {
        match self {
            Self::Raw(envelope) => envelope.extract_with_meta(framed),
            Self::Sony(envelope) => envelope.extract_with_meta(framed),
        }
    }
}

/// Lower validated profile and transport facts into the immutable owner
/// policy.  Request-specific timeout/retry/control facts are lowered by
/// `prepared`; this function only supplies session-wide bounds and protocol
/// capabilities.
pub(crate) fn owner_policy_for(
    profile: &ProfileSpec,
    config: &TransportConfig,
    target: CameraId,
    semantics: SendSemantics,
) -> Result<OwnerPolicy, Error> {
    owner_policy_for_with_tuning(profile, config, target, semantics, OperationalTuning::new())
}

/// Lower validated profile, transport, and immutable session tuning facts into
/// one owner policy. Tuning is validated here as well as during request
/// preparation so a session cannot start with a weaker pacing or deadline
/// policy than the profile permits.
pub(crate) fn owner_policy_for_with_tuning(
    profile: &ProfileSpec,
    config: &TransportConfig,
    target: CameraId,
    semantics: SendSemantics,
    tuning: OperationalTuning,
) -> Result<OwnerPolicy, Error> {
    owner_policy_for_targets_with_tuning(&[(target, profile)], config, semantics, tuning)
}

/// Lower one immutable owner policy for a bounded set of target/profile
/// registrations. All profiles must use the same envelope because one
/// physical adapter owns one framer and one wire mode. Per-target socket and
/// cancellation facts remain local, while shared pacing uses the strictest
/// compatible profile requirement.
pub(crate) fn owner_policy_for_targets(
    profiles: &[(CameraId, &ProfileSpec)],
    config: &TransportConfig,
    semantics: SendSemantics,
) -> Result<OwnerPolicy, Error> {
    owner_policy_for_targets_with_tuning(profiles, config, semantics, OperationalTuning::new())
}

/// Tuning-aware multi-target owner policy lowering. The target registry is
/// validated before any owner state is created, so registration is immutable
/// for the lifetime of the resulting owner.
pub(crate) fn owner_policy_for_targets_with_tuning(
    profiles: &[(CameraId, &ProfileSpec)],
    config: &TransportConfig,
    semantics: SendSemantics,
    tuning: OperationalTuning,
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

    let first_profile = profiles[0].1;
    let envelope = match first_profile.envelope() {
        ProfileEnvelope::RawVisca => EnvelopeKind::Raw,
        ProfileEnvelope::SonyEncapsulated => EnvelopeKind::Sony,
    };
    if config.addressing == AddressingMode::Ip && profiles.len() > 1 {
        return Err(Error::NotSupported);
    }
    let mut target_policies = [None; 9];
    let mut command_spacing = std::time::Duration::ZERO;
    let mut inquiry_spacing = std::time::Duration::ZERO;
    let mut inquiry_cooldown = std::time::Duration::ZERO;
    let mut seen = [false; 9];

    for (target, profile) in profiles {
        if !(1..=7).contains(&target.id()) {
            return Err(Error::InvalidRequest(
                "owner target registry requires individual cameras (1 through 7)".into(),
            ));
        }
        let index = usize::from(target.id());
        if seen[index] {
            return Err(Error::InvalidRequest(
                "owner target registry contains a duplicate camera".into(),
            ));
        }
        seen[index] = true;
        profile.validate_tuning(tuning)?;
        if profile.envelope() != first_profile.envelope() {
            return Err(Error::InvalidRequest(
                "registered session profiles require one wire envelope".into(),
            ));
        }

        let timing = profile.timing();
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
        target_policies[index] = Some(TargetPolicy {
            command_sockets: tuning
                .maximum_command_sockets_override()
                .unwrap_or_else(|| profile.maximum_command_sockets()),
            cancellation: if profile.supports_command_cancel() {
                CancellationPolicy::Supported
            } else {
                CancellationPolicy::Unsupported
            },
        });
    }

    if config.buffer_config.recv_buffer_size == 0 {
        return Err(Error::InvalidRequest(
            "transport receive buffer must be non-zero".into(),
        ));
    }
    if config.buffer_config.max_buffer_size == 0 {
        return Err(Error::InvalidRequest(
            "transport maximum buffer must be non-zero".into(),
        ));
    }

    let transport = match semantics {
        SendSemantics::Datagram => crate::runtime::engine::TransportKind::Datagram,
        SendSemantics::Stream => crate::runtime::engine::TransportKind::Stream,
    };

    let capacity = config.max_pending_queue_depth.get();
    let inquiry_capacity = if envelope == EnvelopeKind::Sony {
        capacity
    } else {
        1
    };
    let mut policy = OwnerPolicy::with_targets(
        ProtocolPolicy {
            capacity,
            envelope,
            transport,
            inquiry_capacity,
            command_spacing,
            inquiry_spacing,
            // A syntax-error inquiry retry needs to respect the strictest
            // profile busy/cooldown fact. Ordinary inquiries do not incur this
            // wait.
            inquiry_cooldown,
        },
        target_policies,
    )?;
    policy.limits.receive_bytes = config.buffer_config.recv_buffer_size;
    policy.limits.framing_bytes = config.buffer_config.max_buffer_size;
    Ok(policy)
}

/// Decode one received chunk into scheduler-independent, owned frames.
///
/// `OwnerBuffers` supplies the bounded receive and framing stores while
/// `ProtocolFramer` retains only incomplete source bytes between reads.  The
/// returned frames own payload bytes and envelope sequence metadata, so the
/// owner never observes a borrow into transport or framer storage.
pub(crate) fn decode_frames(
    envelope: &OwnerEnvelope,
    framer: &mut ProtocolFramer,
    target: CameraId,
    buffers: &mut OwnerBuffers,
    received: usize,
    frame_limit: usize,
) -> Result<Vec<DecodedFrame>, Error> {
    let registry = TargetRegistry::single(target)?;
    let routing = RoutingState::new(envelope.addressing(), registry);
    decode_frames_with_routing(envelope, framer, routing, buffers, received, frame_limit)
}

/// Decode one received chunk using immutable multi-target routing state.
pub(crate) fn decode_frames_with_routing(
    envelope: &OwnerEnvelope,
    framer: &mut ProtocolFramer,
    routing: RoutingState,
    buffers: &mut OwnerBuffers,
    received: usize,
    frame_limit: usize,
) -> Result<Vec<DecodedFrame>, Error> {
    if received > buffers.receive_mut().len() {
        return Err(Error::InvalidResponse {
            expected: Cow::Borrowed("transport read fitting the owner receive buffer"),
            actual: received.to_le_bytes().to_vec(),
        });
    }
    if received == 0 {
        return Ok(Vec::new());
    }

    // Keep the owner-owned framing scratch bounded independently of the
    // protocol framer.  It is consumed immediately after the chunk is handed
    // to the framer; incomplete protocol bytes remain only in `framer`.
    buffers.append_received(received)?;
    let pushed = framer.push_slice_with_resync(buffers.framing());
    buffers.consume_framing(received);
    pushed?;

    let mut frames = Vec::new();
    for framed in framer.drain_frames() {
        if frames.len() >= frame_limit {
            return Err(Error::ResponseTooLarge {
                max_size: frame_limit,
            });
        }
        let framed = framed?;
        if let Some(frame) = decode_frame(envelope, routing, framed)? {
            frames.push(frame);
        }
    }
    Ok(frames)
}

fn decode_frame(
    envelope: &OwnerEnvelope,
    routing: RoutingState,
    framed: Bytes,
) -> Result<Option<DecodedFrame>, Error> {
    let (payload, meta) = envelope.extract_with_meta(framed)?;
    let Some(target) = decode_response_target(routing, &payload)? else {
        // An IP source is ambiguous when more than one target is registered;
        // drop it without falling back to any configured target.
        return Ok(None);
    };
    let sequence = meta.sequence.map(|value| EnvelopeSequence {
        value,
        width: SequenceWidth::Full32,
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
    let response = decode_response(routing, &payload)?;
    Ok(Some(DecodedFrame {
        target,
        sequence,
        response,
    }))
}

/// Classify a VISCA response using the shared `decode_basic` parser contract.
/// Serial source bytes are normalized only in this bounded adapter
/// scratch copy; the strict source and the resulting target remain unchanged.
fn decode_response(routing: RoutingState, payload: &[u8]) -> Result<DecodedResponse, Error> {
    let mut normalized = SmallVec::<[u8; INLINE_BYTES]>::new();
    let frame = if routing.addressing() == AddressingMode::Serial {
        normalized.extend_from_slice(payload);
        if let Some(first) = normalized.first_mut() {
            *first = 0x90;
        }
        normalized.as_slice()
    } else {
        payload
    };
    let Some(basic) = decode_basic(frame) else {
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
        BasicKind::Ack => DecodedResponse::Ack {
            socket: basic.socket,
        },
        BasicKind::Completion => DecodedResponse::Completion {
            socket: basic.socket,
        },
        BasicKind::DataReply => {
            let mut owned = SmallVec::<[u8; INLINE_BYTES]>::new();
            owned.extend_from_slice(basic.payload.as_slice());
            DecodedResponse::InquiryReply {
                // Response route classification is intentionally left to the
                // owner/engine's raw inquiry policy. A transport adapter has
                // no request decoder context from which to invent a route.
                route: None,
                payload: owned,
            }
        }
        BasicKind::Error(code) => DecodedResponse::Error {
            socket: basic.socket,
            code,
        },
        BasicKind::NetworkChange => DecodedResponse::NetworkChange,
        BasicKind::Unknown => DecodedResponse::Unknown,
    })
}

/// Strictly decode the source byte of a VISCA response.
///
/// `decode_basic` intentionally retains its historical broad `0x9x` header
/// behavior for protocol compatibility. The owner boundary is stricter: serial
/// replies use exactly `0x90`, `0xa0`, ..., `0xf0` for camera 1 through 7, while IP and
/// Sony replies use exactly `0x90` and can be attributed only to one registered
/// target.
fn decode_response_target(
    routing: RoutingState,
    payload: &[u8],
) -> Result<Option<CameraId>, Error> {
    let Some(&source) = payload.first() else {
        return Err(invalid_source(payload));
    };
    match routing.addressing() {
        AddressingMode::Serial => {
            // The low nibble is reserved in a serial source byte. Accept only
            // camera sources 0x90..=0xf0 on a 0x10 boundary; 0x80/0x88 are
            // controller/broadcast addresses and never response sources.
            if source & 0x0f != 0 || !(0x90..=0xf0).contains(&source) {
                return Err(invalid_source(payload));
            }
            let id = (source >> 4).saturating_sub(8);
            if !(1..=7).contains(&id) {
                return Err(invalid_source(payload));
            }
            let target = CameraId::new(id).map_err(|_| invalid_source(payload))?;
            Ok(Some(target))
        }
        AddressingMode::Ip => {
            if source != 0x90 {
                return Err(invalid_source(payload));
            }
            Ok(routing.targets().sole_target())
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
    profiles: &[(CameraId, &ProfileSpec)],
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
        profiles::{GenericVisca, PtzOpticsG2},
        runtime::engine::DecodedResponse,
        transport::envelope::RawVisca,
    };

    fn all_targets() -> TargetRegistry {
        TargetRegistry::from_targets(&[
            CameraId::CAMERA_1,
            CameraId::CAMERA_2,
            CameraId::CAMERA_3,
            CameraId::CAMERA_4,
            CameraId::CAMERA_5,
            CameraId::CAMERA_6,
            CameraId::CAMERA_7,
        ])
        .unwrap()
    }

    #[test]
    fn serial_source_bytes_map_exactly_to_all_seven_targets() {
        let routing = RoutingState::new(AddressingMode::Serial, all_targets());
        for (source, expected) in [
            (0x90, CameraId::CAMERA_1),
            (0xa0, CameraId::CAMERA_2),
            (0xb0, CameraId::CAMERA_3),
            (0xc0, CameraId::CAMERA_4),
            (0xd0, CameraId::CAMERA_5),
            (0xe0, CameraId::CAMERA_6),
            (0xf0, CameraId::CAMERA_7),
        ] {
            assert!(matches!(
                decode_response_target(routing, &[source]),
                Ok(Some(actual)) if actual == expected
            ));
        }
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
        let envelope = OwnerEnvelope::Raw(RawVisca::new(AddressingMode::Serial));
        let routing = RoutingState::new(AddressingMode::Serial, all_targets());
        let ack = decode_frame(&envelope, routing, Bytes::from_static(&[0xa0, 0x42, 0xff]))
            .unwrap()
            .unwrap();
        assert_eq!(ack.target, CameraId::CAMERA_2);
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
        assert_eq!(error.target, CameraId::CAMERA_2);
        assert!(matches!(
            error.response,
            DecodedResponse::Error {
                socket: Some(crate::ViscaSocket::S1),
                code: 0x04
            }
        ));
    }

    #[test]
    fn ip_and_sony_style_single_target_responses_require_exact_90_source() {
        let envelope = OwnerEnvelope::Raw(RawVisca::new(AddressingMode::Ip));
        let routing = RoutingState::new(
            AddressingMode::Ip,
            TargetRegistry::single(CameraId::CAMERA_2).unwrap(),
        );
        let frame = decode_frame(&envelope, routing, Bytes::from_static(&[0x90, 0x42, 0xff]))
            .unwrap()
            .unwrap();
        assert_eq!(frame.target, CameraId::CAMERA_2);
        assert!(decode_frame(&envelope, routing, Bytes::from_static(&[0xa0, 0x42, 0xff])).is_err());
    }

    #[test]
    fn valid_unregistered_serial_source_becomes_inert_unknown_frame() {
        let envelope = OwnerEnvelope::Raw(RawVisca::new(AddressingMode::Serial));
        let routing = RoutingState::new(
            AddressingMode::Serial,
            TargetRegistry::single(CameraId::CAMERA_1).unwrap(),
        );
        let frame = decode_frame(&envelope, routing, Bytes::from_static(&[0xa0, 0x42, 0xff]))
            .unwrap()
            .unwrap();
        assert_eq!(frame.target, CameraId::CAMERA_2);
        assert!(matches!(frame.response, DecodedResponse::Unknown));
    }

    /// Issue #565: `90 40 FF` / `90 50 FF` carry no socket nibble. They are
    /// well-formed VISCA and must reach the scheduler with `socket: None`
    /// rather than failing the whole session with `InvalidResponse`.
    #[test]
    fn socketless_ack_and_completion_decode_without_a_socket() {
        let envelope = OwnerEnvelope::Raw(RawVisca::new(AddressingMode::Ip));
        let routing = RoutingState::new(
            AddressingMode::Ip,
            TargetRegistry::single(CameraId::CAMERA_1).unwrap(),
        );
        let ack = decode_frame(&envelope, routing, Bytes::from_static(&[0x90, 0x40, 0xff]))
            .expect("a socketless ACK is not a decode failure")
            .expect("a socketless ACK is attributable");
        assert_eq!(ack.target, CameraId::CAMERA_1);
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
        let envelope = OwnerEnvelope::Raw(RawVisca::new(AddressingMode::Ip));
        let routing = RoutingState::new(
            AddressingMode::Ip,
            TargetRegistry::single(CameraId::CAMERA_1).unwrap(),
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
    fn multi_target_policy_retains_target_local_facts_and_shared_bounds() {
        let profile = ProfileSpec::from_compile_time::<GenericVisca>().unwrap();
        let profiles = [
            (CameraId::CAMERA_1, &profile),
            (CameraId::CAMERA_2, &profile),
        ];
        let config = TransportConfig {
            addressing: AddressingMode::Serial,
            ..TransportConfig::default()
        };
        let policy = owner_policy_for_targets(&profiles, &config, SendSemantics::Stream).unwrap();
        assert!(policy.targets[1].is_some());
        assert!(policy.targets[2].is_some());
        assert_eq!(policy.protocol.inquiry_capacity, 1);
        assert_eq!(
            policy.protocol.command_spacing,
            profile.timing().minimum_command_spacing()
        );
    }

    #[test]
    fn multi_target_policy_uses_strictest_compatible_profile_pacing() {
        let relaxed = ProfileSpec::from_compile_time::<GenericVisca>().unwrap();
        let strict = ProfileSpec::from_compile_time::<PtzOpticsG2>().unwrap();
        let profiles = [
            (CameraId::CAMERA_1, &relaxed),
            (CameraId::CAMERA_2, &strict),
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
}
