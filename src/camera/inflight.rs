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

#[cfg(not(feature = "mode-async"))]
use core::{marker::PhantomData, time::Duration};

#[cfg(not(feature = "mode-async"))]
use crate::{capabilities::Profile, error::Error, Result};

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

/// Whether an operation has a well-defined *settled* (physical-motion-ended) state.
///
/// This distinguishes the two shapes of movement command that share a handle:
///
/// - [`OpKind::Targeted`] — a move that reaches a destination and physically
///   settles (absolute/relative/home/reset, set-position, preset recall).
///   `await_settled` is meaningful for these.
/// - [`OpKind::Continuous`] — a continuous drive or an instantaneous stop, where
///   "physical motion ended" is not a well-defined event (continuous `tele`/
///   `wide` zoom, directional pan/tilt, or a `stop`). For these, `await_settled`
///   returns [`Error::NotSupported`].
///
/// In Phase 1 this distinction is carried as runtime data on the handle. Phase 2
/// promotes it into the marker type `C` so a wrong `await_settled` call becomes a
/// compile error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    /// A targeted move that reaches a destination and physically settles.
    Targeted,
    /// A continuous drive or an instantaneous stop with no well-defined settle.
    Continuous,
}

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
#[must_use = "an InFlight handle does nothing unless you await_applied/await_settled, cancel, or detach it (dropping it detaches: the already-dispatched command keeps running, it is not stopped)"]
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
    /// Wrapped in Mutex to allow `await_applied` to take ownership.
    response_future: Mutex<Option<ResponseFuture>>,
    /// Whether this operation has a well-defined settled state.
    kind: OpKind,
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
        kind: OpKind,
    ) -> Self {
        Self {
            id,
            camera_id,
            runtime,
            response_future: Mutex::new(Some(response_future)),
            kind,
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

    /// Block (asynchronously) until the command is *applied*.
    ///
    /// This awaits the response future stored in this handle, which completes
    /// when the camera has accepted and protocol-completed the command. It is
    /// available on **every** handle, including continuous drives and stops — a
    /// deadman `STOP` uses exactly this. The `timeout` bounds how long to wait
    /// for the camera's response.
    ///
    /// # Returns
    ///
    /// - `Ok(())` if the command was applied within the timeout
    /// - `Err(Error::Timeout)` if the timeout elapsed first
    /// - `Err(Error::*)` for other communication or camera errors
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The wait times out (`Error::Timeout`)
    /// - This method (or `await_settled`) is called more than once (`Error::InvalidState`)
    /// - The internal mutex is poisoned (`Error::LockPoisoned`)
    /// - Other communication or camera errors occur
    pub async fn await_applied(&self, timeout: Duration) -> Result<()> {
        // Take the response future from the mutex (can only be awaited once)
        let future = self
            .response_future
            .lock()
            .ok()
            .ok_or(Error::LockPoisoned("InFlight response_future"))?
            .take()
            .ok_or_else(|| Error::InvalidState("await_applied called more than once".into()))?;

        // Use the executor's timeout mechanism to enforce the deadline
        self.runtime.timeout(timeout, future).await?.map(|_| ())
    }

    /// Deprecated alias for [`await_applied`](Self::await_applied).
    ///
    /// Renamed to make the *applied* vs *settled* completion distinction
    /// explicit. This shim delegates with zero duplicated logic.
    #[deprecated(
        since = "1.1.0",
        note = "renamed to `await_applied` to distinguish applied vs settled completion; this alias will be removed in 2.0"
    )]
    pub async fn await_completion(&self, timeout: Duration) -> Result<()> {
        self.await_applied(timeout).await
    }

    /// Explicitly detach: give up the handle without awaiting or canceling.
    ///
    /// This is the intentional fire-and-forget escape hatch. The already-
    /// dispatched command keeps running on the camera; only this handle's ability
    /// to observe completion or cancel it is discarded. Semantically identical to
    /// dropping the handle, but explicit (and it silences the `#[must_use]` lint).
    #[inline]
    pub fn detach(self) {
        // Nothing to do: dropping the handle performs no cancellation, matching
        // the documented drop == detach rule (never auto-stop).
    }

    /// Block (asynchronously) until physical motion has *settled*.
    ///
    /// This is meaningful only for targeted moves (absolute/relative/home/reset,
    /// set-position, preset recall). On profiles that report
    /// [`SUPPORTS_OPERATION_COMPLETE`], the operation-complete message *is* the
    /// settled signal, so this is equivalent to
    /// [`await_applied`](Self::await_applied).
    ///
    /// [`SUPPORTS_OPERATION_COMPLETE`]: crate::capabilities::ProfileMetadata::SUPPORTS_OPERATION_COMPLETE
    ///
    /// # Errors
    ///
    /// Returns [`Error::NotSupported`] if this handle represents a continuous
    /// drive or a stop (there is no well-defined settled state); [`Error::Timeout`]
    /// on deadline; other communication / camera errors otherwise.
    ///
    /// # Phase 1 limitation
    ///
    /// On profiles **without** an operation-complete message this resolves when
    /// the command is *applied*; for a strict position-based settle, follow up
    /// with [`Camera::await_axes_idle`](crate::camera::Camera::await_axes_idle).
    /// A handle-internal position poll (as already implemented for blocking mode)
    /// is planned alongside the finer targeted/continuous markers.
    pub async fn await_settled(&self, timeout: Duration) -> Result<()> {
        if self.kind == OpKind::Continuous {
            return Err(Error::NotSupported);
        }
        self.await_applied(timeout).await
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

/// A genuine, synchronous handle to an in-flight blocking command.
///
/// This is the blocking-mode counterpart to the async `InFlight` handle. It is
/// returned by [`Camera::submit`](crate::camera::Camera::submit) (and the
/// noun-scoped blocking `submit_*` helpers) and provides the same *mode-honest*
/// surface as the async handle — `await_applied`, `await_settled`, `cancel`,
/// `detach` — but every method runs **synchronously on the caller's thread**. No
/// async executor is ever spun up: `await_applied` drives the blocking runner's
/// `run_until_complete` loop directly.
///
/// # Completion semantics
///
/// - [`await_applied`](Self::await_applied) resolves when the camera has accepted
///   and protocol-completed the command. This is what a deadman `STOP` or a
///   continuous drive uses.
/// - [`await_settled`](Self::await_settled) resolves when physical motion has
///   ended — via the operation-complete message on profiles that support it,
///   otherwise via position polling. It is meaningful only for
///   [`OpKind::Targeted`] handles; on a continuous/stop handle it returns
///   [`Error::NotSupported`].
///
/// # Safety / lifecycle
///
/// The handle is `#[must_use]`: dropping it without `await_applied`,
/// `await_settled`, `cancel`, or `detach` triggers a lint. **Dropping never
/// stops the command** — drop is equivalent to [`detach`](Self::detach): the
/// queued command is left to be flushed by the next blocking operation. Use
/// [`cancel`](Self::cancel) to discard it instead.
#[cfg(not(feature = "mode-async"))]
#[must_use = "a BlockingInFlight handle does nothing unless you await_applied/await_settled, cancel, or detach it (dropping leaves the command queued, it does not stop it)"]
pub struct BlockingInFlight<'a, C, P, Tr>
where
    P: Profile + Default,
    Tr: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + Send + 'static,
{
    /// The command ID assigned by the blocking runner.
    id: CommandId,
    /// The camera that owns the runner and transport this command lives in.
    camera: &'a crate::camera::Camera<crate::mode::Blocking, P, Tr, ()>,
    /// Whether this operation has a well-defined settled state.
    kind: OpKind,
    /// Axes to poll for a position-based settle (used when the profile has no
    /// operation-complete message).
    axes: crate::camera::Axes,
    /// Zero-sized marker for the category.
    _c: PhantomData<C>,
}

#[cfg(not(feature = "mode-async"))]
impl<'a, C, P, Tr> BlockingInFlight<'a, C, P, Tr>
where
    P: Profile + Default,
    Tr: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + Send + 'static,
{
    /// Create a new blocking handle. Crate-internal; built by `Camera::submit`.
    #[inline]
    pub(crate) fn new(
        id: CommandId,
        camera: &'a crate::camera::Camera<crate::mode::Blocking, P, Tr, ()>,
        kind: OpKind,
        axes: crate::camera::Axes,
    ) -> Self {
        Self {
            id,
            camera,
            kind,
            axes,
            _c: PhantomData,
        }
    }

    /// Get the command ID assigned by the runner.
    #[inline]
    #[must_use]
    pub fn id(&self) -> CommandId {
        self.id
    }

    /// Block until the camera has accepted and protocol-completed the command.
    ///
    /// This drives the blocking runner synchronously on the caller's thread until
    /// the command completes or `timeout` elapses. It is available on every
    /// handle, including continuous drives and stops.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Timeout`] if the deadline elapses, or a communication /
    /// camera error otherwise.
    pub fn await_applied(self, timeout: Duration) -> Result<()> {
        let deadline = crate::timeout::Deadline::from_timeout(timeout);
        self.camera.await_command_applied(self.id, Some(deadline))
    }

    /// Cancel this command via the runner.
    ///
    /// If the command is still queued (nothing sent yet), it is discarded and its
    /// completion resolves as canceled.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TransportBusy`] if the runner is concurrently borrowed.
    pub fn cancel(self) -> Result<()> {
        self.camera.cancel_command_id(self.id)
    }

    /// Explicitly detach: give up the handle without waiting or canceling.
    ///
    /// This is the intentional fire-and-forget escape hatch. The queued command
    /// is left in place to be flushed by the next blocking operation; it is not
    /// stopped. Semantically identical to dropping the handle, but explicit (and
    /// silences the `#[must_use]` lint).
    #[inline]
    pub fn detach(self) {
        // Nothing to do: the command stays queued in the runner. Dropping `self`
        // performs no cancellation, matching the documented drop == detach rule.
    }
}

#[cfg(not(feature = "mode-async"))]
impl<C, P, Tr> BlockingInFlight<'_, C, P, Tr>
where
    P: Profile + Default + crate::capabilities::ProfileMetadata,
    Tr: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + Send + 'static,
{
    /// Block until physical motion has ended.
    ///
    /// On profiles that report [`SUPPORTS_OPERATION_COMPLETE`], the operation
    /// completion message *is* the settled signal, so this is equivalent to
    /// [`await_applied`](Self::await_applied). On profiles without it, this first
    /// waits for the command to be applied and then polls the relevant axis
    /// positions until they stop changing (or `timeout` elapses).
    ///
    /// [`SUPPORTS_OPERATION_COMPLETE`]: crate::capabilities::ProfileMetadata::SUPPORTS_OPERATION_COMPLETE
    ///
    /// # Errors
    ///
    /// Returns [`Error::NotSupported`] if this handle represents a continuous
    /// drive or a stop (there is no well-defined settled state). Returns
    /// [`Error::Timeout`] if motion does not settle within `timeout`, or a
    /// communication / camera error otherwise.
    pub fn await_settled(self, timeout: Duration) -> Result<()> {
        if self.kind == OpKind::Continuous {
            return Err(Error::NotSupported);
        }

        let deadline = crate::timeout::Deadline::from_timeout(timeout);

        // Applied first: reach the protocol completion for this exact command.
        self.camera.await_command_applied(self.id, Some(deadline))?;

        // On operation-complete profiles, "applied" already means "settled".
        if P::SUPPORTS_OPERATION_COMPLETE {
            return Ok(());
        }

        // Otherwise fall back to position polling on the affected axes.
        self.camera.await_axes_settled(self.axes, deadline)
    }
}

#[cfg(not(feature = "mode-async"))]
impl<C, P, Tr> std::fmt::Debug for BlockingInFlight<'_, C, P, Tr>
where
    P: Profile + Default,
    Tr: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + Send + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlockingInFlight")
            .field("id", &self.id)
            .field("kind", &self.kind)
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
