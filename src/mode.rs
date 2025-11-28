//! Mode trait system for unified async/blocking API.
//!
//! This module provides the Mode trait that enables a single API surface
//! to work in both blocking and async modes through type-state parameters.

#[cfg(feature = "mode-async")]
use async_lock;

use core::{future::Ready, pin::Pin};
use std::future::Future;

/// Type alias for boxed futures used in async mode.
/// This provides a more readable name for the boxed future type.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Mode trait that abstracts over async and blocking execution modes.
///
/// This trait uses GATs (Generic Associated Types) to allow the same API
/// to work in both async and blocking contexts through type parameters.
///
/// The lifetime-aware return type allows async futures to borrow from `&self`,
/// eliminating the need for all transports to be `Sync`.
pub trait Mode {
    /// The return type for operations in this mode.
    ///
    /// For async mode, this will be a boxed Future that can borrow from the caller.
    /// For blocking mode, this will be a `Ready<T>` (immediate result).
    type Fut<'a, T>: Future<Output = T> + Send + 'a
    where
        Self: 'a,
        T: Send + 'a;

    /// Mode-specific storage for shared state (like the transport).
    ///
    /// For async mode, this uses `Arc<async_lock::Mutex<T>>` to allow sharing.
    /// For blocking mode, this stores `T` directly without synchronization overhead.
    type Shared<T>;

    /// Create shared storage for the given value.
    fn share<T>(value: T) -> Self::Shared<T>;

    /// Create a return value from a result.
    fn ready<'a, T>(result: T) -> Self::Fut<'a, T>
    where
        T: Send + 'a;

    /// Create a return value from a future.
    ///
    /// This method accepts futures with non-'static lifetimes, allowing
    /// futures to borrow from the caller without requiring 'static promotion.
    ///
    /// # Behavior by mode
    ///
    /// - **Async mode**: Wraps the future in a boxed future, evaluated when awaited.
    /// - **Blocking mode**: Executes the future eagerly/synchronously and returns
    ///   an already-completed `Ready<T>`.
    fn from_future<'a, F, T>(future: F) -> Self::Fut<'a, T>
    where
        F: Future<Output = T> + Send + 'a,
        T: Send + 'a;
}

/// Zero-sized type representing async execution mode.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Async;

/// Zero-sized type representing blocking execution mode.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Blocking;

impl Mode for Async {
    // NOTE: We use boxed futures here because TAIT (Type Alias Impl Trait)
    // in associated types is not yet stable in Rust. Once it becomes stable,
    // this can be changed to `impl Future<Output = T> + Send + 'a` to achieve
    // zero-cost futures. The current approach still avoids double-boxing and
    // maintains a single allocation per async operation.
    // See: https://github.com/rust-lang/rust/issues/63063
    type Fut<'a, T>
        = BoxFuture<'a, T>
    where
        T: Send + 'a;

    #[cfg(feature = "mode-async")]
    type Shared<T> = std::sync::Arc<async_lock::Mutex<T>>;
    #[cfg(not(feature = "mode-async"))]
    type Shared<T> = T; // Fallback for when async is not available

    #[cfg(feature = "mode-async")]
    fn share<T>(value: T) -> Self::Shared<T> {
        std::sync::Arc::new(async_lock::Mutex::new(value))
    }

    #[cfg(not(feature = "mode-async"))]
    fn share<T>(value: T) -> Self::Shared<T> {
        value
    }

    fn ready<'a, T>(result: T) -> Self::Fut<'a, T>
    where
        T: Send + 'a,
    {
        Box::pin(core::future::ready(result))
    }

    fn from_future<'a, F, T>(future: F) -> Self::Fut<'a, T>
    where
        F: Future<Output = T> + Send + 'a,
        T: Send + 'a,
    {
        Box::pin(future)
    }
}

impl Mode for Blocking {
    type Fut<'a, T>
        = Ready<T>
    where
        T: Send + 'a;

    type Shared<T> = std::cell::RefCell<T>;

    fn share<T>(value: T) -> Self::Shared<T> {
        std::cell::RefCell::new(value)
    }

    fn ready<'a, T>(result: T) -> Self::Fut<'a, T>
    where
        T: Send + 'a,
    {
        core::future::ready(result)
    }

    fn from_future<'a, F, T>(future: F) -> Self::Fut<'a, T>
    where
        F: Future<Output = T> + Send + 'a,
        T: Send + 'a,
    {
        // Execute the future synchronously using pollster and wrap in Ready<T>.
        // This allows mode-generic code (like impl_diagnostics!) to work with both
        // Async and Blocking modes without special-casing.
        let value = pollster::block_on(future);
        core::future::ready(value)
    }
}

/// Extension trait for blocking futures to provide ergonomic API.
///
/// This trait allows blocking mode futures (which are `Ready<T>`) to be
/// easily converted to their inner values without needing explicit
/// `pollster::block_on()` calls everywhere.
#[cfg(not(feature = "mode-async"))]
pub trait BlockingFutureExt: Future {
    /// Block on this future and return its output.
    ///
    /// For `Ready<T>` futures (used in blocking mode), this is a no-op
    /// that immediately returns the value.
    fn block(self) -> Self::Output
    where
        Self: Sized;

    /// Alias for `block()` that provides a more intuitive name for extracting
    /// the inner value from a `Ready<T>` future.
    ///
    /// This is semantically identical to `block()` but may be clearer in contexts
    /// where you're working with immediately-ready futures.
    #[inline]
    fn into_inner(self) -> Self::Output
    where
        Self: Sized,
    {
        self.block()
    }
}

#[cfg(not(feature = "mode-async"))]
impl<T> BlockingFutureExt for Ready<T> {
    #[inline]
    fn block(self) -> T {
        // NOTE: Ready futures are immediately ready, so we can use pollster
        // which is zero-cost for already-ready futures
        pollster::block_on(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_markers_are_zero_sized() {
        use std::mem::size_of;
        assert_eq!(size_of::<Async>(), 0);
        assert_eq!(size_of::<Blocking>(), 0);
    }

    #[tokio::test]
    async fn test_async_mode() {
        let result = Async::ready(42).await;
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_blocking_mode() {
        let result = Blocking::ready("test").await;
        assert_eq!(result, "test");
    }

    #[tokio::test]
    async fn test_async_from_future() {
        let fut = Async::from_future(async { 42 });
        let result = fut.await;
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_blocking_from_future() {
        // Blocking::from_future executes the future synchronously and returns Ready<T>
        let fut = Blocking::from_future(async { 42 });
        let result = fut.await;
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_blocking_from_future_with_computation() {
        // Test that the async block is actually executed
        let fut = Blocking::from_future(async {
            let a = 10;
            let b = 20;
            a + b
        });
        let result = fut.await;
        assert_eq!(result, 30);
    }

    #[test]
    fn test_blocking_from_future_returns_ready() {
        // Verify that Blocking::from_future returns a Ready<T> (already completed)
        use core::task::{Context, Poll};
        use std::pin::Pin;

        let fut = Blocking::from_future(async { "test_value" });

        // Create a dummy waker and context using futures-lite (dev dependency)
        let waker = futures_lite::future::block_on(std::future::poll_fn(|cx| {
            Poll::Ready(cx.waker().clone())
        }));
        let mut cx = Context::from_waker(&waker);

        // Ready<T> should be immediately ready without needing to be polled multiple times
        let mut fut = fut;
        let pinned = Pin::new(&mut fut);
        match pinned.poll(&mut cx) {
            Poll::Ready(val) => assert_eq!(val, "test_value"),
            Poll::Pending => {
                unreachable!("Blocking::from_future should return Ready<T>, not Pending")
            }
        }
    }
}
