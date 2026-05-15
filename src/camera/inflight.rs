//! Typed operation handles for long-running camera commands.
//!
//! This module provides `InFlight<C>` handles that are returned by long-running
//! control methods (like pan/tilt, zoom, focus movements). These handles enable:
//!
//! - **Socket-safe cancellation**: Cancel commands by ID without needing to know
//!   which socket they're using. The runtime handles socket resolution automatically.
//! - **Exact completion waits**: Each handle owns the response future for the
//!   command it represents, so completion and command errors are never inferred
//!   from later movement state.
//!
//! # Design
//!
//! The design uses zero-sized type (ZST) markers to preserve the command category
//! in the type system while the handle stores the command's own response future.
//! Cancellation stays ID-based, and completion waits are tied to the exact VISCA
//! response channel returned when the command was submitted.
//!
//! # Example
//!
//! ```ignore
//! use grafton_visca::{camera::Connect, camera::profiles::PtzOpticsG2};
//! use std::time::Duration;
//!
//! # async fn example() -> Result<(), grafton_visca::Error> {
//! let camera = Connect::open_tcp_async::<PtzOpticsG2, _>("192.168.0.110", runtime).await?;
//!
//! // Start a pan/tilt operation and get a typed handle
//! let handle = camera.pan_tilt().pan_tilt_absolute_op(45.0, 15.0, SpeedLevel::Fast).await?;
//!
//! // Wait for the command's own completion response
//! handle.await_completion(Duration::from_secs(5)).await?;
//!
//! // Or cancel the operation
//! handle.cancel().await?;
//! # Ok(())
//! # }
//! ```

#[cfg(feature = "mode-async")]
use core::{future::Future, marker::PhantomData, pin::Pin, time::Duration};

#[cfg(feature = "mode-async")]
use std::sync::Mutex;

#[cfg(feature = "mode-async")]
use crate::{
    camera_id::CameraId, capabilities::Profile, command::Response, error::Error,
    executor::Executor, Result,
};

use core::num::NonZeroU32;

/// Opaque, type-safe identifier for in-flight commands.
///
/// This newtype provides compile-time safety for command IDs:
/// - Cannot be constructed directly by external callers (private field)
/// - Only the library runtime can create valid `CommandId`s
/// - Guaranteed to be non-zero by construction
/// - Enables niche optimization for `Option<CommandId>` (zero-cost)
///
/// # Obtaining a CommandId
///
/// `CommandId`s are returned by async camera methods such as `Camera::start_command_with_id`
/// and `InFlight::id` (requires `mode-async` feature).
///
/// # Cancellation
///
/// Use the returned `CommandId` with `Camera::cancel` to cancel a running command
/// (requires `mode-async` feature).
///
/// # Example
///
/// ```ignore
/// // Start a command and get its ID
/// let (id, future) = camera.start_command_with_id(&cmd).await?;
///
/// // Cancel the command by its ID
/// camera.cancel(id).await?;
///
/// // Queued commands resolve with CommandCanceled; already-sent commands
/// // resolve according to the camera's cancel response or later runtime state.
/// let result = future.await;
/// assert!(result.is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommandId(NonZeroU32);

impl CommandId {
    /// Creates a new `CommandId` from a raw `u32`, returning `None` if the value is zero.
    ///
    /// This is only available within the crate during ID generation.
    /// Used by both async runtime (RuntimeHandle) and blocking runtime (BlockingRunner).
    #[inline]
    pub(crate) fn from_raw(value: u32) -> Option<Self> {
        NonZeroU32::new(value).map(Self)
    }

    /// Returns the underlying `u32` value.
    ///
    /// This is useful for logging, debugging, and protocol serialization.
    /// The returned value is guaranteed to be non-zero.
    #[inline]
    #[must_use]
    pub fn get(self) -> u32 {
        self.0.get()
    }

    /// Returns the underlying `NonZeroU32` value.
    ///
    /// This provides direct access to the non-zero type for cases
    /// that need to preserve the non-zero guarantee.
    #[inline]
    #[must_use]
    pub fn as_nonzero(self) -> NonZeroU32 {
        self.0
    }
}

impl core::fmt::Display for CommandId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Zero-sized marker for pan/tilt operations.
#[derive(Debug, Clone, Copy, Default)]
pub struct PanTilt;

/// Zero-sized marker for zoom operations.
#[derive(Debug, Clone, Copy, Default)]
pub struct Zoom;

/// Zero-sized marker for focus operations.
#[derive(Debug, Clone, Copy, Default)]
pub struct Focus;

/// Zero-sized marker for preset operations.
#[derive(Debug, Clone, Copy, Default)]
pub struct Preset;

/// Type alias for boxed response futures returned by `start_command_with_id`.
#[cfg(feature = "mode-async")]
pub type ResponseFuture = Pin<Box<dyn Future<Output = Result<Response, Error>> + Send + 'static>>;

/// A typed handle to an in-flight camera command.
///
/// This handle provides:
/// - **Command ID access** for debugging and telemetry
/// - **Socket-safe cancellation** via the runtime's ID-based cancel path
/// - **Direct completion waiting** via the stored response future
///
/// The handle stores a response future that completes when the camera sends
/// a VISCA completion message. This ensures the completion event is never
/// missed, even if it arrives immediately after the command is sent.
///
/// # Type Parameters
///
/// - `'a`: Lifetime of the camera/session reference
/// - `C`: Category marker (PanTilt, Zoom, Focus, or Preset)
/// - `P`: Camera profile type
/// - `Exec`: Async executor type
#[cfg(feature = "mode-async")]
pub struct InFlight<'a, C, P, Exec>
where
    P: Profile,
    Exec: Executor,
{
    /// The command ID assigned by the runtime.
    id: CommandId,
    /// Camera ID for addressing cancel messages.
    camera_id: CameraId,
    /// Runtime that owns the in-flight command.
    runtime: &'a crate::runtime::RuntimeHandle<P, Exec>,
    /// The response future that completes when the camera reports completion.
    /// Wrapped in Mutex to allow `await_completion` to take ownership.
    response_future: Mutex<Option<ResponseFuture>>,
    /// Zero-sized marker for the category.
    _c: PhantomData<C>,
}

#[cfg(feature = "mode-async")]
impl<'a, C, P, Exec> InFlight<'a, C, P, Exec>
where
    P: Profile + 'static,
    Exec: Executor + Send + Sync + 'static,
{
    /// Create a new in-flight handle with the response future.
    ///
    /// The response future completes when the camera sends a VISCA completion
    /// message. This ensures the completion event is captured even if it arrives
    /// immediately after the command is sent.
    ///
    /// This is public within the crate but not exposed to external users.
    #[inline]
    pub(crate) fn new(
        id: CommandId,
        camera_id: CameraId,
        runtime: &'a crate::runtime::RuntimeHandle<P, Exec>,
        response_future: ResponseFuture,
    ) -> Self {
        Self {
            id,
            camera_id,
            runtime,
            response_future: Mutex::new(Some(response_future)),
            _c: PhantomData,
        }
    }

    /// Get the command ID assigned by the runtime.
    ///
    /// This is useful for debugging and telemetry purposes.
    #[inline]
    pub fn id(&self) -> CommandId {
        self.id
    }

    /// Cancel this command via the runtime's ID-based cancel path.
    ///
    /// The cancel message is addressed to the camera ID that was used when
    /// this command was originally sent, ensuring correct multi-camera behavior.
    ///
    /// # Errors
    ///
    /// Returns an error if the cancellation request cannot be sent to the runtime.
    pub async fn cancel(&self) -> Result<()> {
        self.runtime.cancel(self.camera_id, self.id).await
    }

    /// Wait for this operation to complete.
    ///
    /// This method awaits the response future stored in this handle, which
    /// completes when the camera sends a VISCA completion message. The timeout
    /// parameter controls how long to wait for the camera's response.
    ///
    /// # Arguments
    ///
    /// - `timeout`: Maximum time to wait for completion
    ///
    /// # Returns
    ///
    /// - `Ok(())` if the operation completed within the timeout
    /// - `Err(Error::Timeout)` if the timeout elapsed before completion
    /// - `Err(Error::*)` for other communication or camera errors
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The wait times out (`Error::Timeout`)
    /// - This method is called more than once (`Error::InvalidState`)
    /// - The internal mutex is poisoned (`Error::LockPoisoned`)
    /// - Other communication or camera errors occur
    pub async fn await_completion(&self, timeout: Duration) -> Result<()> {
        // Take the response future from the mutex (can only be awaited once)
        let future = self
            .response_future
            .lock()
            .ok()
            .ok_or(Error::LockPoisoned("InFlight response_future"))?
            .take()
            .ok_or_else(|| Error::InvalidState("await_completion called more than once".into()))?;

        // Use the executor's timeout mechanism to enforce the deadline
        self.runtime.timeout(timeout, future).await?.map(|_| ())
    }

    /// Consume this handle and return the parts needed for type-erased handling.
    ///
    /// This is used by the dynamic API so `InFlightDyn` can preserve the exact
    /// command response future from the static API instead of approximating
    /// completion with category-level idle polling.
    #[cfg(feature = "dyn-api")]
    pub(crate) fn into_parts(self) -> Result<(CommandId, CameraId, ResponseFuture)> {
        let response_future = self
            .response_future
            .into_inner()
            .map_err(|_| Error::LockPoisoned("InFlight response_future"))?
            .ok_or_else(|| Error::InvalidState("await_completion called more than once".into()))?;

        Ok((self.id, self.camera_id, response_future))
    }
}

#[cfg(feature = "mode-async")]
impl<'a, C, P, Exec> std::fmt::Debug for InFlight<'a, C, P, Exec>
where
    P: Profile,
    Exec: Executor,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InFlight")
            .field("id", &self.id)
            .field("category", &std::any::type_name::<C>())
            .finish()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// Test that CommandId::from_raw returns None for zero.
    #[test]
    fn test_command_id_from_raw_zero_returns_none() {
        assert!(CommandId::from_raw(0).is_none());
    }

    /// Test that CommandId::from_raw returns Some for non-zero values.
    #[test]
    fn test_command_id_from_raw_nonzero_returns_some() {
        let id = CommandId::from_raw(1);
        assert!(id.is_some());
        assert_eq!(id.map(|i| i.get()), Some(1));

        let id = CommandId::from_raw(42);
        assert!(id.is_some());
        assert_eq!(id.map(|i| i.get()), Some(42));

        let id = CommandId::from_raw(u32::MAX);
        assert!(id.is_some());
        assert_eq!(id.map(|i| i.get()), Some(u32::MAX));
    }

    /// Test CommandId::get returns the correct value.
    #[test]
    fn test_command_id_get() {
        let Some(id) = CommandId::from_raw(123) else {
            panic!("non-zero should succeed");
        };
        assert_eq!(id.get(), 123);
    }

    /// Test CommandId::as_nonzero returns the underlying NonZeroU32.
    #[test]
    fn test_command_id_as_nonzero() {
        let Some(id) = CommandId::from_raw(456) else {
            panic!("non-zero should succeed");
        };
        assert_eq!(id.as_nonzero().get(), 456);
    }

    /// Test CommandId implements PartialEq correctly.
    #[test]
    fn test_command_id_equality() {
        let Some(id1) = CommandId::from_raw(100) else {
            panic!("non-zero should succeed");
        };
        let Some(id2) = CommandId::from_raw(100) else {
            panic!("non-zero should succeed");
        };
        let Some(id3) = CommandId::from_raw(200) else {
            panic!("non-zero should succeed");
        };

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    /// Test CommandId Display implementation.
    #[test]
    fn test_command_id_display() {
        let Some(id) = CommandId::from_raw(789) else {
            panic!("non-zero should succeed");
        };
        assert_eq!(format!("{id}"), "789");
    }

    /// Test that CommandId is Copy (zero-cost copying).
    #[test]
    fn test_command_id_is_copy() {
        let Some(id) = CommandId::from_raw(42) else {
            panic!("non-zero should succeed");
        };
        let id_copy = id; // Copy
        assert_eq!(id, id_copy);
        // id is still valid after copy
        assert_eq!(id.get(), 42);
    }

    /// Test Option<CommandId> has same size as CommandId (niche optimization).
    #[test]
    fn test_option_command_id_niche_optimization() {
        use core::mem::size_of;
        assert_eq!(
            size_of::<Option<CommandId>>(),
            size_of::<CommandId>(),
            "Option<CommandId> should have same size as CommandId due to NonZeroU32 niche"
        );
    }
}
