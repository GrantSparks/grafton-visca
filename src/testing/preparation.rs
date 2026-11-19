//! Exercise profile validation and encoding without owner admission or I/O.
//!
//! These probes let integration tests measure pure preparation separately from
//! session construction and engine admission.

use crate::{
    completion, Inquiry, OperationCommand, OperationalTuning, PlainCommand, Result,
    TargetRegistration,
};

/// Validate and encode a plain command for `target`, then discard the prepared
/// value without admitting it to an owner.
///
/// # Errors
/// Returns the same profile, policy or encoding refusal as normal submission.
pub fn check_command<C: PlainCommand + ?Sized>(
    command: &C,
    target: &TargetRegistration,
    tuning: OperationalTuning,
) -> Result<()> {
    std::hint::black_box(crate::prepared::prepare_command(
        command,
        target,
        crate::session_config::ValidatedTuning::validate(tuning, [target.profile()])?,
        crate::prepared::ClassSelection::Request,
    )?);
    Ok(())
}

/// Validate and encode a typed inquiry for `target`, then discard the prepared
/// value without admitting it to an owner.
///
/// # Errors
/// Returns the same profile, policy or encoding refusal as normal submission.
pub fn check_inquiry<Q: Inquiry + ?Sized>(
    inquiry: &Q,
    target: &TargetRegistration,
    tuning: OperationalTuning,
) -> Result<()> {
    std::hint::black_box(crate::prepared::prepare_inquiry(
        inquiry,
        target,
        crate::session_config::ValidatedTuning::validate(tuning, [target.profile()])?,
        crate::prepared::ClassSelection::Request,
    )?);
    Ok(())
}

/// Inert operation preparation retained only for allocation characterization.
///
/// This probe never admits a request or performs transport I/O.
#[derive(Debug)]
pub struct OperationPreparation<K: completion::Kind> {
    prepared: crate::prepared::PreparedOperation<K>,
}

impl OperationPreparation<completion::Targeted> {
    /// Materializes the cached polling templates as `settled()` would do.
    /// Clones the cached plan as each settlement observation would do.
    /// Repeated calls reuse the same templates without performing I/O.
    ///
    /// # Errors
    /// Returns a settlement inquiry preparation refusal.
    pub fn check_settlement(&self) -> Result<()> {
        if let crate::prepared::SettlementPlan::Poll { queries, .. } =
            self.prepared.settlement_plan()
        {
            std::hint::black_box(queries.get()?.clone());
        }
        Ok(())
    }
}

/// Validate and encode an operation for `target` without admitting it to an owner.
///
/// This measures submission preparation independently from the engine's shared
/// frame storage and from any later settlement observation.
///
/// # Errors
/// Returns the same profile, policy or encoding refusal as normal submission.
pub fn check_operation<K: completion::Kind, O: OperationCommand<K> + ?Sized>(
    operation: &O,
    target: &TargetRegistration,
    tuning: OperationalTuning,
) -> Result<OperationPreparation<K>> {
    Ok(OperationPreparation {
        prepared: crate::prepared::prepare_operation::<K, _>(
            operation,
            target,
            crate::session_config::ValidatedTuning::validate(tuning, [target.profile()])?,
            crate::prepared::ClassSelection::Request,
        )?,
    })
}
