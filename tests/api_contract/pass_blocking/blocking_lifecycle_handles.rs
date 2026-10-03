#![cfg(feature = "blocking")]

use std::{marker::PhantomData, time::Duration};

use grafton_visca::{
    blocking::{Operation, OperationId},
    completion::{AppliedOnly, Targeted},
    CancellationOutcome, Error,
};

fn applied_only(mut handle: Operation<'static, AppliedOnly>) -> Result<(), Error> {
    let _: OperationId = handle.id();
    handle.applied_with_timeout(Duration::from_secs(1))?;
    handle.applied()
}

/// Waits borrow the handle (#777): application then settlement on one handle,
/// and a timed-out wait leaves the handle usable.
fn targeted(mut handle: Operation<'static, Targeted>) -> Result<(), Error> {
    if handle
        .applied_with_timeout(Duration::from_millis(1))
        .is_err()
    {
        handle.applied()?;
    }
    handle.settled()?;
    handle.settled_with_timeout(Duration::from_secs(1))
}

/// Cancellation borrows the handle and is idempotent; the outcome enum is
/// non-exhaustive, so a match needs a wildcard arm. A refused cancellation is
/// a plain error and the handle keeps observing the operation.
fn cancellation(mut handle: Operation<'static, Targeted>) -> Result<bool, Error> {
    match handle.cancel_with_timeout(Duration::from_secs(1)) {
        Ok(_) => {}
        Err(Error::NotSupported) => return handle.settled().map(|()| false),
        Err(error) => return Err(error),
    }
    let cancelled = match handle.cancel()? {
        CancellationOutcome::Cancelled => true,
        CancellationOutcome::Completed => false,
        _ => false,
    };
    handle.detach();
    Ok(cancelled)
}

fn borrowing_waits() {
    fn applied_method(handle: &mut Operation<'static, AppliedOnly>) -> Result<(), Error> {
        handle.applied()
    }

    fn cancel_method(
        handle: &mut Operation<'static, Targeted>,
    ) -> Result<CancellationOutcome, Error> {
        handle.cancel()
    }

    let _ = (applied_method, cancel_method);
}

fn generic_free_handles() {
    let _: PhantomData<Operation<'static, AppliedOnly>> = PhantomData;
    let _: PhantomData<Operation<'static, Targeted>> = PhantomData;
    let _ = (applied_only, targeted, cancellation, borrowing_waits);
}

fn main() {
    generic_free_handles();
}
