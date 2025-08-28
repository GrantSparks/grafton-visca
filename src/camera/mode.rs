//! Camera mode markers for compile-time async/blocking dispatch.
//!
//! This module provides zero-size type markers that enable a single Camera type
//! to support both async and blocking operations without runtime overhead.

/// Marker type for async mode cameras.
///
/// This zero-size type is used to specialize Camera implementations for async operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsyncMode {}

/// Marker type for blocking mode cameras.
///
/// This zero-size type is used to specialize Camera implementations for blocking operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockingMode {}

// Intentionally NO public generic async camera alias.
// Use runtime-specific aliases (TokioCamera, AsyncStdCamera, SmolCamera)
// that fix the executor type for ergonomics and clarity.

/// Type alias for blocking cameras.
///
/// This provides a convenient way to specify blocking cameras without the mode parameter.
#[cfg(not(feature = "async"))]
pub type CameraBlocking<P, T> = super::BlockingCamera<P, T>;

/// Sealed trait to prevent external mode implementations.
mod sealed {
    pub trait Sealed {}
    impl Sealed for super::AsyncMode {}
    impl Sealed for super::BlockingMode {}
}

/// Trait for camera operation modes.
///
/// This trait is sealed and can only be implemented by AsyncMode and BlockingMode.
pub trait CameraMode: sealed::Sealed + Send + Sync + 'static {
    /// Whether this mode is async.
    const IS_ASYNC: bool;
}

impl CameraMode for AsyncMode {
    const IS_ASYNC: bool = true;
}

impl CameraMode for BlockingMode {
    const IS_ASYNC: bool = false;
}
