//! Mode trait system for unified async/blocking API.
//!
//! This module provides the Mode trait that enables a single API surface
//! to work in both blocking and async modes through type-state parameters.

#[cfg(feature = "async")]
use async_lock;

use core::{
    future::{ready, Ready},
    pin::Pin,
};
use std::future::Future;

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
    type Ret<'a, T>: Future<Output = T> + Send + 'a
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
    fn ret<T>(result: T) -> Self::Ret<'static, T>
    where
        T: Send + 'static;

    /// Create a return value from a future.
    fn ret_fut<F, T>(future: F) -> Self::Ret<'static, T>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static;
}

/// Zero-sized type representing async execution mode.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Async;

/// Zero-sized type representing blocking execution mode.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Blocking;

impl Mode for Async {
    type Ret<'a, T>
        = Pin<Box<dyn Future<Output = T> + Send + 'a>>
    where
        T: Send + 'a;

    #[cfg(feature = "async")]
    type Shared<T> = std::sync::Arc<async_lock::Mutex<T>>;
    #[cfg(not(feature = "async"))]
    type Shared<T> = T; // Fallback for when async is not available

    #[cfg(feature = "async")]
    fn share<T>(value: T) -> Self::Shared<T> {
        std::sync::Arc::new(async_lock::Mutex::new(value))
    }

    #[cfg(not(feature = "async"))]
    fn share<T>(value: T) -> Self::Shared<T> {
        value // Fallback for when async is not available
    }

    fn ret<T>(result: T) -> Self::Ret<'static, T>
    where
        T: Send + 'static,
    {
        Box::pin(ready(result))
    }

    fn ret_fut<F, T>(future: F) -> Self::Ret<'static, T>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        Box::pin(future)
    }
}

impl Mode for Blocking {
    type Ret<'a, T>
        = Ready<T>
    where
        T: Send + 'a;

    type Shared<T> = std::cell::RefCell<T>;

    fn share<T>(value: T) -> Self::Shared<T> {
        std::cell::RefCell::new(value)
    }

    fn ret<T>(result: T) -> Self::Ret<'static, T>
    where
        T: Send + 'static,
    {
        ready(result)
    }

    fn ret_fut<F, T>(_future: F) -> Self::Ret<'static, T>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        // For blocking mode, futures should be avoided.
        unreachable!("Blocking mode should not use futures directly - use Mode::ret() instead")
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
        let result = Async::ret(42).await;
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_blocking_mode() {
        let result = Blocking::ret("test").await;
        assert_eq!(result, "test");
    }
}
