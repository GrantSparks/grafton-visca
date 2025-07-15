//! New camera API with compile-time safety.
//!
//! This module implements the new Camera API where methods only exist
//! for cameras that support the corresponding capabilities.

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
    socket_manager::SocketManagerHandle,
    transport::core::{BlockingTransport, Transport},
    Response,
};

use super::profiles::{GenericVisca, PTZOpticsG2, SonyFR7};

/// Camera profile wrapper that erases the concrete profile type.
///
/// This enum allows us to store different camera profiles without exposing
/// generic parameters to the user.
#[derive(Debug, Clone, Copy)]
pub enum CameraProfile {
    /// PTZOptics G2 camera profile
    PTZOpticsG2(PTZOpticsG2),
    /// Sony FR7 camera profile  
    SonyFR7(SonyFR7),
    /// Generic VISCA camera profile (default)
    GenericVisca(GenericVisca),
}

impl CameraProfile {
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
    /// Sony FR7 camera
    SonyFR7,
    /// Generic VISCA camera (default)
    GenericVisca,
}

impl CameraModel {
    /// Convert to a camera profile instance.
    #[must_use]
    pub fn to_profile(self) -> CameraProfile {
        match self {
            Self::PTZOpticsG2 => CameraProfile::PTZOpticsG2(PTZOpticsG2),
            Self::SonyFR7 => CameraProfile::SonyFR7(SonyFR7),
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
/// use grafton_visca::{Camera, CameraModel};
/// use grafton_visca::command::power::PowerCommand;
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
    address: u8,
    socket_manager: Option<SocketManagerHandle>,
}

impl Clone for Camera {
    fn clone(&self) -> Self {
        Self {
            profile: self.profile,
            transport: Arc::clone(&self.transport),
            address: self.address,
            socket_manager: self.socket_manager.clone(),
        }
    }
}

impl std::fmt::Debug for Camera {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Camera")
            .field("profile", &self.profile)
            .field("address", &self.address)
            .field("transport", &"<dyn UnifiedTransport>")
            .field("socket_manager", &self.socket_manager.is_some())
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
        Self::with_profile_and_transport(CameraModel::default(), transport)
    }

    /// Create a new unified camera with a specific profile.
    pub fn with_profile<T>(profile: CameraModel, transport: T) -> Self
    where
        T: Transport + Send + Sync + 'static,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        Self::with_profile_and_transport(profile, transport)
    }

    /// Create with a blocking transport.
    pub(crate) fn new_blocking<T>(transport: T) -> Self
    where
        T: BlockingTransport + Send + Sync + 'static,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        Self::with_profile_and_blocking_transport(CameraModel::default(), transport)
    }

    /// Create with a specific profile and blocking transport.
    pub(crate) fn with_profile_blocking<T>(profile: CameraModel, transport: T) -> Self
    where
        T: BlockingTransport + Send + Sync + 'static,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        Self::with_profile_and_blocking_transport(profile, transport)
    }

    /// Internal constructor for async transports.
    fn with_profile_and_transport<T>(profile: CameraModel, transport: T) -> Self
    where
        T: Transport + Send + Sync + 'static,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        let profile = profile.to_profile();
        let address = profile.default_address();
        let transport = Arc::new(AsyncTransportWrapper { transport });

        let mut camera = Self {
            profile,
            transport,
            address,
            socket_manager: None,
        };

        // Initialize socket manager automatically for better reliability
        if let Err(e) = camera.initialize_socket_manager() {
            log::warn!("Failed to initialize socket manager: {}", e);
        }

        camera
    }

    /// Internal constructor for blocking transports.
    fn with_profile_and_blocking_transport<T>(profile: CameraModel, transport: T) -> Self
    where
        T: BlockingTransport + Send + Sync + 'static,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        let profile = profile.to_profile();
        let address = profile.default_address();
        let transport = Arc::new(BlockingTransportWrapper { transport });

        let mut camera = Self {
            profile,
            transport,
            address,
            socket_manager: None,
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

    /// Set the VISCA address for this camera.
    pub fn set_address(&mut self, address: u8) {
        self.address = address;
    }

    /// Initialize the socket manager for this camera.
    /// This enables queued command processing for proper two-socket management.
    pub fn initialize_socket_manager(&mut self) -> Result<(), Error> {
        if self.socket_manager.is_some() {
            return Ok(()); // Already initialized
        }

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

        #[cfg(feature = "tokio")]
        {
            let actor = crate::socket_manager::SocketManagerActor::new(
                transport,
                command_receiver,
                profile,
            );
            tokio::spawn(async move {
                if let Err(e) = actor.run().await {
                    log::error!("Socket manager actor failed: {}", e);
                }
            });
        }

        #[cfg(not(feature = "tokio"))]
        {
            let actor = crate::socket_manager::SocketManagerActor::new(
                transport,
                command_receiver,
                profile,
            );
            std::thread::spawn(move || {
                // For non-tokio, we need to create a simple blocking event loop
                // This is a simplified implementation
                use std::future::Future;
                use std::task::{Context, Poll};

                struct SimpleExecutor;

                impl SimpleExecutor {
                    fn block_on<F: Future>(future: F) -> F::Output {
                        let mut future = Box::pin(future);

                        loop {
                            let waker = futures::task::noop_waker();
                            let mut cx = Context::from_waker(&waker);

                            match future.as_mut().poll(&mut cx) {
                                Poll::Ready(result) => return result,
                                Poll::Pending => {
                                    // In a real implementation, we would wait for events
                                    // For now, we'll just yield to avoid busy waiting
                                    std::thread::yield_now();
                                }
                            }
                        }
                    }
                }

                if let Err(e) = SimpleExecutor::block_on(actor.run()) {
                    log::error!("Socket manager actor failed: {}", e);
                }
            });
        }

        Ok(())
    }

    /// Cancel a command on a specific socket.
    ///
    /// This sends a VISCA command cancel request to the camera for the specified socket.
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

    /// Cancel a command on a specific socket (blocking version).
    ///
    /// This sends a VISCA command cancel request to the camera for the specified socket.
    pub(crate) fn cancel_command_blocking(
        &self,
        socket: crate::command::system::Socket,
    ) -> Result<(), Error> {
        // Use executor to block on the async method
        futures::executor::block_on(self.cancel_command(socket))
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
        let size = command.encode_into(&mut buffer)?;
        let mut cmd_bytes = buffer[..size].to_vec();

        // Add VISCA terminator if not present
        if cmd_bytes.last() != Some(&crate::command::const_encoding::VISCA_TERMINATOR) {
            cmd_bytes.push(crate::command::const_encoding::VISCA_TERMINATOR);
        }

        // Apply protocol-specific framing if needed
        let framed_bytes = match self.profile.protocol_style() {
            ProtocolStyle::RawVisca => cmd_bytes,
            ProtocolStyle::SonyEncapsulated { use_sequence: _ } => {
                // TODO: Implement Sony encapsulation
                log::warn!("Sony encapsulation not yet implemented, using raw VISCA");
                cmd_bytes
            }
        };

        log::debug!(
            "Sending VISCA command via socket manager: {:02X?}",
            framed_bytes
        );

        // Determine if this is an inquiry command
        let is_inquiry = command.response_type().is_some();

        // Get timeout category from command
        let category = command.timeout_kind();

        // Send via socket manager
        socket_manager
            .send_command(framed_bytes, category, is_inquiry)
            .await
    }

    /// Send command directly via transport (legacy approach)
    async fn send_command_direct<C>(&self, command: &C) -> Result<Response, Error>
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
    async fn wait_for_response_with_type(
        &self,
        expected_type: ResponseType,
        _timeout: Duration,
    ) -> Result<Response, Error> {
        // For inquiry commands, we may receive an ACK first, then the inquiry response
        loop {
            match self.transport.recv().await {
                Ok(bytes) => {
                    // First try to parse as a regular response
                    match Response::parse(&bytes) {
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
                            match Response::parse_with_type(&bytes, &expected_type) {
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
                CameraProfile::PTZOpticsG2(p) => p.supports_pan_tilt(),
                CameraProfile::SonyFR7(p) => p.supports_pan_tilt(),
                CameraProfile::GenericVisca(p) => p.supports_pan_tilt(),
            },
            "zoom" => match &self.profile {
                CameraProfile::PTZOpticsG2(p) => p.supports_zoom(),
                CameraProfile::SonyFR7(p) => p.supports_zoom(),
                CameraProfile::GenericVisca(p) => p.supports_zoom(),
            },
            "focus" => match &self.profile {
                CameraProfile::PTZOpticsG2(p) => p.supports_focus(),
                CameraProfile::SonyFR7(p) => p.supports_focus(),
                CameraProfile::GenericVisca(p) => p.supports_focus(),
            },
            "exposure" => match &self.profile {
                CameraProfile::PTZOpticsG2(p) => p.supports_exposure(),
                CameraProfile::SonyFR7(p) => p.supports_exposure(),
                CameraProfile::GenericVisca(p) => p.supports_exposure(),
            },
            "white_balance" => match &self.profile {
                CameraProfile::PTZOpticsG2(p) => p.supports_white_balance(),
                CameraProfile::SonyFR7(p) => p.supports_white_balance(),
                CameraProfile::GenericVisca(p) => p.supports_white_balance(),
            },
            "image_processing" => match &self.profile {
                CameraProfile::PTZOpticsG2(p) => p.supports_image_processing(),
                CameraProfile::SonyFR7(p) => p.supports_image_processing(),
                CameraProfile::GenericVisca(p) => p.supports_image_processing(),
            },
            "presets" => match &self.profile {
                CameraProfile::PTZOpticsG2(p) => p.supports_presets(),
                CameraProfile::SonyFR7(p) => p.supports_presets(),
                CameraProfile::GenericVisca(p) => p.supports_presets(),
            },
            "power" => match &self.profile {
                CameraProfile::PTZOpticsG2(p) => p.supports_power(),
                CameraProfile::SonyFR7(p) => p.supports_power(),
                CameraProfile::GenericVisca(p) => p.supports_power(),
            },
            "nd_filter" => match &self.profile {
                CameraProfile::PTZOpticsG2(p) => p.supports_nd_filter(),
                CameraProfile::SonyFR7(p) => p.supports_nd_filter(),
                CameraProfile::GenericVisca(p) => p.supports_nd_filter(),
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
            CameraProfile::PTZOpticsG2(_) => PTZOpticsG2::POWER_ON_TIME,
            CameraProfile::SonyFR7(_) => SonyFR7::POWER_ON_TIME,
            CameraProfile::GenericVisca(_) => GenericVisca::POWER_ON_TIME,
        }
    }

    /// Get the standby time for the camera.
    #[must_use]
    pub fn standby_time(&self) -> Duration {
        use crate::capabilities::Power;
        match &self.profile {
            CameraProfile::PTZOpticsG2(_) => PTZOpticsG2::STANDBY_TIME,
            CameraProfile::SonyFR7(_) => SonyFR7::STANDBY_TIME,
            CameraProfile::GenericVisca(_) => GenericVisca::STANDBY_TIME,
        }
    }

    /// Get the maximum number of presets supported.
    #[must_use]
    pub fn max_presets(&self) -> u8 {
        use crate::capabilities::Presets;
        match &self.profile {
            CameraProfile::PTZOpticsG2(_) => PTZOpticsG2::MAX_PRESETS,
            CameraProfile::SonyFR7(_) => SonyFR7::MAX_PRESETS,
            CameraProfile::GenericVisca(_) => 0, // GenericVisca doesn't support presets
        }
    }

    /// Get the zoom speed range.
    #[must_use]
    pub fn zoom_speed_range(&self) -> std::ops::Range<u8> {
        use crate::capabilities::Zoom;
        match &self.profile {
            CameraProfile::PTZOpticsG2(_) => PTZOpticsG2::ZOOM_SPEED_RANGE,
            CameraProfile::SonyFR7(_) => SonyFR7::ZOOM_SPEED_RANGE,
            CameraProfile::GenericVisca(_) => GenericVisca::ZOOM_SPEED_RANGE,
        }
    }

    /// Get the optical zoom maximum value.
    #[must_use]
    pub fn optical_zoom_max(&self) -> u16 {
        use crate::capabilities::Zoom;
        match &self.profile {
            CameraProfile::PTZOpticsG2(_) => PTZOpticsG2::OPTICAL_ZOOM_MAX,
            CameraProfile::SonyFR7(_) => SonyFR7::OPTICAL_ZOOM_MAX,
            CameraProfile::GenericVisca(_) => GenericVisca::OPTICAL_ZOOM_MAX,
        }
    }

    /// Get the digital zoom maximum value.
    #[must_use]
    pub fn digital_zoom_max(&self) -> Option<u16> {
        use crate::capabilities::Zoom;
        match &self.profile {
            CameraProfile::PTZOpticsG2(_) => PTZOpticsG2::DIGITAL_ZOOM_MAX,
            CameraProfile::SonyFR7(_) => SonyFR7::DIGITAL_ZOOM_MAX,
            CameraProfile::GenericVisca(_) => GenericVisca::DIGITAL_ZOOM_MAX,
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
            CameraProfile::PTZOpticsG2(_) => (
                PTZOpticsG2::PAN_DEGREES_TO_UNITS,
                PTZOpticsG2::TILT_DEGREES_TO_UNITS,
            ),
            CameraProfile::SonyFR7(_) => (
                SonyFR7::PAN_DEGREES_TO_UNITS,
                SonyFR7::TILT_DEGREES_TO_UNITS,
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
            CameraProfile::SonyFR7(_) => (
                SonyFR7::PAN_DEGREES_TO_UNITS,
                SonyFR7::TILT_DEGREES_TO_UNITS,
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
            CameraProfile::SonyFR7(_) => SonyFR7::PAN_RANGE,
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
            CameraProfile::SonyFR7(_) => SonyFR7::TILT_RANGE,
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

    /// Get the ND filter mode.
    #[must_use]
    pub fn nd_filter_mode(&self) -> Option<crate::capabilities::NDFilterMode> {
        use crate::capabilities::NDFilter;
        match &self.profile {
            CameraProfile::PTZOpticsG2(_) => None, // PTZOpticsG2 doesn't support ND filters
            CameraProfile::SonyFR7(_) => Some(SonyFR7::ND_MODE),
            CameraProfile::GenericVisca(_) => None, // GenericVisca doesn't support ND filters
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
            CameraProfile::SonyFR7(p) => {
                // Use the NDFilterExt trait method for validation
                p.validate_nd_filter(level).map_err(Into::into)
            }
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
    pub fn r#async(self) -> crate::r#async::Camera {
        crate::r#async::Camera::new(self)
    }
}

// Note: The implementation of specific camera methods (zoom, pan_tilt, etc.) will be added
// through extension traits that provide high-level convenience methods on top of send_command.

// Internal trait for camera wrapper access
pub(crate) mod camera_like;

pub mod methods;
pub mod profiles;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_selection() {
        let profile = CameraModel::PTZOpticsG2.to_profile();
        assert_eq!(profile.model_name(), "PTZOptics G2");

        let profile = CameraModel::SonyFR7.to_profile();
        assert_eq!(profile.model_name(), "Sony FR7");

        let profile = CameraModel::default().to_profile();
        assert_eq!(profile.model_name(), "Generic VISCA Camera");
    }
}
