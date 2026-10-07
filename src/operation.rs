//! Public async operation handle.
//!
//! The handle is deliberately independent of profiles, transports, executors,
//! and runtime implementations. The serialized owner retains all protocol
//! state; the handle owns the operation's observation slots and caches what it
//! has observed, and delegates waiting and cancellation to the owner receipt.

#![cfg(feature = "async")]

use std::time::Duration;

use crate::{
    completion, runtime::owner::AsyncOperationReceipt, CancellationOutcome, Error, OperationId,
};

/// An async handle on one admitted operation.
///
/// The completion marker `K` is the only generic parameter. The owner remains
/// authoritative for protocol lifecycle, timeouts, settlement, and
/// cancellation policy; the handle observes it.
///
/// # Waits borrow the handle
///
/// Every wait takes `&mut self`, and the handle caches the authoritative
/// result once received. Waiting again returns the cached result, an
/// [`applied`](Self::applied) wait can be followed by
/// [`settled`](Operation::settled), and a wait that times out or is dropped
/// (for example, the losing branch of a `select!`) releases only that wait:
/// the operation keeps running and the handle keeps observing it.
///
/// # Dropping never stops the camera
///
/// Dropping this handle is exactly [`detach`](Self::detach): it relinquishes
/// observation and nothing else. The owner keeps the protocol lifecycle,
/// never reads a dropped handle as cancellation, and no STOP is emitted, so
/// an early `?` return or a panic unwinding past a live handle leaves physical
/// movement running until something ends it.
///
/// To bound movement by a scope, write a small guard whose own `Drop` submits
/// the typed STOP — see the guard pattern in `docs/migration_2_0.md` and
/// `examples/operation_handles_async.rs`. For an explicit stop on a path you
/// control, use `camera.pan_tilt().stop()`, `camera.zoom().stop()`,
/// `camera.focus().stop()`, or `camera.motion().stop_all_motion()`;
/// [`cancel`](Self::cancel) is protocol cancellation and does not by itself
/// prove motion ended.
#[must_use = "await, cancel, or explicitly detach this operation"]
pub struct Operation<K>
where
    K: completion::Kind,
{
    receipt: AsyncOperationReceipt<K>,
}

impl<K> std::fmt::Debug for Operation<K>
where
    K: completion::Kind,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.receipt.fmt_public_handle(formatter)
    }
}

impl<K> Operation<K>
where
    K: completion::Kind,
{
    /// Creates a public operation handle over an admitted owner receipt.
    ///
    /// This is crate-private because only the typed admission boundary may
    /// create a receipt.
    pub(crate) fn from_receipt(receipt: AsyncOperationReceipt<K>) -> Self {
        Self { receipt }
    }

    /// Returns the opaque identity assigned when this operation was admitted.
    #[must_use]
    pub fn id(&self) -> OperationId {
        OperationId::from_raw(self.receipt.id())
    }

    /// Waits for the exact submitted command to reach `Applied`, bounded by
    /// its configured observer deadline.
    pub async fn applied(&mut self) -> Result<(), Error> {
        self.receipt.applied(None).await
    }

    /// Waits for application, bounded by `timeout`.
    ///
    /// The deadline bounds only this wait. If it expires the wait returns
    /// [`Error::ObservationTimeout`]: the operation keeps running, its
    /// scheduler deadline is unchanged, and the handle can wait again.
    pub async fn applied_with_timeout(&mut self, timeout: Duration) -> Result<(), Error> {
        self.receipt.applied(Some(timeout)).await
    }

    /// Cancels the operation and waits for the cancellation's conclusion,
    /// bounded by the configured cancellation deadline.
    ///
    /// The result is [`CancellationOutcome::Cancelled`] when cancellation
    /// won, [`CancellationOutcome::Completed`] when the operation was applied
    /// first, and the operation's own error when it failed first.
    ///
    /// Cancelling is idempotent. The handle has one cancellation intent;
    /// calling `cancel` again, including after a timed-out or dropped call,
    /// observes that intent instead of sending a second cancellation. Once
    /// the operation's outcome is known, `cancel` answers from it without
    /// sending anything.
    ///
    /// # A refused cancellation leaves the operation running
    ///
    /// Cancelling a request that is still queued always succeeds. Cancelling
    /// one that has already been written needs profile support for the
    /// standard VISCA socket-cancel command; without it — the built-in
    /// PTZOptics profiles ([`PtzOpticsG2`], [`PtzOpticsG3`] and
    /// [`PtzOptics30X`]) are in that position — the owner refuses with
    /// [`Error::NotSupported`] and leaves the original request scheduled,
    /// retryable, and able to complete. The handle is unaffected and can
    /// still wait for it. A refusal does not stop the camera; a moving axis
    /// ends with an applied typed STOP.
    ///
    /// A cancellation that was accepted but then failed without ending the
    /// operation (its cancel write failed, or its observation deadline
    /// expired) returns that error; the handle still observes the operation's
    /// own outcome.
    ///
    /// [`PtzOpticsG2`]: crate::profiles::PtzOpticsG2
    /// [`PtzOpticsG3`]: crate::profiles::PtzOpticsG3
    /// [`PtzOptics30X`]: crate::profiles::PtzOptics30X
    pub async fn cancel(&mut self) -> Result<CancellationOutcome, Error> {
        self.receipt.cancel(None).await
    }

    /// [`cancel`](Self::cancel), bounded by `timeout` instead of the
    /// configured cancellation deadline. If it expires, the cancellation
    /// intent stays recorded and a later `cancel` observes it.
    pub async fn cancel_with_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<CancellationOutcome, Error> {
        self.receipt.cancel(Some(timeout)).await
    }

    /// Explicitly relinquishes this operation's observation without changing
    /// protocol state, cancelling the request, or sending a physical STOP.
    ///
    /// This is the explicit spelling of what dropping the handle already does;
    /// movement continues until something else ends it.
    pub fn detach(self) {}
}

impl Operation<completion::Targeted> {
    /// Waits for exact application and then the profile-selected protocol
    /// settlement condition, bounded by the configured settlement budget.
    ///
    /// Application is cached, so calling this after
    /// [`applied`](Operation::applied) continues from it. Polled settlement proves observed stability, not arrival at the requested endpoint.
    /// Later conflicting admission supersedes unfinished polled evidence. Settlement proven
    /// by position polling is cached once proven; a polling wait that is
    /// abandoned or times out restarts with a fresh proof.
    pub async fn settled(&mut self) -> Result<crate::Settlement, Error> {
        self.receipt.settled(None).await
    }

    /// [`settled`](Self::settled), bounded by `timeout`.
    ///
    /// The deadline bounds only this wait; the prepared scheduler and
    /// settlement policy are unchanged. Each abandoned polling attempt may
    /// leave its admitted position inquiries running until their own
    /// deadlines, within the session's admission capacity.
    pub async fn settled_with_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<crate::Settlement, Error> {
        self.receipt.settled(Some(timeout)).await
    }
}
