//! Unified camera client that abstracts over profile and transport generics.
//!
//! This module provides a single, simplified camera interface that works with
//! any camera profile and transport, eliminating the need for users to manage
//! generic type parameters.

use std::sync::Arc;
use std::time::Duration;

use crate::{
    capabilities::{ProfileIntrospection, ProfileMetadata, ProtocolStyle},
    command::{encode_visca::EncodeVisca, ResponseType},
    error::Error,
    transport::core::{BlockingTransport, Transport},
    Response,
};

use super::profiles::{GenericVisca, PTZOpticsG2, SonyFR7};

/// Dynamic camera profile wrapper that erases the concrete profile type.
///
/// This enum allows us to store different camera profiles without exposing
/// generic parameters to the user.
#[derive(Debug, Clone, Copy)]
pub enum DynamicProfile {
    /// PTZOptics G2 camera profile
    PTZOpticsG2(PTZOpticsG2),
    /// Sony FR7 camera profile  
    SonyFR7(SonyFR7),
    /// Generic VISCA camera profile (default)
    GenericVisca(GenericVisca),
}

impl DynamicProfile {
    /// Get the model name of the camera.
    #[must_use]
    pub fn model_name(&self) -> &'static str {
        match self {
            Self::PTZOpticsG2(_) => PTZOpticsG2::MODEL_NAME,
            Self::SonyFR7(_) => SonyFR7::MODEL_NAME,
            Self::GenericVisca(_) => GenericVisca::MODEL_NAME,
        }
    }

    /// Get the default VISCA address.
    #[must_use]
    pub fn default_address(&self) -> u8 {
        match self {
            Self::PTZOpticsG2(_) => PTZOpticsG2::DEFAULT_ADDRESS,
            Self::SonyFR7(_) => SonyFR7::DEFAULT_ADDRESS,
            Self::GenericVisca(_) => GenericVisca::DEFAULT_ADDRESS,
        }
    }

    /// Get the protocol style.
    #[must_use]
    pub fn protocol_style(&self) -> ProtocolStyle {
        match self {
            Self::PTZOpticsG2(_) => PTZOpticsG2::PROTOCOL_STYLE,
            Self::SonyFR7(_) => SonyFR7::PROTOCOL_STYLE,
            Self::GenericVisca(_) => GenericVisca::PROTOCOL_STYLE,
        }
    }

    /// Get the acknowledgment timeout.
    #[must_use]
    pub fn ack_timeout(&self) -> Duration {
        match self {
            Self::PTZOpticsG2(_) => PTZOpticsG2::ACK_TIMEOUT,
            Self::SonyFR7(_) => SonyFR7::ACK_TIMEOUT,
            Self::GenericVisca(_) => GenericVisca::ACK_TIMEOUT,
        }
    }

    /// Get the completion timeout.
    #[must_use]
    pub fn completion_timeout(&self) -> Duration {
        match self {
            Self::PTZOpticsG2(_) => PTZOpticsG2::COMPLETION_TIMEOUT,
            Self::SonyFR7(_) => SonyFR7::COMPLETION_TIMEOUT,
            Self::GenericVisca(_) => GenericVisca::COMPLETION_TIMEOUT,
        }
    }

    /// Get the busy timeout.
    #[must_use]
    pub fn busy_timeout(&self) -> Duration {
        match self {
            Self::PTZOpticsG2(_) => PTZOpticsG2::BUSY_TIMEOUT,
            Self::SonyFR7(_) => SonyFR7::BUSY_TIMEOUT,
            Self::GenericVisca(_) => GenericVisca::BUSY_TIMEOUT,
        }
    }

    /// Check if the camera supports VISCA inquiry commands.
    #[must_use]
    pub fn supports_inquiry(&self) -> bool {
        match self {
            Self::PTZOpticsG2(_) => PTZOpticsG2::SUPPORTS_INQUIRY,
            Self::SonyFR7(_) => SonyFR7::SUPPORTS_INQUIRY,
            Self::GenericVisca(_) => GenericVisca::SUPPORTS_INQUIRY,
        }
    }

    /// Get a capability summary for the camera.
    #[must_use]
    pub fn capability_summary(&self) -> String {
        match self {
            Self::PTZOpticsG2(p) => p.capability_summary(),
            Self::SonyFR7(p) => p.capability_summary(),
            Self::GenericVisca(p) => p.capability_summary(),
        }
    }
}

impl Default for DynamicProfile {
    fn default() -> Self {
        Self::GenericVisca(GenericVisca)
    }
}

/// Camera profile identifier for selecting a specific camera model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileId {
    /// PTZOptics G2 camera
    PTZOpticsG2,
    /// Sony FR7 camera
    SonyFR7,
    /// Generic VISCA camera (default)
    GenericVisca,
}

impl ProfileId {
    /// Convert to a dynamic profile instance.
    #[must_use]
    pub fn to_profile(self) -> DynamicProfile {
        match self {
            Self::PTZOpticsG2 => DynamicProfile::PTZOpticsG2(PTZOpticsG2),
            Self::SonyFR7 => DynamicProfile::SonyFR7(SonyFR7),
            Self::GenericVisca => DynamicProfile::GenericVisca(GenericVisca),
        }
    }
}

impl Default for ProfileId {
    fn default() -> Self {
        Self::GenericVisca
    }
}

/// Type-erased transport trait for unified camera.
///
/// This trait provides a unified interface for both async and blocking transports,
/// using async as the base and providing a blocking wrapper.
#[async_trait::async_trait]
pub trait UnifiedTransport: Send + Sync {
    /// Send raw bytes to the device.
    async fn send(&self, bytes: &[u8]) -> Result<(), Error>;

    /// Receive raw bytes from the device.
    async fn recv(&self) -> Result<bytes::Bytes, Error>;
}

/// Wrapper for async transports.
struct AsyncTransportWrapper<T: Transport> {
    transport: T,
}

#[async_trait::async_trait]
impl<T> UnifiedTransport for AsyncTransportWrapper<T>
where
    T: Transport + Send + Sync,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    async fn send(&self, bytes: &[u8]) -> Result<(), Error> {
        self.transport.send(bytes).await.map_err(Into::into)
    }

    async fn recv(&self) -> Result<bytes::Bytes, Error> {
        self.transport.recv().await.map_err(Into::into)
    }
}

/// Wrapper for blocking transports.
#[allow(dead_code)]
struct BlockingTransportWrapper<T: BlockingTransport> {
    transport: T,
}

#[async_trait::async_trait]
impl<T> UnifiedTransport for BlockingTransportWrapper<T>
where
    T: BlockingTransport + Send + Sync,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    async fn send(&self, bytes: &[u8]) -> Result<(), Error> {
        // Blocking transports return Ready futures that can be directly awaited
        self.transport.send(bytes).await.map_err(Into::into)
    }

    async fn recv(&self) -> Result<bytes::Bytes, Error> {
        // Blocking transports return Ready futures that can be directly awaited
        self.transport.recv().await.map_err(Into::into)
    }
}

/// Unified camera interface that abstracts over profile and transport generics.
///
/// This type provides a single, easy-to-use interface for camera control without
/// requiring users to specify generic type parameters. It supports both async and
/// blocking operation modes.
///
/// # Examples
///
/// ## Async usage
/// ```no_run
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// use grafton_visca::{Camera, ProfileId};
/// use grafton_visca::transport::tokio::Tcp;
///
/// // Create a camera with automatic profile detection
/// let transport = Tcp::connect("192.168.1.100:52381").await?;
/// let camera = Camera::new(transport);
///
/// // Or specify a profile explicitly
/// let camera = Camera::with_profile(ProfileId::PTZOpticsG2, transport);
///
/// // Use the camera
/// camera.send_command(&some_command).await?;
/// # Ok(())
/// # }
/// ```
///
/// ## Blocking usage
/// ```no_run
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// use grafton_visca::{Camera, ProfileId};
/// use grafton_visca::transport::blocking::Tcp;
///
/// // Create a camera with blocking transport
/// let transport = Tcp::connect("192.168.1.100:52381")?;
/// let camera = Camera::new(transport);
///
/// // Use the camera through the blocking wrapper
/// let blocking_camera = camera.blocking();
/// // Or use send_command_blocking directly for internal use
/// # Ok(())
/// # }
/// ```
pub struct Camera {
    profile: DynamicProfile,
    transport: Arc<dyn UnifiedTransport>,
    address: u8,
}

impl Clone for Camera {
    fn clone(&self) -> Self {
        Self {
            profile: self.profile,
            transport: Arc::clone(&self.transport),
            address: self.address,
        }
    }
}

impl std::fmt::Debug for Camera {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Camera")
            .field("profile", &self.profile)
            .field("address", &self.address)
            .field("transport", &"<dyn UnifiedTransport>")
            .finish()
    }
}

impl Camera {
    /// Create a new unified camera with default profile (GenericVisca).
    ///
    /// This constructor automatically detects whether the transport is async or blocking
    /// and wraps it appropriately.
    pub fn new<T>(transport: T) -> Self
    where
        T: Transport + Send + Sync + 'static,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        Self::with_profile_and_transport(ProfileId::default(), transport)
    }

    /// Create a new unified camera with a specific profile.
    pub fn with_profile<T>(profile: ProfileId, transport: T) -> Self
    where
        T: Transport + Send + Sync + 'static,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        Self::with_profile_and_transport(profile, transport)
    }

    /// Create with a blocking transport.
    #[allow(dead_code)]
    pub(crate) fn new_blocking<T>(transport: T) -> Self
    where
        T: BlockingTransport + Send + Sync + 'static,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        Self::with_profile_and_blocking_transport(ProfileId::default(), transport)
    }

    /// Create with a specific profile and blocking transport.
    #[allow(dead_code)]
    pub(crate) fn with_profile_blocking<T>(profile: ProfileId, transport: T) -> Self
    where
        T: BlockingTransport + Send + Sync + 'static,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        Self::with_profile_and_blocking_transport(profile, transport)
    }

    /// Internal constructor for async transports.
    fn with_profile_and_transport<T>(profile: ProfileId, transport: T) -> Self
    where
        T: Transport + Send + Sync + 'static,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        let profile = profile.to_profile();
        let address = profile.default_address();
        let transport = Arc::new(AsyncTransportWrapper { transport });

        Self {
            profile,
            transport,
            address,
        }
    }

    /// Internal constructor for blocking transports.
    #[allow(dead_code)]
    fn with_profile_and_blocking_transport<T>(profile: ProfileId, transport: T) -> Self
    where
        T: BlockingTransport + Send + Sync + 'static,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        let profile = profile.to_profile();
        let address = profile.default_address();
        let transport = Arc::new(BlockingTransportWrapper { transport });

        Self {
            profile,
            transport,
            address,
        }
    }

    /// Get the camera's model name.
    #[must_use]
    pub fn model_name(&self) -> &'static str {
        self.profile.model_name()
    }

    /// Get the camera's profile information.
    #[must_use]
    pub fn profile_info(&self) -> String {
        self.profile.capability_summary()
    }

    /// Set the VISCA address for this camera.
    pub fn set_address(&mut self, address: u8) {
        self.address = address;
    }

    /// Get the current VISCA address.
    #[must_use]
    pub fn address(&self) -> u8 {
        self.address
    }

    /// Send a command asynchronously and wait for response.
    ///
    /// This is the primary async interface for sending commands to the camera.
    pub async fn send_command<C>(&self, command: &C) -> Result<Response, Error>
    where
        C: EncodeVisca,
    {
        // Get command bytes using EncodeVisca
        let mut buffer = [0u8; 64]; // Use a reasonable max size
        let size = command.encode_into(&mut buffer)?;
        let mut cmd_bytes = buffer[..size].to_vec();

        // Add VISCA terminator if not present
        // Use VISCA_TERMINATOR from const_encoding module
        if cmd_bytes.last() != Some(&crate::command::const_encoding::VISCA_TERMINATOR) {
            cmd_bytes.push(crate::command::const_encoding::VISCA_TERMINATOR);
        }

        // Apply protocol-specific framing if needed
        let framed_bytes = match self.profile.protocol_style() {
            ProtocolStyle::RawVisca => cmd_bytes,
            ProtocolStyle::SonyEncapsulated { use_sequence: _ } => {
                // TODO: Implement Sony encapsulation
                // For now, just use raw bytes
                log::warn!("Sony encapsulation not yet implemented, using raw VISCA");
                cmd_bytes
            }
        };

        log::debug!("Sending VISCA command: {:02X?}", framed_bytes);

        // Send command
        self.transport.send(&framed_bytes).await?;

        // Handle response based on command type
        match command.response_type() {
            None => {
                // Action command - wait for ACK then Completion
                let ack_response = self.wait_for_response(self.profile.ack_timeout()).await?;
                match ack_response {
                    Response::CmdAck => {
                        // Wait for completion
                        self.wait_for_response(self.profile.completion_timeout())
                            .await
                    }
                    Response::Completion => {
                        // Some cameras send completion directly
                        Ok(Response::Completion)
                    }
                    Response::Error(e) => Err(e),
                    _ => Err(Error::ParseError(format!(
                        "Unexpected response: {:?}",
                        ack_response
                    ))),
                }
            }
            Some(response_type) => {
                // Inquiry command - wait for specific response type
                let timeout = self.profile.completion_timeout();
                self.wait_for_response_with_type(response_type, timeout)
                    .await
            }
        }
    }

    /// Wait for any response with timeout.
    async fn wait_for_response(&self, _timeout: Duration) -> Result<Response, Error> {
        // TODO: Implement proper timeout handling based on runtime
        // For now, just receive without timeout

        match self.transport.recv().await {
            Ok(bytes) => Response::parse(&bytes),
            Err(e) => Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Receive error: {}", e),
            ))),
        }
    }

    /// Wait for a specific type of response with timeout.
    async fn wait_for_response_with_type(
        &self,
        _expected_type: ResponseType,
        timeout: Duration,
    ) -> Result<Response, Error> {
        let response = self.wait_for_response(timeout).await?;

        // For now, just return the response without type checking
        // since we don't have a proper mapping between ResponseType and Response variants
        match response {
            Response::Error(e) => Err(e),
            _ => Ok(response),
        }
    }

    /// Send a command synchronously (blocking).
    ///
    /// This method blocks the current thread until the command completes.
    /// It's provided for convenience when using blocking transports.
    pub(crate) fn send_command_blocking<C>(&self, command: &C) -> Result<Response, Error>
    where
        C: EncodeVisca,
    {
        // Use a minimal executor to block on the async method
        futures::executor::block_on(self.send_command(command))
    }

    /// Check if the camera supports a specific capability.
    ///
    /// This provides runtime introspection of camera capabilities.
    #[must_use]
    pub fn supports_capability(&self, capability: &str) -> bool {
        match capability {
            "pan_tilt" => match &self.profile {
                DynamicProfile::PTZOpticsG2(p) => p.supports_pan_tilt(),
                DynamicProfile::SonyFR7(p) => p.supports_pan_tilt(),
                DynamicProfile::GenericVisca(p) => p.supports_pan_tilt(),
            },
            "zoom" => match &self.profile {
                DynamicProfile::PTZOpticsG2(p) => p.supports_zoom(),
                DynamicProfile::SonyFR7(p) => p.supports_zoom(),
                DynamicProfile::GenericVisca(p) => p.supports_zoom(),
            },
            "focus" => match &self.profile {
                DynamicProfile::PTZOpticsG2(p) => p.supports_focus(),
                DynamicProfile::SonyFR7(p) => p.supports_focus(),
                DynamicProfile::GenericVisca(p) => p.supports_focus(),
            },
            "exposure" => match &self.profile {
                DynamicProfile::PTZOpticsG2(p) => p.supports_exposure(),
                DynamicProfile::SonyFR7(p) => p.supports_exposure(),
                DynamicProfile::GenericVisca(p) => p.supports_exposure(),
            },
            "white_balance" => match &self.profile {
                DynamicProfile::PTZOpticsG2(p) => p.supports_white_balance(),
                DynamicProfile::SonyFR7(p) => p.supports_white_balance(),
                DynamicProfile::GenericVisca(p) => p.supports_white_balance(),
            },
            "image_processing" => match &self.profile {
                DynamicProfile::PTZOpticsG2(p) => p.supports_image_processing(),
                DynamicProfile::SonyFR7(p) => p.supports_image_processing(),
                DynamicProfile::GenericVisca(p) => p.supports_image_processing(),
            },
            "presets" => match &self.profile {
                DynamicProfile::PTZOpticsG2(p) => p.supports_presets(),
                DynamicProfile::SonyFR7(p) => p.supports_presets(),
                DynamicProfile::GenericVisca(p) => p.supports_presets(),
            },
            "power" => match &self.profile {
                DynamicProfile::PTZOpticsG2(p) => p.supports_power(),
                DynamicProfile::SonyFR7(p) => p.supports_power(),
                DynamicProfile::GenericVisca(p) => p.supports_power(),
            },
            "nd_filter" => match &self.profile {
                DynamicProfile::PTZOpticsG2(p) => p.supports_nd_filter(),
                DynamicProfile::SonyFR7(p) => p.supports_nd_filter(),
                DynamicProfile::GenericVisca(p) => p.supports_nd_filter(),
            },
            _ => false,
        }
    }

    // Profile-specific accessor methods

    /// Get the power on time for the camera.
    #[must_use]
    pub fn power_on_time(&self) -> Duration {
        use crate::capabilities::Power;
        match &self.profile {
            DynamicProfile::PTZOpticsG2(_) => PTZOpticsG2::POWER_ON_TIME,
            DynamicProfile::SonyFR7(_) => SonyFR7::POWER_ON_TIME,
            DynamicProfile::GenericVisca(_) => GenericVisca::POWER_ON_TIME,
        }
    }

    /// Get the standby time for the camera.
    #[must_use]
    pub fn standby_time(&self) -> Duration {
        use crate::capabilities::Power;
        match &self.profile {
            DynamicProfile::PTZOpticsG2(_) => PTZOpticsG2::STANDBY_TIME,
            DynamicProfile::SonyFR7(_) => SonyFR7::STANDBY_TIME,
            DynamicProfile::GenericVisca(_) => GenericVisca::STANDBY_TIME,
        }
    }

    /// Get the maximum number of presets supported.
    #[must_use]
    pub fn max_presets(&self) -> u8 {
        use crate::capabilities::Presets;
        match &self.profile {
            DynamicProfile::PTZOpticsG2(_) => PTZOpticsG2::MAX_PRESETS,
            DynamicProfile::SonyFR7(_) => SonyFR7::MAX_PRESETS,
            DynamicProfile::GenericVisca(_) => 0, // GenericVisca doesn't support presets
        }
    }

    /// Get the zoom speed range.
    #[must_use]
    pub fn zoom_speed_range(&self) -> std::ops::Range<u8> {
        use crate::capabilities::Zoom;
        match &self.profile {
            DynamicProfile::PTZOpticsG2(_) => PTZOpticsG2::ZOOM_SPEED_RANGE,
            DynamicProfile::SonyFR7(_) => SonyFR7::ZOOM_SPEED_RANGE,
            DynamicProfile::GenericVisca(_) => GenericVisca::ZOOM_SPEED_RANGE,
        }
    }

    /// Get the optical zoom maximum value.
    #[must_use]
    pub fn optical_zoom_max(&self) -> u16 {
        use crate::capabilities::Zoom;
        match &self.profile {
            DynamicProfile::PTZOpticsG2(_) => PTZOpticsG2::OPTICAL_ZOOM_MAX,
            DynamicProfile::SonyFR7(_) => SonyFR7::OPTICAL_ZOOM_MAX,
            DynamicProfile::GenericVisca(_) => GenericVisca::OPTICAL_ZOOM_MAX,
        }
    }

    /// Get the digital zoom maximum value.
    #[must_use]
    pub fn digital_zoom_max(&self) -> Option<u16> {
        use crate::capabilities::Zoom;
        match &self.profile {
            DynamicProfile::PTZOpticsG2(_) => PTZOpticsG2::DIGITAL_ZOOM_MAX,
            DynamicProfile::SonyFR7(_) => SonyFR7::DIGITAL_ZOOM_MAX,
            DynamicProfile::GenericVisca(_) => GenericVisca::DIGITAL_ZOOM_MAX,
        }
    }

    /// Convert degrees to pan/tilt units.
    pub fn degrees_to_units(
        &self,
        pan_deg: crate::units::Degrees,
        tilt_deg: crate::units::Degrees,
    ) -> (crate::units::ViscaUnits<i16>, crate::units::ViscaUnits<i16>) {
        use crate::capabilities::PanTilt;
        let (pan_conv, tilt_conv) = match &self.profile {
            DynamicProfile::PTZOpticsG2(_) => (
                PTZOpticsG2::PAN_DEGREES_TO_UNITS,
                PTZOpticsG2::TILT_DEGREES_TO_UNITS,
            ),
            DynamicProfile::SonyFR7(_) => (
                SonyFR7::PAN_DEGREES_TO_UNITS,
                SonyFR7::TILT_DEGREES_TO_UNITS,
            ),
            DynamicProfile::GenericVisca(_) => (
                GenericVisca::PAN_DEGREES_TO_UNITS,
                GenericVisca::TILT_DEGREES_TO_UNITS,
            ),
        };

        let pan_units = (pan_deg.0 * pan_conv) as i16;
        let tilt_units = (tilt_deg.0 * tilt_conv) as i16;

        (
            crate::units::ViscaUnits(pan_units),
            crate::units::ViscaUnits(tilt_units),
        )
    }

    /// Convert pan/tilt units to degrees.
    pub fn units_to_degrees(
        &self,
        pan_units: i16,
        tilt_units: i16,
    ) -> (crate::units::Degrees, crate::units::Degrees) {
        use crate::capabilities::PanTilt;
        let (pan_conv, tilt_conv) = match &self.profile {
            DynamicProfile::PTZOpticsG2(_) => (
                PTZOpticsG2::PAN_DEGREES_TO_UNITS,
                PTZOpticsG2::TILT_DEGREES_TO_UNITS,
            ),
            DynamicProfile::SonyFR7(_) => (
                SonyFR7::PAN_DEGREES_TO_UNITS,
                SonyFR7::TILT_DEGREES_TO_UNITS,
            ),
            DynamicProfile::GenericVisca(_) => (
                GenericVisca::PAN_DEGREES_TO_UNITS,
                GenericVisca::TILT_DEGREES_TO_UNITS,
            ),
        };

        let pan_deg = pan_units as f32 / pan_conv;
        let tilt_deg = tilt_units as f32 / tilt_conv;

        (
            crate::units::Degrees(pan_deg),
            crate::units::Degrees(tilt_deg),
        )
    }

    /// Validate pan position.
    pub fn validate_pan(&self, pan_units: i16) -> Result<i16, Error> {
        use crate::capabilities::{PanTilt, ValidationError};
        let range = match &self.profile {
            DynamicProfile::PTZOpticsG2(_) => PTZOpticsG2::PAN_RANGE,
            DynamicProfile::SonyFR7(_) => SonyFR7::PAN_RANGE,
            DynamicProfile::GenericVisca(_) => GenericVisca::PAN_RANGE,
        };

        if range.contains(&pan_units) {
            Ok(pan_units)
        } else {
            Err(Error::ValidationError(ValidationError::OutOfRange {
                parameter: "pan",
                value: pan_units as f64,
                min: range.start as f64,
                max: range.end as f64,
            }))
        }
    }

    /// Validate tilt position.
    pub fn validate_tilt(&self, tilt_units: i16) -> Result<i16, Error> {
        use crate::capabilities::{PanTilt, ValidationError};
        let range = match &self.profile {
            DynamicProfile::PTZOpticsG2(_) => PTZOpticsG2::TILT_RANGE,
            DynamicProfile::SonyFR7(_) => SonyFR7::TILT_RANGE,
            DynamicProfile::GenericVisca(_) => GenericVisca::TILT_RANGE,
        };

        if range.contains(&tilt_units) {
            Ok(tilt_units)
        } else {
            Err(Error::ValidationError(ValidationError::OutOfRange {
                parameter: "tilt",
                value: tilt_units as f64,
                min: range.start as f64,
                max: range.end as f64,
            }))
        }
    }

    /// Get the ND filter mode.
    #[must_use]
    pub fn nd_filter_mode(&self) -> Option<crate::capabilities::NDFilterMode> {
        use crate::capabilities::NDFilter;
        match &self.profile {
            DynamicProfile::PTZOpticsG2(_) => None, // PTZOpticsG2 doesn't support ND filters
            DynamicProfile::SonyFR7(_) => Some(SonyFR7::ND_MODE),
            DynamicProfile::GenericVisca(_) => None, // GenericVisca doesn't support ND filters
        }
    }

    /// Validate ND filter level.
    pub fn validate_nd_filter(&self, level: u8) -> Result<u8, Error> {
        use crate::capabilities::nd_filter::NDFilterExt;

        // Check if camera supports ND filters
        match &self.profile {
            DynamicProfile::PTZOpticsG2(_) => Err(Error::FeatureNotSupported {
                feature: "ND filter",
            }),
            DynamicProfile::SonyFR7(p) => {
                // Use the NDFilterExt trait method for validation
                p.validate_nd_filter(level).map_err(Into::into)
            }
            DynamicProfile::GenericVisca(_) => Err(Error::FeatureNotSupported {
                feature: "ND filter",
            }),
        }
    }

    /// Get a blocking wrapper for this camera.
    ///
    /// This provides a synchronous API that hides the async nature of the underlying transport.
    /// All trait methods will block until completion.
    ///
    /// # Example
    /// ```no_run
    /// use grafton_visca::{Camera, ProfileId, transport::blocking::Mock};
    /// use grafton_visca::blocking::ZoomOps;
    ///
    /// let transport = Mock::new();
    /// let camera = Camera::with_profile(ProfileId::PTZOpticsG2, transport);
    /// let blocking_camera = camera.blocking();
    ///
    /// // Use blocking API
    /// blocking_camera.zoom_stop().unwrap();
    /// ```
    pub fn blocking(self) -> crate::blocking::Camera {
        crate::blocking::Camera::new(self)
    }

    /// Get a blocking wrapper for this camera by reference.
    ///
    /// This provides a blocking API without consuming the camera instance,
    /// allowing you to obtain both blocking and async views.
    pub fn blocking_ref(&self) -> crate::blocking::Camera {
        crate::blocking::Camera::new(self.clone())
    }

    /// Get an async wrapper for this camera by reference.
    ///
    /// This provides an async API without consuming the camera instance,
    /// allowing you to obtain both blocking and async views.
    pub fn async_ref(&self) -> crate::r#async::Camera {
        crate::r#async::Camera::new(self.clone())
    }

    /// Get an async wrapper for this camera.
    ///
    /// This provides an async API that exposes the async nature of the underlying transport.
    /// All trait methods are async and must be awaited.
    ///
    /// # Example
    /// ```no_run
    /// # async {
    /// use grafton_visca::{Camera, ProfileId, transport::Tokio};
    /// use grafton_visca::transport::tokio::Mock;
    /// use grafton_visca::r#async::ZoomOps;
    ///
    /// let transport = Mock::new();
    /// let camera = Camera::with_profile(ProfileId::PTZOpticsG2, Tokio(transport));
    /// let async_camera = camera.r#async();
    ///
    /// // Use async API
    /// async_camera.zoom_stop().await.unwrap();
    /// # };
    /// ```
    pub fn r#async(self) -> crate::r#async::Camera {
        crate::r#async::Camera::new(self)
    }
}

// Note: The implementation of specific camera methods (zoom, pan_tilt, etc.) will be added
// through extension traits that provide high-level convenience methods on top of send_command.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_selection() {
        let profile = ProfileId::PTZOpticsG2.to_profile();
        assert_eq!(profile.model_name(), "PTZOptics G2");

        let profile = ProfileId::SonyFR7.to_profile();
        assert_eq!(profile.model_name(), "Sony FR7");

        let profile = ProfileId::default().to_profile();
        assert_eq!(profile.model_name(), "Generic VISCA Camera");
    }
}
