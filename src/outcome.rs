//! Public terminal cancellation outcomes.

use core::fmt;

use crate::Error;

/// The conclusive result of observing a cancellation request.
///
/// Cancellation can lose a race with the original operation's successful
/// completion. That exact race is reported as [`Self::Completed`], never as a
/// synthetic cancellation success.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum CancellationOutcome {
    /// The original operation was conclusively cancelled.
    Cancelled,
    /// The original operation completed successfully before cancellation won.
    Completed,
}

/// A cancellation the owner refused, carrying the operation handle back.
///
/// Cancelling a command that has already been written requires profile support
/// for the standard VISCA socket-cancel command; a profile without it — the
/// only one in the built-in registry is [`PtzOpticsG2`] — rejects the request
/// with [`Error::NotSupported`]. That rejection is deliberately *not*
/// cancellation intent: the owner leaves the original request scheduled,
/// retryable, and able to complete.
///
/// Because the original is still live, the handle that observes it is still
/// meaningful, so a refused `cancel` hands it back rather than consuming it.
/// Take it with [`into_operation`](Self::into_operation) and keep waiting,
/// retry the cancel, or drop it to detach. The handle is absent only when the
/// owner itself is gone — a closed session or a terminated actor — in which
/// case there is nothing left to observe on any handle.
///
/// The `?` operator still works in a function returning [`Error`]: the
/// [`From`] conversion keeps the reason and drops the handle, which detaches
/// it exactly as dropping the handle always does. Use the explicit accessors
/// when the handle is worth keeping.
///
/// Recovering the handle never stops the camera by itself. To request that a
/// continuous drive end, submit a typed STOP —
/// `camera.pan_tilt().stop()`, `camera.zoom().stop()`,
/// `camera.focus().stop()`, or `camera.motion().stop_all_motion()`.
///
/// The handle is boxed, so carrying one back costs a single allocation on the
/// refusal path and keeps the success path's `Result` small.
///
/// [`PtzOpticsG2`]: crate::profiles::PtzOpticsG2
#[derive(Debug)]
pub struct CancelRejected<H> {
    operation: Option<Box<H>>,
    error: Error,
}

impl<H> CancelRejected<H> {
    /// Builds a rejection that hands `operation` back to the caller.
    pub(crate) fn new(operation: Option<H>, error: Error) -> Self {
        Self {
            operation: operation.map(Box::new),
            error,
        }
    }

    /// Returns why the owner refused the cancellation.
    #[must_use]
    pub const fn error(&self) -> &Error {
        &self.error
    }

    /// Returns whether the operation handle came back with the rejection.
    #[must_use]
    pub const fn has_operation(&self) -> bool {
        self.operation.is_some()
    }

    /// Takes the operation handle back, discarding the reason.
    ///
    /// This is [`None`] only when the owner is gone, so nothing could be
    /// observed through the handle anyway.
    #[must_use = "dropping the recovered handle detaches it"]
    pub fn into_operation(self) -> Option<H> {
        self.operation.map(|operation| *operation)
    }

    /// Discards the operation handle — detaching it — and returns the reason.
    #[must_use]
    pub fn into_error(self) -> Error {
        self.error
    }

    /// Splits the rejection into the recovered handle and the reason.
    #[must_use = "dropping the recovered handle detaches it"]
    pub fn into_parts(self) -> (Option<H>, Error) {
        (self.operation.map(|operation| *operation), self.error)
    }
}

impl<H> fmt::Display for CancelRejected<H> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cancellation rejected: {}", self.error)
    }
}

impl<H> std::error::Error for CancelRejected<H>
where
    H: fmt::Debug,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

impl<H> From<CancelRejected<H>> for Error {
    fn from(rejected: CancelRejected<H>) -> Self {
        rejected.into_error()
    }
}
