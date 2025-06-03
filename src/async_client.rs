//! Async VISCA client implementation for non-blocking camera control.
//! 
//! This module provides an asynchronous interface to control VISCA cameras,
//! allowing concurrent command execution while respecting the VISCA protocol's
//! two-socket limitation.
//! 
//! # Example
//! 
//! ```no_run
//! # #[cfg(feature = "async")]
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use grafton_visca::{AsyncViscaClient, ViscaResponse};
//! use grafton_visca::command::{PowerCommand, power::Power};
//! 
//! // Connect to camera
//! let camera = AsyncViscaClient::connect_udp("192.168.1.100:5678").await?;
//! 
//! // Send command
//! let response = camera.send(&PowerCommand { power: Power::On }).await?;
//! 
//! match response {
//!     ViscaResponse::Completion => println!("Camera powered on"),
//!     ViscaResponse::Error(e) => println!("Error: {:?}", e),
//!     _ => println!("Unexpected response"),
//! }
//! # Ok(())
//! # }
//! ```

use crate::{
    async_transport::AsyncViscaTransport,
    async_udp_transport::AsyncUdpTransport,
    async_tcp_transport::AsyncTcpTransport,
    session::ViscaSession,
    ViscaCommand, ViscaError, ViscaResponse,
};
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinHandle;
use std::net::SocketAddr;
use std::time::Duration;
use std::collections::HashMap;

/// Async VISCA client for non-blocking camera control.
/// 
/// This client manages concurrent command execution with proper socket management
/// and response correlation. It enforces the VISCA two-socket limitation.
/// 
/// # Features
/// 
/// - Concurrent command execution (up to 2 simultaneous commands)
/// - Automatic socket management and response correlation
/// - Background response handling
/// - Thread-safe (can be cloned and shared across tasks)
/// 
/// # Example
/// 
/// ```no_run
/// # #[cfg(feature = "async")]
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use grafton_visca::{AsyncViscaClient, ViscaResponse};
/// use grafton_visca::command::InquiryCommand;
/// 
/// let camera = AsyncViscaClient::connect_udp("192.168.1.100:5678").await?;
/// 
/// // Query camera status
/// match camera.send(&InquiryCommand::ZoomPosition).await? {
///     ViscaResponse::InquiryResponse(resp) => println!("Status: {:?}", resp),
///     _ => println!("Unexpected response"),
/// }
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "async")]
pub struct AsyncViscaClient {
    inner: Arc<AsyncViscaClientInner>,
}

#[cfg(feature = "async")]
struct AsyncViscaClientInner {
    transport: Arc<Mutex<Box<dyn AsyncViscaTransport>>>,
    session: Arc<Mutex<ViscaSession>>,
    semaphore: Arc<Semaphore>,
    results: Arc<Mutex<HashMap<u8, Result<ViscaResponse, ViscaError>>>>,
    _background_task: JoinHandle<()>,
}

#[cfg(feature = "async")]
impl AsyncViscaClient {
    /// Connect to a camera using UDP transport.
    pub async fn connect_udp(camera_addr: &str) -> Result<Self, ViscaError> {
        let addr: SocketAddr = camera_addr.parse()
            .map_err(|_| ViscaError::InvalidParameter("Invalid socket address".into()))?;
        
        let transport = AsyncUdpTransport::new(addr).await?;
        Self::new(Box::new(transport))
    }
    
    /// Connect to a camera using TCP transport.
    pub async fn connect_tcp(camera_addr: &str) -> Result<Self, ViscaError> {
        let addr: SocketAddr = camera_addr.parse()
            .map_err(|_| ViscaError::InvalidParameter("Invalid socket address".into()))?;
        
        let transport = AsyncTcpTransport::new(addr).await?;
        Self::new(Box::new(transport))
    }
    
    /// Create a new client with a custom transport.
    fn new(transport: Box<dyn AsyncViscaTransport>) -> Result<Self, ViscaError> {
        let transport = Arc::new(Mutex::new(transport));
        let session = Arc::new(Mutex::new(ViscaSession::new()));
        let semaphore = Arc::new(Semaphore::new(2)); // VISCA allows max 2 concurrent commands
        let results = Arc::new(Mutex::new(HashMap::new()));
        
        // Clone for background task
        let transport_clone = Arc::clone(&transport);
        let session_clone = Arc::clone(&session);
        let results_clone = Arc::clone(&results);
        
        // Spawn background response handler
        let background_task = tokio::spawn(async move {
            Self::response_handler(transport_clone, session_clone, results_clone).await;
        });
        
        let inner = Arc::new(AsyncViscaClientInner {
            transport,
            session,
            semaphore,
            results,
            _background_task: background_task,
        });
        
        Ok(Self { inner })
    }
    
    /// Send a command and wait for the response.
    pub async fn send(&self, command: &dyn ViscaCommand) -> Result<ViscaResponse, ViscaError> {
        // Acquire permit (blocks if 2 commands already in flight)
        let permit = self.inner.semaphore.acquire().await
            .map_err(|_| ViscaError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Semaphore closed"
            )))?;
        
        // Get response type
        let response_type = command.response_type();
        
        // Send command through transport
        {
            let mut transport = self.inner.transport.lock().await;
            transport.send_command(command).await?;
        }
        
        // Register command with session and get socket ID
        let socket_id = {
            let mut session = self.inner.session.lock().await;
            session.assign_socket(response_type)?
        };
        
        log::debug!("Command assigned to socket {}", socket_id);
        
        // Wait for completion
        let result = self.wait_for_completion(socket_id).await;
        
        // Release socket
        {
            let mut session = self.inner.session.lock().await;
            session.release_socket(socket_id);
        }
        
        // Permit is automatically released when dropped
        drop(permit);
        
        result
    }
    
    /// Wait for a command to complete.
    async fn wait_for_completion(&self, socket_id: u8) -> Result<ViscaResponse, ViscaError> {
        let mut interval = tokio::time::interval(Duration::from_millis(10));
        let timeout = Duration::from_secs(30);
        let start = tokio::time::Instant::now();
        
        loop {
            interval.tick().await;
            
            if start.elapsed() > timeout {
                // Clean up on timeout
                let mut session = self.inner.session.lock().await;
                session.release_socket(socket_id);
                return Err(ViscaError::Timeout);
            }
            
            // Check if result is available
            {
                let mut results = self.inner.results.lock().await;
                if let Some(result) = results.remove(&socket_id) {
                    return result;
                }
            }
            
            // Yield to allow other tasks to run
            tokio::task::yield_now().await;
        }
    }
    
    /// Background task that continuously reads responses from the transport.
    async fn response_handler(
        transport: Arc<Mutex<Box<dyn AsyncViscaTransport>>>,
        session: Arc<Mutex<ViscaSession>>,
        results: Arc<Mutex<HashMap<u8, Result<ViscaResponse, ViscaError>>>>,
    ) {
        let mut consecutive_errors = 0;
        
        loop {
            let result = {
                let mut transport = transport.lock().await;
                transport.receive_response().await
            };
            
            match result {
                Ok(responses) => {
                    consecutive_errors = 0;
                    
                    if !responses.is_empty() {
                        let mut session = session.lock().await;
                        for response in responses {
                            match session.process_response(&response) {
                                Ok(Some((socket_id, visca_response))) => {
                                    log::debug!("Response for socket {}: {:?}", socket_id, visca_response);
                                    
                                    // Store result for the waiting command
                                    match visca_response {
                                        ViscaResponse::Completion => {
                                            let mut results = results.lock().await;
                                            results.insert(socket_id, Ok(ViscaResponse::Completion));
                                        }
                                        ViscaResponse::Error(e) => {
                                            let mut results = results.lock().await;
                                            results.insert(socket_id, Err(e));
                                        }
                                        ViscaResponse::InquiryResponse(resp) => {
                                            let mut results = results.lock().await;
                                            results.insert(socket_id, Ok(ViscaResponse::InquiryResponse(resp)));
                                        }
                                        ViscaResponse::Ack => {
                                            // ACK is handled internally by session, continue waiting
                                            log::debug!("ACK received for socket {}", socket_id);
                                        }
                                        ViscaResponse::Unknown(data) => {
                                            log::warn!("Unknown response for socket {}: {:02X?}", socket_id, data);
                                            let mut results = results.lock().await;
                                            results.insert(socket_id, Ok(ViscaResponse::Unknown(data)));
                                        }
                                    }
                                }
                                Ok(None) => {
                                    // Response processed but no result yet (e.g., ACK)
                                }
                                Err(e) => {
                                    log::error!("Error processing response: {:?}", e);
                                }
                            }
                        }
                    }
                }
                Err(ViscaError::Timeout) => {
                    // Timeout is normal in async context, continue
                    consecutive_errors = 0;
                    continue;
                }
                Err(e) => {
                    log::error!("Transport error: {:?}", e);
                    consecutive_errors += 1;
                    
                    // If we get too many consecutive errors, slow down
                    if consecutive_errors > 5 {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        }
    }
}

#[cfg(feature = "async")]
impl Clone for AsyncViscaClient {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}