#![cfg(feature = "async")]

use std::{
    cell::Cell,
    marker::PhantomPinned,
    panic::{RefUnwindSafe, UnwindSafe},
};

use grafton_visca::runtime::Executor;

fn stable_join<J: Send + Sync + Unpin + UnwindSafe + RefUnwindSafe>() {}

fn main() {
    #[cfg(feature = "runtime-smol")]
    {
        use grafton_visca::{runtime::SmolExecutor, runtime::SmolRuntime};

        stable_join::<<SmolExecutor as Executor>::Join<u8>>();
        stable_join::<<SmolRuntime as Executor>::Join<u8>>();
        // Join handles remain shareable and movable even when their output
        // itself is not Sync or Unpin.
        stable_join::<<SmolExecutor as Executor>::Join<Cell<u8>>>();
        stable_join::<<SmolExecutor as Executor>::Join<PhantomPinned>>();
    }

    #[cfg(feature = "runtime-tokio")]
    {
        use grafton_visca::{runtime::TokioExecutor, runtime::TokioRuntime};

        stable_join::<<TokioExecutor as Executor>::Join<u8>>();
        stable_join::<<TokioRuntime as Executor>::Join<u8>>();
        stable_join::<<TokioExecutor as Executor>::Join<Cell<u8>>>();
        stable_join::<<TokioExecutor as Executor>::Join<PhantomPinned>>();
    }
}
