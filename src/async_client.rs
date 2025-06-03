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
    async_tcp_transport::AsyncTcpTransport, async_transport::AsyncViscaTransport,
    async_udp_transport::AsyncUdpTransport, session::ViscaSession, ViscaCommand, ViscaError,
    ViscaResponse,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinHandle;
use tokio::sync::oneshot;

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
    pending_commands: Arc<Mutex<HashMap<u8, oneshot::Sender<Result<ViscaResponse, ViscaError>>>>>,
    background_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    shutdown_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

#[cfg(feature = "async")]
impl AsyncViscaClient {
    /// Connect to a camera using UDP transport.
    pub async fn connect_udp(camera_addr: &str) -> Result<Self, ViscaError> {
        let addr: SocketAddr = camera_addr
            .parse()
            .map_err(|_| ViscaError::InvalidParameter("Invalid socket address".into()))?;

        let transport = AsyncUdpTransport::new(addr).await?;
        Self::new(Box::new(transport))
    }

    /// Connect to a camera using TCP transport.
    pub async fn connect_tcp(camera_addr: &str) -> Result<Self, ViscaError> {
        let addr: SocketAddr = camera_addr
            .parse()
            .map_err(|_| ViscaError::InvalidParameter("Invalid socket address".into()))?;

        let transport = AsyncTcpTransport::new(addr).await?;
        Self::new(Box::new(transport))
    }

    /// Create a new client with a custom transport.
    fn new(transport: Box<dyn AsyncViscaTransport>) -> Result<Self, ViscaError> {
        let transport = Arc::new(Mutex::new(transport));
        let session = Arc::new(Mutex::new(ViscaSession::new()));
        let semaphore = Arc::new(Semaphore::new(2)); // VISCA allows max 2 concurrent commands
        let pending_commands = Arc::new(Mutex::new(HashMap::new()));

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        // Clone for background task
        let transport_clone = Arc::clone(&transport);
        let session_clone = Arc::clone(&session);
        let pending_commands_clone = Arc::clone(&pending_commands);

        // Spawn background response handler
        let background_task = tokio::spawn(async move {
            Self::response_handler(transport_clone, session_clone, pending_commands_clone, shutdown_rx).await;
        });

        let inner = Arc::new(AsyncViscaClientInner {
            transport,
            session,
            semaphore,
            pending_commands,
            background_task: Arc::new(Mutex::new(Some(background_task))),
            shutdown_tx: Arc::new(Mutex::new(Some(shutdown_tx))),
        });

        Ok(Self { inner })
    }

    /// Send a command and wait for the response.
    pub async fn send(&self, command: &dyn ViscaCommand) -> Result<ViscaResponse, ViscaError> {
        // Acquire permit (blocks if 2 commands already in flight)
        let permit = self
            .inner
            .semaphore
            .acquire()
            .await
            .map_err(|_| ViscaError::Io(std::io::Error::other("Semaphore closed")))?;

        // Get response type
        let response_type = command.response_type();

        // Create channel for response
        let (tx, rx) = oneshot::channel();

        // Register command with session and get socket ID BEFORE sending
        let socket_id = {
            let mut session = self.inner.session.lock().await;
            let socket_id = session.assign_socket(response_type)?;
            
            // Register the response channel
            let mut pending = self.inner.pending_commands.lock().await;
            pending.insert(socket_id, tx);
            
            socket_id
        };

        // Send command through transport
        if let Err(e) = async {
            let mut transport = self.inner.transport.lock().await;
            transport.send_command(command).await
        }.await {
            // Clean up on send error
            let mut pending = self.inner.pending_commands.lock().await;
            pending.remove(&socket_id);
            
            let mut session = self.inner.session.lock().await;
            session.release_socket(socket_id);
            
            return Err(e);
        }

        log::debug!("Command assigned to socket {}", socket_id);

        // Wait for response with timeout
        let result = match tokio::time::timeout(Duration::from_secs(30), rx).await {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => {
                // Channel was closed without sending a response
                Err(ViscaError::Io(std::io::Error::other("Response channel closed")))
            }
            Err(_) => {
                // Timeout - clean up pending command
                let mut pending = self.inner.pending_commands.lock().await;
                pending.remove(&socket_id);
                Err(ViscaError::Timeout)
            }
        };

        // Release socket
        {
            let mut session = self.inner.session.lock().await;
            session.release_socket(socket_id);
        }

        // Permit is automatically released when dropped
        drop(permit);

        result
    }


    /// Background task that continuously reads responses from the transport.
    async fn response_handler(
        transport: Arc<Mutex<Box<dyn AsyncViscaTransport>>>,
        session: Arc<Mutex<ViscaSession>>,
        pending_commands: Arc<Mutex<HashMap<u8, oneshot::Sender<Result<ViscaResponse, ViscaError>>>>>,
        mut shutdown_rx: oneshot::Receiver<()>,
    ) {
        let mut consecutive_errors = 0;

        loop {
            // Check for shutdown signal
            if shutdown_rx.try_recv().is_ok() {
                log::debug!("Background task shutting down");
                break;
            }
            // Use timeout to prevent indefinite blocking
            let result = tokio::time::timeout(Duration::from_millis(100), async {
                let mut transport = transport.lock().await;
                transport.receive_response().await
            })
            .await;

            match result {
                Ok(Ok(responses)) => {
                    consecutive_errors = 0;

                    if !responses.is_empty() {
                        let mut session = session.lock().await;
                        for response in responses {
                            match session.process_response(&response) {
                                Ok(Some((socket_id, visca_response))) => {
                                    log::debug!(
                                        "Response for socket {}: {:?}",
                                        socket_id,
                                        visca_response
                                    );

                                    // Send result to the waiting command (except for ACK)
                                    match visca_response {
                                        ViscaResponse::Ack => {
                                            // ACK is handled internally by session, continue waiting
                                            log::debug!("ACK received for socket {}", socket_id);
                                        }
                                        response => {
                                            // Get the response channel
                                            let mut pending = pending_commands.lock().await;
                                            if let Some(tx) = pending.remove(&socket_id) {
                                                // Convert response to result
                                                let result = match response {
                                                    ViscaResponse::Error(e) => Err(e),
                                                    other => Ok(other),
                                                };
                                                
                                                // Send response (ignore if receiver dropped)
                                                let _ = tx.send(result);
                                            } else {
                                                log::warn!(
                                                    "Received response for unknown socket {}: {:?}",
                                                    socket_id,
                                                    response
                                                );
                                            }
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
                Ok(Err(ViscaError::Timeout)) => {
                    // Timeout is normal in async context, continue
                    consecutive_errors = 0;
                    continue;
                }
                Ok(Err(e)) => {
                    log::error!("Transport error: {:?}", e);
                    consecutive_errors += 1;

                    // Circuit breaker - if too many errors, exit the task
                    if consecutive_errors > 10 {
                        log::error!("Too many consecutive transport errors, shutting down background task");
                        
                        // Notify all pending commands of the error
                        let mut pending = pending_commands.lock().await;
                        for (_socket_id, tx) in pending.drain() {
                            let _ = tx.send(Err(ViscaError::Io(std::io::Error::other(
                                "Background task shut down due to repeated errors"
                            ))));
                        }
                        
                        break;
                    }
                    
                    // Progressive backoff
                    let backoff = Duration::from_millis(100 * consecutive_errors as u64);
                    tokio::time::sleep(backoff.min(Duration::from_secs(5))).await;
                }
                Err(_) => {
                    // Timeout on receive - this is normal, continue
                    consecutive_errors = 0;
                    continue;
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

#[cfg(feature = "async")]
impl Drop for AsyncViscaClient {
    fn drop(&mut self) {
        // Only perform cleanup if this is the last reference
        if Arc::strong_count(&self.inner) == 1 {
            // Create a runtime handle to perform async cleanup
            let handle = tokio::runtime::Handle::try_current();
            if let Ok(handle) = handle {
                let inner = Arc::clone(&self.inner);
                handle.spawn(async move {
                    // Send shutdown signal
                    if let Some(tx) = inner.shutdown_tx.lock().await.take() {
                        let _ = tx.send(());
                        log::debug!("Shutdown signal sent to background task");
                    }
                    
                    // Wait for background task to complete with timeout
                    if let Some(task) = inner.background_task.lock().await.take() {
                        match tokio::time::timeout(Duration::from_secs(5), task).await {
                            Ok(_) => log::debug!("Background task completed"),
                            Err(_) => {
                                log::warn!("Background task did not complete within timeout");
                            }
                        }
                    }
                });
            }
        }
    }
}
