//! Synchronization primitives that work in both blocking and async contexts.
//!
//! This module provides unified types for mutex that automatically
//! select the appropriate implementation based on enabled features.

// Re-export the appropriate mutex type based on features
#[cfg(all(feature = "async", feature = "tokio"))]
pub use tokio::sync::Mutex;

#[cfg(not(feature = "async"))]
pub use parking_lot::Mutex;

// When async is enabled but tokio is not, we need a different approach
#[cfg(all(feature = "async", not(feature = "tokio")))]
pub use futures::lock::Mutex;
