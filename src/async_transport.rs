// Standard library imports
use std::future::Future;
use std::pin::Pin;

// Crate imports
use crate::{ViscaCommand, ViscaError};

/// Type alias for the future returned by async transport methods
pub type TransportFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ViscaError>> + Send + 'a>>;

/// Async version of the `ViscaTransport` trait for non-blocking I/O operations.
///
/// This trait provides the async equivalent of `ViscaTransport`, allowing for
/// concurrent command execution and non-blocking network operations.
///
/// # Deprecated
/// This trait is deprecated in favor of the unified `Transport` trait in the `transport` module.
#[cfg(feature = "async-client")]
#[deprecated(since = "0.4.0", note = "Use `transport::Transport` trait instead")]
pub trait AsyncViscaTransport: Send + Sync {
    /// Send a VISCA command to the camera asynchronously.
    fn send_command<'a>(&'a mut self, command: &'a dyn ViscaCommand) -> TransportFuture<'a, ()>;

    /// Receive response frames from the camera asynchronously.
    ///
    /// Returns a vector of response frames that have been received.
    /// May return an empty vector if no complete frames are available yet.
    fn receive_response(&mut self) -> TransportFuture<'_, Vec<Vec<u8>>>;
}
