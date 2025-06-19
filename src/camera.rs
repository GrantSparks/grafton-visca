//! Camera profile system for type-safe, model-specific VISCA control.

use std::fmt::Display;
use std::ops::RangeInclusive;
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use std::sync::Arc;

use crate::error::Error as ViscaError;
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use crate::session::Session;
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use crate::sync_primitives::{Mutex, Semaphore};
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use crate::transport::Transport;
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use crate::{Command, Response};

pub mod builder;
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
pub mod command_builder;
pub mod commands;
pub mod extensions;
pub mod inquiry;
pub mod profiles;

// Re-export commonly used types
pub use builder::{CustomProfile, CustomProfileBuilder, CustomProfileTypedBuilder};
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
pub use command_builder::CommandBuilderExt;
pub use extensions::{CameraExtension, CustomManufacturerExt, DiagnosticsExt, ScriptingExt};
pub use inquiry::{CameraState, Exposure, ImageSettings, Optics, Position, WhiteBalance};
pub use profiles::{GenericVisca, PTZOptics30X, PTZOpticsG2, SonyEVID70};

/// Minimal executor for polling std::future::Ready futures without an async runtime.
/// This is used for the blocking API when no async runtime is available.
#[cfg(all(feature = "blocking-client", not(feature = "async-client")))]
mod minimal_executor {
    use std::future::Future;
    use std::sync::Arc;
    use std::task::{Context, Poll};

    /// A no-op waker that does nothing when woken.
    /// This is safe for Ready futures since they are already complete.
    struct NoopWaker;

    impl std::task::Wake for NoopWaker {
        fn wake(self: Arc<Self>) {}
        fn wake_by_ref(self: &Arc<Self>) {}
    }

    /// Block on a future by polling it once.
    ///
    /// This only works for futures that are immediately ready (like std::future::Ready).
    /// Returns a Result to handle the case where the future is not ready.
    pub fn block_on_ready<F: Future>(fut: F) -> Result<F::Output, &'static str>
    where
        F::Output: std::fmt::Debug,
    {
        let waker = Arc::new(NoopWaker).into();
        let mut cx = Context::from_waker(&waker);
        let mut fut = Box::pin(fut);

        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(output) => Ok(output),
            Poll::Pending => {
                // This should never happen with the futures we use (which are immediately ready)
                // Return an error instead of panicking
                Err("Future not ready - this executor only supports immediately-ready futures")
            }
        }
    }
}

/// Core camera abstraction with compile-time profile information.
pub struct Camera<P: CameraProfile> {
    profile: P,
    #[cfg(any(feature = "blocking-client", feature = "async-client"))]
    transport: Arc<Mutex<Box<dyn Transport>>>,
    #[cfg(any(feature = "blocking-client", feature = "async-client"))]
    session: Arc<Mutex<Session>>,
    #[cfg(any(feature = "blocking-client", feature = "async-client"))]
    semaphore: Arc<Semaphore>,
}

impl<P: CameraProfile> std::fmt::Debug for Camera<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Camera")
            .field("profile", &self.profile)
            .field("model", &P::MODEL_NAME)
            .finish()
    }
}

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
impl<P: CameraProfile + Clone> Clone for Camera<P> {
    fn clone(&self) -> Self {
        Self {
            profile: self.profile.clone(),
            transport: self.transport.clone(),
            session: self.session.clone(),
            semaphore: self.semaphore.clone(),
        }
    }
}

/// Trait defining camera-specific capabilities and conversions.
pub trait CameraProfile: Default + Send + Sync + std::fmt::Debug {
    /// Camera model name for identification.
    const MODEL_NAME: &'static str;

    /// Valid pan position range in VISCA units.
    const PAN_RANGE: RangeInclusive<i16>;

    /// Valid tilt position range in VISCA units.
    const TILT_RANGE: RangeInclusive<i16>;

    /// Valid zoom position range in VISCA units.
    const ZOOM_RANGE: RangeInclusive<u16>;

    /// Valid focus position range in VISCA units.
    const FOCUS_RANGE: RangeInclusive<u16>;

    /// Whether this camera supports digital zoom.
    const DIGITAL_ZOOM_SUPPORTED: bool = false;

    /// Maximum pan speed supported by this camera.
    const MAX_PAN_SPEED: u8 = 24;

    /// Maximum tilt speed supported by this camera.
    const MAX_TILT_SPEED: u8 = 20;

    /// Convert pan position from VISCA units to degrees.
    fn pan_units_to_degrees(&self, units: i16) -> f32;

    /// Convert tilt position from VISCA units to degrees.
    fn tilt_units_to_degrees(&self, units: i16) -> f32;

    /// Convert pan position from degrees to VISCA units.
    fn pan_degrees_to_units(&self, degrees: f32) -> i16;

    /// Convert tilt position from degrees to VISCA units.
    fn tilt_degrees_to_units(&self, degrees: f32) -> i16;

    /// Get the model name (allows instance override).
    fn model_name(&self) -> &str {
        Self::MODEL_NAME
    }

    /// Get the pan range (allows instance override).
    fn pan_range(&self) -> RangeInclusive<i16> {
        Self::PAN_RANGE
    }

    /// Get the tilt range (allows instance override).
    fn tilt_range(&self) -> RangeInclusive<i16> {
        Self::TILT_RANGE
    }

    /// Get the zoom range (allows instance override).
    fn zoom_range(&self) -> RangeInclusive<u16> {
        Self::ZOOM_RANGE
    }

    /// Get the focus range (allows instance override).
    fn focus_range(&self) -> RangeInclusive<u16> {
        Self::FOCUS_RANGE
    }

    /// Check if digital zoom is supported (allows instance override).
    fn digital_zoom_supported(&self) -> bool {
        Self::DIGITAL_ZOOM_SUPPORTED
    }

    /// Get the maximum pan speed (allows instance override).
    fn max_pan_speed(&self) -> u8 {
        Self::MAX_PAN_SPEED
    }

    /// Get the maximum tilt speed (allows instance override).
    fn max_tilt_speed(&self) -> u8 {
        Self::MAX_TILT_SPEED
    }

    /// Get the total pan range in degrees.
    fn pan_degree_range(&self) -> RangeInclusive<f32> {
        let min = self.pan_units_to_degrees(*self.pan_range().start());
        let max = self.pan_units_to_degrees(*self.pan_range().end());
        min..=max
    }

    /// Get the total tilt range in degrees.
    fn tilt_degree_range(&self) -> RangeInclusive<f32> {
        let min = self.tilt_units_to_degrees(*self.tilt_range().start());
        let max = self.tilt_units_to_degrees(*self.tilt_range().end());
        min..=max
    }

    /// Associated type for camera-specific preset IDs.
    type PresetId: Into<u8> + TryFrom<u8, Error = ViscaError> + Copy + Display;

    /// Associated type for camera-specific gain values.
    type GainValue: Into<u8> + TryFrom<u8, Error = ViscaError> + Copy + Display;

    /// Get the maximum preset ID for this camera.
    fn max_preset_id() -> u8;

    // Capability query methods

    /// Check if the camera supports auto-focus.
    fn supports_auto_focus(&self) -> bool {
        true // Most VISCA cameras support auto-focus
    }

    /// Check if the camera supports manual focus.
    fn supports_manual_focus(&self) -> bool {
        true // Most VISCA cameras support manual focus
    }

    /// Check if the camera supports auto-exposure.
    fn supports_auto_exposure(&self) -> bool {
        true // Most VISCA cameras support auto-exposure
    }

    /// Check if the camera supports manual exposure.
    fn supports_manual_exposure(&self) -> bool {
        true // Most VISCA cameras support manual exposure
    }

    /// Check if the camera supports white balance adjustment.
    fn supports_white_balance(&self) -> bool {
        true // Most VISCA cameras support white balance
    }

    /// Check if the camera supports image flip (horizontal/vertical).
    fn supports_image_flip(&self) -> bool {
        true // Most modern VISCA cameras support image flip
    }

    /// Check if the camera supports backlight compensation.
    fn supports_backlight_compensation(&self) -> bool {
        true // Common feature in VISCA cameras
    }

    /// Check if the camera supports wide dynamic range (WDR).
    fn supports_wide_dynamic_range(&self) -> bool {
        false // Not all cameras support this
    }

    /// Check if the camera supports image stabilization.
    fn supports_image_stabilization(&self) -> bool {
        false // Camera-specific feature
    }

    /// Check if the camera supports low-light mode.
    fn supports_low_light_mode(&self) -> bool {
        false // Camera-specific feature
    }

    /// Check if the camera supports color control (saturation, hue).
    fn supports_color_control(&self) -> bool {
        true // Most VISCA cameras support basic color control
    }

    /// Check if the camera supports gain control.
    fn supports_gain_control(&self) -> bool {
        true // Most VISCA cameras support gain control
    }

    /// Check if the camera supports shutter speed control.
    fn supports_shutter_control(&self) -> bool {
        true // Most VISCA cameras support shutter control
    }

    /// Check if the camera supports iris control.
    fn supports_iris_control(&self) -> bool {
        true // Most VISCA cameras support iris control
    }

    /// Check if the camera supports noise reduction.
    fn supports_noise_reduction(&self) -> bool {
        false // Camera-specific feature
    }

    /// Check if the camera supports privacy zones.
    fn supports_privacy_zones(&self) -> bool {
        false // Not a standard VISCA feature
    }

    /// Check if the camera supports motion detection.
    fn supports_motion_detection(&self) -> bool {
        false // Not a standard VISCA feature
    }

    /// Get the number of available white balance modes.
    fn white_balance_mode_count(&self) -> u8 {
        5 // Typical: Auto, Indoor, Outdoor, One-Push, Manual
    }

    /// Get the number of available exposure modes.
    fn exposure_mode_count(&self) -> u8 {
        4 // Typical: Auto, Manual, Shutter Priority, Iris Priority
    }

    /// Check if the camera supports a specific zoom speed.
    fn supports_zoom_speed(&self, speed: u8) -> bool {
        speed <= 7 // Most VISCA cameras support zoom speeds 0-7
    }

    /// Check if the camera supports a specific focus speed.
    fn supports_focus_speed(&self, speed: u8) -> bool {
        speed <= 7 // Most VISCA cameras support focus speeds 0-7
    }

    /// Get the supported shutter speed range (if applicable).
    /// Returns None if shutter control is not supported.
    fn shutter_speed_range(&self) -> Option<RangeInclusive<u8>> {
        if self.supports_shutter_control() {
            Some(0..=21) // Typical VISCA shutter speed range
        } else {
            None
        }
    }

    /// Get the supported iris range (if applicable).
    /// Returns None if iris control is not supported.
    fn iris_range(&self) -> Option<RangeInclusive<u8>> {
        if self.supports_iris_control() {
            Some(0..=20) // Typical VISCA iris range
        } else {
            None
        }
    }

    /// Get the supported gain range (if applicable).
    /// Returns None if gain control is not supported.
    fn gain_range(&self) -> Option<RangeInclusive<u8>> {
        if self.supports_gain_control() {
            Some(0..=15) // Typical VISCA gain range
        } else {
            None
        }
    }

    /// Check if the camera supports continuous pan/tilt movement.
    fn supports_continuous_movement(&self) -> bool {
        true // Most VISCA cameras support this
    }

    /// Check if the camera supports absolute position commands.
    fn supports_absolute_positioning(&self) -> bool {
        true // Standard VISCA feature
    }

    /// Check if the camera supports relative position commands.
    fn supports_relative_positioning(&self) -> bool {
        true // Standard VISCA feature
    }

    /// Get a list of supported features as a capability summary.
    fn capability_summary(&self) -> CapabilitySummary {
        CapabilitySummary {
            model: self.model_name().to_string(),
            movement: MovementCapabilities {
                continuous: self.supports_continuous_movement(),
                absolute: self.supports_absolute_positioning(),
                relative: self.supports_relative_positioning(),
                pan_range: self.pan_degree_range(),
                tilt_range: self.tilt_degree_range(),
                max_pan_speed: self.max_pan_speed(),
                max_tilt_speed: self.max_tilt_speed(),
            },
            zoom: ZoomCapabilities {
                optical_range: self.zoom_range(),
                digital_zoom: self.digital_zoom_supported(),
                speed_levels: 8, // Standard VISCA zoom speed levels
            },
            focus: FocusCapabilities {
                auto_focus: self.supports_auto_focus(),
                manual_focus: self.supports_manual_focus(),
                range: self.focus_range(),
                speed_levels: 8, // Standard VISCA focus speed levels
            },
            exposure: ExposureCapabilities {
                auto_exposure: self.supports_auto_exposure(),
                manual_exposure: self.supports_manual_exposure(),
                shutter_control: self.supports_shutter_control(),
                iris_control: self.supports_iris_control(),
                gain_control: self.supports_gain_control(),
                backlight_comp: self.supports_backlight_compensation(),
                wide_dynamic_range: self.supports_wide_dynamic_range(),
            },
            image: ImageCapabilities {
                white_balance: self.supports_white_balance(),
                color_control: self.supports_color_control(),
                image_flip: self.supports_image_flip(),
                image_stabilization: self.supports_image_stabilization(),
                noise_reduction: self.supports_noise_reduction(),
                low_light_mode: self.supports_low_light_mode(),
            },
            presets: PresetCapabilities {
                count: Self::max_preset_id() + 1,
                speed_support: true, // Most VISCA cameras support preset recall speed
            },
        }
    }
}

/// Summary of all camera capabilities.
#[derive(Debug, Clone)]
pub struct CapabilitySummary {
    /// Camera model name.
    pub model: String,
    /// Movement capabilities.
    pub movement: MovementCapabilities,
    /// Zoom capabilities.
    pub zoom: ZoomCapabilities,
    /// Focus capabilities.
    pub focus: FocusCapabilities,
    /// Exposure capabilities.
    pub exposure: ExposureCapabilities,
    /// Image processing capabilities.
    pub image: ImageCapabilities,
    /// Preset capabilities.
    pub presets: PresetCapabilities,
}

/// Movement-related capabilities.
#[derive(Debug, Clone)]
pub struct MovementCapabilities {
    /// Supports continuous pan/tilt movement.
    pub continuous: bool,
    /// Supports absolute positioning.
    pub absolute: bool,
    /// Supports relative positioning.
    pub relative: bool,
    /// Pan range in degrees.
    pub pan_range: RangeInclusive<f32>,
    /// Tilt range in degrees.
    pub tilt_range: RangeInclusive<f32>,
    /// Maximum pan speed.
    pub max_pan_speed: u8,
    /// Maximum tilt speed.
    pub max_tilt_speed: u8,
}

/// Zoom-related capabilities.
#[derive(Debug, Clone)]
pub struct ZoomCapabilities {
    /// Optical zoom range.
    pub optical_range: RangeInclusive<u16>,
    /// Digital zoom support.
    pub digital_zoom: bool,
    /// Number of zoom speed levels.
    pub speed_levels: u8,
}

/// Focus-related capabilities.
#[derive(Debug, Clone)]
pub struct FocusCapabilities {
    /// Auto-focus support.
    pub auto_focus: bool,
    /// Manual focus support.
    pub manual_focus: bool,
    /// Focus position range.
    pub range: RangeInclusive<u16>,
    /// Number of focus speed levels.
    pub speed_levels: u8,
}

/// Exposure-related capabilities.
#[derive(Debug, Clone, Copy)]
pub struct ExposureCapabilities {
    /// Auto-exposure support.
    pub auto_exposure: bool,
    /// Manual exposure support.
    pub manual_exposure: bool,
    /// Shutter speed control.
    pub shutter_control: bool,
    /// Iris control.
    pub iris_control: bool,
    /// Gain control.
    pub gain_control: bool,
    /// Backlight compensation.
    pub backlight_comp: bool,
    /// Wide dynamic range.
    pub wide_dynamic_range: bool,
}

/// Image processing capabilities.
#[derive(Debug, Clone, Copy)]
pub struct ImageCapabilities {
    /// White balance control.
    pub white_balance: bool,
    /// Color control (saturation, hue).
    pub color_control: bool,
    /// Image flip support.
    pub image_flip: bool,
    /// Image stabilization.
    pub image_stabilization: bool,
    /// Noise reduction.
    pub noise_reduction: bool,
    /// Low-light mode.
    pub low_light_mode: bool,
}

/// Preset-related capabilities.
#[derive(Debug, Clone, Copy)]
pub struct PresetCapabilities {
    /// Number of available presets.
    pub count: u8,
    /// Preset recall speed support.
    pub speed_support: bool,
}

/// Runtime camera capabilities for discovery and validation.
#[derive(Debug, Clone)]
pub struct CameraCapabilities {
    /// Camera model name.
    pub model_name: String,
    /// Pan range in degrees.
    pub pan_range_degrees: RangeInclusive<f32>,
    /// Tilt range in degrees.
    pub tilt_range_degrees: RangeInclusive<f32>,
    /// Total number of zoom steps.
    pub zoom_steps: usize,
    /// Total number of focus steps.
    pub focus_steps: usize,
    /// Number of available presets.
    pub preset_count: u8,
    /// Whether digital zoom is supported.
    pub supports_digital_zoom: bool,
    /// Maximum pan speed.
    pub max_pan_speed: u8,
    /// Maximum tilt speed.
    pub max_tilt_speed: u8,
}

// Default implementation for Camera when no features are enabled
#[cfg(not(any(feature = "blocking-client", feature = "async-client")))]
impl<P: CameraProfile> Default for Camera<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P: CameraProfile> Camera<P> {
    /// Create a new camera with the given transport.
    #[cfg(any(feature = "blocking-client", feature = "async-client"))]
    pub fn new(transport: impl Transport + 'static) -> Self {
        Self {
            profile: P::default(),
            transport: Arc::new(Mutex::new(Box::new(transport))),
            session: Arc::new(Mutex::new(Session::new())),
            semaphore: Arc::new(Semaphore::new(2)), // VISCA supports 2 concurrent commands
        }
    }

    /// Create a new camera without transport (profile only).
    #[cfg(not(any(feature = "blocking-client", feature = "async-client")))]
    pub fn new() -> Self {
        Self {
            profile: P::default(),
        }
    }

    /// Create a camera with a custom profile instance.
    #[cfg(any(feature = "blocking-client", feature = "async-client"))]
    pub fn with_profile(transport: impl Transport + 'static, profile: P) -> Self {
        Self {
            profile,
            transport: Arc::new(Mutex::new(Box::new(transport))),
            session: Arc::new(Mutex::new(Session::new())),
            semaphore: Arc::new(Semaphore::new(2)), // VISCA supports 2 concurrent commands
        }
    }

    /// Create a camera with a custom profile instance (no transport).
    #[cfg(not(any(feature = "blocking-client", feature = "async-client")))]
    pub fn with_profile(profile: P) -> Self {
        Self { profile }
    }

    /// Get the camera's capabilities.
    pub fn capabilities(&self) -> CameraCapabilities {
        CameraCapabilities {
            model_name: self.profile.model_name().to_string(),
            pan_range_degrees: self.profile.pan_degree_range(),
            tilt_range_degrees: self.profile.tilt_degree_range(),
            zoom_steps: self.profile.zoom_range().count(),
            focus_steps: self.profile.focus_range().count(),
            preset_count: P::max_preset_id(),
            supports_digital_zoom: self.profile.digital_zoom_supported(),
            max_pan_speed: self.profile.max_pan_speed(),
            max_tilt_speed: self.profile.max_tilt_speed(),
        }
    }

    /// Get the full capability summary for this camera.
    pub fn capability_summary(&self) -> CapabilitySummary {
        self.profile.capability_summary()
    }

    // Note: transport() and transport_mut() methods have been removed because
    // the transport is now behind a Mutex. Use send_raw methods instead for sending commands.

    /// Get the camera profile.
    pub fn profile(&self) -> &P {
        &self.profile
    }

    /// Check if the camera can accept a new command without blocking.
    ///
    /// Returns `true` if at least one command slot is available,
    /// `false` if all slots are in use and the next command will block.
    #[cfg(any(feature = "blocking-client", feature = "async-client"))]
    pub fn is_ready(&self) -> bool {
        self.semaphore.available_permits() > 0
    }

    /// Get the number of currently pending commands.
    ///
    /// Returns a value between 0 and 2, as VISCA supports up to 2 concurrent commands.
    #[cfg(any(feature = "blocking-client", feature = "async-client"))]
    pub fn pending_commands(&self) -> usize {
        2 - self.semaphore.available_permits()
    }

    /// Send a command and wait for completion (async).
    #[cfg(feature = "async-client")]
    async fn send_and_wait(&self, command: &dyn Command) -> Result<(), ViscaError> {
        match self.send_raw_async(command).await? {
            Response::Completion => Ok(()),
            Response::Ack => {
                // Wait for completion after ACK
                // This is handled internally by send_raw_async
                Ok(())
            }
            response => Err(ViscaError::InvalidResponse {
                expected: "Completion".to_string(),
                actual: format!("{:?}", response).into_bytes(),
            }),
        }
    }

    /// Send a command and wait for completion (blocking).
    #[cfg(all(feature = "blocking-client", not(feature = "async-client")))]
    fn send_and_wait(&self, command: &dyn Command) -> Result<(), ViscaError> {
        match self.send_raw(command)? {
            Response::Completion => Ok(()),
            Response::Ack => {
                // Wait for completion after ACK
                // This is handled internally by send_raw
                Ok(())
            }
            response => Err(ViscaError::InvalidResponse {
                expected: "Completion".to_string(),
                actual: format!("{:?}", response).into_bytes(),
            }),
        }
    }

    /// Send a raw command to the camera (blocking).
    #[cfg(all(feature = "blocking-client", not(feature = "async-client")))]
    pub fn send_raw(&self, command: &dyn Command) -> Result<Response, ViscaError> {
        use self::minimal_executor::block_on_ready;
        use crate::sync_primitives::SemaphoreExt;

        // Acquire semaphore permit for concurrency control
        let _permit = self.semaphore.acquire_permit();

        // Get socket assignment from session
        let socket_id = {
            let mut session = self.session.lock();
            session.assign_socket(command.response_type())?
        };

        // Send command with socket ID using the minimal executor
        {
            let mut transport = self.transport.lock();
            let send_result = block_on_ready(transport.send_command(command, socket_id))
                .map_err(|e| ViscaError::InvalidState(e.to_string()))?;
            if let Err(e) = send_result {
                // Release socket on error
                let mut session = self.session.lock();
                session.release_socket(socket_id);
                return Err(e);
            }
        }

        // Wait for response
        loop {
            let (_resp_socket_id, response_data) = {
                let mut transport = self.transport.lock();
                let receive_result = block_on_ready(transport.receive_response())
                    .map_err(|e| ViscaError::InvalidState(e.to_string()))?;
                receive_result?
            };

            let mut session = self.session.lock();
            match session.process_response(&response_data) {
                Ok(Some((socket, response))) if socket == socket_id => {
                    session.release_socket(socket_id);
                    return Ok(response);
                }
                Ok(_) => continue, // Response for different socket, keep waiting
                Err(e) => {
                    session.release_socket(socket_id);
                    return Err(e);
                }
            }
        }
    }

    /// Send a raw command to the camera (async).
    #[cfg(feature = "async-client")]
    pub async fn send_raw_async(&self, command: &dyn Command) -> Result<Response, ViscaError> {
        use crate::sync_primitives::SemaphoreExt;

        // Acquire semaphore permit for concurrency control
        let _permit = self.semaphore.acquire_permit().await?;

        // Get socket assignment from session
        let socket_id = {
            let mut session = self.session.lock().await;
            session.assign_socket(command.response_type())?
        };

        // Send command with socket ID
        {
            let mut transport = self.transport.lock().await;
            if let Err(e) = transport.send_command(command, socket_id).await {
                // Release socket on error
                let mut session = self.session.lock().await;
                session.release_socket(socket_id);
                return Err(e);
            }
        }

        // Wait for response
        loop {
            let (_resp_socket_id, response_data) = {
                let mut transport = self.transport.lock().await;
                transport.receive_response().await?
            };

            let mut session = self.session.lock().await;

            match session.process_response(&response_data) {
                Ok(Some((socket, response))) if socket == socket_id => {
                    session.release_socket(socket_id);
                    return Ok(response);
                }
                Ok(_) => continue, // Response for different socket, keep waiting
                Err(e) => {
                    session.release_socket(socket_id);
                    return Err(e);
                }
            }
        }
    }
}

/// Position units for type-safe coordinate handling.
pub mod units {
    /// Position in degrees.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Degrees<T>(pub T);

    /// Position in VISCA protocol units.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ViscaUnits<T>(pub T);

    /// Normalized position (-1.0 to 1.0).
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Normalized<T>(pub T);

    impl<T> Degrees<T> {
        /// Create a new position in degrees.
        pub fn new(value: T) -> Self {
            Self(value)
        }

        /// Get the inner value.
        pub fn value(&self) -> &T {
            &self.0
        }

        /// Consume and return the inner value.
        pub fn into_inner(self) -> T {
            self.0
        }
    }

    impl<T> ViscaUnits<T> {
        /// Create a new position in VISCA units.
        pub fn new(value: T) -> Self {
            Self(value)
        }

        /// Get the inner value.
        pub fn value(&self) -> &T {
            &self.0
        }

        /// Consume and return the inner value.
        pub fn into_inner(self) -> T {
            self.0
        }
    }

    impl<T> Normalized<T> {
        /// Create a new normalized position.
        pub fn new(value: T) -> Self {
            Self(value)
        }

        /// Get the inner value.
        pub fn value(&self) -> &T {
            &self.0
        }

        /// Consume and return the inner value.
        pub fn into_inner(self) -> T {
            self.0
        }
    }
}
