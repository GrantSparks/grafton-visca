#![cfg(feature = "async")]

use std::{future::Future, marker::PhantomData, time::Duration};

use grafton_visca::{
    completion::{AppliedOnly, Targeted},
    CancelRejected, Cancellation, CancellationOutcome, Error, Operation, OperationId,
};

fn assert_send_sync<T: Send + Sync>() {}

async fn applied_only(handle: Operation<AppliedOnly>) -> Result<(), Error> {
    let id: OperationId = handle.id();
    let _ = id.get();
    handle.applied_with_timeout(Duration::from_secs(1)).await
}

async fn targeted(handle: Operation<Targeted>) -> Result<(), Error> {
    let _: OperationId = handle.id();
    handle.settled().await
}

async fn cancellation(token: Cancellation) -> Result<CancellationOutcome, Error> {
    token.outcome(Duration::from_secs(1)).await
}

/// A refused cancellation hands the handle back, so the caller is never left
/// holding nothing (#612), and `?` still coerces into [`Error`].
async fn cancel_recovers(handle: Operation<AppliedOnly>) -> Result<Cancellation, Error> {
    match handle.cancel().await {
        Ok(token) => Ok(token),
        Err(rejected) => {
            let (handle, error): (Option<Operation<AppliedOnly>>, Error) = rejected.into_parts();
            match handle {
                Some(handle) => handle.cancel().await.map_err(Error::from),
                None => Err(error),
            }
        }
    }
}

fn generic_free_handles() {
    // The only operation parameter is the closed completion marker.  Profiles,
    // transports, executors, and runtime kinds do not occur in these types.
    let _: PhantomData<Operation<AppliedOnly>> = PhantomData;
    let _: PhantomData<Operation<Targeted>> = PhantomData;
    let _: PhantomData<Cancellation> = PhantomData;
    assert_send_sync::<Operation<AppliedOnly>>();
    assert_send_sync::<Operation<Targeted>>();
    assert_send_sync::<Cancellation>();
    assert_send_sync::<CancelRejected<Operation<AppliedOnly>>>();
}

fn consuming_terminal_actions() {
    fn applied_method(handle: Operation<AppliedOnly>) -> impl Future<Output = Result<(), Error>> {
        handle.applied()
    }

    fn targeted_method(handle: Operation<Targeted>) -> impl Future<Output = Result<(), Error>> {
        handle.settled_with_timeout(Duration::from_secs(1))
    }

    let _ = (
        applied_method,
        targeted_method,
        applied_only,
        targeted,
        cancellation,
        cancel_recovers,
    );
}

fn contract() {}

fn main() {}
