//! Public async operation and cancellation lifecycle handles.
//!
//! The handles in this module are deliberately independent of profiles,
//! transports, executors, and runtime implementations.  The serialized owner
//! retains all protocol state; these values only own the corresponding one-shot
//! observer and delegate waiting and cancellation to the owner receipts.

#![cfg(feature = "async")]

use std::time::Duration;

use crate::{
    completion,
    runtime::owner::{AsyncCancellationReceipt, AsyncOperationReceipt, AsyncReceiptControl},
    CancelRejected, CancellationOutcome, Error, OperationId,
};

/// A linear async operation handle.
///
/// The completion marker `K` is the only generic parameter.  [`Operation`]
/// owns the exact owner receipt and its observer control, while the owner
/// remains authoritative for protocol lifecycle, timeout, settlement, and
/// cancellation policy.
///
/// # Dropping never stops the camera
///
/// Dropping this handle is exactly [`detach`](Self::detach): it relinquishes
/// the observer and nothing else.  The owner keeps the protocol lifecycle,
/// never reads a dropped handle as cancellation, and no STOP is emitted, so
/// an early `?` return or a panic unwinding past a live handle leaves physical
/// movement running until something ends it.  This matches 1.x exactly and is
/// not a 2.0 behaviour change.
///
/// To bound movement by a scope, write a small guard whose own `Drop` submits
/// the typed STOP — see the guard pattern in `docs/migration_2_0.md` and
/// `examples/operation_handles_async.rs`.  For an explicit stop on a path you
/// control, use `camera.pan_tilt().stop()`, `camera.zoom().stop()`,
/// `camera.focus().stop()`, or `camera.motion().stop_all_motion()`;
/// [`cancel`](Self::cancel) records protocol cancellation but does not by
/// itself prove motion ended.
#[must_use = "await, cancel, or explicitly detach this operation"]
#[derive(Debug)]
pub struct Operation<K>
where
    K: completion::Kind,
{
    receipt: Option<AsyncOperationReceipt<K>>,
    control: AsyncReceiptControl,
    id: OperationId,
}

impl<K> Operation<K>
where
    K: completion::Kind,
{
    /// Creates a public operation handle over an admitted owner receipt.
    ///
    /// This is crate-private because only the typed admission boundary may
    /// create a receipt.  Keeping construction here ensures all public
    /// terminal actions delegate to the same owner implementation.
    pub(crate) fn from_receipt(
        receipt: AsyncOperationReceipt<K>,
        control: AsyncReceiptControl,
    ) -> Self {
        let id = OperationId::from_raw(receipt.id());
        Self {
            receipt: Some(receipt),
            control,
            id,
        }
    }

    /// Returns the opaque identity assigned when this operation was admitted.
    #[must_use]
    pub fn id(&self) -> OperationId {
        self.id
    }

    /// Waits for the exact submitted command to reach `Applied` using its
    /// configured observer deadline.
    pub async fn applied(self) -> Result<(), Error> {
        let mut this = self;
        let Some(receipt) = this.receipt.take() else {
            return Err(consumed_handle_error("operation"));
        };
        receipt.applied(this.control.clone()).await
    }

    /// Waits for application using an explicit observer deadline.
    ///
    /// The scheduler deadline and protocol lifecycle are unchanged.  If the
    /// observer deadline expires, this handle is consumed and detached.
    pub async fn applied_with_timeout(self, timeout: Duration) -> Result<(), Error> {
        let mut this = self;
        let Some(receipt) = this.receipt.take() else {
            return Err(consumed_handle_error("operation"));
        };
        receipt
            .applied_with_timeout(this.control.clone(), timeout)
            .await
    }

    /// Records cancellation intent for this operation and returns its exact
    /// terminal cancellation observer.
    ///
    /// # A refused cancellation hands this handle back
    ///
    /// Cancelling a request that is still queued always succeeds. Cancelling
    /// one that has already been written needs profile support for the
    /// standard VISCA socket-cancel command; without it — [`PtzOpticsG2`] is
    /// the only built-in profile in that position — the owner refuses with
    /// [`Error::NotSupported`] and deliberately leaves the original request
    /// scheduled, retryable, and able to complete.
    ///
    /// Because the original is still live, this method only consumes the
    /// handle when it succeeds. A refusal returns [`CancelRejected`], which
    /// carries the handle back so the caller can keep waiting on it, retry the
    /// cancel, or drop it to detach. Recovering the handle does not stop the
    /// camera; a moving axis ends with an applied typed STOP.
    ///
    /// `?` in a function returning [`Error`] still works — the [`From`]
    /// conversion keeps the reason and detaches the handle.
    ///
    /// [`PtzOpticsG2`]: crate::profiles::PtzOpticsG2
    pub async fn cancel(self) -> Result<Cancellation, CancelRejected<Self>> {
        let mut this = self;
        let Some(receipt) = this.receipt.take() else {
            return Err(CancelRejected::new(
                None,
                consumed_handle_error("operation"),
            ));
        };
        let control = this.control.clone();
        let id = this.id;
        match receipt.cancel().await {
            Ok(receipt) => Ok(Cancellation::from_receipt(receipt, control)),
            Err((receipt, error)) => Err(CancelRejected::new(
                receipt.map(|receipt| Self {
                    receipt: Some(receipt),
                    control,
                    id,
                }),
                error,
            )),
        }
    }

    /// Explicitly relinquishes this operation's observer without changing
    /// protocol state, cancelling the request, or sending a physical STOP.
    ///
    /// This is the explicit spelling of what dropping the handle already does;
    /// movement continues until something else ends it.
    pub fn detach(self) {
        let mut this = self;
        if let Some(receipt) = this.receipt.take() {
            receipt.detach();
        }
    }
}

impl Operation<completion::Targeted> {
    /// Waits for exact application and then the profile-selected protocol
    /// settlement condition using the prepared plan and configured observer
    /// deadline.
    pub async fn settled(self) -> Result<(), Error> {
        let mut this = self;
        let Some(receipt) = this.receipt.take() else {
            return Err(consumed_handle_error("operation"));
        };
        receipt.settled(this.control.clone()).wait().await.map(drop)
    }

    /// Waits for exact application and the profile-selected protocol settlement
    /// condition using an explicit observer deadline. The prepared scheduler
    /// and settlement policy are not changed.
    pub async fn settled_with_timeout(self, timeout: Duration) -> Result<(), Error> {
        let mut this = self;
        let Some(receipt) = this.receipt.take() else {
            return Err(consumed_handle_error("operation"));
        };
        receipt
            .settled_with_timeout(this.control.clone(), timeout)
            .wait()
            .await
            .map(drop)
    }
}

impl<K> Drop for Operation<K>
where
    K: completion::Kind,
{
    fn drop(&mut self) {
        // Drop is detach: it relinquishes observation only. The owner keeps
        // protocol state, never interprets handle drop as cancellation, and no
        // STOP is emitted.
        if let Some(receipt) = self.receipt.take() {
            receipt.detach();
        }
    }
}

/// A linear async cancellation observer for one exact operation.
///
/// The token retains the operation's original terminal receiver.  Its outcome
/// therefore reports whether cancellation won (`Cancelled`) or the original
/// operation completed first (`Completed`) without introducing a second
/// lifecycle source.
#[must_use = "observe or explicitly detach this cancellation"]
#[derive(Debug)]
pub struct Cancellation {
    receipt: Option<AsyncCancellationReceipt>,
    control: AsyncReceiptControl,
}

impl Cancellation {
    pub(crate) fn from_receipt(
        receipt: AsyncCancellationReceipt,
        control: AsyncReceiptControl,
    ) -> Self {
        Self {
            receipt: Some(receipt),
            control,
        }
    }

    /// Observes the exact terminal cancellation outcome using the supplied
    /// observer deadline.
    pub async fn outcome(self, timeout: Duration) -> Result<CancellationOutcome, Error> {
        let mut this = self;
        let Some(receipt) = this.receipt.take() else {
            return Err(consumed_handle_error("cancellation"));
        };
        receipt.outcome(this.control.clone(), timeout).await
    }

    /// Explicitly relinquishes cancellation observation without undoing the
    /// recorded cancellation intent.
    pub fn detach(self) {
        let mut this = self;
        if let Some(receipt) = this.receipt.take() {
            receipt.detach();
        }
    }
}

impl Drop for Cancellation {
    fn drop(&mut self) {
        if let Some(receipt) = self.receipt.take() {
            receipt.detach();
        }
    }
}

fn consumed_handle_error(kind: &'static str) -> Error {
    Error::InvalidState(format!("{kind} handle was already consumed").into())
}
