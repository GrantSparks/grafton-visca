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

crate::operation_handle::operation_handle! {
    receipt: AsyncOperationReceipt,
    async: [async],
    await: [.await],
    description: "An async handle on one admitted operation.",
    wait_doc: "Dropping a waiting future releases only that wait, including a losing `select!` branch. A later wait continues observing the same operation or cancellation intent.",
    must_use: "await, cancel, or explicitly detach this operation",
}
