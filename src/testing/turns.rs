//! Production engine and owner turns without device or executor I/O.

use crate::{
    transport::{SendSemantics, TransportConfig},
    ProfileSpec, Result,
};

/// A deterministic production owner for protocol-turn allocation tests.
///
/// The probe expands the same turn driver as both facade owners. Request
/// admission and bounded observer storage are prepared separately so a measured
/// turn includes framing, engine decisions, recursive effect draining and
/// terminal delivery without counting permitted lifecycle allocations.
#[derive(Debug)]
pub struct TurnProbe {
    inner: crate::runtime::owner::TurnProbe,
}

impl TurnProbe {
    /// Build the same wire authority, policy and bounded buffers as an owner.
    pub fn new(
        profile: &ProfileSpec,
        config: TransportConfig,
        semantics: SendSemantics,
    ) -> Result<Self> {
        Ok(Self {
            inner: crate::runtime::owner::TurnProbe::new(profile, config, semantics)?,
        })
    }

    /// Stage one command and reserve its bounded lifecycle storage.
    ///
    /// Call this outside an allocation measurement, then call `command_turn`.
    pub fn stage_command(&mut self) -> Result<()> {
        self.inner.stage_command()
    }

    /// Drive the staged command through write, ACK and completion to its observer.
    pub fn command_turn(&mut self) -> Result<()> {
        self.inner.command_turn()
    }

    /// Drive a harmless unmatched reply through one complete owner frame turn.
    pub fn frame_turn(&mut self) -> Result<()> {
        self.inner.frame_turn()
    }
}
