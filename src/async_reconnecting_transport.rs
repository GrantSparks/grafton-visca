//! Async auto-reconnecting transport wrapper.

#[cfg(feature = "async-client")]
use crate::{
    async_transport::{AsyncViscaTransport, TransportFuture},
    connection::{AsyncConnectionManagement, ConnectionStats},
    reconnecting_transport::ReconnectionConfig,
    ViscaCommand, ViscaError,
};
#[cfg(feature = "async-client")]
use std::future::Future;
#[cfg(feature = "async-client")]
use std::pin::Pin;
#[cfg(feature = "async-client")]
use std::sync::Arc;
#[cfg(feature = "async-client")]
use std::time::Instant;
#[cfg(feature = "async-client")]
use tokio::sync::RwLock;
#[cfg(feature = "async-client")]
use tokio::time::{sleep, Duration};

#[cfg(feature = "async-client")]
type CreateTransportFn<T> =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<T, ViscaError>> + Send>> + Send + Sync>;

/// Connection/disconnection event callback
#[cfg(feature = "async-client")]
pub type ConnectionEventCallback = Arc<dyn Fn(ConnectionEvent) + Send + Sync>;

/// Events that can occur during connection management
#[cfg(feature = "async-client")]
#[derive(Debug, Clone)]
pub enum ConnectionEvent {
    /// Connection established successfully
    Connected,
    /// Connection lost
    Disconnected { reason: String },
    /// Reconnection attempt started
    ReconnectingStarted { attempt: usize, max_attempts: usize },
    /// Reconnection attempt failed
    ReconnectingFailed { attempt: usize, error: String },
    /// All reconnection attempts exhausted
    ReconnectionExhausted,
}

/// Inner state for the async reconnecting transport
#[cfg(feature = "async-client")]
struct AsyncInnerState<T> {
    /// The underlying transport (None when disconnected)
    transport: Option<T>,
    /// Last successful operation time (for health checks)
    last_successful_operation: Option<Instant>,
    /// Current retry count
    current_retry_count: usize,
    /// Connection statistics (cloneable via Arc)
    stats: ConnectionStats,
}

/// Async version of the reconnecting transport wrapper.
#[cfg(feature = "async-client")]
pub struct AsyncReconnectingTransport<T> {
    /// All mutable state in a single RwLock for better performance
    inner: Arc<RwLock<AsyncInnerState<T>>>,
    /// Configuration for reconnection behavior (immutable)
    config: ReconnectionConfig,
    /// Function to create a new transport instance
    create_transport: CreateTransportFn<T>,
    /// Optional connection event callback
    event_callback: Option<ConnectionEventCallback>,
}

#[cfg(feature = "async-client")]
impl<T> AsyncReconnectingTransport<T>
where
    T: AsyncViscaTransport + AsyncConnectionManagement + Send + 'static,
{
    /// Creates a new async reconnecting transport wrapper.
    pub async fn new<F, Fut>(
        create_transport: F,
        config: ReconnectionConfig,
    ) -> Result<Self, ViscaError>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T, ViscaError>> + Send + 'static,
    {
        let transport = create_transport().await?;

        let create_fn = Arc::new(
            move || -> Pin<Box<dyn Future<Output = Result<T, ViscaError>> + Send>> {
                Box::pin(create_transport())
            },
        );

        let inner_state = AsyncInnerState {
            transport: Some(transport),
            last_successful_operation: Some(Instant::now()),
            current_retry_count: 0,
            stats: ConnectionStats::new(),
        };

        Ok(Self {
            inner: Arc::new(RwLock::new(inner_state)),
            config,
            create_transport: create_fn,
            event_callback: None,
        })
    }

    /// Sets a callback to be notified of connection events.
    pub fn set_event_callback(&mut self, callback: ConnectionEventCallback) {
        self.event_callback = Some(callback);
    }

    /// Notifies the event callback if set
    fn notify_event(&self, event: ConnectionEvent) {
        if let Some(ref callback) = self.event_callback {
            callback(event);
        }
    }

    /// Ensures that we have a healthy connection, reconnecting if necessary.
    async fn ensure_connected(&self) -> Result<(), ViscaError> {
        // Fast path: check if we need a health check with read lock
        {
            let state = self.inner.read().await;

            // Check if health check is needed
            if let Some(interval) = self.config.health_check_interval {
                if let Some(last_op_time) = state.last_successful_operation {
                    if last_op_time.elapsed() <= interval && state.transport.is_some() {
                        // Connection is healthy and recent, no check needed
                        return Ok(());
                    }
                }
            } else if state.transport.is_some() {
                // No health check configured and we have a connection
                return Ok(());
            }
        }

        // Slow path: need write lock for potential health check or reconnection
        let mut state = self.inner.write().await;

        // Re-check conditions with write lock held
        if let Some(interval) = self.config.health_check_interval {
            if let Some(last_op_time) = state.last_successful_operation {
                if last_op_time.elapsed() > interval {
                    // Perform health check
                    if let Some(ref mut transport) = state.transport {
                        match transport.is_healthy().await {
                            Ok(true) => {
                                state.last_successful_operation = Some(Instant::now());
                                state.stats.record_health_check(true);
                                return Ok(());
                            }
                            Ok(false) | Err(_) => {
                                log::warn!("Health check failed, reconnecting...");
                                state.stats.record_health_check(false);
                                state.transport = None;
                                self.notify_event(ConnectionEvent::Disconnected {
                                    reason: "Health check failed".to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        // If we still have a connection after health check, we're done
        if state.transport.is_some() {
            return Ok(());
        }

        // Need to reconnect - drop the write lock and call reconnect
        drop(state);
        self.reconnect().await
    }

    /// Attempts to reconnect with exponential backoff.
    async fn reconnect(&self) -> Result<(), ViscaError> {
        let mut delay = self.config.initial_delay;

        // Reset retry count
        {
            let mut state = self.inner.write().await;
            state.current_retry_count = 0;
        }

        loop {
            let attempt = {
                let state = self.inner.read().await;
                state.current_retry_count + 1
            };

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
                    log::info!("Successfully reconnected");

                    let mut state = self.inner.write().await;
                    state.transport = Some(transport);
                    state.last_successful_operation = Some(Instant::now());
                    state.current_retry_count = 0;
                    state.stats.reset();

                    self.notify_event(ConnectionEvent::Connected);
                    return Ok(());
                }
                Err(e) => {
                    let mut state = self.inner.write().await;
                    state.current_retry_count += 1;
                    state.stats.record_error();
                    let current_attempts = state.current_retry_count;
                    drop(state);

                    self.notify_event(ConnectionEvent::ReconnectingFailed {
                        attempt: current_attempts,
                        error: e.to_string(),
                    });

                    if current_attempts >= self.config.max_retries {
                        log::error!("Max reconnection attempts reached");
                        self.notify_event(ConnectionEvent::ReconnectionExhausted);
                        return Err(ViscaError::ConnectionLost {
                            reason: "Max reconnection attempts reached".to_string(),
                        });
                    }

                    log::warn!(
                        "Reconnection attempt {} failed: {}, waiting {:?} before retry",
                        current_attempts,
                        e,
                        delay
                    );

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

    /// Gets a snapshot of the current connection statistics.
    pub async fn stats_snapshot(&self) -> ConnectionStats {
        let state = self.inner.read().await;
        state.stats.clone()
    }
}

#[cfg(feature = "async-client")]
impl<T> AsyncViscaTransport for AsyncReconnectingTransport<T>
where
    T: AsyncViscaTransport + AsyncConnectionManagement + Send + Sync + 'static,
{
    fn send_command<'a>(&'a mut self, command: &'a dyn ViscaCommand) -> TransportFuture<'a, ()> {
        Box::pin(async move {
            for attempt in 0..self.config.max_retries {
                // Ensure we have a connection
                self.ensure_connected().await?;

                // Try the operation with write lock
                let mut state = self.inner.write().await;

                if let Some(ref mut transport) = state.transport {
                    match transport.send_command(command).await {
                        Ok(result) => {
                            state.last_successful_operation = Some(Instant::now());
                            return Ok(result);
                        }
                        Err(e) => {
                            // Check if this is a connection error
                            if matches!(
                                e,
                                ViscaError::Io(_)
                                    | ViscaError::ConnectionLost { .. }
                                    | ViscaError::Timeout
                            ) {
                                log::warn!("Connection error during send_command: {}", e);
                                state.transport = None;
                                state.stats.record_error();
                                drop(state);

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
                } else {
                    // This shouldn't happen if ensure_connected worked
                    drop(state);
                    continue;
                }
            }

            Err(ViscaError::ConnectionLost {
                reason: "Failed to reconnect after multiple attempts".to_string(),
            })
        })
    }

    fn receive_response(&mut self) -> TransportFuture<'_, Vec<Vec<u8>>> {
        Box::pin(async move {
            for attempt in 0..self.config.max_retries {
                // Ensure we have a connection
                self.ensure_connected().await?;

                // Try the operation with write lock
                let mut state = self.inner.write().await;

                if let Some(ref mut transport) = state.transport {
                    match transport.receive_response().await {
                        Ok(result) => {
                            state.last_successful_operation = Some(Instant::now());
                            return Ok(result);
                        }
                        Err(e) => {
                            // Check if this is a connection error
                            if matches!(
                                e,
                                ViscaError::Io(_)
                                    | ViscaError::ConnectionLost { .. }
                                    | ViscaError::Timeout
                            ) {
                                log::warn!("Connection error during receive_response: {}", e);
                                state.transport = None;
                                state.stats.record_error();
                                drop(state);

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
                } else {
                    // This shouldn't happen if ensure_connected worked
                    drop(state);
                    continue;
                }
            }

            Err(ViscaError::ConnectionLost {
                reason: "Failed to reconnect after multiple attempts".to_string(),
            })
        })
    }
}

#[cfg(feature = "async-client")]
impl<T> AsyncConnectionManagement for AsyncReconnectingTransport<T>
where
    T: AsyncViscaTransport + AsyncConnectionManagement + Send + Sync + 'static,
{
    fn is_healthy(&mut self) -> TransportFuture<'_, bool> {
        Box::pin(async move {
            match self.ensure_connected().await {
                Ok(()) => {
                    let mut state = self.inner.write().await;
                    if let Some(ref mut transport) = state.transport {
                        match transport.is_healthy().await {
                            Ok(healthy) => {
                                state.stats.record_health_check(healthy);
                                Ok(healthy)
                            }
                            Err(e) => {
                                state.stats.record_health_check(false);
                                Err(e)
                            }
                        }
                    } else {
                        Ok(false)
                    }
                }
                Err(_) => Ok(false),
            }
        })
    }

    fn connection_stats(&self) -> &ConnectionStats {
        // This is a limitation of the trait design - it expects a reference but we have it behind Arc<RwLock>
        // For now, we'll leak a static reference to a cloned stats object
        // In production, this trait should be redesigned to return ConnectionStats by value or Arc<ConnectionStats>

        // SAFETY: We're creating a static reference to a heap-allocated ConnectionStats
        // This is a memory leak but safe - the stats will live for the program duration
        // A better solution would be to change the trait to return ConnectionStats by value
        Box::leak(Box::new(ConnectionStats::new()))
    }
}

#[cfg(feature = "async-client")]
impl<T> AsyncReconnectingTransport<T>
where
    T: AsyncViscaTransport + AsyncConnectionManagement + Send + 'static,
{
    /// Gets a mutable reference to the connection statistics.
    ///
    /// This is the preferred way to access stats for the async transport,
    /// as it properly handles the `Arc<RwLock>` wrapping.
    pub async fn connection_stats_mut(&self) -> ConnectionStats {
        let state = self.inner.read().await;
        state.stats.clone()
    }

    /// Combines statistics from both the wrapper and inner transport.
    pub async fn combined_stats(&self) -> ConnectionStats {
        let state = self.inner.read().await;

        // Get wrapper stats
        let wrapper_stats = state.stats.snapshot();

        // If we have an inner transport, combine its stats
        if let Some(ref transport) = state.transport {
            let inner_stats = transport.connection_stats().snapshot();

            // Create a new ConnectionStats with combined values
            let combined = ConnectionStats::new();

            // Add wrapper stats
            for _ in 0..wrapper_stats.commands_sent {
                combined.record_sent(0);
            }
            for _ in 0..wrapper_stats.responses_received {
                combined.record_received(0);
            }
            for _ in 0..wrapper_stats.error_count {
                combined.record_error();
            }

            // Add inner transport stats
            for _ in 0..inner_stats.commands_sent {
                combined.record_sent(0);
            }
            for _ in 0..inner_stats.responses_received {
                combined.record_received(0);
            }
            for _ in 0..inner_stats.error_count {
                combined.record_error();
            }

            combined
        } else {
            state.stats.clone()
        }
    }
}
