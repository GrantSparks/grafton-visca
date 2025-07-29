//! New camera API with compile-time safety.
//!
//! This module implements the new Camera API where methods only exist
//! for cameras that support the corresponding capabilities.

//! Unified camera client that abstracts over profile and transport generics.
//!
//! This module provides a single, simplified camera interface that works with
//! any camera profile and transport, eliminating the need for users to manage
//! generic type parameters.

use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

use crate::{
    camera_id::CameraId,
    capabilities::{
        CameraFeature, CommandFeatures, FeatureDetection, ProfileIntrospection, ProfileMetadata,
        ProtocolStyle,
    },
    command::{encode_visca::EncodeVisca, Response, ResponseType},
    error::Error,
    transport::{core::Transport, TransportEnvelope},
};

#[cfg(feature = "async")]
use crate::socket_manager::SocketManagerHandle;

#[cfg(feature = "async")]
use crate::executor::Spawner;

use crate::camera::profiles::{
    GenericVisca, NearusBRC300, PTZOptics30X, PTZOpticsG2, PTZOpticsG3, SonyBRC300, SonyBRCH900,
    SonyEVIH100, SonyFR7,
};

// Ergonomic camera module - commented out for demo
// pub mod ergonomic_camera;

/// Camera profile wrapper that erases the concrete profile type.
///
/// This enum allows us to store different camera profiles without exposing
/// generic parameters to the user.
#[derive(Debug, Clone, Copy)]
pub enum CameraProfile {
    /// PTZOptics G2 camera profile
    PTZOpticsG2(PTZOpticsG2),
    /// PTZOptics G3 camera profile
    PTZOpticsG3(PTZOpticsG3),
    /// PTZOptics 30X camera profile
    PTZOptics30X(PTZOptics30X),
    /// Sony FR7 camera profile  
    SonyFR7(SonyFR7),
    /// Sony BRC-H900 camera profile
    SonyBRCH900(SonyBRCH900),
    /// Sony EVI-H100 camera profile
    SonyEVIH100(SonyEVIH100),
    /// Sony BRC-300 camera profile
    SonyBRC300(SonyBRC300),
    /// Nearus BRC-300 camera profile
    NearusBRC300(NearusBRC300),
    /// Generic VISCA camera profile (default)
    GenericVisca(GenericVisca),
}

impl CameraProfile {
    /// Get the model name of the camera.
    #[must_use]
    pub fn model_name(&self) -> &'static str {
        match self {
            Self::PTZOpticsG2(_) => PTZOpticsG2::MODEL_NAME,
            Self::PTZOpticsG3(_) => PTZOpticsG3::MODEL_NAME,
            Self::PTZOptics30X(_) => PTZOptics30X::MODEL_NAME,
            Self::SonyFR7(_) => SonyFR7::MODEL_NAME,
            Self::SonyBRCH900(_) => SonyBRCH900::MODEL_NAME,
            Self::SonyEVIH100(_) => SonyEVIH100::MODEL_NAME,
            Self::SonyBRC300(_) => SonyBRC300::MODEL_NAME,
            Self::NearusBRC300(_) => NearusBRC300::MODEL_NAME,
            Self::GenericVisca(_) => GenericVisca::MODEL_NAME,
        }
    }

    /// Get the default VISCA address.
    #[must_use]
    pub fn default_address(&self) -> u8 {
        match self {
            Self::PTZOpticsG2(_) => PTZOpticsG2::DEFAULT_ADDRESS,
            Self::PTZOpticsG3(_) => PTZOpticsG3::DEFAULT_ADDRESS,
            Self::PTZOptics30X(_) => PTZOptics30X::DEFAULT_ADDRESS,
            Self::SonyFR7(_) => SonyFR7::DEFAULT_ADDRESS,
            Self::SonyBRCH900(_) => SonyBRCH900::DEFAULT_ADDRESS,
            Self::SonyEVIH100(_) => SonyEVIH100::DEFAULT_ADDRESS,
            Self::SonyBRC300(_) => SonyBRC300::DEFAULT_ADDRESS,
            Self::NearusBRC300(_) => NearusBRC300::DEFAULT_ADDRESS,
            Self::GenericVisca(_) => GenericVisca::DEFAULT_ADDRESS,
        }
    }

    /// Get the protocol style.
    #[must_use]
    pub fn protocol_style(&self) -> ProtocolStyle {
        match self {
            Self::PTZOpticsG2(_) => PTZOpticsG2::PROTOCOL_STYLE,
            Self::PTZOpticsG3(_) => PTZOpticsG3::PROTOCOL_STYLE,
            Self::PTZOptics30X(_) => PTZOptics30X::PROTOCOL_STYLE,
            Self::SonyFR7(_) => SonyFR7::PROTOCOL_STYLE,
            Self::SonyBRCH900(_) => SonyBRCH900::PROTOCOL_STYLE,
            Self::SonyEVIH100(_) => SonyEVIH100::PROTOCOL_STYLE,
            Self::SonyBRC300(_) => SonyBRC300::PROTOCOL_STYLE,
            Self::NearusBRC300(_) => NearusBRC300::PROTOCOL_STYLE,
            Self::GenericVisca(_) => GenericVisca::PROTOCOL_STYLE,
        }
    }

    /// Get the acknowledgment timeout.
    #[must_use]
    pub fn ack_timeout(&self) -> Duration {
        match self {
            Self::PTZOpticsG2(_) => PTZOpticsG2::ACK_TIMEOUT,
            Self::PTZOpticsG3(_) => PTZOpticsG3::ACK_TIMEOUT,
            Self::PTZOptics30X(_) => PTZOptics30X::ACK_TIMEOUT,
            Self::SonyFR7(_) => SonyFR7::ACK_TIMEOUT,
            Self::SonyBRCH900(_) => SonyBRCH900::ACK_TIMEOUT,
            Self::SonyEVIH100(_) => SonyEVIH100::ACK_TIMEOUT,
            Self::SonyBRC300(_) => SonyBRC300::ACK_TIMEOUT,
            Self::NearusBRC300(_) => NearusBRC300::ACK_TIMEOUT,
            Self::GenericVisca(_) => GenericVisca::ACK_TIMEOUT,
        }
    }

    /// Get the completion timeout.
    #[must_use]
    pub fn completion_timeout(&self) -> Duration {
        match self {
            Self::PTZOpticsG2(_) => PTZOpticsG2::COMPLETION_TIMEOUT,
            Self::PTZOpticsG3(_) => PTZOpticsG3::COMPLETION_TIMEOUT,
            Self::PTZOptics30X(_) => PTZOptics30X::COMPLETION_TIMEOUT,
            Self::SonyFR7(_) => SonyFR7::COMPLETION_TIMEOUT,
            Self::SonyBRCH900(_) => SonyBRCH900::COMPLETION_TIMEOUT,
            Self::SonyEVIH100(_) => SonyEVIH100::COMPLETION_TIMEOUT,
            Self::SonyBRC300(_) => SonyBRC300::COMPLETION_TIMEOUT,
            Self::NearusBRC300(_) => NearusBRC300::COMPLETION_TIMEOUT,
            Self::GenericVisca(_) => GenericVisca::COMPLETION_TIMEOUT,
        }
    }

    /// Get the busy timeout.
    #[must_use]
    pub fn busy_timeout(&self) -> Duration {
        match self {
            Self::PTZOpticsG2(_) => PTZOpticsG2::BUSY_TIMEOUT,
            Self::PTZOpticsG3(_) => PTZOpticsG3::BUSY_TIMEOUT,
            Self::PTZOptics30X(_) => PTZOptics30X::BUSY_TIMEOUT,
            Self::SonyFR7(_) => SonyFR7::BUSY_TIMEOUT,
            Self::SonyBRCH900(_) => SonyBRCH900::BUSY_TIMEOUT,
            Self::SonyEVIH100(_) => SonyEVIH100::BUSY_TIMEOUT,
            Self::SonyBRC300(_) => SonyBRC300::BUSY_TIMEOUT,
            Self::NearusBRC300(_) => NearusBRC300::BUSY_TIMEOUT,
            Self::GenericVisca(_) => GenericVisca::BUSY_TIMEOUT,
        }
    }

    /// Check if the camera supports VISCA inquiry commands.
    #[must_use]
    pub fn supports_inquiry(&self) -> bool {
        match self {
            Self::PTZOpticsG2(_) => PTZOpticsG2::SUPPORTS_INQUIRY,
            Self::PTZOpticsG3(_) => PTZOpticsG3::SUPPORTS_INQUIRY,
            Self::PTZOptics30X(_) => PTZOptics30X::SUPPORTS_INQUIRY,
            Self::SonyFR7(_) => SonyFR7::SUPPORTS_INQUIRY,
            Self::SonyBRCH900(_) => SonyBRCH900::SUPPORTS_INQUIRY,
            Self::SonyEVIH100(_) => SonyEVIH100::SUPPORTS_INQUIRY,
            Self::SonyBRC300(_) => SonyBRC300::SUPPORTS_INQUIRY,
            Self::NearusBRC300(_) => NearusBRC300::SUPPORTS_INQUIRY,
            Self::GenericVisca(_) => GenericVisca::SUPPORTS_INQUIRY,
        }
    }

    /// Get a capability summary for the camera.
    #[must_use]
    pub fn capability_summary(&self) -> String {
        match self {
            Self::PTZOpticsG2(p) => p.capability_summary(),
            Self::PTZOpticsG3(p) => p.capability_summary(),
            Self::PTZOptics30X(p) => p.capability_summary(),
            Self::SonyFR7(p) => p.capability_summary(),
            Self::SonyBRCH900(p) => p.capability_summary(),
            Self::SonyEVIH100(p) => p.capability_summary(),
            Self::SonyBRC300(p) => p.capability_summary(),
            Self::NearusBRC300(p) => p.capability_summary(),
            Self::GenericVisca(p) => p.capability_summary(),
        }
    }
}

impl Default for CameraProfile {
    fn default() -> Self {
        Self::GenericVisca(GenericVisca)
    }
}

/// Type alias for dynamic camera profile.
pub type DynamicProfile = CameraProfile;

/// Camera model identifier for selecting a specific camera model and its associated profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraModel {
    /// PTZOptics G2 camera
    PTZOpticsG2,
    /// PTZOptics G3 camera
    PTZOpticsG3,
    /// PTZOptics 30X camera
    PTZOptics30X,
    /// Sony FR7 camera
    SonyFR7,
    /// Sony BRC-H900 camera
    SonyBRCH900,
    /// Sony EVI-H100 camera
    SonyEVIH100,
    /// Sony BRC-300 camera
    SonyBRC300,
    /// Nearus BRC-300 camera
    NearusBRC300,
    /// Generic VISCA camera (default)
    GenericVisca,
}

impl CameraModel {
    /// Convert to a camera profile instance.
    #[must_use]
    pub fn to_profile(self) -> CameraProfile {
        match self {
            Self::PTZOpticsG2 => CameraProfile::PTZOpticsG2(PTZOpticsG2),
            Self::PTZOpticsG3 => CameraProfile::PTZOpticsG3(PTZOpticsG3),
            Self::PTZOptics30X => CameraProfile::PTZOptics30X(PTZOptics30X),
            Self::SonyFR7 => CameraProfile::SonyFR7(SonyFR7),
            Self::SonyBRCH900 => CameraProfile::SonyBRCH900(SonyBRCH900),
            Self::SonyEVIH100 => CameraProfile::SonyEVIH100(SonyEVIH100),
            Self::SonyBRC300 => CameraProfile::SonyBRC300(SonyBRC300),
            Self::NearusBRC300 => CameraProfile::NearusBRC300(NearusBRC300),
            Self::GenericVisca => CameraProfile::GenericVisca(GenericVisca),
        }
    }
}

impl Default for CameraModel {
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

    /// Send raw bytes to the device (blocking).
    fn send_blocking(&self, bytes: &[u8]) -> Result<(), Error>;

    /// Receive raw bytes from the device with timeout (blocking).
    fn recv_blocking_timeout(&self, timeout: Duration) -> Result<bytes::Bytes, Error>;
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

    fn send_blocking(&self, bytes: &[u8]) -> Result<(), Error> {
        // For async transports, use futures::executor to block
        futures::executor::block_on(self.send(bytes))
    }

    fn recv_blocking_timeout(&self, timeout: Duration) -> Result<bytes::Bytes, Error> {
        // For async transports, use futures::executor with timeout
        #[cfg(feature = "tokio")]
        {
            futures::executor::block_on(async {
                tokio::time::timeout(timeout, self.recv())
                    .await
                    .map_err(|_| Error::Timeout)?
            })
        }

        #[cfg(not(feature = "tokio"))]
        {
            // Without tokio, use a simpler timeout approach
            use std::time::Instant;
            let _start = Instant::now();

            // For non-tokio async transports, we just block on recv without timeout
            // This is a limitation when not using tokio
            log::warn!(
                "Timeout ({:?}) not supported without tokio feature - blocking on recv",
                timeout
            );
            futures::executor::block_on(self.recv())
        }
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
/// use grafton_visca::{Camera, CameraModel};
/// use grafton_visca::prelude::*;
/// #[cfg(feature = "tokio")]
/// use grafton_visca::transport::tokio::Tcp;
///
/// # #[cfg(feature = "tokio")]
/// # {
/// // Create a camera with automatic profile detection
/// let transport = Tcp::connect("192.168.1.100:52381").await?;
/// let camera = Camera::new(transport);
///
/// // Or specify a profile explicitly
/// let transport2 = Tcp::connect("192.168.1.100:52381").await?;
/// let camera = Camera::with_profile(CameraModel::PTZOpticsG2, transport2);
///
/// // Use the camera
/// let power_on_cmd = PowerCommand::On;
/// camera.send_command(&power_on_cmd).await?;
/// # }
/// # Ok(())
/// # }
/// ```
///
/// ## Blocking usage
/// ```no_run
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// use grafton_visca::{Camera, CameraModel};
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
    profile: CameraProfile,
    transport: Arc<dyn UnifiedTransport>,
    camera_id: CameraId,
    #[cfg(feature = "async")]
    socket_manager: Option<SocketManagerHandle>,
    envelope: TransportEnvelope,
    #[cfg(feature = "async")]
    spawner: Option<Arc<dyn Spawner>>,
}

impl Clone for Camera {
    fn clone(&self) -> Self {
        Self {
            profile: self.profile,
            transport: Arc::clone(&self.transport),
            camera_id: self.camera_id,
            #[cfg(feature = "async")]
            socket_manager: self.socket_manager.clone(),
            envelope: TransportEnvelope::new(self.profile.protocol_style()),
            #[cfg(feature = "async")]
            spawner: self.spawner.clone(),
        }
    }
}

impl std::fmt::Debug for Camera {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("Camera");
        debug
            .field("profile", &self.profile)
            .field("camera_id", &self.camera_id)
            .field("transport", &"<dyn UnifiedTransport>");
        #[cfg(feature = "async")]
        debug.field("socket_manager", &self.socket_manager.is_some());
        debug.finish()
    }
}

impl Camera {
    /// Create a new unified camera with default profile (GenericVisca).
    ///
    /// This constructor is suitable for both async and blocking usage.
    /// If you need async operations with background task management,
    /// use `new_with_spawner` instead.
    pub fn new<T>(transport: T) -> Self
    where
        T: Transport + Send + Sync + 'static,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        Self::with_profile_and_transport(CameraModel::default(), transport)
    }

    /// Create a new unified camera with a spawner for async operations.
    ///
    /// Use this constructor when you need async operations with background
    /// task management (e.g., socket manager for concurrent commands).
    #[cfg(feature = "async")]
    pub fn new_with_spawner<T, S>(transport: T, spawner: S) -> Self
    where
        T: Transport + Send + Sync + 'static,
        S: Spawner,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        Self::with_profile_transport_and_spawner(CameraModel::default(), transport, spawner)
    }

    /// Create a new unified camera with a specific profile.
    ///
    /// This constructor is suitable for both async and blocking usage.
    /// If you need async operations with background task management,
    /// use `with_profile_and_spawner` instead.
    pub fn with_profile<T>(profile: CameraModel, transport: T) -> Self
    where
        T: Transport + Send + Sync + 'static,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        Self::with_profile_and_transport(profile, transport)
    }

    /// Create a new unified camera with a specific profile and spawner.
    ///
    /// Use this constructor when you need async operations with background
    /// task management (e.g., socket manager for concurrent commands).
    #[cfg(feature = "async")]
    pub fn with_profile_and_spawner<T, S>(profile: CameraModel, transport: T, spawner: S) -> Self
    where
        T: Transport + Send + Sync + 'static,
        S: Spawner,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        Self::with_profile_transport_and_spawner(profile, transport, spawner)
    }

    /// Internal constructor for all transports without spawner.
    fn with_profile_and_transport<T>(profile: CameraModel, transport: T) -> Self
    where
        T: Transport + Send + Sync + 'static,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        let profile = profile.to_profile();
        let camera_id = CameraId::new(profile.default_address()).unwrap_or(CameraId::CAMERA_1);
        let transport = Arc::new(AsyncTransportWrapper { transport });

        Self {
            envelope: TransportEnvelope::new(profile.protocol_style()),
            profile,
            transport,
            camera_id,
            #[cfg(feature = "async")]
            socket_manager: None,
            #[cfg(feature = "async")]
            spawner: None,
        }
    }

    /// Internal constructor for async transports with spawner.
    #[cfg(feature = "async")]
    fn with_profile_transport_and_spawner<T, S>(
        profile: CameraModel,
        transport: T,
        spawner: S,
    ) -> Self
    where
        T: Transport + Send + Sync + 'static,
        S: Spawner,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        let profile = profile.to_profile();
        let camera_id = CameraId::new(profile.default_address()).unwrap_or(CameraId::CAMERA_1);
        let transport = Arc::new(AsyncTransportWrapper { transport });
        let spawner: Option<Arc<dyn Spawner>> = Some(Arc::new(spawner));

        let mut camera = Self {
            envelope: TransportEnvelope::new(profile.protocol_style()),
            profile,
            transport,
            camera_id,
            #[cfg(feature = "async")]
            socket_manager: None,
            spawner,
        };

        // Initialize socket manager automatically for better reliability
        if let Err(e) = camera.initialize_socket_manager() {
            log::warn!("Failed to initialize socket manager: {}", e);
        }

        camera
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

    /// Set the camera ID for this camera.
    ///
    /// # Errors
    /// Returns an error if the ID is not in the valid range (1-8).
    pub fn set_camera_id(&mut self, id: u8) -> Result<(), Error> {
        self.camera_id = CameraId::new(id)?;
        Ok(())
    }

    /// Get the current camera ID.
    #[must_use]
    pub fn camera_id(&self) -> CameraId {
        self.camera_id
    }

    /// Initialize the socket manager for this camera.
    /// This enables queued command processing for proper two-socket management.
    pub fn initialize_socket_manager(&mut self) -> Result<(), Error> {
        #[cfg(feature = "async")]
        if self.socket_manager.is_some() {
            return Ok(()); // Already initialized
        }

        // Socket manager is only available in async builds
        #[cfg(feature = "async")]
        {
            // Create socket manager components
            #[cfg(feature = "tokio")]
            let (command_sender, command_receiver) = tokio::sync::mpsc::unbounded_channel();

            #[cfg(not(feature = "tokio"))]
            let (command_sender, command_receiver) = std::sync::mpsc::channel();

            // Store the handle
            let handle = SocketManagerHandle::new(command_sender);
            self.socket_manager = Some(handle);

            // Start the socket manager actor
            let transport = Arc::clone(&self.transport);
            let profile = self.profile;
            let actor = crate::socket_manager::SocketManagerActor::new(
                transport,
                command_receiver,
                profile,
                self.camera_id,
            );

            if let Some(spawner) = &self.spawner {
                // Use the provided spawner
                let future = Box::pin(async move {
                    if let Err(e) = actor.run().await {
                        log::error!("Socket manager actor failed: {}", e);
                    }
                });
                spawner.spawn(future);
            } else {
                // No spawner provided - this is expected when using standard constructors
                log::error!("Cannot initialize socket manager without a spawner");
                return Err(Error::InvalidState(
                    "Socket manager requires a spawner. Use Camera::new_with_spawner() or Camera::with_profile_and_spawner() to provide one.".to_string(),
                ));
            }
        }

        // For blocking-only builds, we cannot use the socket manager
        #[cfg(not(feature = "async"))]
        {
            log::warn!("Socket manager not available in blocking-only builds");
            // Don't return an error, just don't initialize the socket manager
        }

        Ok(())
    }

    /// Cancel a command on a specific socket.
    ///
    /// This sends a VISCA command cancel request to the camera for the specified socket.
    #[cfg(feature = "async")]
    pub async fn cancel_command(
        &self,
        socket: crate::command::system::Socket,
    ) -> Result<(), Error> {
        if let Some(socket_manager) = &self.socket_manager {
            socket_manager.cancel_command(socket).await
        } else {
            Err(Error::InvalidState(
                "Socket manager not initialized".to_string(),
            ))
        }
    }

    /// Get the current VISCA address.
    #[must_use]
    pub fn address(&self) -> u8 {
        self.camera_id.id()
    }

    /// Send a command asynchronously and wait for response.
    ///
    /// This is the primary async interface for sending commands to the camera.
    #[cfg(feature = "async")]
    pub async fn send_command<C>(&self, command: &C) -> Result<Response, Error>
    where
        C: EncodeVisca,
    {
        // Check if socket manager is available
        if let Some(socket_manager) = &self.socket_manager {
            return self
                .send_command_via_socket_manager(command, socket_manager)
                .await;
        }

        // Fall back to direct transport (legacy behavior)
        self.send_command_direct(command).await
    }

    /// Send command via socket manager (new queued approach)
    #[cfg(feature = "async")]
    async fn send_command_via_socket_manager<C>(
        &self,
        command: &C,
        socket_manager: &SocketManagerHandle,
    ) -> Result<Response, Error>
    where
        C: EncodeVisca,
    {
        // Get command bytes using EncodeVisca
        let mut buffer = [0u8; 64]; // Use a reasonable max size
        let size = command.encode_into(self.camera_id, &mut buffer)?;
        let mut cmd_bytes = buffer[..size].to_vec();

        // Add VISCA terminator if not present
        if cmd_bytes.last() != Some(&crate::command::const_encoding::VISCA_TERMINATOR) {
            cmd_bytes.push(crate::command::const_encoding::VISCA_TERMINATOR);
        }

        // Apply protocol-specific framing using transport envelope
        let is_inquiry = command.response_type().is_some();
        let framed_bytes = self.envelope.frame_command(&cmd_bytes, is_inquiry);

        log::debug!("Sending VISCA command via socket manager: {framed_bytes:02X?}");

        // Get timeout category from command
        let category = command.timeout_kind();

        // Send via socket manager
        socket_manager
            .send_command(framed_bytes, category, is_inquiry)
            .await
    }

    /// Validate that a command is supported by this camera.
    ///
    /// This method checks if the camera has the necessary capabilities to execute
    /// the given command. It returns an error if the command is not supported.
    ///
    /// # Example
    /// ```ignore
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # use grafton_visca::{Camera, CameraModel};
    /// # use grafton_visca::transport::tokio::Tcp;
    /// let transport = Tcp::connect("192.168.1.100:52381").await?;
    /// let camera = Camera::with_profile(CameraModel::SonyFR7, transport);
    ///
    /// // Commands internally validate their features before execution
    /// // If a command is not supported, you'll get an error when trying to execute it
    /// # Ok(())
    /// # }
    /// ```
    pub fn validate_command<C>(&self, command: &C) -> Result<(), Error>
    where
        C: EncodeVisca + CommandFeatures,
    {
        // Check if all required features are supported by the camera
        let required_features = command.required_features();
        for feature in required_features {
            if !self.supports_feature(*feature) {
                return Err(Error::FeatureNotSupported {
                    feature: feature.name(),
                });
            }
        }
        Ok(())
    }

    /// Send a command with validation.
    ///
    /// This method first validates that the command is supported by the camera,
    /// then sends it. Use this for safer command execution that prevents sending
    /// unsupported commands to cameras.
    #[cfg(feature = "async")]
    pub async fn send_command_validated<C>(&self, command: &C) -> Result<Response, Error>
    where
        C: EncodeVisca + CommandFeatures,
    {
        self.validate_command(command)?;
        self.send_command(command).await
    }

    /// Send command directly via transport (legacy approach)
    #[cfg(feature = "async")]
    async fn send_command_direct<C>(&self, command: &C) -> Result<Response, Error>
    where
        C: EncodeVisca,
    {
        // Get command bytes using EncodeVisca
        let mut buffer = [0u8; 64]; // Use a reasonable max size
        let size = command.encode_into(self.camera_id, &mut buffer)?;
        let mut cmd_bytes = buffer[..size].to_vec();

        // Add VISCA terminator if not present
        // Use VISCA_TERMINATOR from const_encoding module
        if cmd_bytes.last() != Some(&crate::command::const_encoding::VISCA_TERMINATOR) {
            cmd_bytes.push(crate::command::const_encoding::VISCA_TERMINATOR);
        }

        // Apply protocol-specific framing using transport envelope
        let is_inquiry = command.response_type().is_some();
        let framed_bytes = self.envelope.frame_command(&cmd_bytes, is_inquiry);

        log::debug!("Sending VISCA command: {framed_bytes:02X?}");

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
                        "Unexpected response: {ack_response:?}"
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
    #[cfg(feature = "async")]
    async fn wait_for_response(&self, _timeout: Duration) -> Result<Response, Error> {
        // TODO: Implement proper timeout handling based on runtime
        // For now, just receive without timeout

        match self.transport.recv().await {
            Ok(bytes) => {
                // Extract VISCA payload from envelope if needed
                let visca_bytes = self.envelope.extract_response(&bytes)?;
                Response::parse(&visca_bytes)
            }
            Err(e) => {
                // Preserve the original error type
                if e.to_string().contains("Operation timed out") {
                    Err(Error::Timeout)
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Wait for a specific type of response with timeout.
    #[cfg(feature = "async")]
    async fn wait_for_response_with_type(
        &self,
        expected_type: ResponseType,
        #[allow(unused_variables)] timeout: Duration,
    ) -> Result<Response, Error> {
        // TODO: Implement timeout using runtime-specific timeout mechanisms
        // Currently, timeout is not implemented as it requires runtime-specific code
        // For inquiry commands, we may receive an ACK first, then the inquiry response
        loop {
            match self.transport.recv().await {
                Ok(bytes) => {
                    // Extract VISCA payload from envelope if needed
                    let visca_bytes = match self.envelope.extract_response(&bytes) {
                        Ok(payload) => payload,
                        Err(e) => return Err(e),
                    };

                    // First try to parse as a regular response
                    match Response::parse(&visca_bytes) {
                        Ok(Response::CmdAck) => {
                            // Skip ACK for inquiry commands and wait for the actual response
                            log::debug!("Skipping ACK response for inquiry command");
                            continue;
                        }
                        Ok(Response::Error(e)) => return Err(e),
                        Ok(Response::Completion) => {
                            // Unexpected completion for inquiry
                            return Err(Error::UnexpectedResponseType);
                        }
                        Ok(other) => {
                            // This shouldn't happen with parse() but handle it
                            return Ok(other);
                        }
                        Err(_) => {
                            // If regular parse fails, it might be an inquiry response
                            // Try parsing with the expected type
                            match Response::parse_with_type(&visca_bytes, &expected_type) {
                                Ok(response) => return Ok(response),
                                Err(e) => return Err(e),
                            }
                        }
                    }
                }
                Err(e) => {
                    // Preserve the original error type
                    if e.to_string().contains("Operation timed out") {
                        return Err(Error::Timeout);
                    } else {
                        return Err(e);
                    }
                }
            }
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
        #[cfg(feature = "async")]
        {
            // Use a minimal executor to block on the async method
            futures::executor::block_on(self.send_command(command))
        }

        #[cfg(not(feature = "async"))]
        {
            // Direct blocking implementation
            self.send_command_blocking_direct(command)
        }
    }

    /// Direct blocking implementation without async
    #[cfg(not(feature = "async"))]
    fn send_command_blocking_direct<C>(&self, command: &C) -> Result<Response, Error>
    where
        C: EncodeVisca,
    {
        // Get command bytes using EncodeVisca
        let mut buffer = [0u8; 64]; // Use a reasonable max size
        let size = command.encode_into(self.camera_id, &mut buffer)?;
        let mut cmd_bytes = buffer[..size].to_vec();

        // Add VISCA terminator if not present
        if cmd_bytes.last() != Some(&crate::command::const_encoding::VISCA_TERMINATOR) {
            cmd_bytes.push(crate::command::const_encoding::VISCA_TERMINATOR);
        }

        // Apply protocol-specific framing using transport envelope
        let is_inquiry = command.response_type().is_some();
        let framed_bytes = self.envelope.frame_command(&cmd_bytes, is_inquiry);

        log::debug!("Sending VISCA command: {framed_bytes:02X?}");

        // Send command using blocking transport
        self.transport.send_blocking(&framed_bytes)?;

        // Handle response based on command type
        match command.response_type() {
            None => {
                // Action command - wait for ACK then Completion
                let ack_response = self.wait_for_response_blocking(self.profile.ack_timeout())?;
                match ack_response {
                    Response::CmdAck => {
                        // Wait for completion
                        self.wait_for_response_blocking(self.profile.completion_timeout())
                    }
                    Response::Completion => {
                        // Some cameras send completion directly
                        Ok(Response::Completion)
                    }
                    Response::Error(e) => Err(e),
                    _ => Err(Error::ParseError(format!(
                        "Unexpected response: {ack_response:?}"
                    ))),
                }
            }
            Some(expected_type) => {
                // Inquiry command - wait for specific response with type
                self.wait_for_response_with_type_blocking(
                    expected_type,
                    self.profile.completion_timeout(),
                )
            }
        }
    }

    /// Wait for a specific type of response with timeout (blocking version).
    #[cfg(not(feature = "async"))]
    fn wait_for_response_with_type_blocking(
        &self,
        expected_type: ResponseType,
        timeout: Duration,
    ) -> Result<Response, Error> {
        let start = std::time::Instant::now();
        loop {
            // Try to receive a response
            match self.transport.recv_blocking_timeout(timeout) {
                Ok(bytes) => {
                    log::debug!("Received response: {:02X?}", bytes);

                    // Deframe the response
                    let response_bytes = self.envelope.extract_response(&bytes)?;

                    // First try to parse as a regular response
                    match Response::parse(&response_bytes) {
                        Ok(Response::CmdAck) => {
                            // Skip ACK for inquiry commands and wait for the actual response
                            log::debug!("Skipping ACK response for inquiry command");
                            continue;
                        }
                        Ok(Response::Error(e)) => return Err(e),
                        Ok(Response::Completion) => {
                            // Unexpected completion for inquiry
                            return Err(Error::UnexpectedResponseType);
                        }
                        Ok(other) => {
                            // This shouldn't happen with parse() but handle it
                            return Ok(other);
                        }
                        Err(_) => {
                            // If regular parse fails, it might be an inquiry response
                            // Try parsing with the expected type
                            match Response::parse_with_type(&response_bytes, &expected_type) {
                                Ok(response) => return Ok(response),
                                Err(e) => return Err(e),
                            }
                        }
                    }
                }
                Err(Error::CommandTimeout { .. }) => {
                    if start.elapsed() >= timeout {
                        return Err(Error::CommandTimeout {
                            duration: timeout,
                            command: "wait_for_response_with_type_blocking".to_string(),
                        });
                    }
                    // If we haven't exceeded our timeout, continue waiting
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Wait for a response with timeout (blocking version)
    #[cfg(not(feature = "async"))]
    fn wait_for_response_blocking(&self, timeout: Duration) -> Result<Response, Error> {
        let start = std::time::Instant::now();
        loop {
            // Try to receive a response
            match self.transport.recv_blocking_timeout(timeout) {
                Ok(bytes) => {
                    log::debug!("Received response: {:02X?}", bytes);

                    // Deframe the response
                    let response_bytes = self.envelope.extract_response(&bytes)?;

                    // Parse using the generic Response parser
                    match Response::parse(&response_bytes) {
                        Ok(response) => return Ok(response),
                        Err(e) => {
                            log::warn!("Failed to parse response: {:?}", e);
                            // Continue waiting for a valid response
                        }
                    }
                }
                Err(Error::CommandTimeout { .. }) => {
                    if start.elapsed() >= timeout {
                        return Err(Error::CommandTimeout {
                            duration: Duration::from_secs(5),
                            command: "recv_blocking_timeout".to_string(),
                        });
                    }
                    // Continue waiting
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Check if the camera supports a specific capability.
    ///
    /// This provides runtime introspection of camera capabilities.
    #[must_use]
    pub fn supports_capability(&self, capability: &str) -> bool {
        // Helper macro to reduce boilerplate
        macro_rules! check_support {
            ($method:ident) => {
                match &self.profile {
                    CameraProfile::PTZOpticsG2(p) => p.$method(),
                    CameraProfile::PTZOpticsG3(p) => p.$method(),
                    CameraProfile::PTZOptics30X(p) => p.$method(),
                    CameraProfile::SonyFR7(p) => p.$method(),
                    CameraProfile::SonyBRCH900(p) => p.$method(),
                    CameraProfile::SonyEVIH100(p) => p.$method(),
                    CameraProfile::SonyBRC300(p) => p.$method(),
                    CameraProfile::NearusBRC300(p) => p.$method(),
                    CameraProfile::GenericVisca(p) => p.$method(),
                }
            };
        }

        match capability {
            "pan_tilt" => check_support!(supports_pan_tilt),
            "zoom" => check_support!(supports_zoom),
            "focus" => check_support!(supports_focus),
            "exposure" => check_support!(supports_exposure),
            "white_balance" => check_support!(supports_white_balance),
            "image_processing" => check_support!(supports_image_processing),
            "presets" => check_support!(supports_presets),
            "power" => check_support!(supports_power),
            "nd_filter" => check_support!(supports_nd_filter),
            _ => false,
        }
    }

    // Profile-specific accessor methods

    /// Get the power on time for the camera.
    #[must_use]
    pub fn power_on_time(&self) -> Duration {
        use crate::capabilities::Power;
        match &self.profile {
            CameraProfile::PTZOpticsG2(_) => PTZOpticsG2::POWER_ON_TIME,
            CameraProfile::PTZOpticsG3(_) => PTZOpticsG3::POWER_ON_TIME,
            CameraProfile::PTZOptics30X(_) => PTZOptics30X::POWER_ON_TIME,
            CameraProfile::SonyFR7(_) => SonyFR7::POWER_ON_TIME,
            CameraProfile::SonyBRCH900(_) => SonyBRCH900::POWER_ON_TIME,
            CameraProfile::SonyEVIH100(_) => SonyEVIH100::POWER_ON_TIME,
            CameraProfile::SonyBRC300(_) => SonyBRC300::POWER_ON_TIME,
            CameraProfile::NearusBRC300(_) => NearusBRC300::POWER_ON_TIME,
            CameraProfile::GenericVisca(_) => GenericVisca::POWER_ON_TIME,
        }
    }

    /// Get the standby time for the camera.
    #[must_use]
    pub fn standby_time(&self) -> Duration {
        use crate::capabilities::Power;
        match &self.profile {
            CameraProfile::PTZOpticsG2(_) => PTZOpticsG2::STANDBY_TIME,
            CameraProfile::PTZOpticsG3(_) => PTZOpticsG3::STANDBY_TIME,
            CameraProfile::PTZOptics30X(_) => PTZOptics30X::STANDBY_TIME,
            CameraProfile::SonyFR7(_) => SonyFR7::STANDBY_TIME,
            CameraProfile::SonyBRCH900(_) => SonyBRCH900::STANDBY_TIME,
            CameraProfile::SonyEVIH100(_) => SonyEVIH100::STANDBY_TIME,
            CameraProfile::SonyBRC300(_) => SonyBRC300::STANDBY_TIME,
            CameraProfile::NearusBRC300(_) => NearusBRC300::STANDBY_TIME,
            CameraProfile::GenericVisca(_) => GenericVisca::STANDBY_TIME,
        }
    }

    /// Get the maximum number of presets supported.
    #[must_use]
    pub fn max_presets(&self) -> u8 {
        use crate::capabilities::Presets;
        match &self.profile {
            CameraProfile::PTZOpticsG2(_) => PTZOpticsG2::MAX_PRESETS,
            CameraProfile::PTZOpticsG3(_) => PTZOpticsG3::MAX_PRESETS,
            CameraProfile::PTZOptics30X(_) => PTZOptics30X::MAX_PRESETS,
            CameraProfile::SonyFR7(_) => SonyFR7::MAX_PRESETS,
            CameraProfile::SonyBRCH900(_) => SonyBRCH900::MAX_PRESETS,
            CameraProfile::SonyEVIH100(_) => SonyEVIH100::MAX_PRESETS,
            CameraProfile::SonyBRC300(_) => SonyBRC300::MAX_PRESETS,
            CameraProfile::NearusBRC300(_) => NearusBRC300::MAX_PRESETS,
            CameraProfile::GenericVisca(_) => 0, // GenericVisca doesn't support presets
        }
    }

    /// Get the zoom speed range.
    #[must_use]
    pub fn zoom_speed_range(&self) -> std::ops::Range<u8> {
        use crate::capabilities::Zoom;
        match &self.profile {
            CameraProfile::PTZOpticsG2(_) => PTZOpticsG2::ZOOM_SPEED_RANGE,
            CameraProfile::PTZOpticsG3(_) => PTZOpticsG3::ZOOM_SPEED_RANGE,
            CameraProfile::PTZOptics30X(_) => PTZOptics30X::ZOOM_SPEED_RANGE,
            CameraProfile::SonyFR7(_) => SonyFR7::ZOOM_SPEED_RANGE,
            CameraProfile::SonyBRCH900(_) => SonyBRCH900::ZOOM_SPEED_RANGE,
            CameraProfile::SonyEVIH100(_) => SonyEVIH100::ZOOM_SPEED_RANGE,
            CameraProfile::SonyBRC300(_) => SonyBRC300::ZOOM_SPEED_RANGE,
            CameraProfile::NearusBRC300(_) => NearusBRC300::ZOOM_SPEED_RANGE,
            CameraProfile::GenericVisca(_) => GenericVisca::ZOOM_SPEED_RANGE,
        }
    }

    /// Get the optical zoom maximum value.
    #[must_use]
    pub fn optical_zoom_max(&self) -> u16 {
        use crate::capabilities::Zoom;
        match &self.profile {
            CameraProfile::PTZOpticsG2(_) => PTZOpticsG2::OPTICAL_ZOOM_MAX,
            CameraProfile::PTZOpticsG3(_) => PTZOpticsG3::OPTICAL_ZOOM_MAX,
            CameraProfile::PTZOptics30X(_) => PTZOptics30X::OPTICAL_ZOOM_MAX,
            CameraProfile::SonyFR7(_) => SonyFR7::OPTICAL_ZOOM_MAX,
            CameraProfile::SonyBRCH900(_) => SonyBRCH900::OPTICAL_ZOOM_MAX,
            CameraProfile::SonyEVIH100(_) => SonyEVIH100::OPTICAL_ZOOM_MAX,
            CameraProfile::SonyBRC300(_) => SonyBRC300::OPTICAL_ZOOM_MAX,
            CameraProfile::NearusBRC300(_) => NearusBRC300::OPTICAL_ZOOM_MAX,
            CameraProfile::GenericVisca(_) => GenericVisca::OPTICAL_ZOOM_MAX,
        }
    }

    /// Get the digital zoom maximum value.
    #[must_use]
    pub fn digital_zoom_max(&self) -> Option<u16> {
        use crate::capabilities::Zoom;
        match &self.profile {
            CameraProfile::PTZOpticsG2(_) => PTZOpticsG2::DIGITAL_ZOOM_MAX,
            CameraProfile::PTZOpticsG3(_) => PTZOpticsG3::DIGITAL_ZOOM_MAX,
            CameraProfile::PTZOptics30X(_) => PTZOptics30X::DIGITAL_ZOOM_MAX,
            CameraProfile::SonyFR7(_) => SonyFR7::DIGITAL_ZOOM_MAX,
            CameraProfile::SonyBRCH900(_) => SonyBRCH900::DIGITAL_ZOOM_MAX,
            CameraProfile::SonyEVIH100(_) => SonyEVIH100::DIGITAL_ZOOM_MAX,
            CameraProfile::SonyBRC300(_) => SonyBRC300::DIGITAL_ZOOM_MAX,
            CameraProfile::NearusBRC300(_) => NearusBRC300::DIGITAL_ZOOM_MAX,
            CameraProfile::GenericVisca(_) => GenericVisca::DIGITAL_ZOOM_MAX,
        }
    }

    /// Convert degrees to pan/tilt units.
    #[must_use]
    pub fn degrees_to_units(
        &self,
        pan_deg: crate::units::Degrees,
        tilt_deg: crate::units::Degrees,
    ) -> (crate::units::ViscaUnits<i16>, crate::units::ViscaUnits<i16>) {
        use crate::capabilities::PanTilt;
        let (pan_conv, tilt_conv) = match &self.profile {
            CameraProfile::PTZOpticsG2(_) => (
                PTZOpticsG2::PAN_DEGREES_TO_UNITS,
                PTZOpticsG2::TILT_DEGREES_TO_UNITS,
            ),
            CameraProfile::PTZOpticsG3(_) => (
                PTZOpticsG3::PAN_DEGREES_TO_UNITS,
                PTZOpticsG3::TILT_DEGREES_TO_UNITS,
            ),
            CameraProfile::PTZOptics30X(_) => (
                PTZOptics30X::PAN_DEGREES_TO_UNITS,
                PTZOptics30X::TILT_DEGREES_TO_UNITS,
            ),
            CameraProfile::SonyFR7(_) => (
                SonyFR7::PAN_DEGREES_TO_UNITS,
                SonyFR7::TILT_DEGREES_TO_UNITS,
            ),
            CameraProfile::SonyBRCH900(_) => (
                SonyBRCH900::PAN_DEGREES_TO_UNITS,
                SonyBRCH900::TILT_DEGREES_TO_UNITS,
            ),
            CameraProfile::SonyEVIH100(_) => (
                SonyEVIH100::PAN_DEGREES_TO_UNITS,
                SonyEVIH100::TILT_DEGREES_TO_UNITS,
            ),
            CameraProfile::SonyBRC300(_) => (
                SonyBRC300::PAN_DEGREES_TO_UNITS,
                SonyBRC300::TILT_DEGREES_TO_UNITS,
            ),
            CameraProfile::NearusBRC300(_) => (
                NearusBRC300::PAN_DEGREES_TO_UNITS,
                NearusBRC300::TILT_DEGREES_TO_UNITS,
            ),
            CameraProfile::GenericVisca(_) => (
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
    #[must_use]
    pub fn units_to_degrees(
        &self,
        pan_units: i16,
        tilt_units: i16,
    ) -> (crate::units::Degrees, crate::units::Degrees) {
        use crate::capabilities::PanTilt;
        let (pan_conv, tilt_conv) = match &self.profile {
            CameraProfile::PTZOpticsG2(_) => (
                PTZOpticsG2::PAN_DEGREES_TO_UNITS,
                PTZOpticsG2::TILT_DEGREES_TO_UNITS,
            ),
            CameraProfile::PTZOpticsG3(_) => (
                PTZOpticsG3::PAN_DEGREES_TO_UNITS,
                PTZOpticsG3::TILT_DEGREES_TO_UNITS,
            ),
            CameraProfile::PTZOptics30X(_) => (
                PTZOptics30X::PAN_DEGREES_TO_UNITS,
                PTZOptics30X::TILT_DEGREES_TO_UNITS,
            ),
            CameraProfile::SonyFR7(_) => (
                SonyFR7::PAN_DEGREES_TO_UNITS,
                SonyFR7::TILT_DEGREES_TO_UNITS,
            ),
            CameraProfile::SonyBRCH900(_) => (
                SonyBRCH900::PAN_DEGREES_TO_UNITS,
                SonyBRCH900::TILT_DEGREES_TO_UNITS,
            ),
            CameraProfile::SonyEVIH100(_) => (
                SonyEVIH100::PAN_DEGREES_TO_UNITS,
                SonyEVIH100::TILT_DEGREES_TO_UNITS,
            ),
            CameraProfile::SonyBRC300(_) => (
                SonyBRC300::PAN_DEGREES_TO_UNITS,
                SonyBRC300::TILT_DEGREES_TO_UNITS,
            ),
            CameraProfile::NearusBRC300(_) => (
                NearusBRC300::PAN_DEGREES_TO_UNITS,
                NearusBRC300::TILT_DEGREES_TO_UNITS,
            ),
            CameraProfile::GenericVisca(_) => (
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
            CameraProfile::PTZOpticsG2(_) => PTZOpticsG2::PAN_RANGE,
            CameraProfile::PTZOpticsG3(_) => PTZOpticsG3::PAN_RANGE,
            CameraProfile::PTZOptics30X(_) => PTZOptics30X::PAN_RANGE,
            CameraProfile::SonyFR7(_) => SonyFR7::PAN_RANGE,
            CameraProfile::SonyBRCH900(_) => SonyBRCH900::PAN_RANGE,
            CameraProfile::SonyEVIH100(_) => SonyEVIH100::PAN_RANGE,
            CameraProfile::SonyBRC300(_) => SonyBRC300::PAN_RANGE,
            CameraProfile::NearusBRC300(_) => NearusBRC300::PAN_RANGE,
            CameraProfile::GenericVisca(_) => GenericVisca::PAN_RANGE,
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
            CameraProfile::PTZOpticsG2(_) => PTZOpticsG2::TILT_RANGE,
            CameraProfile::PTZOpticsG3(_) => PTZOpticsG3::TILT_RANGE,
            CameraProfile::PTZOptics30X(_) => PTZOptics30X::TILT_RANGE,
            CameraProfile::SonyFR7(_) => SonyFR7::TILT_RANGE,
            CameraProfile::SonyBRCH900(_) => SonyBRCH900::TILT_RANGE,
            CameraProfile::SonyEVIH100(_) => SonyEVIH100::TILT_RANGE,
            CameraProfile::SonyBRC300(_) => SonyBRC300::TILT_RANGE,
            CameraProfile::NearusBRC300(_) => NearusBRC300::TILT_RANGE,
            CameraProfile::GenericVisca(_) => GenericVisca::TILT_RANGE,
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

    /// Get the coordinate system used by this camera.
    #[must_use]
    pub fn coordinate_system(&self) -> crate::capabilities::CoordinateSystem {
        use crate::capabilities::PanTilt;
        match &self.profile {
            CameraProfile::PTZOpticsG2(_) => PTZOpticsG2::COORDINATE_SYSTEM,
            CameraProfile::PTZOpticsG3(_) => PTZOpticsG3::COORDINATE_SYSTEM,
            CameraProfile::PTZOptics30X(_) => PTZOptics30X::COORDINATE_SYSTEM,
            CameraProfile::SonyFR7(_) => SonyFR7::COORDINATE_SYSTEM,
            CameraProfile::SonyBRCH900(_) => SonyBRCH900::COORDINATE_SYSTEM,
            CameraProfile::SonyEVIH100(_) => SonyEVIH100::COORDINATE_SYSTEM,
            CameraProfile::SonyBRC300(_) => SonyBRC300::COORDINATE_SYSTEM,
            CameraProfile::NearusBRC300(_) => NearusBRC300::COORDINATE_SYSTEM,
            CameraProfile::GenericVisca(_) => GenericVisca::COORDINATE_SYSTEM,
        }
    }

    /// Convert logical pan/tilt coordinates to camera-specific coordinates.
    /// This handles coordinate system differences between camera models.
    #[must_use]
    pub fn to_camera_coords(&self, pan: i16, tilt: i16) -> (u16, u16) {
        self.coordinate_system().to_camera_coords(pan, tilt)
    }

    /// Convert camera-specific coordinates to logical pan/tilt coordinates.
    /// This handles coordinate system differences between camera models.
    #[must_use]
    pub fn from_camera_coords(&self, pan: u16, tilt: u16) -> (i16, i16) {
        self.coordinate_system()
            .convert_from_camera_coords(pan, tilt)
    }

    /// Get the ND filter mode.
    #[must_use]
    pub fn nd_filter_mode(&self) -> Option<crate::capabilities::NDFilterMode> {
        use crate::capabilities::NDFilter;
        match &self.profile {
            CameraProfile::PTZOpticsG2(_) => None, // PTZOpticsG2 doesn't support ND filters
            CameraProfile::PTZOpticsG3(_) => None,
            CameraProfile::PTZOptics30X(_) => None,
            CameraProfile::SonyFR7(_) => Some(SonyFR7::ND_MODE),
            CameraProfile::SonyBRCH900(_) => None,
            CameraProfile::SonyEVIH100(_) => None,
            CameraProfile::SonyBRC300(_) => None,
            CameraProfile::NearusBRC300(_) => None,
            CameraProfile::GenericVisca(_) => None, // GenericVisca doesn't support ND filters
        }
    }

    /// Check if the camera supports Motion Sync.
    #[must_use]
    pub fn supports_motion_sync(&self) -> bool {
        use crate::capabilities::MotionSync;
        match &self.profile {
            CameraProfile::PTZOpticsG2(_) => PTZOpticsG2::SUPPORTS_MOTION_SYNC,
            CameraProfile::PTZOpticsG3(_) => PTZOpticsG3::SUPPORTS_MOTION_SYNC,
            CameraProfile::PTZOptics30X(_) => PTZOptics30X::SUPPORTS_MOTION_SYNC,
            CameraProfile::SonyFR7(_) => false, // Sony cameras don't support Motion Sync
            CameraProfile::SonyBRCH900(_) => false,
            CameraProfile::SonyEVIH100(_) => false,
            CameraProfile::SonyBRC300(_) => false,
            CameraProfile::NearusBRC300(_) => false,
            CameraProfile::GenericVisca(_) => false, // Generic cameras don't support Motion Sync
        }
    }

    /// Validate ND filter level.
    pub fn validate_nd_filter(&self, level: u8) -> Result<u8, Error> {
        use crate::capabilities::nd_filter::NDFilterExt;

        // Check if camera supports ND filters
        match &self.profile {
            CameraProfile::PTZOpticsG2(_) => Err(Error::FeatureNotSupported {
                feature: "ND filter",
            }),
            CameraProfile::PTZOpticsG3(_) => Err(Error::FeatureNotSupported {
                feature: "ND filter",
            }),
            CameraProfile::PTZOptics30X(_) => Err(Error::FeatureNotSupported {
                feature: "ND filter",
            }),
            CameraProfile::SonyFR7(p) => {
                // Use the NDFilterExt trait method for validation
                p.validate_nd_filter(level).map_err(Into::into)
            }
            CameraProfile::SonyBRCH900(_) => Err(Error::FeatureNotSupported {
                feature: "ND filter",
            }),
            CameraProfile::SonyEVIH100(_) => Err(Error::FeatureNotSupported {
                feature: "ND filter",
            }),
            CameraProfile::SonyBRC300(_) => Err(Error::FeatureNotSupported {
                feature: "ND filter",
            }),
            CameraProfile::NearusBRC300(_) => Err(Error::FeatureNotSupported {
                feature: "ND filter",
            }),
            CameraProfile::GenericVisca(_) => Err(Error::FeatureNotSupported {
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
    /// use grafton_visca::{Camera, CameraModel};
    /// use grafton_visca::transport::blocking::Tcp;
    /// use grafton_visca::blocking::ZoomOps;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let transport = Tcp::connect("192.168.1.100:52381")?;
    /// let camera = Camera::with_profile(CameraModel::PTZOpticsG2, transport);
    /// let blocking_camera = camera.blocking();
    ///
    /// // Use blocking API
    /// blocking_camera.zoom_stop()?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn blocking(self) -> crate::blocking::Camera {
        crate::blocking::Camera::new(self)
    }

    /// Get a blocking wrapper for this camera by reference.
    ///
    /// This provides a blocking API without consuming the camera instance,
    /// allowing you to obtain both blocking and async views.
    #[must_use]
    pub fn blocking_ref(&self) -> crate::blocking::Camera {
        crate::blocking::Camera::new(self.clone())
    }

    /// Get an async wrapper for this camera by reference.
    ///
    /// This provides an async API without consuming the camera instance,
    /// allowing you to obtain both blocking and async views.
    #[must_use]
    #[cfg(feature = "async")]
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
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// #[cfg(feature = "tokio")]
    /// use grafton_visca::transport::tokio::Tcp;
    /// use grafton_visca::{Camera, CameraModel};
    /// use grafton_visca::r#async::ZoomOps;
    ///
    /// # #[cfg(feature = "tokio")]
    /// # {
    /// let transport = Tcp::connect("192.168.1.100:52381").await?;
    /// let camera = Camera::with_profile(CameraModel::PTZOpticsG2, transport);
    /// let async_camera = camera.r#async();
    ///
    /// // Use async API
    /// async_camera.zoom_stop().await?;
    /// # }
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    #[cfg(feature = "async")]
    pub fn r#async(self) -> crate::r#async::Camera {
        crate::r#async::Camera::new(self)
    }
}

impl FeatureDetection for Camera {
    fn supports_feature(&self, feature: CameraFeature) -> bool {
        match feature {
            CameraFeature::PanTilt => match &self.profile {
                CameraProfile::PTZOpticsG2(_) => true, // Implements PanTilt trait
                CameraProfile::PTZOpticsG3(_) => true,
                CameraProfile::PTZOptics30X(_) => true,
                CameraProfile::SonyFR7(_) => true,
                CameraProfile::SonyBRCH900(_) => true,
                CameraProfile::SonyEVIH100(_) => true,
                CameraProfile::SonyBRC300(_) => true,
                CameraProfile::NearusBRC300(_) => true,
                CameraProfile::GenericVisca(_) => true,
            },
            CameraFeature::Zoom => match &self.profile {
                CameraProfile::PTZOpticsG2(_) => true,
                CameraProfile::PTZOpticsG3(_) => true,
                CameraProfile::PTZOptics30X(_) => true,
                CameraProfile::SonyFR7(_) => true,
                CameraProfile::SonyBRCH900(_) => true,
                CameraProfile::SonyEVIH100(_) => true,
                CameraProfile::SonyBRC300(_) => true,
                CameraProfile::NearusBRC300(_) => true,
                CameraProfile::GenericVisca(_) => true,
            },
            CameraFeature::Focus | CameraFeature::FocusLock => match &self.profile {
                CameraProfile::PTZOpticsG2(_) => true,
                CameraProfile::PTZOpticsG3(_) => true,
                CameraProfile::PTZOptics30X(_) => true,
                CameraProfile::SonyFR7(_) => true,
                CameraProfile::SonyBRCH900(_) => true,
                CameraProfile::SonyEVIH100(_) => true,
                CameraProfile::SonyBRC300(_) => true,
                CameraProfile::NearusBRC300(_) => true,
                CameraProfile::GenericVisca(_) => true,
            },
            CameraFeature::Exposure
            | CameraFeature::Gain
            | CameraFeature::Shutter
            | CameraFeature::Iris
            | CameraFeature::Backlight => match &self.profile {
                CameraProfile::PTZOpticsG2(_) => true,
                CameraProfile::PTZOpticsG3(_) => true,
                CameraProfile::PTZOptics30X(_) => true,
                CameraProfile::SonyFR7(_) => true,
                CameraProfile::SonyBRCH900(_) => true,
                CameraProfile::SonyEVIH100(_) => true,
                CameraProfile::SonyBRC300(_) => true,
                CameraProfile::NearusBRC300(_) => true,
                CameraProfile::GenericVisca(_) => true,
            },
            CameraFeature::WhiteBalance => match &self.profile {
                CameraProfile::PTZOpticsG2(_) => true,
                CameraProfile::PTZOpticsG3(_) => true,
                CameraProfile::PTZOptics30X(_) => true,
                CameraProfile::SonyFR7(_) => true,
                CameraProfile::SonyBRCH900(_) => true,
                CameraProfile::SonyEVIH100(_) => true,
                CameraProfile::SonyBRC300(_) => true,
                CameraProfile::NearusBRC300(_) => true,
                CameraProfile::GenericVisca(_) => true,
            },
            CameraFeature::ImageProcessing
            | CameraFeature::ColorSaturation
            | CameraFeature::Gamma
            | CameraFeature::BlackWhiteMode
            | CameraFeature::NoiseReduction
            | CameraFeature::Sharpness => match &self.profile {
                CameraProfile::PTZOpticsG2(_) => true,
                CameraProfile::PTZOpticsG3(_) => true,
                CameraProfile::PTZOptics30X(_) => true,
                CameraProfile::SonyFR7(_) => true,
                CameraProfile::SonyBRCH900(_) => true,
                CameraProfile::SonyEVIH100(_) => true,
                CameraProfile::SonyBRC300(_) => true,
                CameraProfile::NearusBRC300(_) => true,
                CameraProfile::GenericVisca(_) => false, // Generic doesn't support image processing
            },
            CameraFeature::Power => match &self.profile {
                CameraProfile::PTZOpticsG2(_) => true,
                CameraProfile::PTZOpticsG3(_) => true,
                CameraProfile::PTZOptics30X(_) => true,
                CameraProfile::SonyFR7(_) => true,
                CameraProfile::SonyBRCH900(_) => true,
                CameraProfile::SonyEVIH100(_) => true,
                CameraProfile::SonyBRC300(_) => true,
                CameraProfile::NearusBRC300(_) => true,
                CameraProfile::GenericVisca(_) => true,
            },
            CameraFeature::Presets => match &self.profile {
                CameraProfile::PTZOpticsG2(_) => true,
                CameraProfile::PTZOpticsG3(_) => true,
                CameraProfile::PTZOptics30X(_) => true,
                CameraProfile::SonyFR7(_) => true,
                CameraProfile::SonyBRCH900(_) => true,
                CameraProfile::SonyEVIH100(_) => true,
                CameraProfile::SonyBRC300(_) => true,
                CameraProfile::NearusBRC300(_) => true,
                CameraProfile::GenericVisca(_) => true,
            },
            CameraFeature::NDFilter => match &self.profile {
                CameraProfile::PTZOpticsG2(_) => false,
                CameraProfile::PTZOpticsG3(_) => false,
                CameraProfile::PTZOptics30X(_) => false,
                CameraProfile::SonyFR7(_) => true, // Only FR7 has ND filter
                CameraProfile::SonyBRCH900(_) => false,
                CameraProfile::SonyEVIH100(_) => false,
                CameraProfile::SonyBRC300(_) => false,
                CameraProfile::NearusBRC300(_) => false,
                CameraProfile::GenericVisca(_) => false,
            },
            CameraFeature::MotionSync => self.supports_motion_sync(),

            // Camera-specific features
            CameraFeature::Tally => {
                // Tally is supported by most cameras with variations
                matches!(
                    self.profile,
                    CameraProfile::PTZOpticsG2(_)
                        | CameraProfile::PTZOpticsG3(_)
                        | CameraProfile::PTZOptics30X(_)
                        | CameraProfile::SonyFR7(_)
                        | CameraProfile::SonyBRCH900(_)
                        | CameraProfile::SonyBRC300(_)
                        | CameraProfile::NearusBRC300(_)
                )
            }
            CameraFeature::ImageFreeze => {
                // Image freeze is supported by Sony BRC series
                matches!(
                    self.profile,
                    CameraProfile::SonyBRCH900(_)
                        | CameraProfile::SonyBRC300(_)
                        | CameraProfile::NearusBRC300(_)
                )
            }
            CameraFeature::ImageFlip => {
                // Image flip is supported by most modern cameras
                matches!(
                    self.profile,
                    CameraProfile::PTZOpticsG2(_)
                        | CameraProfile::PTZOpticsG3(_)
                        | CameraProfile::PTZOptics30X(_)
                        | CameraProfile::SonyFR7(_)
                        | CameraProfile::SonyBRCH900(_)
                )
            }
            CameraFeature::VariableSpeedMode => {
                // Variable speed mode is Sony FR7 specific
                matches!(self.profile, CameraProfile::SonyFR7(_))
            }
            CameraFeature::MenuControl => {
                // All modern VISCA cameras support basic menu control
                true
            }
            CameraFeature::Privacy => {
                // Privacy mode is supported by PTZOptics and some Sony models
                matches!(
                    self.profile,
                    CameraProfile::PTZOpticsG2(_)
                        | CameraProfile::PTZOpticsG3(_)
                        | CameraProfile::PTZOptics30X(_)
                        | CameraProfile::SonyFR7(_)
                )
            }
            CameraFeature::SystemReset | CameraFeature::CommandCancel => {
                // System commands are supported by all VISCA cameras
                true
            }
            CameraFeature::PictureEffect => {
                // Picture effects are supported by most cameras
                true
            }
            CameraFeature::NDI => match &self.profile {
                CameraProfile::PTZOpticsG2(_) => true, // NDI model variants
                CameraProfile::PTZOpticsG3(_) => true, // NDI model variants
                CameraProfile::PTZOptics30X(_) => false,
                CameraProfile::SonyFR7(_) => false,
                CameraProfile::SonyBRCH900(_) => false,
                CameraProfile::SonyEVIH100(_) => false,
                CameraProfile::SonyBRC300(_) => false,
                CameraProfile::NearusBRC300(_) => false,
                CameraProfile::GenericVisca(_) => false,
            },
        }
    }

    fn supports_command(&self, _command: &dyn Any) -> bool {
        // For now, we can't determine command support without CommandFeatures trait
        // This would require all commands to implement CommandFeatures
        // Return true for backward compatibility
        true
    }

    fn supported_features(&self) -> Vec<CameraFeature> {
        let all_features = vec![
            CameraFeature::PanTilt,
            CameraFeature::Zoom,
            CameraFeature::MotionSync,
            CameraFeature::Focus,
            CameraFeature::FocusLock,
            CameraFeature::Exposure,
            CameraFeature::Gain,
            CameraFeature::Shutter,
            CameraFeature::Iris,
            CameraFeature::Backlight,
            CameraFeature::WhiteBalance,
            CameraFeature::ImageProcessing,
            CameraFeature::ColorSaturation,
            CameraFeature::Gamma,
            CameraFeature::BlackWhiteMode,
            CameraFeature::NoiseReduction,
            CameraFeature::Sharpness,
            CameraFeature::Power,
            CameraFeature::Presets,
            CameraFeature::Tally,
            CameraFeature::ImageFreeze,
            CameraFeature::ImageFlip,
            CameraFeature::NDFilter,
            CameraFeature::VariableSpeedMode,
            CameraFeature::MenuControl,
            CameraFeature::Privacy,
            CameraFeature::SystemReset,
            CameraFeature::CommandCancel,
        ];

        all_features
            .into_iter()
            .filter(|&feature| self.supports_feature(feature))
            .collect()
    }
}

// Note: The implementation of specific camera methods (zoom, pan_tilt, etc.) will be added
// through extension traits that provide high-level convenience methods on top of send_command.

// Internal trait for camera wrapper access

pub mod methods;
pub mod profiles;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::{
        command::{
            menu::{MenuAction, MenuActionCommand},
            motion_sync::MotionSyncModeCommand,
            nd_filter::{NDFilterMode, NDFilterModeCommand},
            pan_tilt::{PanTilt, PanTiltDirection},
            power::PowerCommand,
            preset::{PresetAction, PresetCommand, PresetNumber},
            variable_speed::{VariableSpeedMode, VariableSpeedModeCommand},
        },
        transport::blocking::Udp as UdpTransport,
        types::{PanSpeed, TiltSpeed},
        MotionSyncMode,
    };

    #[test]
    fn test_profile_selection() {
        let profile = CameraModel::PTZOpticsG2.to_profile();
        assert_eq!(profile.model_name(), "PTZOptics G2");

        let profile = CameraModel::SonyFR7.to_profile();
        assert_eq!(profile.model_name(), "Sony FR7");

        let profile = CameraModel::default().to_profile();
        assert_eq!(profile.model_name(), "Generic VISCA Camera");
    }

    #[test]
    fn test_basic_command_validation() {
        // Create a dummy transport - it won't be used for validation
        let transport = UdpTransport::connect("127.0.0.1:52381").unwrap();

        // Create a Sony FR7 camera (has ND filter support)
        let camera = Camera::with_profile(CameraModel::SonyFR7, transport);

        // ND filter command should be valid for FR7
        let nd_command = NDFilterModeCommand::new(NDFilterMode::Variable);
        assert!(camera.validate_command(&nd_command).is_ok());

        // Create a PTZOptics G2 camera (no ND filter support)
        let transport = UdpTransport::connect("127.0.0.1:52381").unwrap();
        let camera = Camera::with_profile(CameraModel::PTZOpticsG2, transport);

        // ND filter command should fail for PTZOptics G2
        let nd_command = NDFilterModeCommand::new(NDFilterMode::Variable);
        match camera.validate_command(&nd_command) {
            Err(Error::FeatureNotSupported { feature }) => {
                assert_eq!(feature, "ND Filter");
            }
            _ => panic!("Expected FeatureNotSupported error"),
        }
    }

    #[test]
    fn test_universal_command_validation() {
        let transport = UdpTransport::connect("127.0.0.1:52381").unwrap();

        // Power commands should work on all cameras
        let camera = Camera::with_profile(CameraModel::GenericVisca, transport);

        let power_command = PowerCommand::On;
        assert!(camera.validate_command(&power_command).is_ok());

        // Pan/tilt commands should work on all cameras
        let pan_tilt_command = PanTilt::Move {
            direction: PanTiltDirection::Up,
            pan_speed: PanSpeed::new(10).unwrap(),
            tilt_speed: TiltSpeed::new(10).unwrap(),
        };
        assert!(camera.validate_command(&pan_tilt_command).is_ok());
    }

    #[test]
    fn test_preset_validation() {
        let transport = UdpTransport::connect("127.0.0.1:52381").unwrap();

        // All cameras support presets
        let camera = Camera::with_profile(CameraModel::SonyEVIH100, transport);

        let preset_command = PresetCommand {
            action: PresetAction::Recall,
            preset_number: PresetNumber::new(5).unwrap(),
        };
        assert!(camera.validate_command(&preset_command).is_ok());
    }

    #[test]
    fn test_motion_sync_validation() {
        // PTZOptics cameras support motion sync
        let transport = UdpTransport::connect("127.0.0.1:52381").unwrap();
        let camera = Camera::with_profile(CameraModel::PTZOpticsG3, transport);

        let motion_sync_command = MotionSyncModeCommand::new(MotionSyncMode::On);
        assert!(camera.validate_command(&motion_sync_command).is_ok());

        // Sony cameras don't support motion sync
        let transport = UdpTransport::connect("127.0.0.1:52381").unwrap();
        let camera = Camera::with_profile(CameraModel::SonyFR7, transport);

        match camera.validate_command(&motion_sync_command) {
            Err(Error::FeatureNotSupported { feature }) => {
                assert_eq!(feature, "Motion Sync");
            }
            _ => panic!("Expected FeatureNotSupported error"),
        }
    }

    #[test]
    fn test_menu_control_validation() {
        let transport = UdpTransport::connect("127.0.0.1:52381").unwrap();

        // All cameras support basic menu control
        let camera = Camera::with_profile(CameraModel::SonyBRC300, transport);

        let menu_command = MenuActionCommand::new(MenuAction::Select);
        assert!(camera.validate_command(&menu_command).is_ok());
    }

    #[test]
    fn test_variable_speed_validation() {
        // Only Sony FR7 supports variable speed mode
        let transport = UdpTransport::connect("127.0.0.1:52381").unwrap();
        let camera = Camera::with_profile(CameraModel::SonyFR7, transport);

        let var_speed_command = VariableSpeedModeCommand::new(VariableSpeedMode::Fine50);
        assert!(camera.validate_command(&var_speed_command).is_ok());

        // Other cameras don't support it
        let transport = UdpTransport::connect("127.0.0.1:52381").unwrap();
        let camera = Camera::with_profile(CameraModel::PTZOptics30X, transport);

        match camera.validate_command(&var_speed_command) {
            Err(Error::FeatureNotSupported { feature }) => {
                assert_eq!(feature, "Variable Speed Mode");
            }
            _ => panic!("Expected FeatureNotSupported error"),
        }
    }

    #[test]
    fn test_command_features_trait() {
        // Test that commands report correct required features
        let nd_command = NDFilterModeCommand::new(NDFilterMode::Preset);
        assert_eq!(nd_command.required_features(), &[CameraFeature::NDFilter]);

        let power_command = PowerCommand::On;
        assert_eq!(power_command.required_features(), &[CameraFeature::Power]);

        let pan_tilt_command = PanTilt::Home;
        assert_eq!(
            pan_tilt_command.required_features(),
            &[CameraFeature::PanTilt]
        );
    }
}
