//! Auto-reconnecting transport wrapper that provides automatic connection recovery.
//!
//! This module implements a wrapper around any `ViscaTransport` that automatically
//! handles connection failures and reconnection with configurable retry policies.

use crate::{
    connection::{ConnectionManagement, ConnectionStats},
    ViscaCommand, ViscaError, ViscaResponse, ViscaTransport,
};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Configuration for automatic reconnection behavior.
#[derive(Debug, Clone)]
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
    Disconnected { reason: String },
    /// Reconnection attempt started
    ReconnectingStarted { attempt: usize, max_attempts: usize },
    /// Reconnection attempt failed
    ReconnectingFailed { attempt: usize, error: String },
    /// All reconnection attempts exhausted
    ReconnectionExhausted,
}

/// A transport wrapper that automatically reconnects on connection failures.
///
/// This wrapper can be used with any transport that implements both `ViscaTransport`
/// and `ConnectionManagement` traits.
pub struct ReconnectingTransport<T> {
    /// The underlying transport (None when disconnected)
    inner: Option<T>,
    /// Configuration for reconnection behavior
    config: ReconnectionConfig,
    /// Function to create a new transport instance
    create_transport: Box<dyn Fn() -> Result<T, ViscaError> + Send>,
    /// Last successful operation time (for health checks)
    last_successful_operation: Option<Instant>,
    /// Current retry count
    current_retry_count: usize,
    /// Connection statistics
    stats: ConnectionStats,
    /// Optional connection event callback
    event_callback: Option<ConnectionEventCallback>,
}

impl<T> ReconnectingTransport<T>
where
    T: ViscaTransport + ConnectionManagement,
{
    /// Creates a new reconnecting transport wrapper.
    ///
    /// # Arguments
    /// * `create_transport` - A function that creates a new transport instance
    /// * `config` - Configuration for reconnection behavior
    pub fn new<F>(create_transport: F, config: ReconnectionConfig) -> Result<Self, ViscaError>
    where
        F: Fn() -> Result<T, ViscaError> + Send + 'static,
    {
        let transport = create_transport()?;
        Ok(Self {
            inner: Some(transport),
            config,
            create_transport: Box::new(create_transport),
            last_successful_operation: Some(Instant::now()),
            current_retry_count: 0,
            stats: ConnectionStats::new(),
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
    fn ensure_connected(&mut self) -> Result<(), ViscaError> {
        // Check if we need a health check
        if let Some(interval) = self.config.health_check_interval {
            if let Some(last_op) = self.last_successful_operation {
                if last_op.elapsed() > interval {
                    // Perform health check
                    if let Some(ref mut transport) = self.inner {
                        match transport.is_healthy() {
                            Ok(true) => {
                                self.last_successful_operation = Some(Instant::now());
                                self.stats.record_health_check(true);
                                return Ok(());
                            }
                            Ok(false) | Err(_) => {
                                log::warn!("Health check failed, reconnecting...");
                                self.stats.record_health_check(false);
                                self.inner = None;
                                self.notify_event(ConnectionEvent::Disconnected {
                                    reason: "Health check failed".to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        // If we have a connection and no health check is needed, we're done
        if self.inner.is_some() {
            return Ok(());
        }

        // Try to reconnect
        self.reconnect()
    }

    /// Attempts to reconnect with exponential backoff.
    fn reconnect(&mut self) -> Result<(), ViscaError> {
        self.current_retry_count = 0;
        let mut delay = self.config.initial_delay;

        loop {
            log::info!(
                "Attempting to reconnect (attempt {}/{})",
                self.current_retry_count + 1,
                self.config.max_retries
            );

            self.notify_event(ConnectionEvent::ReconnectingStarted {
                attempt: self.current_retry_count + 1,
                max_attempts: self.config.max_retries,
            });

            match (self.create_transport)() {
                Ok(transport) => {
                    log::info!("Successfully reconnected");
                    self.inner = Some(transport);
                    self.last_successful_operation = Some(Instant::now());
                    self.current_retry_count = 0;
                    self.stats.reset();
                    self.notify_event(ConnectionEvent::Connected);
                    return Ok(());
                }
                Err(e) => {
                    self.current_retry_count += 1;
                    self.stats.record_error();

                    self.notify_event(ConnectionEvent::ReconnectingFailed {
                        attempt: self.current_retry_count,
                        error: e.to_string(),
                    });

                    if self.current_retry_count >= self.config.max_retries {
                        log::error!("Max reconnection attempts reached");
                        self.notify_event(ConnectionEvent::ReconnectionExhausted);
                        return Err(ViscaError::ConnectionLost {
                            reason: "Max reconnection attempts reached".to_string(),
                        });
                    }

                    log::warn!(
                        "Reconnection attempt {} failed: {}, waiting {:?} before retry",
                        self.current_retry_count,
                        e,
                        delay
                    );

                    thread::sleep(delay);

                    // Calculate next delay with exponential backoff
                    delay = Duration::from_secs_f64(
                        (delay.as_secs_f64() * self.config.backoff_factor)
                            .min(self.config.max_delay.as_secs_f64()),
                    );
                }
            }
        }
    }

    /// Executes an operation with automatic retry on connection errors.
    fn with_retry<F, R>(&mut self, operation: F) -> Result<R, ViscaError>
    where
        F: Fn(&mut T) -> Result<R, ViscaError>,
    {
        for attempt in 0..self.config.max_retries {
            // Ensure we have a connection
            self.ensure_connected()?;

            // Try the operation
            if let Some(ref mut transport) = self.inner {
                match operation(transport) {
                    Ok(result) => {
                        self.last_successful_operation = Some(Instant::now());
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
                            log::warn!("Connection error during operation: {}", e);
                            self.inner = None;
                            self.stats.record_error();
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
            reason: "Failed to reconnect after multiple attempts".to_string(),
        })
    }

    /// Gets a snapshot of the current connection statistics.
    pub fn stats_snapshot(&self) -> ConnectionStats {
        self.stats.clone()
    }

    /// Combines statistics from both the wrapper and inner transport.
    pub fn combined_stats(&self) -> ConnectionStats {
        // Get wrapper stats
        let wrapper_stats = self.stats.snapshot();

        // If we have an inner transport, combine its stats
        self.inner.as_ref().map_or_else(
            || self.stats.clone(),
            |transport| {
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
            },
        )
    }
}

impl<T> ViscaTransport for ReconnectingTransport<T>
where
    T: ViscaTransport + ConnectionManagement,
{
    fn send_command(&mut self, command: &dyn ViscaCommand) -> Result<(), ViscaError> {
        self.with_retry(|transport| {
            transport.send_command(command)?;
            Ok(())
        })
    }

    fn receive_response(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
        self.with_retry(super::ViscaTransport::receive_response)
    }

    fn send_and_wait(&mut self, command: &dyn ViscaCommand) -> Result<ViscaResponse, ViscaError> {
        self.with_retry(|transport| transport.send_and_wait(command))
    }
}

impl<T> ConnectionManagement for ReconnectingTransport<T>
where
    T: ViscaTransport + ConnectionManagement,
{
    fn is_healthy(&mut self) -> Result<bool, ViscaError> {
        match self.ensure_connected() {
            Ok(()) => {
                if let Some(ref mut transport) = self.inner {
                    match transport.is_healthy() {
                        Ok(healthy) => {
                            self.stats.record_health_check(healthy);
                            Ok(healthy)
                        }
                        Err(e) => {
                            self.stats.record_health_check(false);
                            Err(e)
                        }
                    }
                } else {
                    Ok(false)
                }
            }
            Err(_) => Ok(false),
        }
    }

    fn connection_stats(&self) -> &ConnectionStats {
        // Return wrapper stats - use combined_stats() method for full statistics
        &self.stats
    }
}
