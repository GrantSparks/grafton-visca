//! Unified VISCA client for v0.4.0 - async-first with blocking façade.
//!
//! This module provides a single `ViscaClient` that works in both blocking
//! and async contexts, replacing the previous split-brain approach.

// Standard library imports
use std::sync::Arc;

// Crate imports
use crate::{
    ptz_builder::PtzBuilder,
    session::ViscaSession,
    sync_primitives::{Mutex, Semaphore, SemaphoreExt},
    ViscaCommand, ViscaError, ViscaResponse,
};

// Feature-gated imports - Blocking client
#[cfg(feature = "blocking-client")]
use crate::transport::{
    BlockingAdapter, TcpTransport as BlockingTcpTransport, UdpTransport as BlockingUdpTransport,
};

// Feature-gated imports - Async client
#[cfg(feature = "async-client")]
use crate::transport::{AsyncTcpTransport, AsyncUdpTransport, Transport};

/// Maximum number of concurrent commands (`PTZOptics` G2 limitation).
const MAX_CONCURRENT_COMMANDS: usize = 2;

/// Internal transport variant that supports both blocking and async transports.
enum TransportVariant {
    /// Blocking UDP transport wrapped in an adapter
    #[cfg(feature = "blocking-client")]
    BlockingUdp(BlockingAdapter<BlockingUdpTransport>),

    /// Blocking TCP transport wrapped in an adapter
    #[cfg(feature = "blocking-client")]
    BlockingTcp(BlockingAdapter<BlockingTcpTransport>),

    /// Native async UDP transport
    #[cfg(feature = "async-client")]
    AsyncUdp(AsyncUdpTransport),

    /// Native async TCP transport
    #[cfg(feature = "async-client")]
    AsyncTcp(AsyncTcpTransport),

}

/// Unified VISCA client with async-first design and blocking façade.
///
/// This client provides a single API surface for both blocking and async usage,
/// with automatic runtime management for blocking operations.
#[derive(Clone)]
pub struct ViscaClient {
    /// The underlying transport (blocking or async)
    transport: Arc<Mutex<TransportVariant>>,

    /// Session management for command sequencing and socket assignment
    session: Arc<Mutex<ViscaSession>>,

    /// Concurrency control (max 2 concurrent commands)
    semaphore: Arc<Semaphore>,
}

impl ViscaClient {
    /// Creates a new client from a transport variant.
    fn new_from_variant(variant: TransportVariant) -> Self {
        Self {
            transport: Arc::new(Mutex::new(variant)),
            session: Arc::new(Mutex::new(ViscaSession::new())),
            semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_COMMANDS)),
        }
    }

    /// Connect to a camera using UDP transport.
    ///
    /// This constructor automatically selects the appropriate transport based on
    /// the enabled features and the calling context.
    #[cfg(feature = "blocking-client")]
    pub fn connect_udp(camera_addr: &str) -> Result<Self, ViscaError> {
        let transport = BlockingUdpTransport::new(camera_addr).map_err(ViscaError::Io)?;
        let adapter = BlockingAdapter(transport);
        Ok(Self::new_from_variant(TransportVariant::BlockingUdp(
            adapter,
        )))
    }

    /// Connect to a camera using TCP transport.
    ///
    /// This constructor automatically selects the appropriate transport based on
    /// the enabled features and the calling context.
    #[cfg(feature = "blocking-client")]
    pub fn connect_tcp(camera_addr: &str) -> Result<Self, ViscaError> {
        let transport = BlockingTcpTransport::new(camera_addr).map_err(ViscaError::Io)?;
        let adapter = BlockingAdapter(transport);
        Ok(Self::new_from_variant(TransportVariant::BlockingTcp(
            adapter,
        )))
    }

    /// Connect to a camera using UDP transport (async).
    #[cfg(feature = "async-client")]
    pub async fn connect_udp_async(camera_addr: &str) -> Result<Self, ViscaError> {
        let transport = AsyncUdpTransport::new(camera_addr).await?;
        Ok(Self::new_from_variant(TransportVariant::AsyncUdp(
            transport,
        )))
    }

    /// Connect to a camera using TCP transport (async).
    #[cfg(feature = "async-client")]
    pub async fn connect_tcp_async(camera_addr: &str) -> Result<Self, ViscaError> {
        let transport = AsyncTcpTransport::new(camera_addr).await?;
        Ok(Self::new_from_variant(TransportVariant::AsyncTcp(
            transport,
        )))
    }


    /// Send a command and wait for the response (blocking).
    ///
    /// Sends a command and waits for the response (blocking).
    ///
    /// This method provides a blocking interface that works in both sync and async contexts:
    /// - For async-enabled builds:
    ///   - If called from within a Tokio runtime, uses `block_in_place` to avoid blocking the runtime
    ///   - If called from outside a runtime, creates a temporary runtime
    /// - For blocking-only builds:
    ///   - Directly calls the blocking implementation
    #[cfg(feature = "blocking-client")]
    pub fn send(&self, command: &dyn ViscaCommand) -> Result<ViscaResponse, ViscaError> {
        #[cfg(feature = "async-client")]
        {
            // Clone self to move into the async block
            let client = self.clone();

            // Try to use existing Tokio runtime if available
            match tokio::runtime::Handle::try_current() {
                Ok(_) => tokio::task::block_in_place(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|e| ViscaError::Io(std::io::Error::other(e)))?;
                    rt.block_on(client.send_async(command))
                }),
                Err(_) => {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|e| ViscaError::Io(std::io::Error::other(e)))?;
                    rt.block_on(client.send_async(command))
                }
            }
        }

        #[cfg(not(feature = "async-client"))]
        {
            self.send_blocking(command)
        }
    }

    #[cfg(not(feature = "async-client"))]
    fn send_blocking(&self, command: &dyn ViscaCommand) -> Result<ViscaResponse, ViscaError> {
        let _permit = self.semaphore.acquire_permit();

        // Acquire session lock and assign socket
        let socket_id = {
            let mut session = self.session.lock();
            session.assign_socket(command.response_type())?
        };

        let result = (|| {
            // Send command
            {
                let mut transport = self.transport.lock();
                match &mut *transport {
                    #[cfg(feature = "blocking-client")]
                    TransportVariant::BlockingUdp(t) => {
                        use crate::transport::BlockingTransport;
                        t.0.send_command_blocking(command)?;
                    }
                    #[cfg(feature = "blocking-client")]
                    TransportVariant::BlockingTcp(t) => {
                        use crate::transport::BlockingTransport;
                        t.0.send_command_blocking(command)?;
                    }
                    #[cfg(all(feature = "async-client", not(feature = "blocking-client")))]
                    _ => unreachable!(
                        "Async transports should not be present in blocking-only builds"
                    ),
                }
            }

            // Wait for response with proper session management
            self.wait_for_response_blocking(socket_id)
        })();

        // Always release socket
        {
            let mut session = self.session.lock();
            session.release_socket(socket_id);
        }

        result
    }

    /// Wait for a response on a specific socket with proper session management (blocking).
    #[cfg(not(feature = "async-client"))]
    fn wait_for_response_blocking(&self, socket_id: u8) -> Result<ViscaResponse, ViscaError> {
        use ViscaResponse::{Ack, Completion, Error, InquiryResponse};

        loop {
            let responses = {
                let mut transport = self.transport.lock();
                match &mut *transport {
                    #[cfg(feature = "blocking-client")]
                    TransportVariant::BlockingUdp(t) => {
                        use crate::transport::BlockingTransport;
                        t.0.receive_response_blocking()?
                    }
                    #[cfg(feature = "blocking-client")]
                    TransportVariant::BlockingTcp(t) => {
                        use crate::transport::BlockingTransport;
                        t.0.receive_response_blocking()?
                    }
                    #[cfg(all(feature = "async-client", not(feature = "blocking-client")))]
                    _ => unreachable!(
                        "Async transports should not be present in blocking-only builds"
                    ),
                }
            };

            for response in responses {
                let mut session = self.session.lock();
                if let Some((resp_socket_id, parsed_response)) =
                    session.process_response(&response)?
                {
                    if resp_socket_id != socket_id {
                        log::debug!(
                            "Response for socket {} (expected {})",
                            resp_socket_id,
                            socket_id
                        );
                        continue;
                    }

                    match parsed_response {
                        Ack => {
                            log::debug!("ACK received for socket {}", socket_id);
                            // Continue waiting for completion
                        }
                        Completion => {
                            log::debug!("Completion received for socket {}", socket_id);
                            return Ok(Completion);
                        }
                        InquiryResponse(inquiry) => {
                            log::debug!("Inquiry response received for socket {}: {:?}", socket_id, inquiry);
                            return Ok(InquiryResponse(inquiry));
                        }
                        Error(err) => {
                            log::error!("Command error on socket {}: {:?}", socket_id, err);
                            return Err(err);
                        }
                        ViscaResponse::Unknown(_) => {
                            log::debug!("Unexpected response: {:?}", parsed_response);
                        }
                    }
                }
            }
        }
    }

    /// Sends a command and waits for the response (async).
    ///
    /// This method:
    /// - Acquires a semaphore permit to enforce concurrency limits  
    /// - Uses session management for proper VISCA socket assignment
    /// - Sends the command through the transport
    /// - Waits for and returns the response with proper ACK/completion tracking
    #[cfg(feature = "async-client")]
    pub async fn send_async(
        &self,
        command: &dyn ViscaCommand,
    ) -> Result<ViscaResponse, ViscaError> {
        let _permit = self.semaphore.acquire_permit().await?;

        // Acquire session lock and assign socket
        let socket_id = {
            let mut session = self.session.lock().await;
            session.assign_socket(command.response_type())?
        };

        let result = async {
            // Send command
            {
                let mut transport = self.transport.lock().await;
                match &mut *transport {
                    #[cfg(feature = "blocking-client")]
                    TransportVariant::BlockingUdp(t) => {
                        t.send_command(command).await?;
                    }
                    #[cfg(feature = "blocking-client")]
                    TransportVariant::BlockingTcp(t) => {
                        t.send_command(command).await?;
                    }
                    #[cfg(feature = "async-client")]
                    TransportVariant::AsyncUdp(t) => {
                        t.send_command(command).await?;
                    }
                    #[cfg(feature = "async-client")]
                    TransportVariant::AsyncTcp(t) => {
                        t.send_command(command).await?;
                    }
                }
            }

            // Wait for response with proper session management
            self.wait_for_response_async(socket_id).await
        }
        .await;

        // Always release socket
        {
            let mut session = self.session.lock().await;
            session.release_socket(socket_id);
        }

        result
    }

    /// Wait for a response on a specific socket with proper session management.
    #[cfg(feature = "async-client")]
    async fn wait_for_response_async(&self, socket_id: u8) -> Result<ViscaResponse, ViscaError> {
        use ViscaResponse::{Ack, Completion, Error, InquiryResponse};

        loop {
            let responses = {
                let mut transport = self.transport.lock().await;
                match &mut *transport {
                    #[cfg(feature = "blocking-client")]
                    TransportVariant::BlockingUdp(t) => t.receive_response().await?,
                    #[cfg(feature = "blocking-client")]
                    TransportVariant::BlockingTcp(t) => t.receive_response().await?,
                    #[cfg(feature = "async-client")]
                    TransportVariant::AsyncUdp(t) => t.receive_response().await?,
                    #[cfg(feature = "async-client")]
                    TransportVariant::AsyncTcp(t) => t.receive_response().await?,
                }
            };

            for response in responses {
                let mut session = self.session.lock().await;
                if let Some((resp_socket_id, parsed_response)) =
                    session.process_response(&response)?
                {
                    if resp_socket_id != socket_id {
                        log::debug!(
                            "Response for socket {} (expected {})",
                            resp_socket_id,
                            socket_id
                        );
                        continue;
                    }

                    match parsed_response {
                        Ack => {
                            log::debug!("ACK received for socket {}", socket_id);
                            // Continue waiting for completion
                        }
                        Completion => {
                            log::debug!("Completion received for socket {}", socket_id);
                            return Ok(Completion);
                        }
                        InquiryResponse(inquiry) => {
                            log::debug!("Inquiry response received for socket {}: {:?}", socket_id, inquiry);
                            return Ok(InquiryResponse(inquiry));
                        }
                        Error(err) => {
                            log::error!("Command error on socket {}: {:?}", socket_id, err);
                            return Err(err);
                        }
                        ViscaResponse::Unknown(_) => {
                            log::debug!("Unexpected response: {:?}", parsed_response);
                        }
                    }
                }
            }
        }
    }

    /// Check if the camera connection is healthy.
    #[cfg(feature = "async-client")]
    pub async fn is_healthy(&self) -> Result<bool, ViscaError> {
        use crate::command::InquiryCommand;

        match self.send_async(&InquiryCommand::Power).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Check if the camera connection is healthy (blocking).
    #[cfg(feature = "blocking-client")]
    pub fn is_healthy_blocking(&self) -> Result<bool, ViscaError> {
        use crate::command::InquiryCommand;

        match self.send(&InquiryCommand::Power) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

/// Extension trait for PTZ builder functionality on `Arc<ViscaClient>`.
pub trait ViscaClientPtzExt {
    /// Create a PTZ builder for fluent command sequences.
    ///
    /// Returns a builder that allows chaining multiple PTZ commands together
    /// and executing them either sequentially or concurrently.
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "blocking-client")] {
    /// # use grafton_visca::{ViscaClient, ViscaClientPtzExt};
    /// # use grafton_visca::command::pan_tilt::{PanSpeed, TiltSpeed, PanTiltDirection};
    /// # use std::sync::Arc;
    /// # let client = Arc::new(ViscaClient::connect_udp("192.168.1.100:5678").unwrap());
    /// // Build and execute a PTZ sequence
    /// client.ptz()
    ///     .pan_tilt_home()
    ///     .zoom_in(5).unwrap()
    ///     .focus_auto()
    ///     .execute_sequential()
    ///     .unwrap();
    /// # }
    /// ```
    fn ptz(self) -> PtzBuilder;
}

impl ViscaClientPtzExt for Arc<ViscaClient> {
    fn ptz(self) -> PtzBuilder {
        PtzBuilder::new(self)
    }
}

// Implement ViscaDevice to support extension traits
#[cfg(feature = "blocking-client")]
impl crate::ViscaDevice for ViscaClient {
    fn execute_command(&mut self, command: &dyn ViscaCommand) -> Result<ViscaResponse, ViscaError> {
        self.send(command)
    }
}

