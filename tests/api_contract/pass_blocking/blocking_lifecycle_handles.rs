#![cfg(feature = "blocking")]

use std::{marker::PhantomData, time::Duration};

use grafton_visca::{
    blocking::{Cancellation, Operation, OperationId},
    completion::{AppliedOnly, Targeted},
    CancelRejected, CancellationOutcome, Error,
};

fn applied_only(handle: Operation<'static, AppliedOnly>) -> Result<(), Error> {
    let _: OperationId = handle.id();
    handle.applied()
}

fn applied_only_with_timeout(handle: Operation<'static, AppliedOnly>) -> Result<(), Error> {
    handle.applied_with_timeout(Duration::from_secs(1))
}

fn applied_only_cancel(
    handle: Operation<'static, AppliedOnly>,
) -> Result<Cancellation<'static>, CancelRejected<Operation<'static, AppliedOnly>>> {
    handle.cancel()
}

/// A refused cancellation hands the handle back, so the caller is never left
/// holding nothing (#612), and `?` still coerces into [`Error`].
fn applied_only_cancel_recovers(
    handle: Operation<'static, AppliedOnly>,
) -> Result<Cancellation<'static>, Error> {
    match handle.cancel() {
        Ok(token) => Ok(token),
        Err(rejected) => {
            let (handle, error): (Option<Operation<'static, AppliedOnly>>, Error) =
                rejected.into_parts();
            match handle {
                Some(handle) => handle.cancel().map_err(Error::from),
                None => Err(error),
            }
        }
    }
}

fn applied_only_detach(handle: Operation<'static, AppliedOnly>) {
    handle.detach();
}

fn targeted(handle: Operation<'static, Targeted>) -> Result<(), Error> {
    handle.settled()
}

fn targeted_with_timeout(handle: Operation<'static, Targeted>) -> Result<(), Error> {
    handle.settled_with_timeout(Duration::from_secs(1))
}

fn targeted_applied(handle: Operation<'static, Targeted>) -> Result<(), Error> {
    handle.applied()
}

fn targeted_cancel(
    handle: Operation<'static, Targeted>,
) -> Result<Cancellation<'static>, CancelRejected<Operation<'static, Targeted>>> {
    handle.cancel()
}

fn cancellation(token: Cancellation<'static>) -> Result<CancellationOutcome, Error> {
    token.outcome(Duration::from_secs(1))
}

fn cancellation_detach(token: Cancellation<'static>) {
    token.detach();
}

fn generic_free_handles() {
    let _: PhantomData<Operation<'static, AppliedOnly>> = PhantomData;
    let _: PhantomData<Operation<'static, Targeted>> = PhantomData;
    let _: PhantomData<Cancellation<'static>> = PhantomData;
    let _ = (
        applied_only,
        applied_only_with_timeout,
        applied_only_cancel,
        applied_only_cancel_recovers,
        applied_only_detach,
        targeted,
        targeted_with_timeout,
        targeted_applied,
        targeted_cancel,
        cancellation,
        cancellation_detach,
    );
}

fn main() {
    generic_free_handles();
}
