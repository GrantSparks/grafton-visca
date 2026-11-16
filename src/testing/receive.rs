//! Production receive, decode and batch recycling without transport I/O.

use crate::{
    transport::{SendSemantics, TransportConfig},
    ProfileSpec, Result,
};

/// A warmed production receive path for allocation and framing tests.
#[derive(Debug)]
pub struct ReceiveProbe {
    inner: crate::runtime::owner::ReceiveProbe,
}

impl ReceiveProbe {
    /// Build the same bounded framing and batch storage an owner uses.
    pub fn new(
        profile: &ProfileSpec,
        config: TransportConfig,
        semantics: SendSemantics,
    ) -> Result<Self> {
        Ok(Self {
            inner: crate::runtime::owner::ReceiveProbe::new(profile, config, semantics)?,
        })
    }

    /// Lower one successful transport read, decode its frames and recycle the batch.
    /// Returns how many complete frames were consumed.
    pub fn receive(&mut self, chunk: &[u8]) -> Result<usize> {
        self.inner.receive(chunk)
    }
}

#[cfg(test)]
mod tests {
    use super::ReceiveProbe;

    #[test]
    fn public_receive_probe_preserves_both_unwind_markers() {
        fn assert_unwind_safe<T: std::panic::UnwindSafe + std::panic::RefUnwindSafe>() {}
        assert_unwind_safe::<ReceiveProbe>();
    }
}
