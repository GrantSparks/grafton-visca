//! The one session authority for wire encoding and protocol identity.
//!
//! Plans bind a concrete encoder while the engine selects a logical write.
//! Executing the plan allocates a fresh Sony identity only after payload
//! validation, or replays the exact old identity. Receive framing is derived
//! from the same immutable authority when the adapter is constructed.

use std::sync::Arc;

use crate::{
    command::CommandKind,
    transport::{
        builder::AddressingMode,
        envelope::{Envelope, RawVisca, SonyEncapsulated, SonyRequestSequence},
    },
    Error,
};

use super::{EnvelopeKind, TransmissionMeta};

/// Immutable, shared configuration for exactly one session wire protocol.
#[derive(Debug, Clone)]
pub(crate) struct WireProtocol(Arc<ProtocolEncoder>);

impl WireProtocol {
    #[cfg(test)]
    pub(crate) fn new(kind: EnvelopeKind, addressing: AddressingMode) -> Self {
        match kind {
            EnvelopeKind::Raw => Self::raw(RawVisca::new(addressing)),
            EnvelopeKind::Sony => Self::sony(SonyEncapsulated::new(addressing)),
        }
    }

    pub(crate) fn raw(envelope: RawVisca) -> Self {
        Self(Arc::new(ProtocolEncoder::Raw(RawEncoder(envelope))))
    }

    pub(crate) fn sony(envelope: SonyEncapsulated) -> Self {
        Self(Arc::new(ProtocolEncoder::Sony(SonyEncoder(envelope))))
    }

    pub(crate) fn kind(&self) -> EnvelopeKind {
        match self.encoder() {
            ProtocolEncoder::Raw(_) => EnvelopeKind::Raw,
            ProtocolEncoder::Sony(_) => EnvelopeKind::Sony,
        }
    }

    pub(crate) fn encoder(&self) -> &ProtocolEncoder {
        &self.0
    }

    fn addressing(&self) -> AddressingMode {
        match self.encoder() {
            ProtocolEncoder::Raw(encoder) => encoder.0.addressing(),
            ProtocolEncoder::Sony(encoder) => encoder.0.addressing(),
        }
    }
}

// Policy equality compares immutable configuration, not the evolving Sony
// sequence allocator shared by every encoder and plan from this authority.
impl PartialEq for WireProtocol {
    fn eq(&self, other: &Self) -> bool {
        self.kind() == other.kind() && self.addressing() == other.addressing()
    }
}
impl Eq for WireProtocol {}

/// A typed projection of the authority. Raw has no Sony sequence argument.
#[derive(Debug)]
pub(crate) enum ProtocolEncoder {
    Raw(RawEncoder),
    Sony(SonyEncoder),
}

#[derive(Debug)]
pub(crate) struct RawEncoder(RawVisca);

impl RawEncoder {
    pub(crate) fn plan(&self) -> WriteFraming {
        WriteFraming(FramingPlan::Raw(self.0))
    }
}

#[derive(Debug)]
pub(crate) struct SonyEncoder(SonyEncapsulated);

impl SonyEncoder {
    pub(crate) fn plan(&self, sequence: SonyRequestSequence) -> WriteFraming {
        WriteFraming(FramingPlan::Sony {
            envelope: self.0.clone(),
            sequence,
        })
    }

    #[cfg(test)]
    pub(crate) fn envelope(&self) -> SonyEncapsulated {
        self.0.clone()
    }
}

/// An opaque write plan bound to its concrete encoder and Sony allocator.
///
/// The adapter executes this plan without choosing another envelope. A Sony
/// plan clones the existing allocator handle; minting and execution allocate
/// no per-write storage, and the owner keeps its reusable frame buffer.
#[derive(Debug, Clone)]
pub(crate) struct WriteFraming(FramingPlan);

#[derive(Debug, Clone)]
enum FramingPlan {
    Raw(RawVisca),
    Sony {
        envelope: SonyEncapsulated,
        sequence: SonyRequestSequence,
    },
}

impl WriteFraming {
    #[cfg(test)]
    pub(super) fn sony_sequence(&self) -> Option<SonyRequestSequence> {
        match self.0 {
            FramingPlan::Raw(_) => None,
            FramingPlan::Sony { sequence, .. } => Some(sequence),
        }
    }

    pub(crate) fn frame_into(
        &self,
        bytes: &[u8],
        kind: CommandKind,
        out: &mut bytes::BytesMut,
    ) -> Result<TransmissionMeta, Error> {
        match &self.0 {
            FramingPlan::Raw(envelope) => {
                envelope.frame_into(bytes, kind, out)?;
                Ok(TransmissionMeta::Known)
            }
            FramingPlan::Sony { envelope, sequence } => envelope
                .frame_into_with_sequence(bytes, kind, *sequence, out)
                .map(|allocated| match sequence {
                    SonyRequestSequence::AllocateFresh => TransmissionMeta::Allocated {
                        sequence: allocated,
                    },
                    SonyRequestSequence::Replay(_) => TransmissionMeta::Known,
                }),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    const COMMAND: &[u8] = &[0x81, 0x01, 0x04, 0x00, 0x02, 0xff];

    #[test]
    fn sony_plans_share_one_allocator_and_replay_without_advancing_it() {
        let authority = WireProtocol::new(EnvelopeKind::Sony, AddressingMode::Ip);
        let ProtocolEncoder::Sony(encoder) = authority.encoder() else {
            unreachable!("Sony authority");
        };
        let first = encoder.plan(SonyRequestSequence::AllocateFresh);
        let next = encoder.plan(SonyRequestSequence::AllocateFresh);
        let replay = encoder.plan(SonyRequestSequence::Replay(0));
        let mut buffer = bytes::BytesMut::new();
        assert_eq!(
            first
                .frame_into(COMMAND, CommandKind::Command, &mut buffer)
                .expect("first"),
            TransmissionMeta::Allocated { sequence: 0 }
        );
        let original = buffer.clone();
        assert_eq!(
            replay
                .frame_into(COMMAND, CommandKind::Command, &mut buffer)
                .expect("replay"),
            TransmissionMeta::Known
        );
        assert_eq!(buffer, original);
        assert!(encoder
            .plan(SonyRequestSequence::AllocateFresh)
            .frame_into(&[], CommandKind::Command, &mut buffer)
            .is_err());
        assert_eq!(
            buffer, original,
            "refusal leaves the reusable buffer unchanged"
        );
        assert_eq!(
            next.frame_into(COMMAND, CommandKind::Command, &mut buffer)
                .expect("next"),
            TransmissionMeta::Allocated { sequence: 1 }
        );
        let shared = authority.clone();
        let ProtocolEncoder::Sony(shared) = shared.encoder() else {
            unreachable!("shared Sony authority");
        };
        assert_eq!(
            shared
                .plan(SonyRequestSequence::AllocateFresh)
                .frame_into(COMMAND, CommandKind::Command, &mut buffer)
                .expect("shared"),
            TransmissionMeta::Allocated { sequence: 2 }
        );
    }

    #[test]
    fn receive_side_reset_and_pending_plans_share_the_same_sony_counter() {
        let authority = WireProtocol::new(EnvelopeKind::Sony, AddressingMode::Ip);
        let ProtocolEncoder::Sony(encoder) = authority.encoder() else {
            unreachable!("Sony authority");
        };
        let receiver = encoder.envelope();
        let mut buffer = bytes::BytesMut::new();
        encoder
            .plan(SonyRequestSequence::AllocateFresh)
            .frame_into(COMMAND, CommandKind::Command, &mut buffer)
            .expect("first");
        let pending = encoder.plan(SonyRequestSequence::AllocateFresh);
        receiver.frame_sequence_reset_into(&mut buffer);
        assert_eq!(
            pending
                .frame_into(COMMAND, CommandKind::Command, &mut buffer)
                .expect("after reset"),
            TransmissionMeta::Allocated { sequence: 0 }
        );
    }

    #[test]
    fn raw_plan_has_no_sequence_axis_and_preserves_its_bound_addressing() {
        let authority = WireProtocol::new(EnvelopeKind::Raw, AddressingMode::Serial);
        let ProtocolEncoder::Raw(encoder) = authority.encoder() else {
            unreachable!("raw authority");
        };
        let plan = encoder.plan();
        let mut buffer = bytes::BytesMut::new();
        let command = [0x82, 0x01, 0x04, 0x00, 0x02, 0xff];
        assert_eq!(
            plan.frame_into(&command, CommandKind::Command, &mut buffer)
                .expect("raw frame"),
            TransmissionMeta::Known
        );
        assert_eq!(buffer.as_ref(), command);
    }
}
