//! Auto-reconnecting transport wrapper that provides automatic connection recovery.
//!
//! This module implements a wrapper around any `Transport` that automatically
//! handles connection failures and reconnection with configurable retry policies.

// Standard library imports
use std::sync::Arc;
use std::time::{Duration, Instant};

// Third-party crate imports
use tokio::sync::Mutex;
use tokio::time::sleep;

// Workspace / local-crate imports
use crate::{
    connection::ConnectionStats,
    error::Error as ViscaError,
    transport::{Transport, TransportFuture},
    Command,
};

/// Type alias for transport creation function
type TransportCreator<T> = Arc<
    dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, ViscaError>> + Send>>
        + Send
        + Sync,
>;

/// Configuration for automatic reconnection behavior.
#[derive(Debug, Clone, Copy)]
pub struct ReconnectionConfig {
    /// Maximum number of reconnection attempts before giving up
    pub max_retries: usize,
    /// Initial delay between reconnection attempts
    pub initial_delay: Duration,
    /// Maximum delay between reconnection attempts
    pub max_delay: Duration,
    /// Factor by which to increase the delay after each failed attempt
    pub backoff_factor: f64,
    /// Interval for periodic health checks (None to disable)
    pub health_check_interval: Option<Duration>,
}

impl Default for ReconnectionConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_factor: 2.0,
            health_check_interval: Some(Duration::from_secs(30)),
        }
    }
}

/// Connection/disconnection event callback
pub type ConnectionEventCallback = Arc<dyn Fn(ConnectionEvent) + Send + Sync>;

/// Events that can occur during connection management
#[derive(Debug, Clone)]
pub enum ConnectionEvent {
    /// Connection established successfully
    Connected,
    /// Connection lost
    Disconnected {
        /// Reason for disconnection
        reason: String,
    },
    /// Reconnection attempt started
    ReconnectingStarted {
        /// Current attempt number
        attempt: usize,
        /// Maximum number of attempts configured
        max_attempts: usize,
    },
    /// Reconnection attempt failed
    ReconnectingFailed {
        /// Attempt number that failed
        attempt: usize,
        /// Error message describing the failure
        error: String,
    },
    /// All reconnection attempts exhausted
    ReconnectionExhausted,
}

/// Internal state for the reconnecting transport
struct TransportState<T> {
    /// The underlying transport (None when disconnected)
    inner: Option<T>,
    /// Last successful operation time (for health checks)
    last_successful_operation: Option<Instant>,
    /// Current retry count
    current_retry_count: usize,
    /// Connection statistics
    stats: ConnectionStats,
}

/// A transport wrapper that automatically reconnects on connection failures.
///
/// This wrapper can be used with any transport that implements the `Transport` trait.
pub struct ReconnectingTransport<T> {
    /// State protected by mutex for async safety
    state: Arc<Mutex<TransportState<T>>>,
    /// Configuration for reconnection behavior
    config: Arc<ReconnectionConfig>,
    /// Function to create a new transport instance
    create_transport: TransportCreator<T>,
    /// Optional connection event callback
    event_callback: Option<ConnectionEventCallback>,
}

impl<T> std::fmt::Debug for ReconnectingTransport<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReconnectingTransport")
            .field("config", &self.config)
            .field("has_event_callback", &self.event_callback.is_some())
            .finish_non_exhaustive()
    }
}

impl<T> ReconnectingTransport<T>
where
    T: Transport + Send + 'static,
{
    /// Creates a new reconnecting transport wrapper.
    ///
    /// # Arguments
    /// * `create_transport` - An async function that creates a new transport instance
    /// * `config` - Configuration for reconnection behavior
    ///
    /// # Errors
    /// Returns a `ViscaError` if the initial transport creation fails.
    pub async fn new<F, Fut>(
        create_transport: F,
        config: ReconnectionConfig,
    ) -> Result<Self, ViscaError>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<T, ViscaError>> + Send + 'static,
    {
        let transport = create_transport().await?;

        let state = TransportState {
            inner: Some(transport),
            last_successful_operation: Some(Instant::now()),
            current_retry_count: 0,
            stats: ConnectionStats::new(),
        };

        Ok(Self {
            state: Arc::new(Mutex::new(state)),
            config: Arc::new(config),
            create_transport: Arc::new(move || Box::pin(create_transport())),
            event_callback: None,
        })
    }

    /// Sets a callback for connection events.
    pub fn set_event_callback(&mut self, callback: ConnectionEventCallback) {
        self.event_callback = Some(callback);
    }

    /// Notifies about a connection event.
    fn notify_event(&self, event: ConnectionEvent) {
        if let Some(ref callback) = self.event_callback {
            callback(event);
        }
    }

    /// Ensures the transport is connected, attempting to reconnect if necessary.
    ///
    /// # Errors
    /// Returns `ViscaError` if the health check fails, if reconnection attempts
    /// are exhausted, or if the transport creation function fails.
    async fn ensure_connected(&self) -> Result<(), ViscaError> {
        let mut state = self.state.lock().await;

        // Check if we need to reconnect
        if state.inner.is_some() {
            // Check if health check interval has elapsed
            if let Some(interval) = self.config.health_check_interval {
                if let Some(last_op) = state.last_successful_operation {
                    if last_op.elapsed() > interval {
                        // Perform health check
                        if let Some(ref mut transport) = state.inner {
                            // Try a simple operation to check health
                            if transport.receive_response().await.is_ok() {
                                state.last_successful_operation = Some(Instant::now());
                                return Ok(());
                            }
                            log::warn!("Health check failed, will reconnect");
                            state.inner = None;
                        }
                    } else {
                        return Ok(());
                    }
                } else {
                    return Ok(());
                }
            } else {
                return Ok(());
            }
        }

        // Need to reconnect
        state.current_retry_count = 0;
        let mut delay = self.config.initial_delay;

        // Drop the lock before the reconnection loop
        drop(state);

        for attempt in 1..=self.config.max_retries {
            log::info!(
                "Attempting to reconnect (attempt {}/{})",
                attempt,
                self.config.max_retries
            );

            self.notify_event(ConnectionEvent::ReconnectingStarted {
                attempt,
                max_attempts: self.config.max_retries,
            });

            match (self.create_transport)().await {
                Ok(transport) => {
                    let mut state = self.state.lock().await;
                    state.inner = Some(transport);
                    state.current_retry_count = 0;
                    state.last_successful_operation = Some(Instant::now());
                    drop(state);
                    self.notify_event(ConnectionEvent::Connected);
                    log::info!("Reconnection successful");
                    return Ok(());
                }
                Err(e) => {
                    log::error!("Reconnection attempt {attempt} failed: {e}");
                    self.notify_event(ConnectionEvent::ReconnectingFailed {
                        attempt,
                        error: e.to_string(),
                    });

                    if attempt < self.config.max_retries {
                        log::info!("Waiting {delay:?} before next reconnection attempt");
                        sleep(delay).await;

                        // Calculate next delay with exponential backoff
                        delay = Duration::from_secs_f64(
                            (delay.as_secs_f64() * self.config.backoff_factor)
                                .min(self.config.max_delay.as_secs_f64()),
                        );
                    }
                }
            }
        }

        self.notify_event(ConnectionEvent::ReconnectionExhausted);
        Err(ViscaError::ConnectionLost {
            reason: "Failed to reconnect after maximum attempts".to_string(),
        })
    }

    /// Gets a snapshot of the current connection statistics.
    #[must_use]
    pub async fn stats_snapshot(&self) -> ConnectionStats {
        let state = self.state.lock().await;
        state.stats.clone()
    }

    /// Checks if the transport is currently connected.
    #[must_use]
    pub async fn is_connected(&self) -> bool {
        let state = self.state.lock().await;
        state.inner.is_some()
    }

    /// Forces a disconnection (for testing purposes).
    #[doc(hidden)]
    pub async fn force_disconnect(&self) {
        let mut state = self.state.lock().await;
        state.inner = None;
    }
}

impl<T> Transport for ReconnectingTransport<T>
where
    T: Transport + Send + 'static,
{
    /// Sends a VISCA command through the reconnecting transport wrapper.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command serialization fails, if all
    /// reconnection attempts are exhausted, or if the underlying transport
    /// encounters a non-recoverable error.
    fn send_command<'a>(&'a mut self, command: &'a dyn Command) -> TransportFuture<'a, ()> {
        Box::pin(async move {
            // Try operation with retry on connection errors
            for attempt in 0..self.config.max_retries {
                // Ensure connected
                self.ensure_connected().await?;

                // Try to send
                let mut state = self.state.lock().await;
                if let Some(ref mut transport) = state.inner {
                    match transport.send_command(command).await {
                        Ok(()) => {
                            state.last_successful_operation = Some(Instant::now());
                            state.stats.record_sent(command.to_bytes()?.len());
                            return Ok(());
                        }
                        Err(e) => {
                            // Check if this is a connection error
                            if matches!(
                                e,
                                ViscaError::Io(_)
                                    | ViscaError::ConnectionLost { .. }
                                    | ViscaError::Timeout
                            ) {
                                log::warn!("Connection error during send_command: {e}");
                                state.inner = None;
                                state.stats.record_error();
                                drop(state); // Release lock before notifying

                                self.notify_event(ConnectionEvent::Disconnected {
                                    reason: e.to_string(),
                                });

                                if attempt < self.config.max_retries - 1 {
                                    continue;
                                }
                            }
                            return Err(e);
                        }
                    }
                }
            }

            Err(ViscaError::ConnectionLost {
                reason: "Failed to send command after multiple attempts".to_string(),
            })
        })
    }

    /// Receives VISCA response frames through the reconnecting transport wrapper.
    ///
    /// # Errors
    /// Returns `ViscaError` if all reconnection attempts are exhausted,
    /// if the underlying transport encounters a non-recoverable error,
    /// or if a timeout occurs during reception.
    fn receive_response(&mut self) -> TransportFuture<'_, Vec<Vec<u8>>> {
        Box::pin(async move {
            // Try operation with retry on connection errors
            for attempt in 0..self.config.max_retries {
                // Ensure connected
                self.ensure_connected().await?;

                // Try to receive
                let mut state = self.state.lock().await;
                if let Some(ref mut transport) = state.inner {
                    match transport.receive_response().await {
                        Ok(responses) => {
                            state.last_successful_operation = Some(Instant::now());
                            for response in &responses {
                                state.stats.record_received(response.len());
                            }
                            return Ok(responses);
                        }
                        Err(e) => {
                            // Check if this is a connection error
                            if matches!(
                                e,
                                ViscaError::Io(_)
                                    | ViscaError::ConnectionLost { .. }
                                    | ViscaError::Timeout
                            ) {
                                log::warn!("Connection error during receive_response: {e}");
                                state.inner = None;
                                state.stats.record_error();
                                drop(state); // Release lock before notifying

                                self.notify_event(ConnectionEvent::Disconnected {
                                    reason: e.to_string(),
                                });

                                if attempt < self.config.max_retries - 1 {
                                    continue;
                                }
                            }
                            return Err(e);
                        }
                    }
                }
            }

            Err(ViscaError::ConnectionLost {
                reason: "Failed to receive response after multiple attempts".to_string(),
            })
        })
    }
}

// Clone implementation
impl<T> Clone for ReconnectingTransport<T>
where
    T: Transport + Send + 'static,
{
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            config: self.config.clone(),
            create_transport: self.create_transport.clone(),
            event_callback: self.event_callback.clone(),
        }
    }
}
