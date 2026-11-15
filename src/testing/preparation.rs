//! Exercise profile validation and encoding without owner admission or I/O.
//!
//! These probes let integration tests measure pure preparation separately from
//! session construction and engine admission.

use crate::{CameraId, Inquiry, OperationalTuning, PlainCommand, ProfileSpec, Result};

/// Validate and encode a plain command for `target`, then discard the prepared
/// value without admitting it to an owner.
///
/// # Errors
/// Returns the same profile, policy or encoding refusal as normal submission.
pub fn check_command<C: PlainCommand + ?Sized>(
    command: &C,
    target: CameraId,
    profile: &ProfileSpec,
    tuning: OperationalTuning,
) -> Result<()> {
    std::hint::black_box(crate::prepared::prepare_command(
        command,
        &crate::TargetRegistration::new(target, profile.clone())?,
        tuning,
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
    target: CameraId,
    profile: &ProfileSpec,
    tuning: OperationalTuning,
) -> Result<()> {
    std::hint::black_box(crate::prepared::prepare_inquiry(
        inquiry,
        &crate::TargetRegistration::new(target, profile.clone())?,
        tuning,
        crate::prepared::ClassSelection::Request,
    )?);
    Ok(())
}
