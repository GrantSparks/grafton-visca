#![cfg(feature = "async")]

use std::{future::Future, marker::PhantomData, pin::Pin, time::Duration};

use grafton_visca::{
    completion::{AppliedOnly, Targeted},
    CancellationOutcome, Error, Operation, OperationId, Settlement,
};

fn assert_send_sync<T: Send + Sync>() {}
fn assert_send<T: Send>(_: &T) {}

async fn applied_only(mut handle: Operation<AppliedOnly>) -> Result<(), Error> {
    let id: OperationId = handle.id();
    let _ = id.get();
    handle.applied_with_timeout(Duration::from_secs(1)).await
}

/// Waits borrow the handle (#777): application then settlement on one handle,
/// and a timed-out wait leaves the handle usable.
async fn targeted(mut handle: Operation<Targeted>) -> Result<Settlement, Error> {
    let _: OperationId = handle.id();
    if handle
        .applied_with_timeout(Duration::from_millis(1))
        .await
        .is_err()
    {
        handle.applied().await?;
    }
    handle.settled().await?;
    handle.settled_with_timeout(Duration::from_secs(1)).await
}

/// Cancellation borrows the handle and is idempotent; the outcome enum is
/// non-exhaustive, so a match needs a wildcard arm.
async fn cancellation(mut handle: Operation<AppliedOnly>) -> Result<bool, Error> {
    let _ = handle.cancel_with_timeout(Duration::from_secs(1)).await;
    let cancelled = match handle.cancel().await? {
        CancellationOutcome::Cancelled => true,
        CancellationOutcome::Completed => false,
        _ => false,
    };
    handle.detach();
    Ok(cancelled)
}

fn generic_free_handles() {
    // The only operation parameter is the closed completion marker. Profiles,
    // transports, executors, and runtime kinds do not occur in these types.
    let _: PhantomData<Operation<AppliedOnly>> = PhantomData;
    let _: PhantomData<Operation<Targeted>> = PhantomData;
    assert_send_sync::<Operation<AppliedOnly>>();
    assert_send_sync::<Operation<Targeted>>();
    assert_send_sync::<CancellationOutcome>();
}

fn wait_futures_are_send(handle: &mut Operation<Targeted>) {
    assert_send(&handle.applied());
    assert_send(&handle.settled());
    assert_send(&handle.cancel());
}

/// A user's object-safe trait can box the borrowing waits.
trait Observe {
    fn observe(&mut self) -> Pin<Box<dyn Future<Output = Result<Settlement, Error>> + Send + '_>>;
}

impl Observe for Operation<Targeted> {
    fn observe(&mut self) -> Pin<Box<dyn Future<Output = Result<Settlement, Error>> + Send + '_>> {
        Box::pin(self.settled())
    }
}

fn borrowing_waits() {
    fn applied_method(
        handle: &mut Operation<AppliedOnly>,
    ) -> impl Future<Output = Result<(), Error>> + '_ {
        handle.applied()
    }

    fn targeted_method(
        handle: &mut Operation<Targeted>,
    ) -> impl Future<Output = Result<Settlement, Error>> + '_ {
        handle.settled_with_timeout(Duration::from_secs(1))
    }

    let _ = (
        applied_method,
        targeted_method,
        applied_only,
        targeted,
        cancellation,
        wait_futures_are_send,
    );
    let _: Option<Box<dyn Observe>> = None;
}

fn contract() {}

fn main() {}
