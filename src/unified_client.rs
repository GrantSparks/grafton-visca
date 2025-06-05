//! Unified VISCA client for v0.4.0 - async-first with blocking façade.
//!
//! This module provides a single `ViscaClient` that works in both blocking
//! and async contexts, replacing the previous split-brain approach.

use crate::{
    session::ViscaSession,
    transport::{BlockingAdapter, Transport},
    ViscaCommand, ViscaError, ViscaResponse,
};
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};

#[cfg(feature = "blocking-client")]
use crate::transport::{TcpTransport as BlockingTcpTransport, UdpTransport as BlockingUdpTransport};
#[cfg(feature = "async-client")]
use crate::transport::{AsyncTcpTransport, AsyncUdpTransport};

/// Maximum number of concurrent commands (PTZOptics G2 limitation).
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
    
    /// Session management for command sequencing
    #[allow(dead_code)] // Will be used in Phase C for command sequencing
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

    // B2: Unified constructors
    
    /// Connect to a camera using UDP transport.
    ///
    /// This constructor automatically selects the appropriate transport based on
    /// the enabled features and the calling context.
    #[cfg(feature = "blocking-client")]
    pub fn connect_udp(camera_addr: &str) -> Result<Self, ViscaError> {
        let transport = BlockingUdpTransport::new(camera_addr).map_err(ViscaError::Io)?;
        let adapter = BlockingAdapter(transport);
        Ok(Self::new_from_variant(TransportVariant::BlockingUdp(adapter)))
    }

    /// Connect to a camera using TCP transport.
    ///
    /// This constructor automatically selects the appropriate transport based on
    /// the enabled features and the calling context.
    #[cfg(feature = "blocking-client")]
    pub fn connect_tcp(camera_addr: &str) -> Result<Self, ViscaError> {
        let transport = BlockingTcpTransport::new(camera_addr).map_err(ViscaError::Io)?;
        let adapter = BlockingAdapter(transport);
        Ok(Self::new_from_variant(TransportVariant::BlockingTcp(adapter)))
    }

    /// Connect to a camera using UDP transport (async).
    #[cfg(feature = "async-client")]
    pub async fn connect_udp_async(camera_addr: &str) -> Result<Self, ViscaError> {
        let transport = AsyncUdpTransport::new(camera_addr).await?;
        Ok(Self::new_from_variant(TransportVariant::AsyncUdp(transport)))
    }

    /// Connect to a camera using TCP transport (async).
    #[cfg(feature = "async-client")]
    pub async fn connect_tcp_async(camera_addr: &str) -> Result<Self, ViscaError> {
        let transport = AsyncTcpTransport::new(camera_addr).await?;
        Ok(Self::new_from_variant(TransportVariant::AsyncTcp(transport)))
    }
    
    // B3: Concurrency control is handled by the semaphore field
    
    // B4: Blocking façade with smart runtime handling
    
    /// Send a command and wait for the response (blocking).
    ///
    /// This method provides a blocking interface that works in both sync and async contexts:
    /// - If called from within a Tokio runtime, uses block_in_place to avoid blocking the runtime
    /// - If called from outside a runtime, creates a temporary runtime
    /// - For blocking transports, calls them directly when possible
    #[cfg(feature = "blocking-client")]
    pub fn send(&self, command: &dyn ViscaCommand) -> Result<ViscaResponse, ViscaError> {
        // Clone self to move into the async block
        let client = self.clone();
        
        // Try to use existing Tokio runtime if available
        match tokio::runtime::Handle::try_current() {
            Ok(_) => {
                // We're inside a Tokio runtime, use block_in_place to avoid blocking it
                tokio::task::block_in_place(move || {
                    // Create a new runtime for the blocking operation
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|e| ViscaError::Io(std::io::Error::other(e)))?;
                    rt.block_on(client.send_async(command))
                })
            }
            Err(_) => {
                // No runtime available, create a temporary one
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| ViscaError::Io(std::io::Error::other(e)))?;
                rt.block_on(client.send_async(command))
            }
        }
    }
    
    // B5: Async façade
    
    /// Send a command and wait for the response (async).
    ///
    /// This method:
    /// - Acquires a semaphore permit to enforce concurrency limits
    /// - Sends the command through the transport
    /// - Waits for and returns the response
    pub async fn send_async(&self, command: &dyn ViscaCommand) -> Result<ViscaResponse, ViscaError> {
        // Acquire permit for concurrency control
        let _permit = self.semaphore.acquire().await
            .map_err(|_| ViscaError::InvalidParameter("Semaphore closed".into()))?;
        
        // Lock transport and send command
        let mut transport = self.transport.lock().await;
        let responses = match &mut *transport {
            #[cfg(feature = "blocking-client")]
            TransportVariant::BlockingUdp(t) => {
                t.send_command(command).await?;
                t.receive_response().await?
            }
            #[cfg(feature = "blocking-client")]
            TransportVariant::BlockingTcp(t) => {
                t.send_command(command).await?;
                t.receive_response().await?
            }
            #[cfg(feature = "async-client")]
            TransportVariant::AsyncUdp(t) => {
                t.send_command(command).await?;
                t.receive_response().await?
            }
            #[cfg(feature = "async-client")]
            TransportVariant::AsyncTcp(t) => {
                t.send_command(command).await?;
                t.receive_response().await?
            }
        };
        
        parse_response(responses)
    }

    /// Check if the camera connection is healthy.
    pub async fn is_healthy(&self) -> Result<bool, ViscaError> {
        use crate::command::InquiryCommand;
        
        match self.send_async(&InquiryCommand::Power).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}


/// Parse response frames into a ViscaResponse.
fn parse_response(frames: Vec<Vec<u8>>) -> Result<ViscaResponse, ViscaError> {
    // For now, just parse the last frame
    // TODO: Handle multi-frame responses and response types properly in Phase C
    if let Some(frame) = frames.last() {
        // Check if it's an ACK/completion (no data) or an inquiry response
        if frame.len() == 3 && frame[0] == 0x90 && frame[1] == 0x50 && frame[2] == 0xFF {
            // ACK/Completion response
            Ok(ViscaResponse::Ack)
        } else if frame.len() == 3 && frame[0] == 0x90 && frame[1] == 0x51 && frame[2] == 0xFF {
            // Completion response
            Ok(ViscaResponse::Completion)
        } else if frame.len() >= 3 && frame[0] == 0x90 && (frame[1] & 0x60) == 0x60 {
            // Error response - extract error code
            let error_code = frame[1] & 0x0F;
            Ok(ViscaResponse::Error(ViscaError::from_code(error_code)))
        } else {
            // For now, just return completion for other responses
            // TODO: Properly parse based on command type
            Ok(ViscaResponse::Completion)
        }
    } else {
        Err(ViscaError::Io(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "No response frames received"
        )))
    }
}