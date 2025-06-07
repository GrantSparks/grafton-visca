//! Async VISCA client implementation for non-blocking camera control.

// Standard library imports
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

// Third-party imports
use tokio::sync::{oneshot, watch, Mutex, Semaphore};

// Crate imports
use crate::{
    async_tcp_transport::AsyncTcpTransport, async_transport::AsyncViscaTransport,
    async_udp_transport::AsyncUdpTransport, session::ViscaSession, ViscaCommand, ViscaError,
    ViscaResponse,
};

// Type aliases for clarity
type ResponseSender = oneshot::Sender<Result<ViscaResponse, ViscaError>>;
type PendingCommands = HashMap<u8, ResponseSender>;

/// Async VISCA client for non-blocking camera control.
///
/// # Deprecated
/// This client is deprecated in favor of the unified `ViscaClient` which supports
/// both blocking and async APIs.
#[cfg(feature = "async-client")]
#[derive(Clone)]
#[deprecated(
    since = "0.4.0",
    note = "Use the unified `ViscaClient` with async methods instead"
)]
pub struct AsyncViscaClient {
    transport: Arc<Mutex<Box<dyn AsyncViscaTransport>>>,
    session: Arc<Mutex<ViscaSession>>,
    semaphore: Arc<Semaphore>,
    pending_commands: Arc<Mutex<PendingCommands>>,
    shutdown: Arc<watch::Sender<()>>,
}

#[cfg(feature = "async-client")]
impl AsyncViscaClient {
    /// Connect to a camera using UDP transport.
    pub async fn connect_udp(camera_addr: &str) -> Result<Self, ViscaError> {
        let addr = camera_addr
            .parse::<SocketAddr>()
            .map_err(|_| ViscaError::InvalidParameter("Invalid socket address".into()))?;

        let transport = AsyncUdpTransport::new(addr).await?;
        Ok(Self::new(Box::new(transport)))
    }

    /// Connect to a camera using TCP transport.
    pub async fn connect_tcp(camera_addr: &str) -> Result<Self, ViscaError> {
        let addr = camera_addr
            .parse::<SocketAddr>()
            .map_err(|_| ViscaError::InvalidParameter("Invalid socket address".into()))?;

        let transport = AsyncTcpTransport::new(addr).await?;
        Ok(Self::new(Box::new(transport)))
    }

    /// Create a new client with a custom transport.
    fn new(transport: Box<dyn AsyncViscaTransport>) -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(());

        let client = Self {
            transport: Arc::new(Mutex::new(transport)),
            session: Arc::new(Mutex::new(ViscaSession::new())),
            semaphore: Arc::new(Semaphore::new(2)), // VISCA allows max 2 concurrent commands
            pending_commands: Arc::new(Mutex::new(HashMap::new())),
            shutdown: Arc::new(shutdown_tx),
        };

        // Spawn background response handler
        let client_clone = client.clone();
        tokio::spawn(async move {
            client_clone.response_handler(shutdown_rx).await;
        });

        client
    }

    /// Send a command and wait for the response.
    pub async fn send(&self, command: &dyn ViscaCommand) -> Result<ViscaResponse, ViscaError> {
        // Acquire permit (blocks if 2 commands already in flight)
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| ViscaError::InvalidState("Semaphore closed".into()))?;

        let response_type = command.response_type();
        let (tx, rx) = oneshot::channel();

        // Register command before sending to avoid race condition
        let socket_id = {
            let mut session = self.session.lock().await;
            let socket_id = session.assign_socket(response_type)?;

            let mut pending = self.pending_commands.lock().await;
            pending.insert(socket_id, tx);

            socket_id
        };

        // Send command
        let send_result = {
            let mut transport = self.transport.lock().await;
            transport.send_command(command).await
        };

        if let Err(e) = send_result {
            // Clean up on error
            self.cleanup_socket(socket_id).await;
            return Err(e);
        }

        log::debug!("Command sent on socket {socket_id}");

        // Wait for response
        let result = match tokio::time::timeout(Duration::from_secs(30), rx).await {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => Err(ViscaError::InvalidState("Response channel closed".into())),
            Err(_) => {
                self.pending_commands.lock().await.remove(&socket_id);
                Err(ViscaError::Timeout)
            }
        };

        // Always release socket
        self.session.lock().await.release_socket(socket_id);

        result
    }

    /// Clean up socket on error
    async fn cleanup_socket(&self, socket_id: u8) {
        self.pending_commands.lock().await.remove(&socket_id);
        self.session.lock().await.release_socket(socket_id);
    }

    /// Background task that reads responses
    async fn response_handler(&self, mut shutdown: watch::Receiver<()>) {
        let mut consecutive_errors = 0;

        loop {
            // Check shutdown signal
            tokio::select! {
                _ = shutdown.changed() => {
                    log::debug!("Shutdown signal received");
                    break;
                }
                result = self.receive_and_process() => {
                    match result {
                        Ok(()) => consecutive_errors = 0,
                        Err(ViscaError::Timeout) => consecutive_errors = 0,
                        Err(e) => {
                            log::error!("Transport error: {:?}", e);
                            consecutive_errors += 1;

                            if consecutive_errors > 10 {
                                log::error!("Too many errors, shutting down");
                                self.notify_all_pending_error().await;
                                break;
                            }

                            // Exponential backoff on errors
                            let backoff_ms = 100 * consecutive_errors.min(50);
                            tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                        }
                    }
                }
            }
        }
    }

    /// Receive and process one batch of responses
    async fn receive_and_process(&self) -> Result<(), ViscaError> {
        // Short timeout to keep the loop responsive
        let responses = tokio::time::timeout(Duration::from_millis(500), async {
            let mut transport = self.transport.lock().await;
            transport.receive_response().await
        })
        .await
        .map_err(|_| ViscaError::Timeout)??;

        if responses.is_empty() {
            return Ok(());
        }

        let mut session = self.session.lock().await;

        for response in responses {
            match session.process_response(&response) {
                Ok(Some((socket_id, visca_response))) => {
                    log::debug!("Response for socket {socket_id}: {visca_response:?}");

                    // Skip ACK responses
                    if matches!(visca_response, ViscaResponse::Ack) {
                        continue;
                    }

                    // Send to waiting command
                    let tx_opt = self.pending_commands.lock().await.remove(&socket_id);
                    if let Some(tx) = tx_opt {
                        let result = match visca_response {
                            ViscaResponse::Error(e) => Err(e),
                            other => Ok(other),
                        };
                        let _ = tx.send(result);
                    }
                }
                Ok(None) => {
                    // Intermediate response (e.g., ACK)
                }
                Err(e) => {
                    log::error!("Error processing response: {:?}", e);
                }
            }
        }

        Ok(())
    }

    /// Notify all pending commands of error
    async fn notify_all_pending_error(&self) {
        let mut pending = self.pending_commands.lock().await;
        for (_, tx) in pending.drain() {
            let _ = tx.send(Err(ViscaError::InvalidState(
                "Background task shutdown".into(),
            )));
        }
    }

    /// Gracefully shutdown the client
    pub fn shutdown(self) {
        let _ = self.shutdown.send(());
        // Background task will exit on next iteration
    }
}
