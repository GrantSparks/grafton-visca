//! Mode trait system for unified async/blocking API.
//!
//! This module provides the Mode trait that enables a single API surface
//! to work in both blocking and async modes through type-state parameters.

use core::{
    future::{ready, Ready},
    pin::Pin,
};
use std::future::Future;

/// Mode trait that abstracts over async and blocking execution modes.
///
/// This trait uses GATs (Generic Associated Types) to allow the same API
/// to work in both async and blocking contexts through type parameters.
pub trait Mode {
    /// The return type for operations in this mode.
    ///
    /// For async mode, this will be a boxed Future for flexibility.
    /// For blocking mode, this will be a Ready<T> (immediate result).
    type Ret<T>: Future<Output = T> + Send
    where
        T: Send;

    /// Create a return value from a result.
    fn ret<T>(result: T) -> Self::Ret<T>
    where
        T: Send + 'static;

    /// Create a return value from a future.
    fn ret_fut<F, T>(future: F) -> Self::Ret<T>
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
    type Ret<T>
        = Pin<Box<dyn Future<Output = T> + Send>>
    where
        T: Send;

    fn ret<T>(result: T) -> Self::Ret<T>
    where
        T: Send + 'static,
    {
        Box::pin(ready(result))
    }

    fn ret_fut<F, T>(future: F) -> Self::Ret<T>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        Box::pin(future)
    }
}

impl Mode for Blocking {
    type Ret<T>
        = Ready<T>
    where
        T: Send;

    fn ret<T>(result: T) -> Self::Ret<T>
    where
        T: Send + 'static,
    {
        ready(result)
    }

    fn ret_fut<F, T>(_future: F) -> Self::Ret<T>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        // For blocking mode, futures should be avoided.
        // This is primarily for API completeness - real implementations
        // should call ret() with already-resolved values.
        panic!("Blocking mode should not use futures directly - use Mode::ret() instead")
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
