//! Exercise profile validation and encoding without owner admission or I/O.
//!
//! These probes let integration tests measure pure preparation separately from
//! session construction and engine admission.

use crate::{Inquiry, OperationalTuning, PlainCommand, Result, TargetRegistration};

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
