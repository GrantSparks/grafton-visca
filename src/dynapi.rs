//! Dynamic trait object API with per-operation timeout support.
//!
//! This module provides object-safe trait definitions for runtime polymorphism
//! with cameras. Unlike the static `Camera<M, P, Tr, Exec>` type, these traits
//! can be used with `dyn` dispatch, enabling heterogeneous collections of cameras
//! and runtime camera type selection.
//!
//! # Motivation
//!
//! The standard `Camera` API uses generics and typed `InFlight` handles that
//! include generic type parameters, violating Rust's object safety rules. This
//! forces downstream consumers to maintain parallel trait hierarchies when they
//! need `dyn` dispatch.
//!
//! The `dyn-api` feature provides object-safe alternatives with built-in timeout
//! support, allowing callers to specify per-operation deadlines through
//! `Option<Duration>` parameters.
//!
//! # Quick Start
//!
//! Enable the `dyn-api` feature in your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! grafton-visca = { version = "1", features = ["dyn-api", "runtime-tokio"] }
//! ```
//!
//! Then convert a concrete camera to use the dynamic API:
//!
//! ```ignore
//! use grafton_visca::{
//!     camera::{Connect, profiles::PtzOpticsG2},
//!     dynapi::{DynCameraControl, IntoDynCamera},
//!     runtime::TokioRuntime,
//!     Error,
//! };
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     let runtime = TokioRuntime::from_current()?;
//!     let camera = Connect::open_tcp_async::<PtzOpticsG2, _>("192.168.0.110", runtime).await?;
//!
//!     // Convert to dynamic API
//!     let dyn_camera = camera.into_dyn();
//!
//!     // Use as a trait object
//!     let camera_ref: &dyn DynCameraControl = &dyn_camera;
//!     control_camera(camera_ref).await?;
//!
//!     Ok(())
//! }
//!
//! async fn control_camera(camera: &dyn DynCameraControl) -> Result<(), Error> {
//!     camera.pan_tilt().pan_tilt_home(None).await?;
//!     Ok(())
//! }
//! ```
//!
//! # Timeout Behavior
//!
//! Movement methods that accept an optional `timeout` parameter use it as a
//! deadline for the command's own VISCA completion response:
//!
//! - **`None`**: Uses the camera's default `TimeoutConfig` for the command category.
//!   This is the recommended option for most use cases.
//!
//! - **`Some(Duration)`**: Applies that duration to this command's completion wait.
//!   The wait resolves when the camera reports completion or an error for the
//!   command. It does not infer physical movement completion from idle polling.
//!
//! To wait for physical motion to settle, use
//! [`DynMotionControl::await_idle`](crate::dynapi::DynMotionControl::await_idle)
//! or the axis-specific idle wait methods after issuing the command.
//!
//! ```ignore
//! use std::time::Duration;
//!
//! // Use default timeout (recommended)
//! pt.pan_tilt_home(None).await?;
//!
//! // Use explicit 30-second timeout for this operation
//! pt.pan_tilt_home(Some(Duration::from_secs(30))).await?;
//! ```
//!
//! # Drop and Cancellation Semantics
//!
//! Understanding how futures and operations behave when dropped is critical for
//! reliable camera control:
//!
//! ## Future Drop Behavior
//!
//! When an async future is dropped (e.g., via `select!`, timeout, or early return),
//! the underlying VISCA command **may still complete on the camera**. The library
//! cannot unilaterally stop physical camera movement once a command has been sent.
//!
//! ```ignore
//! // WARNING: Camera may still move even though we dropped the future
//! tokio::select! {
//!     _ = pt.pan_tilt_home(None) => {},
//!     _ = tokio::time::sleep(Duration::from_millis(100)) => {
//!         // Future dropped, but camera command may have been sent
//!         // Camera will continue moving to home position!
//!     }
//! }
//! ```
//!
//! ## Explicit Cancellation
//!
//! For controlled cancellation, use `InFlightDyn::cancel()` or
//! `DynMotionControl::stop_all_motion()`:
//!
//! ```ignore
//! // Method 1: Cancel a specific operation
//! let handle = pt.pan_tilt_home_op().await?;
//! // ... decide to cancel ...
//! handle.cancel().await?;  // Sends VISCA CANCEL command
//!
//! // Method 2: Emergency stop all motion
//! camera.motion().stop_all_motion().await?;
//! ```
//!
//! ## Timeout Layering
//!
//! The dyn-api provides per-call timeout parameters for command completion.
//! The runtime scheduler still owns VISCA retries, cancellation, and transport
//! error reporting. **Avoid adding external timeouts that race with library
//! timeouts**:
//!
//! ```ignore
//! // GOOD: Use the built-in timeout parameter
//! pt.pan_tilt_home(Some(Duration::from_secs(30))).await?;
//!
//! // AVOID: External timeout racing with library timeout
//! // This may drop the future while the library is still waiting
//! tokio::time::timeout(
//!     Duration::from_secs(30),
//!     pt.pan_tilt_home(None)  // Uses TimeoutConfig default
//! ).await??;
//! ```
//!
//! ## Cancellation Patterns Summary
//!
//! | Scenario | Recommended Approach |
//! |----------|---------------------|
//! | Cancel specific command | `InFlightDyn::cancel()` |
//! | Emergency stop all motion | `DynMotionControl::stop_all_motion()` |
//! | Timeout on specific operation | Pass `timeout` parameter to method |
//! | Wait for physical idle | `camera.motion().await_idle(timeout)` |
//! | Graceful shutdown | `stop_all_motion()`, then drop camera |
//!
//! # Capability Detection
//!
//! The `DynCameraControl` trait exposes the same core control families for every
//! profile and provides a structured [`Capabilities`](crate::capabilities::Capabilities)
//! value for runtime feature discovery:
//!
//! ```ignore
//! async fn control_any_camera(camera: &dyn DynCameraControl) -> Result<(), Error> {
//!     let caps = camera.capabilities();
//!
//!     if caps.has_pan_tilt {
//!         camera.pan_tilt().pan_tilt_home(None).await?;
//!     }
//!
//!     if caps.has_zoom {
//!         camera.zoom().zoom_tele(None, None).await?;
//!     }
//!
//!     if caps.has_focus {
//!         camera.focus().focus_auto().await?;
//!     }
//!
//!     if caps.has_presets {
//!         let preset = PresetNumber::new(1)?;
//!         camera.presets().preset_recall(preset, None).await?;
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! # Using with Collections
//!
//! The dynamic API enables heterogeneous camera collections:
//!
//! ```ignore
//! use std::sync::Arc;
//!
//! // Store multiple cameras in a collection
//! let cameras: Vec<Arc<dyn DynCameraControl + Send + Sync>> = vec![
//!     Arc::new(camera1.into_dyn()),
//!     Arc::new(camera2.into_dyn()),
//! ];
//!
//! // Control all cameras uniformly
//! for camera in &cameras {
//!     camera.pan_tilt().pan_tilt_home(None).await?;
//! }
//! ```
//!
//! # Design
//!
//! Each dyn trait mirrors the corresponding static trait but with:
//! - All methods returning `BoxFuture` instead of `impl Future`
//! - An additional command-completion `timeout: Option<Duration>` parameter for
//!   movement methods where per-call deadlines are useful
//! - Structured runtime capability discovery through `DynCameraControl::capabilities()`
//!
//! # Available Traits
//!
//! - `DynCameraControl` - Top-level trait with capability accessors
//! - `DynPanTiltControl` - Pan/tilt operations (movement, home, limits)
//! - `DynZoomControl` - Zoom operations (tele, wide, absolute)
//! - `DynFocusControl` - Focus operations (auto, manual, zones)
//! - `DynPresetsControl` - Preset operations (recall, set, reset)
//! - `DynMotionControl` - Unified motion control (stop all motion)

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    camera::inflight::{CommandId, ResponseFuture},
    command::{
        focus::{AutoFocusSensitivity, FocusZone},
        pan_tilt::{PanTiltDirection, PanTiltLimitCorner},
        preset::PresetNumber,
    },
    mode::BoxFuture,
    types::{
        FocusPosition, PanPosition, PanSpeed, SpeedLevel, TiltPosition, TiltSpeed, ZoomPosition,
        ZoomSpeed,
    },
    Error, Normalized, ZoomDomain,
};

// ============================================================================
// Phase 2: Type-erased Operation Handles
// ============================================================================

/// Operation category for type-erased handles.
///
/// This enum identifies the semantic category of a camera operation. It is
/// exposed for diagnostics and telemetry on [`InFlightDyn`] handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationCategory {
    /// Pan/tilt movement operations.
    PanTilt,
    /// Zoom operations.
    Zoom,
    /// Focus operations.
    Focus,
    /// Preset recall/set operations.
    Preset,
}

impl std::fmt::Display for OperationCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PanTilt => write!(f, "PanTilt"),
            Self::Zoom => write!(f, "Zoom"),
            Self::Focus => write!(f, "Focus"),
            Self::Preset => write!(f, "Preset"),
        }
    }
}

/// Internal trait for type-erased runtime operations.
///
/// This trait enables `InFlightDyn` to perform cancellation and completion
/// waiting without knowing the concrete camera type. It is implemented by
/// the `DynCamera` wrapper.
#[cfg(feature = "dyn-api")]
pub(crate) trait RuntimeDyn: Send + Sync {
    /// Cancel a command by its ID using the specified camera address.
    fn cancel(&self, camera_id: crate::CameraId, id: CommandId)
        -> BoxFuture<'_, Result<(), Error>>;

    /// Wait for the exact command response future owned by a dyn handle.
    fn await_response_completion(
        &self,
        timeout: Duration,
        response_future: ResponseFuture,
    ) -> BoxFuture<'_, Result<(), Error>>;
}

/// Type-erased operation handle for dyn-api.
///
/// This handle provides the same functionality as `InFlight<C, T>` but without
/// generic type parameters, making it object-safe and suitable for use with
/// trait objects.
///
/// # Example
///
/// ```ignore
/// use std::time::Duration;
/// use grafton_visca::dynapi::{DynPanTiltControl, InFlightDyn};
///
/// async fn move_and_wait(pt: &dyn DynPanTiltControl) -> Result<(), Error> {
///     let handle = pt.pan_tilt_home_op().await?;
///
///     // Wait for the movement to complete
///     handle.await_completion(Duration::from_secs(30)).await?;
///
///     // Or cancel the operation
///     // handle.cancel().await?;
///
///     Ok(())
/// }
/// ```
#[cfg(feature = "dyn-api")]
pub struct InFlightDyn {
    /// The command ID assigned by the runtime.
    id: CommandId,
    /// Camera ID for addressing cancel messages.
    camera_id: crate::CameraId,
    /// The operation category for this handle.
    category: OperationCategory,
    /// Reference to the runtime for cancellation and completion waiting.
    runtime: Arc<dyn RuntimeDyn>,
    /// Response future for the command this handle represents.
    ///
    /// This mirrors the static `InFlight` handle: completion is tied to the
    /// command's own VISCA response channel, not inferred from later idle state.
    response_future: Mutex<Option<ResponseFuture>>,
}

#[cfg(feature = "dyn-api")]
impl InFlightDyn {
    /// Create a new type-erased in-flight handle.
    pub(crate) fn new(
        id: CommandId,
        camera_id: crate::CameraId,
        category: OperationCategory,
        runtime: Arc<dyn RuntimeDyn>,
        response_future: ResponseFuture,
    ) -> Self {
        Self {
            id,
            camera_id,
            category,
            runtime,
            response_future: Mutex::new(Some(response_future)),
        }
    }

    /// Get the command ID assigned by the runtime.
    ///
    /// This is useful for debugging and telemetry purposes.
    #[inline]
    pub fn id(&self) -> CommandId {
        self.id
    }

    /// Get the operation category for this handle.
    #[inline]
    pub fn category(&self) -> OperationCategory {
        self.category
    }

    /// Cancel this command via the runtime's ID-based cancel path.
    ///
    /// The cancel message is addressed to the camera ID that was used when
    /// this command was originally sent, ensuring correct multi-camera behavior.
    ///
    /// # Errors
    ///
    /// Returns an error if the cancellation request cannot be sent to the runtime.
    pub fn cancel(&self) -> BoxFuture<'_, Result<(), Error>> {
        self.runtime.cancel(self.camera_id, self.id)
    }

    /// Wait for this operation's command response to complete.
    ///
    /// This uses the same response future as the static `InFlight` API. It
    /// resolves when the camera reports completion or an error for this command.
    /// It does not infer completion from later category-level idle state.
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
    pub fn await_completion(&self, timeout: Duration) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            let response_future = self
                .response_future
                .lock()
                .ok()
                .ok_or(Error::LockPoisoned("InFlightDyn response_future"))?
                .take()
                .ok_or_else(|| {
                    Error::InvalidState("await_completion called more than once".into())
                })?;

            self.runtime
                .await_response_completion(timeout, response_future)
                .await
        })
    }
}

#[cfg(feature = "dyn-api")]
impl std::fmt::Debug for InFlightDyn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InFlightDyn")
            .field("id", &self.id)
            .field("category", &self.category)
            .finish()
    }
}

/// Object-safe camera control trait for runtime polymorphism.
///
/// This trait provides object-safe access to the stable dynamic camera control
/// surface. Core control families are exposed directly; profile-specific feature
/// discovery is provided by [`capabilities`](Self::capabilities).
///
/// # Example
///
/// ```ignore
/// use grafton_visca::dynapi::DynCameraControl;
///
/// async fn demo(camera: &dyn DynCameraControl) -> Result<(), Error> {
///     let caps = camera.capabilities();
///     if caps.has_zoom {
///         camera.zoom().zoom_tele(None, None).await?;
///     }
///     Ok(())
/// }
/// ```
pub trait DynCameraControl: Send + Sync {
    /// Returns runtime-queryable profile capabilities.
    fn capabilities(&self) -> &crate::capabilities::Capabilities;

    /// Returns pan/tilt control.
    fn pan_tilt(&self) -> &dyn DynPanTiltControl;

    /// Returns zoom control.
    fn zoom(&self) -> &dyn DynZoomControl;

    /// Returns focus control.
    fn focus(&self) -> &dyn DynFocusControl;

    /// Returns preset control.
    fn presets(&self) -> &dyn DynPresetsControl;

    /// Returns motion control.
    ///
    /// Motion control provides unified stop operations for all camera axes.
    /// Use this for emergency stops or coordinated motion control.
    ///
    /// # Example
    ///
    /// ```ignore
    /// camera.motion().stop_all_motion().await?;
    /// ```
    fn motion(&self) -> &dyn DynMotionControl;

    /// Returns the state cache for write-only properties.
    ///
    /// The state cache tracks values for properties that have setter commands
    /// but no corresponding VISCA inquiry command. Values are automatically
    /// updated when setter commands succeed.
    ///
    /// # Tracked Properties
    ///
    /// - Auto slow shutter (on/off)
    /// - Spotlight mode (on/off)
    /// - Pan/tilt movement limits
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Check if auto slow shutter was enabled
    /// if let Some(enabled) = camera.state_cache().auto_slow_shutter() {
    ///     println!("Auto slow shutter: {}", if enabled { "on" } else { "off" });
    /// }
    ///
    /// // Check pan/tilt limits
    /// let limits = camera.state_cache().pan_tilt_limits();
    /// if limits.is_set() {
    ///     println!("Pan/tilt limits configured");
    /// }
    /// ```
    fn state_cache(&self) -> &crate::StateCache;
}

/// Object-safe motion control trait.
///
/// This trait provides unified motion control operations, allowing you to stop
/// all camera motion with a single method call. It mirrors the static
/// [`MotionControl`](crate::camera::controls::motion::MotionControl) trait but with object-safe signatures.
///
/// # Example
///
/// ```ignore
/// use grafton_visca::dynapi::DynMotionControl;
///
/// async fn emergency_stop(motion: &dyn DynMotionControl) -> Result<(), Error> {
///     // Stop all camera motion immediately
///     motion.stop_all_motion().await?;
///     Ok(())
/// }
/// ```
///
/// # Behavior
///
/// The `stop_all_motion` method issues stop commands for all motion axes:
/// - Pan/tilt movement
/// - Zoom movement
/// - Focus movement
///
/// All stop commands are attempted even if earlier ones fail. The first error
/// encountered is returned, but all axes will have received stop commands.
pub trait DynMotionControl: Send + Sync {
    /// Stop all camera motion (pan/tilt, zoom, and focus).
    ///
    /// Issues stop commands for all motion axes. All commands are attempted
    /// even if earlier ones fail; returns the first error encountered.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Emergency stop all camera motion
    /// motion.stop_all_motion().await?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns the first error encountered while stopping motion. All stop
    /// commands are attempted for safety even if earlier ones fail.
    fn stop_all_motion(&self) -> BoxFuture<'_, Result<(), Error>>;

    /// Wait for all camera motion to become idle.
    fn await_idle(&self, timeout: Duration) -> BoxFuture<'_, Result<(), Error>>;

    /// Wait for pan/tilt motion to become idle.
    fn await_pan_tilt_idle(&self, timeout: Duration) -> BoxFuture<'_, Result<(), Error>>;

    /// Wait for zoom motion to become idle.
    fn await_zoom_idle(&self, timeout: Duration) -> BoxFuture<'_, Result<(), Error>>;

    /// Wait for focus motion to become idle.
    fn await_focus_idle(&self, timeout: Duration) -> BoxFuture<'_, Result<(), Error>>;
}

/// Object-safe pan/tilt control trait.
///
/// All movement methods accept an optional timeout parameter. When `None`,
/// the default timeout from `TimeoutConfig` is used. When `Some(duration)`,
/// that specific timeout is applied to the operation.
///
/// The `_op` variants return an [`InFlightDyn`] handle for fine-grained control
/// over timeouts and cancellation.
pub trait DynPanTiltControl: Send + Sync {
    /// Stop all pan/tilt movement immediately.
    fn pan_tilt_stop(&self) -> BoxFuture<'_, Result<(), Error>>;

    /// Move to the home position (pan=0°, tilt=0°).
    fn pan_tilt_home(&self, timeout: Option<Duration>) -> BoxFuture<'_, Result<(), Error>>;

    /// Move to the home position and return an operation handle.
    ///
    /// This is the `_op` variant that returns an [`InFlightDyn`] handle for
    /// fine-grained control over timeouts and cancellation.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use std::time::Duration;
    ///
    /// let handle = pt.pan_tilt_home_op().await?;
    /// handle.await_completion(Duration::from_secs(30)).await?;
    /// ```
    fn pan_tilt_home_op(&self) -> BoxFuture<'_, Result<InFlightDyn, Error>>;

    /// Move to an absolute pan/tilt position in degrees.
    ///
    /// # Arguments
    /// * `pan_deg` - Target pan position in degrees
    /// * `tilt_deg` - Target tilt position in degrees
    /// * `speed` - Movement speed (Slow, Medium, Fast)
    /// * `timeout` - Optional timeout for the operation
    fn pan_tilt_absolute(
        &self,
        pan_deg: f64,
        tilt_deg: f64,
        speed: SpeedLevel,
        timeout: Option<Duration>,
    ) -> BoxFuture<'_, Result<(), Error>>;

    /// Move to an absolute pan/tilt position and return an operation handle.
    ///
    /// This is the `_op` variant that returns an [`InFlightDyn`] handle for
    /// fine-grained control over timeouts and cancellation.
    fn pan_tilt_absolute_op(
        &self,
        pan_deg: f64,
        tilt_deg: f64,
        speed: SpeedLevel,
    ) -> BoxFuture<'_, Result<InFlightDyn, Error>>;

    /// Move relative to the current position in degrees.
    ///
    /// # Arguments
    /// * `pan_deg` - Pan offset in degrees (positive=right, negative=left)
    /// * `tilt_deg` - Tilt offset in degrees (positive=up, negative=down)
    /// * `speed` - Movement speed (Slow, Medium, Fast)
    /// * `timeout` - Optional timeout for the operation
    fn pan_tilt_relative(
        &self,
        pan_deg: f64,
        tilt_deg: f64,
        speed: SpeedLevel,
        timeout: Option<Duration>,
    ) -> BoxFuture<'_, Result<(), Error>>;

    /// Move relative to the current position and return an operation handle.
    ///
    /// This is the `_op` variant that returns an [`InFlightDyn`] handle for
    /// fine-grained control over timeouts and cancellation.
    fn pan_tilt_relative_op(
        &self,
        pan_deg: f64,
        tilt_deg: f64,
        speed: SpeedLevel,
    ) -> BoxFuture<'_, Result<InFlightDyn, Error>>;

    /// Start continuous movement in a specific direction.
    fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> BoxFuture<'_, Result<(), Error>>;

    /// Reset pan/tilt mechanism to factory defaults.
    fn pan_tilt_reset(&self, timeout: Option<Duration>) -> BoxFuture<'_, Result<(), Error>>;

    /// Reset pan/tilt mechanism and return an operation handle.
    ///
    /// This is the `_op` variant that returns an [`InFlightDyn`] handle for
    /// fine-grained control over timeouts and cancellation.
    fn pan_tilt_reset_op(&self) -> BoxFuture<'_, Result<InFlightDyn, Error>>;

    /// Set a pan/tilt movement limit for a specific corner.
    fn pan_tilt_limit_set(
        &self,
        corner: PanTiltLimitCorner,
        pan: PanPosition,
        tilt: TiltPosition,
    ) -> BoxFuture<'_, Result<(), Error>>;

    /// Clear a pan/tilt movement limit for a specific corner.
    fn pan_tilt_limit_clear(&self, corner: PanTiltLimitCorner) -> BoxFuture<'_, Result<(), Error>>;
}

/// Object-safe zoom control trait.
///
/// The `_op` variants return an [`InFlightDyn`] handle for fine-grained control
/// over timeouts and cancellation.
pub trait DynZoomControl: Send + Sync {
    /// Stop any zoom operation currently in progress.
    fn zoom_stop(&self) -> BoxFuture<'_, Result<(), Error>>;

    /// Start zooming in (telephoto direction) with optional speed control.
    fn zoom_tele(
        &self,
        speed: Option<ZoomSpeed>,
        timeout: Option<Duration>,
    ) -> BoxFuture<'_, Result<(), Error>>;

    /// Start zooming out (wide angle direction) with optional speed control.
    fn zoom_wide(
        &self,
        speed: Option<ZoomSpeed>,
        timeout: Option<Duration>,
    ) -> BoxFuture<'_, Result<(), Error>>;

    /// Set zoom to an absolute position.
    fn set_zoom(
        &self,
        position: ZoomPosition,
        timeout: Option<Duration>,
    ) -> BoxFuture<'_, Result<(), Error>>;

    /// Set zoom to an absolute position and return an operation handle.
    ///
    /// This is the `_op` variant that returns an [`InFlightDyn`] handle for
    /// fine-grained control over timeouts and cancellation.
    fn set_zoom_op(&self, position: ZoomPosition) -> BoxFuture<'_, Result<InFlightDyn, Error>>;

    /// Set digital zoom on or off.
    fn set_digital_zoom(&self, enabled: bool) -> BoxFuture<'_, Result<(), Error>>;

    /// Set zoom to an absolute normalized position with domain awareness.
    fn zoom_absolute_normalized(
        &self,
        position: Normalized,
        domain: ZoomDomain,
        timeout: Option<Duration>,
    ) -> BoxFuture<'_, Result<(), Error>>;
}

/// Object-safe focus control trait.
///
/// The `_op` variants return an [`InFlightDyn`] handle for fine-grained control
/// over timeouts and cancellation.
pub trait DynFocusControl: Send + Sync {
    /// Set auto focus mode.
    fn focus_auto(&self) -> BoxFuture<'_, Result<(), Error>>;

    /// Set manual focus mode.
    fn focus_manual(&self) -> BoxFuture<'_, Result<(), Error>>;

    /// Focus near at specified speed.
    fn focus_near(&self, speed: SpeedLevel) -> BoxFuture<'_, Result<(), Error>>;

    /// Focus far at specified speed.
    fn focus_far(&self, speed: SpeedLevel) -> BoxFuture<'_, Result<(), Error>>;

    /// Stop focus movement.
    fn focus_stop(&self) -> BoxFuture<'_, Result<(), Error>>;

    /// Trigger one-push auto focus.
    fn focus_one_push(&self) -> BoxFuture<'_, Result<(), Error>>;

    /// Set focus to a specific position.
    fn set_focus(
        &self,
        position: FocusPosition,
        timeout: Option<Duration>,
    ) -> BoxFuture<'_, Result<(), Error>>;

    /// Set focus to a specific position and return an operation handle.
    ///
    /// This is the `_op` variant that returns an [`InFlightDyn`] handle for
    /// fine-grained control over timeouts and cancellation.
    fn set_focus_op(&self, position: FocusPosition) -> BoxFuture<'_, Result<InFlightDyn, Error>>;

    /// Set focus to infinity.
    fn focus_infinity(&self) -> BoxFuture<'_, Result<(), Error>>;

    /// Set the focus zone.
    fn set_focus_zone(&self, zone: FocusZone) -> BoxFuture<'_, Result<(), Error>>;

    /// Set auto focus sensitivity.
    fn set_auto_focus_sensitivity(
        &self,
        sensitivity: AutoFocusSensitivity,
    ) -> BoxFuture<'_, Result<(), Error>>;

    /// Set the focus near limit.
    fn set_focus_near_limit(&self, position: FocusPosition) -> BoxFuture<'_, Result<(), Error>>;
}

/// Object-safe preset control trait.
///
/// The `_op` variants return an [`InFlightDyn`] handle for fine-grained control
/// over timeouts and cancellation.
pub trait DynPresetsControl: Send + Sync {
    /// Recall a preset position.
    fn preset_recall(
        &self,
        preset: PresetNumber,
        timeout: Option<Duration>,
    ) -> BoxFuture<'_, Result<(), Error>>;

    /// Recall a preset position and return an operation handle.
    ///
    /// This is the `_op` variant that returns an [`InFlightDyn`] handle for
    /// fine-grained control over timeouts and cancellation.
    fn preset_recall_op(&self, preset: PresetNumber) -> BoxFuture<'_, Result<InFlightDyn, Error>>;

    /// Set current position as a preset.
    fn preset_set(&self, preset: PresetNumber) -> BoxFuture<'_, Result<(), Error>>;

    /// Reset/clear a preset.
    fn preset_reset(&self, preset: PresetNumber) -> BoxFuture<'_, Result<(), Error>>;
}

// ============================================================================
// Blanket Implementations
// ============================================================================

/// Wrapper type that implements the dyn traits for a concrete camera.
///
/// Adapter that implements the dyn traits for a concrete camera.
///
/// It wraps a camera in an `Arc` so type-erased [`InFlightDyn`] handles can
/// reference the camera for cancellation and completion waiting.
#[cfg(feature = "dyn-api")]
pub struct DynCamera<P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    inner: Arc<DynCameraInner<P, Tr, Exec>>,
}

/// Inner camera storage for `DynCamera`.
///
/// This is separate to allow `Arc<DynCameraInner>` to implement `RuntimeDyn`.
#[cfg(feature = "dyn-api")]
struct DynCameraInner<P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    camera: crate::camera::Camera<crate::mode::Async, P, Tr, Exec>,
    capabilities: crate::capabilities::Capabilities,
}

#[cfg(feature = "dyn-api")]
impl<P, Tr, Exec> DynCamera<P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    /// Create a new DynCamera wrapper from an async camera.
    pub fn new(camera: crate::camera::Camera<crate::mode::Async, P, Tr, Exec>) -> Self {
        Self {
            inner: Arc::new(DynCameraInner {
                camera,
                capabilities: crate::capabilities::Capabilities::from_profile::<P>(),
            }),
        }
    }

    /// Get a reference to the underlying camera.
    pub fn camera(&self) -> &crate::camera::Camera<crate::mode::Async, P, Tr, Exec> {
        &self.inner.camera
    }

    /// Try to consume the wrapper and return the underlying camera.
    ///
    /// Returns `Ok(camera)` if this is the only reference to the inner camera,
    /// or `Err(self)` if there are other references (e.g., from in-flight
    /// operation handles).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let dyn_camera = camera.into_dyn();
    /// // Use the dyn camera...
    /// match dyn_camera.try_into_inner() {
    ///     Ok(camera) => { /* Use concrete camera */ }
    ///     Err(dyn_camera) => { /* Still have references */ }
    /// }
    /// ```
    pub fn try_into_inner(
        self,
    ) -> Result<crate::camera::Camera<crate::mode::Async, P, Tr, Exec>, Self> {
        match Arc::try_unwrap(self.inner) {
            Ok(inner) => Ok(inner.camera),
            Err(inner) => Err(Self { inner }),
        }
    }

    /// Consume the wrapper and return the underlying camera.
    ///
    /// This is a convenience method that unwraps the result of `try_into_inner`.
    /// If there are other references to the camera, this will return an error.
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidState` if there are still references to the inner camera.
    pub fn into_inner(
        self,
    ) -> Result<crate::camera::Camera<crate::mode::Async, P, Tr, Exec>, Error> {
        self.try_into_inner().map_err(|_| {
            Error::InvalidState(std::borrow::Cow::Borrowed(
                "Cannot unwrap DynCamera: there are still references to the inner camera. \
                 Ensure all InFlightDyn handles are dropped before calling into_inner().",
            ))
        })
    }

    /// Get an `Arc<dyn RuntimeDyn>` for creating `InFlightDyn` handles.
    fn runtime_dyn(&self) -> Arc<dyn RuntimeDyn> {
        let inner: Arc<DynCameraInner<P, Tr, Exec>> = Arc::clone(&self.inner);
        inner
    }

    /// Convert a static in-flight handle into the type-erased dyn handle.
    fn erase_inflight<C>(
        &self,
        handle: crate::camera::inflight::InFlight<'_, C, P, Exec>,
        category: OperationCategory,
    ) -> Result<InFlightDyn, Error> {
        let (id, camera_id, response_future) = handle.into_parts()?;
        Ok(InFlightDyn::new(
            id,
            camera_id,
            category,
            self.runtime_dyn(),
            response_future,
        ))
    }
}

// Implement RuntimeDyn for the inner camera wrapper
#[cfg(feature = "dyn-api")]
impl<P, Tr, Exec> RuntimeDyn for DynCameraInner<P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    fn cancel(
        &self,
        camera_id: crate::CameraId,
        id: CommandId,
    ) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(self.camera.runtime().cancel(camera_id, id))
    }

    fn await_response_completion(
        &self,
        timeout: Duration,
        response_future: ResponseFuture,
    ) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            self.camera
                .runtime()
                .timeout(timeout, response_future)
                .await?
                .map(|_| ())
        })
    }
}

#[cfg(feature = "dyn-api")]
impl<P, Tr, Exec> std::fmt::Debug for DynCamera<P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynCamera")
            .field("profile", &std::any::type_name::<P>())
            .finish()
    }
}

// Implement DynCameraControl for DynCamera
#[cfg(feature = "dyn-api")]
impl<P, Tr, Exec> DynCameraControl for DynCamera<P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    fn capabilities(&self) -> &crate::capabilities::Capabilities {
        &self.inner.capabilities
    }

    fn pan_tilt(&self) -> &dyn DynPanTiltControl {
        self
    }

    fn zoom(&self) -> &dyn DynZoomControl {
        self
    }

    fn focus(&self) -> &dyn DynFocusControl {
        self
    }

    fn presets(&self) -> &dyn DynPresetsControl {
        self
    }

    fn motion(&self) -> &dyn DynMotionControl {
        self
    }

    fn state_cache(&self) -> &crate::StateCache {
        self.inner.camera.state_cache()
    }
}

// Implement DynPanTiltControl for DynCamera
#[cfg(feature = "dyn-api")]
impl<P, Tr, Exec> DynPanTiltControl for DynCamera<P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    fn pan_tilt_stop(&self) -> BoxFuture<'_, Result<(), Error>> {
        use crate::camera::controls::pan_tilt::PanTiltControl;
        self.inner.camera.pan_tilt_stop()
    }

    fn pan_tilt_home(&self, timeout: Option<Duration>) -> BoxFuture<'_, Result<(), Error>> {
        match timeout {
            Some(t) => Box::pin(async move {
                let handle = self.inner.camera.pan_tilt_home_op().await?;
                handle.await_completion(t).await
            }),
            None => {
                use crate::camera::controls::pan_tilt::PanTiltControl;
                self.inner.camera.pan_tilt_home()
            }
        }
    }

    fn pan_tilt_home_op(&self) -> BoxFuture<'_, Result<InFlightDyn, Error>> {
        Box::pin(async move {
            let handle = self.inner.camera.pan_tilt_home_op().await?;
            self.erase_inflight(handle, OperationCategory::PanTilt)
        })
    }

    fn pan_tilt_absolute(
        &self,
        pan_deg: f64,
        tilt_deg: f64,
        speed: SpeedLevel,
        timeout: Option<Duration>,
    ) -> BoxFuture<'_, Result<(), Error>> {
        match timeout {
            Some(t) => Box::pin(async move {
                let handle = self
                    .inner
                    .camera
                    .pan_tilt_absolute_op(pan_deg, tilt_deg, speed)
                    .await?;
                handle.await_completion(t).await
            }),
            None => {
                use crate::camera::controls::pan_tilt::PanTiltControl;
                self.inner
                    .camera
                    .pan_tilt_absolute(pan_deg, tilt_deg, speed)
            }
        }
    }

    fn pan_tilt_absolute_op(
        &self,
        pan_deg: f64,
        tilt_deg: f64,
        speed: SpeedLevel,
    ) -> BoxFuture<'_, Result<InFlightDyn, Error>> {
        Box::pin(async move {
            let handle = self
                .inner
                .camera
                .pan_tilt_absolute_op(pan_deg, tilt_deg, speed)
                .await?;
            self.erase_inflight(handle, OperationCategory::PanTilt)
        })
    }

    fn pan_tilt_relative(
        &self,
        pan_deg: f64,
        tilt_deg: f64,
        speed: SpeedLevel,
        timeout: Option<Duration>,
    ) -> BoxFuture<'_, Result<(), Error>> {
        match timeout {
            Some(t) => Box::pin(async move {
                let handle = self
                    .inner
                    .camera
                    .pan_tilt_relative_op(pan_deg, tilt_deg, speed)
                    .await?;
                handle.await_completion(t).await
            }),
            None => {
                use crate::camera::controls::pan_tilt::PanTiltControl;
                self.inner
                    .camera
                    .pan_tilt_relative(pan_deg, tilt_deg, speed)
            }
        }
    }

    fn pan_tilt_relative_op(
        &self,
        pan_deg: f64,
        tilt_deg: f64,
        speed: SpeedLevel,
    ) -> BoxFuture<'_, Result<InFlightDyn, Error>> {
        Box::pin(async move {
            let handle = self
                .inner
                .camera
                .pan_tilt_relative_op(pan_deg, tilt_deg, speed)
                .await?;
            self.erase_inflight(handle, OperationCategory::PanTilt)
        })
    }

    fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> BoxFuture<'_, Result<(), Error>> {
        use crate::camera::controls::pan_tilt::PanTiltControl;
        self.inner
            .camera
            .pan_tilt_move(direction, pan_speed, tilt_speed)
    }

    fn pan_tilt_reset(&self, timeout: Option<Duration>) -> BoxFuture<'_, Result<(), Error>> {
        match timeout {
            Some(t) => Box::pin(async move {
                let handle = self.inner.camera.pan_tilt_reset_op().await?;
                handle.await_completion(t).await
            }),
            None => {
                use crate::camera::controls::pan_tilt::PanTiltControl;
                self.inner.camera.pan_tilt_reset()
            }
        }
    }

    fn pan_tilt_reset_op(&self) -> BoxFuture<'_, Result<InFlightDyn, Error>> {
        Box::pin(async move {
            let handle = self.inner.camera.pan_tilt_reset_op().await?;
            self.erase_inflight(handle, OperationCategory::PanTilt)
        })
    }

    fn pan_tilt_limit_set(
        &self,
        corner: PanTiltLimitCorner,
        pan: PanPosition,
        tilt: TiltPosition,
    ) -> BoxFuture<'_, Result<(), Error>> {
        use crate::camera::controls::pan_tilt::PanTiltControl;
        self.inner.camera.pan_tilt_limit_set(corner, pan, tilt)
    }

    fn pan_tilt_limit_clear(&self, corner: PanTiltLimitCorner) -> BoxFuture<'_, Result<(), Error>> {
        use crate::camera::controls::pan_tilt::PanTiltControl;
        self.inner.camera.pan_tilt_limit_clear(corner)
    }
}

// Implement DynZoomControl for DynCamera
#[cfg(feature = "dyn-api")]
impl<P, Tr, Exec> DynZoomControl for DynCamera<P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    fn zoom_stop(&self) -> BoxFuture<'_, Result<(), Error>> {
        use crate::camera::controls::zoom::ZoomControl;
        self.inner.camera.zoom_stop()
    }

    fn zoom_tele(
        &self,
        speed: Option<ZoomSpeed>,
        timeout: Option<Duration>,
    ) -> BoxFuture<'_, Result<(), Error>> {
        match timeout {
            Some(t) => Box::pin(async move {
                let handle = self.inner.camera.zoom_tele_op(speed).await?;
                handle.await_completion(t).await
            }),
            None => {
                use crate::camera::controls::zoom::ZoomControl;
                self.inner.camera.zoom_tele(speed)
            }
        }
    }

    fn zoom_wide(
        &self,
        speed: Option<ZoomSpeed>,
        timeout: Option<Duration>,
    ) -> BoxFuture<'_, Result<(), Error>> {
        match timeout {
            Some(t) => Box::pin(async move {
                let handle = self.inner.camera.zoom_wide_op(speed).await?;
                handle.await_completion(t).await
            }),
            None => {
                use crate::camera::controls::zoom::ZoomControl;
                self.inner.camera.zoom_wide(speed)
            }
        }
    }

    fn set_zoom(
        &self,
        position: ZoomPosition,
        timeout: Option<Duration>,
    ) -> BoxFuture<'_, Result<(), Error>> {
        match timeout {
            Some(t) => Box::pin(async move {
                let handle = self.inner.camera.set_zoom_op(position).await?;
                handle.await_completion(t).await
            }),
            None => {
                use crate::camera::controls::zoom::ZoomControl;
                self.inner.camera.set_zoom(position)
            }
        }
    }

    fn set_zoom_op(&self, position: ZoomPosition) -> BoxFuture<'_, Result<InFlightDyn, Error>> {
        Box::pin(async move {
            let handle = self.inner.camera.set_zoom_op(position).await?;
            self.erase_inflight(handle, OperationCategory::Zoom)
        })
    }

    fn set_digital_zoom(&self, enabled: bool) -> BoxFuture<'_, Result<(), Error>> {
        use crate::camera::controls::zoom::ZoomControl;
        self.inner.camera.set_digital_zoom(enabled)
    }

    fn zoom_absolute_normalized(
        &self,
        position: Normalized,
        domain: ZoomDomain,
        timeout: Option<Duration>,
    ) -> BoxFuture<'_, Result<(), Error>> {
        match timeout {
            Some(t) => Box::pin(async move {
                let handle = self
                    .inner
                    .camera
                    .zoom_absolute_normalized_op(position, domain)
                    .await?;
                handle.await_completion(t).await
            }),
            None => {
                use crate::camera::controls::zoom::ZoomControl;
                self.inner.camera.zoom_absolute_normalized(position, domain)
            }
        }
    }
}

// Implement DynFocusControl for DynCamera
#[cfg(feature = "dyn-api")]
impl<P, Tr, Exec> DynFocusControl for DynCamera<P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    fn focus_auto(&self) -> BoxFuture<'_, Result<(), Error>> {
        use crate::camera::controls::focus::FocusControl;
        self.inner.camera.focus_auto()
    }

    fn focus_manual(&self) -> BoxFuture<'_, Result<(), Error>> {
        use crate::camera::controls::focus::FocusControl;
        self.inner.camera.focus_manual()
    }

    fn focus_near(&self, speed: SpeedLevel) -> BoxFuture<'_, Result<(), Error>> {
        use crate::camera::controls::focus::FocusControl;
        self.inner.camera.focus_near(speed)
    }

    fn focus_far(&self, speed: SpeedLevel) -> BoxFuture<'_, Result<(), Error>> {
        use crate::camera::controls::focus::FocusControl;
        self.inner.camera.focus_far(speed)
    }

    fn focus_stop(&self) -> BoxFuture<'_, Result<(), Error>> {
        use crate::camera::controls::focus::FocusControl;
        self.inner.camera.focus_stop()
    }

    fn focus_one_push(&self) -> BoxFuture<'_, Result<(), Error>> {
        use crate::camera::controls::focus::FocusControl;
        self.inner.camera.focus_one_push()
    }

    fn set_focus(
        &self,
        position: FocusPosition,
        timeout: Option<Duration>,
    ) -> BoxFuture<'_, Result<(), Error>> {
        match timeout {
            Some(t) => Box::pin(async move {
                let handle = self.inner.camera.set_focus_op(position).await?;
                handle.await_completion(t).await
            }),
            None => {
                use crate::camera::controls::focus::FocusControl;
                self.inner.camera.set_focus(position)
            }
        }
    }

    fn set_focus_op(&self, position: FocusPosition) -> BoxFuture<'_, Result<InFlightDyn, Error>> {
        Box::pin(async move {
            let handle = self.inner.camera.set_focus_op(position).await?;
            self.erase_inflight(handle, OperationCategory::Focus)
        })
    }

    fn focus_infinity(&self) -> BoxFuture<'_, Result<(), Error>> {
        use crate::camera::controls::focus::FocusControl;
        self.inner.camera.focus_infinity()
    }

    fn set_focus_zone(&self, zone: FocusZone) -> BoxFuture<'_, Result<(), Error>> {
        use crate::camera::controls::focus::FocusControl;
        self.inner.camera.set_focus_zone(zone)
    }

    fn set_auto_focus_sensitivity(
        &self,
        sensitivity: AutoFocusSensitivity,
    ) -> BoxFuture<'_, Result<(), Error>> {
        use crate::camera::controls::focus::FocusControl;
        self.inner.camera.set_auto_focus_sensitivity(sensitivity)
    }

    fn set_focus_near_limit(&self, position: FocusPosition) -> BoxFuture<'_, Result<(), Error>> {
        use crate::camera::controls::focus::FocusControl;
        self.inner.camera.set_focus_near_limit(position)
    }
}

// Implement DynPresetsControl for DynCamera
#[cfg(feature = "dyn-api")]
impl<P, Tr, Exec> DynPresetsControl for DynCamera<P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    fn preset_recall(
        &self,
        preset: PresetNumber,
        timeout: Option<Duration>,
    ) -> BoxFuture<'_, Result<(), Error>> {
        match timeout {
            Some(t) => Box::pin(async move {
                let handle = self.inner.camera.preset_recall_op(preset).await?;
                handle.await_completion(t).await
            }),
            None => {
                use crate::camera::controls::presets::PresetsControl;
                self.inner.camera.preset_recall(preset)
            }
        }
    }

    fn preset_recall_op(&self, preset: PresetNumber) -> BoxFuture<'_, Result<InFlightDyn, Error>> {
        Box::pin(async move {
            let handle = self.inner.camera.preset_recall_op(preset).await?;
            self.erase_inflight(handle, OperationCategory::Preset)
        })
    }

    fn preset_set(&self, preset: PresetNumber) -> BoxFuture<'_, Result<(), Error>> {
        use crate::camera::controls::presets::PresetsControl;
        self.inner.camera.preset_set(preset)
    }

    fn preset_reset(&self, preset: PresetNumber) -> BoxFuture<'_, Result<(), Error>> {
        use crate::camera::controls::presets::PresetsControl;
        self.inner.camera.preset_reset(preset)
    }
}

// Implement DynMotionControl for DynCamera
#[cfg(feature = "dyn-api")]
impl<P, Tr, Exec> DynMotionControl for DynCamera<P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    fn stop_all_motion(&self) -> BoxFuture<'_, Result<(), Error>> {
        use crate::camera::controls::motion::MotionControl;
        self.inner.camera.stop_all_motion()
    }

    fn await_idle(&self, timeout: Duration) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(self.inner.camera.await_idle(timeout))
    }

    fn await_pan_tilt_idle(&self, timeout: Duration) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(self.inner.camera.await_pan_tilt_idle(timeout))
    }

    fn await_zoom_idle(&self, timeout: Duration) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(self.inner.camera.await_zoom_idle(timeout))
    }

    fn await_focus_idle(&self, timeout: Duration) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(self.inner.camera.await_focus_idle(timeout))
    }
}

/// Extension trait to convert an async camera into a dyn-compatible wrapper.
#[cfg(feature = "dyn-api")]
pub trait IntoDynCamera {
    /// The profile type.
    type Profile;
    /// The transport type.
    type Transport;
    /// The executor type.
    type Executor;

    /// Convert this camera into a `DynCamera` wrapper for trait object usage.
    fn into_dyn(self) -> DynCamera<Self::Profile, Self::Transport, Self::Executor>
    where
        Self::Profile: crate::capabilities::Profile + Default,
        Self::Transport: crate::transport::AsyncTransport + Send + Sync + 'static,
        Self::Executor: crate::executor::Executor;
}

#[cfg(feature = "dyn-api")]
impl<P, Tr, Exec> IntoDynCamera for crate::camera::Camera<crate::mode::Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    type Profile = P;
    type Transport = Tr;
    type Executor = Exec;

    fn into_dyn(self) -> DynCamera<P, Tr, Exec> {
        DynCamera::new(self)
    }
}

#[cfg(test)]
#[cfg(feature = "dyn-api")]
mod tests {
    use super::*;

    // Compile-time test: verify traits are object-safe
    fn _assert_object_safe(
        _cam: &dyn DynCameraControl,
        _pt: &dyn DynPanTiltControl,
        _zoom: &dyn DynZoomControl,
        _focus: &dyn DynFocusControl,
        _presets: &dyn DynPresetsControl,
        _motion: &dyn DynMotionControl,
    ) {
    }

    // Compile-time test: verify InFlightDyn is Send + Sync
    fn _assert_inflight_dyn_is_send_sync(_h: &InFlightDyn)
    where
        InFlightDyn: Send + Sync,
    {
    }

    #[test]
    fn test_operation_category_display() {
        assert_eq!(format!("{}", OperationCategory::PanTilt), "PanTilt");
        assert_eq!(format!("{}", OperationCategory::Zoom), "Zoom");
        assert_eq!(format!("{}", OperationCategory::Focus), "Focus");
        assert_eq!(format!("{}", OperationCategory::Preset), "Preset");
    }
}
