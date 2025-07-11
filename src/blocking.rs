//! Minimal blocking executor for synchronous operation.
//!
//! This module provides a lightweight way to execute futures synchronously
//! without requiring an async runtime. Used internally by CameraBlocking.

use core::future::Future;
use core::task::{Context, Poll, Waker};
use std::sync::Arc;
use crate::Error;

/// Block on a future, returning its output.
/// 
/// This is a minimal executor that simply polls the future once.
/// For `Ready` futures (as used by blocking transports), this is
/// optimized away by the compiler.
pub fn block_on<F: Future>(fut: F) -> F::Output {
    // Safety: We're pinning the future to execute it
    let mut fut = Box::pin(fut);
    
    // Create a no-op waker (blocking futures should be Ready)
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    
    // Poll the future - for Ready futures this returns immediately
    match fut.as_mut().poll(&mut cx) {
        Poll::Ready(val) => val,
        Poll::Pending => {
            // This should not happen with blocking transports
            panic!("Blocking transport returned Pending future");
        }
    }
}

/// Execute a future with a timeout.
/// 
/// For blocking transports, this uses a simple deadline-based approach.
pub fn timeout<F: Future>(duration: core::time::Duration, fut: F) -> Result<F::Output, Error> {
    use std::time::Instant;
    
    let deadline = Instant::now() + duration;
    let mut fut = Box::pin(fut);
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(val) => return Ok(val),
            Poll::Pending => {
                if Instant::now() >= deadline {
                    return Err(Error::Timeout);
                }
                // For truly async futures in blocking context
                // Sleep briefly to avoid busy waiting
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
    }
}

/// Create a no-op waker for blocking execution.
fn noop_waker() -> Waker {
    struct NoopWaker;
    
    impl std::task::Wake for NoopWaker {
        fn wake(self: Arc<Self>) {}
        fn wake_by_ref(self: &Arc<Self>) {}
    }
    
    Arc::new(NoopWaker).into()
}