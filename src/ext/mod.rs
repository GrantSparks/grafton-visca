//! Unified extension traits for VISCA camera control.
//!
//! This module provides a unified trait system that works seamlessly
//! with both async and blocking contexts, eliminating code duplication
//! while maintaining a consistent API.

pub mod unified_power;

// Re-export unified power trait (deprecated)
#[deprecated(since = "0.5.0", note = "Use `ViscaPowerExt` instead")]
pub use unified_power::UnifiedPowerExt;

/// Helper trait for unified extension implementations.
///
/// This trait helps bridge the gap between sync and async implementations
/// by providing associated types that work with both contexts.
pub trait UnifiedExt: crate::ViscaDevice {
    /// The future type returned by extension methods.
    type ExtFuture<'a, T>: std::future::Future<Output = Result<T, crate::error::Error>> + Send + 'a
    where
        Self: 'a,
        T: Send + 'a;
}

// Helper types for ready futures in blocking context
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// A future that is immediately ready with a value.
///
/// Used to implement async traits in blocking contexts.
pub struct Ready<T>(Option<T>);

impl<T> Ready<T> {
    /// Create a new ready future.
    pub fn new(value: T) -> Self {
        Self(Some(value))
    }
}

impl<T> Future for Ready<T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(self.0.take().expect("Ready polled after completion"))
    }
}

impl<T> Unpin for Ready<T> {}