//! Public terminal cancellation outcomes.

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
