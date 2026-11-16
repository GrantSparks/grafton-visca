//! Profile-selected pan/tilt wire grammars.

/// Profile-owned wire framing for pan/tilt position commands and inquiries.
///
/// The baseline VISCA form carries two speed bytes and four nibbles per axis.
/// Some camera families use a different position payload even though they keep
/// the usual pan/tilt opcodes.  This discriminator keeps that variation a
/// profile fact rather than scattering model checks through encoders and
/// decoders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[non_exhaustive]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum PanTiltWireCodec {
    /// Standard VISCA: two speed bytes and four nibbles for each axis.
    #[default]
    StandardVisca,
    /// Sony BRC-300: one speed byte, fixed `00`, five signed pan nibbles,
    /// and four signed tilt nibbles.
    SonyBrc300,
    /// Sony FR7: two speed bytes and five signed nibbles for each axis.
    SonyFr7,
}

impl PanTiltWireCodec {
    /// Complete size of this grammar's longest position or limit frame.
    pub(crate) const fn maximum_position_frame_bytes(self) -> usize {
        match self {
            Self::StandardVisca => 15,
            Self::SonyBrc300 => 16,
            Self::SonyFr7 => 17,
        }
    }
}
