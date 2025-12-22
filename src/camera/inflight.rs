//! Typed operation handles for long-running camera commands.
//!
//! This module provides `InFlight<C>` handles that are returned by long-running
//! control methods (like pan/tilt, zoom, focus movements). These handles enable:
//!
//! - **Socket-safe cancellation**: Cancel commands by ID without needing to know
//!   which socket they're using. The runtime handles socket resolution automatically.
//! - **Type-directed completion waits**: Each handle knows which completion waiter
//!   to use based on the command category (pan/tilt, zoom, focus, or preset).
//!
//! # Design
//!
//! The design uses zero-sized type (ZST) markers to encode completion categories
//! at compile time. This provides type-safe dispatch to the correct waiter without
//! runtime overhead or dynamic dispatch.
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
//! // Wait for completion (automatically uses await_pan_tilt_idle)
//! handle.await_completion(Duration::from_secs(5)).await?;
//!
//! // Or cancel the operation
//! handle.cancel().await?;
//! # Ok(())
//! # }
//! ```

#[cfg(feature = "mode-async")]
use core::{future::Future, marker::PhantomData, time::Duration};

#[cfg(feature = "mode-async")]
use crate::Result;

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
/// `CommandId`s are returned by:
/// - [`Camera::send_command_with_id`](crate::camera::Camera::send_command_with_id)
/// - [`Camera::start_command_with_id`](crate::camera::Camera::start_command_with_id)
/// - [`InFlight::id`](InFlight::id)
///
/// # Cancellation
///
/// Use the returned `CommandId` with [`Camera::cancel`](crate::camera::Camera::cancel)
/// to cancel a running command.
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
/// // The future will resolve with an error
/// let result = future.await;
/// assert!(result.is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommandId(NonZeroU32);

impl CommandId {
    /// Creates a new `CommandId` from a raw `u32`, returning `None` if the value is zero.
    ///
    /// This is only available within the crate for internal use during ID generation.
    #[cfg(any(feature = "mode-async", test))]
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

/// Trait for types that can provide camera-like operations.
///
/// This trait unifies `Camera` and `CameraSession` behind a minimal interface
/// that the `InFlight` handle needs. It provides access to:
/// - The runtime (for cancellation)
/// - Typed completion waiters (for await_completion)
///
/// This trait is only available in async mode.
#[cfg(feature = "mode-async")]
pub trait CameraLike {
    /// The profile type for this camera.
    type Profile: crate::capabilities::Profile;
    /// The executor type for this camera.
    type Executor: crate::executor::Executor;

    /// Get access to the runtime handle for cancellation.
    fn runtime(&self) -> &crate::runtime::RuntimeHandle<Self::Profile, Self::Executor>;

    /// Wait for all operations to complete (general idle wait).
    fn await_idle(&self, timeout: Duration) -> impl Future<Output = Result<()>> + Send + '_;

    /// Wait for pan/tilt operations to complete.
    fn await_pan_tilt_idle(
        &self,
        timeout: Duration,
    ) -> impl Future<Output = Result<()>> + Send + '_;

    /// Wait for zoom operations to complete.
    fn await_zoom_idle(&self, timeout: Duration) -> impl Future<Output = Result<()>> + Send + '_;

    /// Wait for focus operations to complete.
    fn await_focus_idle(&self, timeout: Duration) -> impl Future<Output = Result<()>> + Send + '_;
}

/// Trait that maps a category marker to the appropriate completion waiter.
///
/// This provides compile-time dispatch from the marker type `C` to the correct
/// waiter method on a `CameraLike` type.
#[cfg(feature = "mode-async")]
pub trait WaitFor<C> {
    /// Wait for operations of category `C` to complete.
    fn wait(&self, timeout: Duration) -> impl Future<Output = Result<()>> + Send + '_;
}

#[cfg(feature = "mode-async")]
impl<T: CameraLike> WaitFor<PanTilt> for T {
    fn wait(&self, timeout: Duration) -> impl Future<Output = Result<()>> + Send + '_ {
        self.await_pan_tilt_idle(timeout)
    }
}

#[cfg(feature = "mode-async")]
impl<T: CameraLike> WaitFor<Zoom> for T {
    fn wait(&self, timeout: Duration) -> impl Future<Output = Result<()>> + Send + '_ {
        self.await_zoom_idle(timeout)
    }
}

#[cfg(feature = "mode-async")]
impl<T: CameraLike> WaitFor<Focus> for T {
    fn wait(&self, timeout: Duration) -> impl Future<Output = Result<()>> + Send + '_ {
        self.await_focus_idle(timeout)
    }
}

#[cfg(feature = "mode-async")]
impl<T: CameraLike> WaitFor<Preset> for T {
    fn wait(&self, timeout: Duration) -> impl Future<Output = Result<()>> + Send + '_ {
        self.await_idle(timeout)
    }
}

/// A typed handle to an in-flight camera command.
///
/// This handle provides:
/// - **Command ID access** for debugging and telemetry
/// - **Socket-safe cancellation** via the runtime's ID-based cancel path
/// - **Type-directed completion waits** that automatically select the correct waiter
///
/// The handle is zero-cost: it contains only a command ID and a reference to the
/// camera/session. The marker type `C` is zero-sized and used only at compile time
/// for type-directed dispatch.
///
/// # Type Parameters
///
/// - `'a`: Lifetime of the camera/session reference
/// - `C`: Category marker (PanTilt, Zoom, Focus, or Preset)
/// - `T`: The camera-like type (Camera or CameraSession)
#[cfg(feature = "mode-async")]
pub struct InFlight<'a, C, T: CameraLike + ?Sized> {
    /// The command ID assigned by the runtime.
    id: CommandId,
    /// Reference to the camera or session.
    cam: &'a T,
    /// Zero-sized marker for the category.
    _c: PhantomData<C>,
}

#[cfg(feature = "mode-async")]
impl<'a, C, T> InFlight<'a, C, T>
where
    T: CameraLike + WaitFor<C> + ?Sized,
{
    /// Create a new in-flight handle.
    ///
    /// This is public within the crate but not exposed to external users.
    #[inline]
    pub(crate) fn new(id: CommandId, cam: &'a T) -> Self {
        Self {
            id,
            cam,
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
    /// This is socket-agnostic: the runtime will determine which socket the
    /// command is using and route the CANCEL message appropriately.
    ///
    /// # Errors
    ///
    /// Returns an error if the cancellation request cannot be sent to the runtime.
    pub async fn cancel(&self) -> Result<()> {
        self.cam.runtime().cancel(self.id).await
    }

    /// Wait for this operation to complete.
    ///
    /// The type of the category marker `C` determines which completion waiter
    /// is used:
    /// - `PanTilt` → `await_pan_tilt_idle`
    /// - `Zoom` → `await_zoom_idle`
    /// - `Focus` → `await_focus_idle`
    /// - `Preset` → `await_idle`
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
    /// Returns an error if the wait fails or times out.
    pub async fn await_completion(&self, timeout: Duration) -> Result<()> {
        <T as WaitFor<C>>::wait(self.cam, timeout).await
    }
}

#[cfg(feature = "mode-async")]
impl<'a, C, T> std::fmt::Debug for InFlight<'a, C, T>
where
    T: CameraLike + ?Sized,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InFlight")
            .field("id", &self.id)
            .field("category", &std::any::type_name::<C>())
            .finish()
    }
}

// Implementations of CameraLike for Camera and CameraSession

#[cfg(feature = "mode-async")]
impl<P, Tr, Exec> CameraLike for crate::camera::Camera<crate::mode::Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile + crate::capabilities::ProfileMetadata + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    type Profile = P;
    type Executor = Exec;

    fn runtime(&self) -> &crate::runtime::RuntimeHandle<Self::Profile, Self::Executor> {
        self.runtime()
    }

    fn await_idle(&self, timeout: Duration) -> impl Future<Output = Result<()>> + Send + '_ {
        self.await_idle(timeout)
    }

    fn await_pan_tilt_idle(
        &self,
        timeout: Duration,
    ) -> impl Future<Output = Result<()>> + Send + '_ {
        self.await_pan_tilt_idle(timeout)
    }

    fn await_zoom_idle(&self, timeout: Duration) -> impl Future<Output = Result<()>> + Send + '_ {
        self.await_zoom_idle(timeout)
    }

    fn await_focus_idle(&self, timeout: Duration) -> impl Future<Output = Result<()>> + Send + '_ {
        self.await_focus_idle(timeout)
    }
}

#[cfg(feature = "mode-async")]
impl<P, Tr, Exec> CameraLike
    for crate::camera::CameraSession<crate::mode::Async, P, Tr, Exec, crate::camera::session::Open>
where
    P: crate::capabilities::Profile + crate::capabilities::ProfileMetadata + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    type Profile = P;
    type Executor = Exec;

    fn runtime(&self) -> &crate::runtime::RuntimeHandle<Self::Profile, Self::Executor> {
        self.camera().runtime()
    }

    fn await_idle(&self, timeout: Duration) -> impl Future<Output = Result<()>> + Send + '_ {
        self.await_idle(timeout)
    }

    fn await_pan_tilt_idle(
        &self,
        timeout: Duration,
    ) -> impl Future<Output = Result<()>> + Send + '_ {
        self.await_pan_tilt_idle(timeout)
    }

    fn await_zoom_idle(&self, timeout: Duration) -> impl Future<Output = Result<()>> + Send + '_ {
        self.await_zoom_idle(timeout)
    }

    fn await_focus_idle(&self, timeout: Duration) -> impl Future<Output = Result<()>> + Send + '_ {
        self.await_focus_idle(timeout)
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
