//! Operation handles for camera commands with explicit lifecycle control.
//!
//! [`Camera::submit`](crate::camera::Camera::submit) is the primary 1.x producer
//! of async handles; blocking cameras expose the same submission vocabulary.
//! These handles enable:
//!
//! - **Socket-safe cancellation**: Cancel commands by ID without needing to know
//!   which socket they're using. The runtime handles socket resolution automatically.
//! - **Exact completion waits**: Each handle owns the response future for the
//!   command it represents, so completion and command errors are never inferred
//!   from later movement state.
//!
//! # Design
//!
//! The primary `submit` path uses `C = ()`. Zero-sized category markers remain
//! for concrete `_op` compatibility shims and dyn erasure/telemetry. In 1.x,
//! applied-versus-settled behavior comes from command-derived runtime metadata,
//! not from the category marker. Cancellation stays ID-based, and completion
//! waits are tied to the exact VISCA response channel returned at submission.
//!
//! # Example
//!
//! ```ignore
//! use grafton_visca::{
//!     camera::{Connect, profiles::PtzOpticsG2},
//!     command::PanTilt,
//! };
//! use std::time::Duration;
//!
//! # async fn example() -> Result<(), grafton_visca::Error> {
//! let camera = Connect::open_tcp_async::<PtzOpticsG2, _>("192.168.0.110", runtime).await?;
//!
//! // Submit a targeted pan/tilt operation and get a typed handle.
//! let handle = camera.submit(&PanTilt::Home).await?;
//!
//! // Wait for protocol completion and physical settling.
//! handle.await_settled(Duration::from_secs(5)).await?;
//!
//! // For an operation that should not continue, request socket-safe cancellation
//! // instead of awaiting it.
//! // handle.cancel().await?;
//! # Ok(())
//! # }
//! ```

#[cfg(feature = "mode-async")]
use core::{future::Future, marker::PhantomData, pin::Pin, time::Duration};

#[cfg(feature = "mode-async")]
use std::{panic::AssertUnwindSafe, sync::Mutex};

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
/// IDs are exposed by `InFlight::id` and async `Camera::start_command_with_id`
/// when `mode-async` is enabled, and by `BlockingInFlight::id` in blocking mode.
///
/// # Cancellation
///
/// Async callers may pass the ID to `Camera::cancel`. Both handle types also
/// provide their mode-specific `cancel` operation.
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
/// - [`OpKind::Continuous`] — the 1.x name for any applied-only operation with no
///   meaningful physical-settle state. This includes continuous drives, stops,
///   instantaneous triggers, and configuration commands. For these,
///   `await_settled` returns [`Error::NotSupported`].
///
/// Throughout 1.x this distinction is runtime data on the handle. The planned
/// 2.0 typed consuming-handle design can promote it into the type system so an
/// invalid `await_settled` call becomes a compile error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    /// A targeted move that reaches a destination and physically settles.
    Targeted,
    /// An applied-only operation with no well-defined physical-settle state.
    Continuous,
}

/// Runtime metadata describing how an actuation command completes.
///
/// Built-in movement commands provide this metadata through
/// [`ViscaCommand::operation_metadata`](crate::command::ViscaCommand::operation_metadata),
/// making command kind and affected axes a single source of truth shared by
/// blocking, async, and dynamic operation handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationMetadata {
    /// Whether the command has a meaningful physical settled state.
    pub kind: OpKind,
    /// Physical axes affected by the command.
    pub axes: crate::camera::Axes,
}

impl OperationMetadata {
    /// Metadata for a targeted operation that eventually settles.
    #[must_use]
    pub const fn targeted(axes: crate::camera::Axes) -> Self {
        Self {
            kind: OpKind::Targeted,
            axes,
        }
    }

    /// Metadata for a continuous drive, stop, or other applied-only operation.
    #[must_use]
    pub const fn applied_only(axes: crate::camera::Axes) -> Self {
        Self {
            kind: OpKind::Continuous,
            axes,
        }
    }
}

/// Object-safe bridge used by async handles for position-based settle waits.
///
/// Keeping this private avoids adding the transport type to the public
/// `InFlight` generic parameters while still routing fallback polling through
/// the concrete camera that submitted the command.
#[cfg(feature = "mode-async")]
pub(crate) trait AsyncSettleWaiter: Send + Sync {
    fn await_axes_settled(
        &self,
        axes: crate::camera::Axes,
        timeout: Duration,
    ) -> crate::mode::BoxFuture<'_, Result<()>>;
}

/// Type alias for boxed response futures returned by `start_command_with_id`.
#[cfg(feature = "mode-async")]
pub type ResponseFuture = Pin<Box<dyn Future<Output = Result<Response, Error>> + Send + 'static>>;

/// A handle to a submitted async camera command.
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
/// - `C`: Compatibility category marker; primary `submit` handles use `()`
/// - `P`: Camera profile type
/// - `Exec`: Async executor type
#[cfg(feature = "mode-async")]
#[must_use = "this operation handle should be awaited, cancelled, or explicitly detached; dropping it leaves the submitted command scheduler-owned"]
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
    /// Axes affected by this operation, used for position-based settling.
    axes: crate::camera::Axes,
    /// Concrete-camera bridge for position polling on profiles without an
    /// operation-complete message. The assertion is confined to this immutable
    /// reference: callers cannot mutate or recover camera state through the
    /// erased bridge, and the public handle carried both unwind auto traits in
    /// 1.0 before fallback settling was added.
    settle_waiter: AssertUnwindSafe<&'a dyn AsyncSettleWaiter>,
    /// Zero-sized marker for the category.
    _c: PhantomData<C>,
}

#[cfg(feature = "mode-async")]
impl<'a, C, P, Exec> InFlight<'a, C, P, Exec>
where
    P: Profile + 'static,
    Exec: Executor + Send + Sync + 'static,
{
    fn take_response_future(&self) -> Result<ResponseFuture> {
        self.response_future
            .lock()
            .ok()
            .ok_or(Error::LockPoisoned("InFlight response_future"))?
            .take()
            .ok_or_else(|| Error::InvalidState("operation handle already awaited".into()))
    }

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
        metadata: OperationMetadata,
        settle_waiter: &'a dyn AsyncSettleWaiter,
    ) -> Self {
        Self {
            id,
            camera_id,
            runtime,
            response_future: Mutex::new(Some(response_future)),
            kind: metadata.kind,
            axes: metadata.axes,
            settle_waiter: AssertUnwindSafe(settle_waiter),
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
    /// Queued commands may be removed without a protocol frame. Once a command
    /// has a socket, the scheduler emits the socket-specific cancel request when
    /// the selected profile supports it.
    /// `Ok(())` means the request was recorded or sent; it is not camera
    /// acknowledgement and does not prove physical motion stopped. This method
    /// borrows the handle, so its exact response may still be awaited.
    ///
    /// Returns [`Error::NotSupported`] when the command has already been sent and
    /// the profile does not support VISCA socket cancellation, or another error
    /// if the request cannot be sent to the runtime.
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
    /// The response wait is one-shot. Once this method starts—including when it
    /// times out—the exact response future is consumed and another completion
    /// wait returns [`Error::InvalidState`].
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
        let future = self.take_response_future()?;

        // Use the executor's timeout mechanism to enforce the deadline
        self.runtime.timeout(timeout, future).await?.map(|_| ())
    }

    /// Await the exact response using the scheduler-owned category deadline.
    pub(crate) async fn await_applied_default(&self) -> Result<()> {
        self.take_response_future()?.await.map(|_| ())
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
    /// submitted command remains scheduler-owned and may still be dispatched and
    /// complete; only this handle's ability to observe completion or cancel it is
    /// discarded. Semantically identical to dropping the handle, but explicit
    /// (and it silences the `#[must_use]` lint).
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
    /// [`await_applied`](Self::await_applied). Profiles without that signal poll
    /// only the affected axes, using the remainder of the same timeout budget.
    /// The response wait is one-shot once settling begins, including on timeout.
    /// An early [`Error::NotSupported`] for an applied-only operation does not
    /// consume that response wait, so `await_applied` remains available.
    ///
    /// [`SUPPORTS_OPERATION_COMPLETE`]: crate::capabilities::ProfileMetadata::SUPPORTS_OPERATION_COMPLETE
    ///
    /// # Errors
    ///
    /// Returns [`Error::NotSupported`] if this handle represents an applied-only
    /// operation (there is no well-defined settled state); [`Error::Timeout`] on
    /// deadline; other communication / camera errors otherwise.
    ///
    pub async fn await_settled(&self, timeout: Duration) -> Result<()> {
        if self.kind == OpKind::Continuous {
            return Err(Error::NotSupported);
        }

        // One caller-supplied budget covers both exact-command completion and
        // any position-polling fallback.
        let started = self.runtime.executor().now();
        self.await_applied(timeout).await?;

        if P::SUPPORTS_OPERATION_COMPLETE {
            return Ok(());
        }

        let elapsed = self
            .runtime
            .executor()
            .now()
            .saturating_duration_since(started);
        let remaining = timeout.saturating_sub(elapsed);
        if remaining.is_zero() {
            return Err(Error::Timeout);
        }

        self.settle_waiter
            .0
            .await_axes_settled(self.axes, remaining)
            .await
    }

    /// Consume this handle and return the parts needed for type-erased handling.
    ///
    /// This is used by the dynamic API so `InFlightDyn` can preserve the exact
    /// command response future from the static API instead of approximating
    /// completion with category-level idle polling.
    #[cfg(feature = "dyn-api")]
    pub(crate) fn into_parts(
        self,
    ) -> Result<(CommandId, CameraId, ResponseFuture, OperationMetadata)> {
        let response_future = self
            .response_future
            .into_inner()
            .map_err(|_| Error::LockPoisoned("InFlight response_future"))?
            .ok_or_else(|| Error::InvalidState("completion wait called more than once".into()))?;

        Ok((
            self.id,
            self.camera_id,
            response_future,
            OperationMetadata {
                kind: self.kind,
                axes: self.axes,
            },
        ))
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
            .field("kind", &self.kind)
            .field("axes", &self.axes)
            .field("category", &std::any::type_name::<C>())
            .finish()
    }
}

/// A genuine, synchronous handle to an in-flight blocking command.
///
/// This is the blocking-mode counterpart to the async `InFlight` handle. It is
/// returned by [`Camera::submit`](crate::camera::Camera::submit) and provides the same *mode-honest*
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
///   [`OpKind::Targeted`] handles; on any applied-only handle it returns
///   [`Error::NotSupported`].
///
/// # Safety / lifecycle
///
/// The handle is `#[must_use]`, so directly ignoring a produced handle triggers
/// a lint. Rust's lint does not track a handle after it has been bound to a
/// variable. **Dropping never stops the command** — drop is equivalent to
/// [`detach`](Self::detach): the already-dispatched command continues running,
/// but its terminal outcome is no longer retained. Use [`cancel`](Self::cancel)
/// to request a protocol-aware socket cancel instead.
#[cfg(not(feature = "mode-async"))]
#[must_use = "this blocking operation handle should be awaited, cancelled, or explicitly detached; dropping it detaches the already-dispatched command"]
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
    /// handle, including every applied-only operation.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Timeout`] if the deadline elapses, or a communication /
    /// camera error otherwise.
    pub fn await_applied(self, timeout: Duration) -> Result<()> {
        let deadline = crate::timeout::Deadline::from_timeout(timeout);
        self.camera.await_command_applied(self.id, Some(deadline))
    }

    /// Await the exact response using the scheduler-owned category deadline.
    pub(crate) fn await_applied_default(self) -> Result<()> {
        self.camera.await_command_applied(self.id, None)
    }

    /// Cancel this command via the runner.
    ///
    /// Before ACK, cancellation is deferred until the scheduler learns the
    /// command's socket. After ACK, the socket cancel is sent immediately.
    /// `Ok(())` means the cancellation was recorded or sent; it is not camera
    /// acknowledgement and does not prove physical motion stopped. Blocking
    /// cancellation consumes the handle.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TransportBusy`] if the runner is concurrently borrowed,
    /// [`Error::NotSupported`] when the command has already been sent and the
    /// profile does not support VISCA socket cancellation, or a transport error
    /// if an immediately eligible cancel frame cannot be sent.
    pub fn cancel(self) -> Result<()> {
        self.camera.cancel_command_id(self.id)
    }

    /// Explicitly detach: give up the handle without waiting or canceling.
    ///
    /// This is the intentional fire-and-forget escape hatch. The already-sent
    /// command continues running and its terminal outcome is discarded. It is
    /// not stopped. Semantically identical to dropping the handle, but explicit
    /// (and it satisfies the `#[must_use]` lint).
    #[inline]
    pub fn detach(self) {
        self.camera.detach_command_id(self.id);
    }
}

#[cfg(not(feature = "mode-async"))]
impl<C, P, Tr> Drop for BlockingInFlight<'_, C, P, Tr>
where
    P: Profile + Default,
    Tr: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + Send + 'static,
{
    fn drop(&mut self) {
        // Drop is detach, never cancel: the command remains scheduler-owned, but
        // no terminal outcome is retained after its last handle disappears.
        self.camera.detach_command_id(self.id);
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
    /// Returns [`Error::NotSupported`] if this handle represents an applied-only
    /// operation (there is no well-defined settled state). Returns
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
