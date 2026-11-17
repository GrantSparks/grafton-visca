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
///
/// # Examples
///
/// ```
/// use grafton_visca::{
///     profiles::{GenericVisca, ProfileSpec},
///     testing::turns::TurnProbe,
///     transport::{SendSemantics, TransportConfig},
/// };
///
/// let profile = ProfileSpec::from_compile_time::<GenericVisca>()?;
/// let mut probe = TurnProbe::new(&profile, TransportConfig::default(), SendSemantics::Stream)?;
/// probe.stage_command()?;
/// probe.command_turn()?;
/// probe.frame_turn()?;
/// # Ok::<(), grafton_visca::Error>(())
/// ```
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

    /// Stage a command that permits one retry after a conclusive buffer-full rejection.
    ///
    /// Call this outside an allocation measurement, then call [`Self::retry_turn`].
    pub fn stage_retry(&mut self) -> Result<()> {
        self.inner.stage_retry()
    }

    /// Drive the staged command through write, ACK and completion to its observer.
    pub fn command_turn(&mut self) -> Result<()> {
        self.inner.command_turn()
    }

    /// Drive a pre-ACK rejection, safe retry and successful completion.
    ///
    /// Checks that both attempts write identical bytes (including the Sony
    /// sequence) using the same framing-buffer pointer and capacity.
    pub fn retry_turn(&mut self) -> Result<()> {
        self.inner.retry_turn()
    }

    /// Drive a harmless unmatched reply through one complete owner frame turn.
    pub fn frame_turn(&mut self) -> Result<()> {
        self.inner.frame_turn()
    }
}

/// The production async actor's receive/select loop, polled without executor
/// scheduling or reactor bookkeeping in an allocation measurement.
///
/// Keep one probe for the measurement, warm it with a receive frame, then
/// alternate [`Self::receive_frame`] and [`Self::poll_parked`]. Every receive
/// uses the real actor loop, boundary arbitration, timers and frame recycling.
#[cfg(feature = "async")]
#[derive(Debug)]
pub struct SelectProbe {
    inner: crate::runtime::owner::SelectProbe,
}

#[cfg(feature = "async")]
impl SelectProbe {
    /// Construct a receive-only actor with a generic VISCA profile and the
    /// supplied executor's timers. Construction is outside the measurement.
    pub fn new(runtime: impl crate::runtime::Executor) -> Result<Self> {
        Ok(Self {
            inner: crate::runtime::owner::SelectProbe::new(runtime)?,
        })
    }

    /// Wake one receive with an inert unmatched completion frame.
    pub fn receive_frame(&mut self) {
        self.inner.receive_frame();
    }

    /// Poll the actor through bounded cooperative handoffs until its next
    /// receive parks, returning the total number of consumed probe frames.
    ///
    /// # Errors
    ///
    /// Returns an error if the actor terminates or does not park within the
    /// normal receive handoff bound.
    pub fn poll_parked(&mut self, context: &mut std::task::Context<'_>) -> Result<usize> {
        self.inner.poll_parked(context)
    }
}
