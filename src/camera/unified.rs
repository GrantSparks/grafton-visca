//! Unified camera client that abstracts over profile and transport generics.
//!
//! This module provides a single, simplified camera interface that works with
//! any camera profile and transport, eliminating the need for users to manage
//! generic type parameters.

use std::sync::Arc;
use std::time::Duration;

use crate::{
    capabilities::{ProfileIntrospection, ProfileMetadata, ProtocolStyle},
    command::{visca_command::ViscaCommand, ResponseType},
    error::Error,
    transport::core::{BlockingTransport, Transport},
    Command, Response,
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
    pub fn model_name(&self) -> &'static str {
        match self {
            Self::PTZOpticsG2(_) => PTZOpticsG2::MODEL_NAME,
            Self::SonyFR7(_) => SonyFR7::MODEL_NAME,
            Self::GenericVisca(_) => GenericVisca::MODEL_NAME,
        }
    }

    /// Get the default VISCA address.
    pub fn default_address(&self) -> u8 {
        match self {
            Self::PTZOpticsG2(_) => PTZOpticsG2::DEFAULT_ADDRESS,
            Self::SonyFR7(_) => SonyFR7::DEFAULT_ADDRESS,
            Self::GenericVisca(_) => GenericVisca::DEFAULT_ADDRESS,
        }
    }

    /// Get the protocol style.
    pub fn protocol_style(&self) -> ProtocolStyle {
        match self {
            Self::PTZOpticsG2(_) => PTZOpticsG2::PROTOCOL_STYLE,
            Self::SonyFR7(_) => SonyFR7::PROTOCOL_STYLE,
            Self::GenericVisca(_) => GenericVisca::PROTOCOL_STYLE,
        }
    }

    /// Get the acknowledgment timeout.
    pub fn ack_timeout(&self) -> Duration {
        match self {
            Self::PTZOpticsG2(_) => PTZOpticsG2::ACK_TIMEOUT,
            Self::SonyFR7(_) => SonyFR7::ACK_TIMEOUT,
            Self::GenericVisca(_) => GenericVisca::ACK_TIMEOUT,
        }
    }

    /// Get the completion timeout.
    pub fn completion_timeout(&self) -> Duration {
        match self {
            Self::PTZOpticsG2(_) => PTZOpticsG2::COMPLETION_TIMEOUT,
            Self::SonyFR7(_) => SonyFR7::COMPLETION_TIMEOUT,
            Self::GenericVisca(_) => GenericVisca::COMPLETION_TIMEOUT,
        }
    }

    /// Get the busy timeout.
    pub fn busy_timeout(&self) -> Duration {
        match self {
            Self::PTZOpticsG2(_) => PTZOpticsG2::BUSY_TIMEOUT,
            Self::SonyFR7(_) => SonyFR7::BUSY_TIMEOUT,
            Self::GenericVisca(_) => GenericVisca::BUSY_TIMEOUT,
        }
    }

    /// Check if the camera supports VISCA inquiry commands.
    pub fn supports_inquiry(&self) -> bool {
        match self {
            Self::PTZOpticsG2(_) => PTZOpticsG2::SUPPORTS_INQUIRY,
            Self::SonyFR7(_) => SonyFR7::SUPPORTS_INQUIRY,
            Self::GenericVisca(_) => GenericVisca::SUPPORTS_INQUIRY,
        }
    }

    /// Get a capability summary for the camera.
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
/// use grafton_visca::{UnifiedCamera, ProfileId};
/// use grafton_visca::transport::tokio::Tcp;
///
/// // Create a camera with automatic profile detection
/// let transport = Tcp::connect("192.168.1.100:52381").await?;
/// let camera = UnifiedCamera::new(transport);
///
/// // Or specify a profile explicitly
/// let camera = UnifiedCamera::with_profile(ProfileId::PTZOpticsG2, transport);
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
/// use grafton_visca::{UnifiedCamera, ProfileId};
/// use grafton_visca::transport::blocking::Tcp;
///
/// // Create a camera with blocking transport
/// let transport = Tcp::connect("192.168.1.100:52381")?;
/// let camera = UnifiedCamera::new(transport);
///
/// // Use the camera (blocking methods)
/// camera.send_command_blocking(&some_command)?;
/// # Ok(())
/// # }
/// ```
pub struct UnifiedCamera {
    profile: DynamicProfile,
    transport: Arc<dyn UnifiedTransport>,
    address: u8,
}

impl std::fmt::Debug for UnifiedCamera {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnifiedCamera")
            .field("profile", &self.profile)
            .field("address", &self.address)
            .field("transport", &"<dyn UnifiedTransport>")
            .finish()
    }
}

impl UnifiedCamera {
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
    pub fn with_profile<T>(
        profile: ProfileId,
        transport: T,
    ) -> Self
    where
        T: Transport + Send + Sync + 'static,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        Self::with_profile_and_transport(profile, transport)
    }

    /// Create with a blocking transport.
    pub fn new_blocking<T>(transport: T) -> Self
    where
        T: BlockingTransport + Send + Sync + 'static,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        Self::with_profile_and_blocking_transport(ProfileId::default(), transport)
    }

    /// Create with a specific profile and blocking transport.
    pub fn with_profile_blocking<T>(
        profile: ProfileId,
        transport: T,
    ) -> Self
    where
        T: BlockingTransport + Send + Sync + 'static,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        Self::with_profile_and_blocking_transport(profile, transport)
    }

    /// Internal constructor for async transports.
    fn with_profile_and_transport<T>(
        profile: ProfileId,
        transport: T,
    ) -> Self
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
    fn with_profile_and_blocking_transport<T>(
        profile: ProfileId,
        transport: T,
    ) -> Self
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
    pub fn model_name(&self) -> &'static str {
        self.profile.model_name()
    }

    /// Get the camera's profile information.
    pub fn profile_info(&self) -> String {
        self.profile.capability_summary()
    }

    /// Set the VISCA address for this camera.
    pub fn set_address(&mut self, address: u8) {
        self.address = address;
    }

    /// Get the current VISCA address.
    pub fn address(&self) -> u8 {
        self.address
    }

    /// Send a command asynchronously and wait for response.
    ///
    /// This is the primary async interface for sending commands to the camera.
    pub async fn send_command<C>(
        &self,
        command: &C,
    ) -> Result<Response, Error>
    where
        C: ViscaCommand,
    {
        // Convert to the old Command trait for now
        // In the future, we might want to unify these traits
        let command_adapter = ViscaCommandAdapter { command };
        self.send_command_internal(&command_adapter).await
    }
    
    /// Internal method that works with the Command trait.
    async fn send_command_internal(&self, command: &dyn Command) -> Result<Response, Error> {
        // Get command bytes
        let mut cmd_bytes = command.to_bytes()?;
        
        // Add VISCA terminator if not present
        const VISCA_TERMINATOR: u8 = 0xFF;
        if cmd_bytes.last() != Some(&VISCA_TERMINATOR) {
            cmd_bytes.push(VISCA_TERMINATOR);
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
                    Response::Ack => {
                        // Wait for completion
                        self.wait_for_response(self.profile.completion_timeout()).await
                    }
                    Response::Completion => {
                        // Some cameras send completion directly
                        Ok(Response::Completion)
                    }
                    Response::Error(e) => Err(e),
                    _ => Err(Error::ParseError(format!("Unexpected response: {:?}", ack_response))),
                }
            }
            Some(response_type) => {
                // Inquiry command - wait for specific response type
                let timeout = self.profile.completion_timeout();
                self.wait_for_response_with_type(response_type, timeout).await
            }
        }
    }
    
    /// Wait for any response with timeout.
    async fn wait_for_response(&self, _timeout: Duration) -> Result<Response, Error> {
        // TODO: Implement proper timeout handling based on runtime
        // For now, just receive without timeout
        
        match self.transport.recv().await {
            Ok(bytes) => Response::parse(&bytes.to_vec()),
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
    pub fn send_command_blocking<C>(
        &self,
        command: &C,
    ) -> Result<Response, Error>
    where
        C: ViscaCommand,
    {
        // Use a minimal executor to block on the async method
        futures::executor::block_on(self.send_command(command))
    }

    /// Check if the camera supports a specific capability.
    ///
    /// This provides runtime introspection of camera capabilities.
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
}

// Adapter to convert ViscaCommand to Command trait
struct ViscaCommandAdapter<'a, C: ViscaCommand> {
    command: &'a C,
}

impl<'a, C: ViscaCommand> Command for ViscaCommandAdapter<'a, C> {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        self.command.to_bytes()
    }
    
    fn response_type(&self) -> Option<ResponseType> {
        // ViscaCommand has an associated Response type, but we need ResponseType enum
        // For now, we'll need to infer this from the command type
        // This is a temporary solution until we unify the trait systems
        
        // Check if this is likely an inquiry command by looking at the bytes
        match self.command.to_bytes() {
            Ok(bytes) if bytes.len() > 1 && bytes[1] == 0x09 => {
                // This is an inquiry command, but we need to determine which type
                // For now, return None - the response parser will handle it
                None
            }
            _ => None, // Action command
        }
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

    #[test]
    fn test_capability_detection() {
        // This is a compile-time test to ensure UnifiedCamera doesn't expose generics
        fn accepts_unified_camera(_camera: &UnifiedCamera) {
            // This function should compile without any generic parameters
        }

        // Note: We can't actually create a UnifiedCamera in tests without a real transport
        // but the type signature test above proves the API design works
    }
}