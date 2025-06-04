//! Auto-reconnecting transport wrapper that provides automatic connection recovery.
//!
//! This module implements a wrapper around any `ViscaTransport` that automatically
//! handles connection failures and reconnection with configurable retry policies.

use crate::{
    connection::{ConnectionManagement, ConnectionStats},
    ViscaCommand, ViscaError, ViscaResponse, ViscaTransport,
};
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
    create_transport: Box<dyn Fn() -> Result<T, ViscaError>>,
    /// Last successful operation time (for health checks)
    last_successful_operation: Option<Instant>,
    /// Current retry count
    current_retry_count: usize,
    /// Connection statistics
    stats: ConnectionStats,
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
        F: Fn() -> Result<T, ViscaError> + 'static,
    {
        let transport = create_transport()?;
        Ok(Self {
            inner: Some(transport),
            config,
            create_transport: Box::new(create_transport),
            last_successful_operation: Some(Instant::now()),
            current_retry_count: 0,
            stats: ConnectionStats::new(),
        })
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
                                return Ok(());
                            }
                            Ok(false) | Err(_) => {
                                log::warn!("Health check failed, reconnecting...");
                                self.inner = None;
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

            match (self.create_transport)() {
                Ok(transport) => {
                    log::info!("Successfully reconnected");
                    self.inner = Some(transport);
                    self.last_successful_operation = Some(Instant::now());
                    self.current_retry_count = 0;
                    self.stats.record_error(); // Count the reconnection as an error recovered from
                    return Ok(());
                }
                Err(e) => {
                    self.current_retry_count += 1;

                    if self.current_retry_count >= self.config.max_retries {
                        log::error!("Max reconnection attempts reached");
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
        self.with_retry(|transport| transport.receive_response())
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
                    transport.is_healthy()
                } else {
                    Ok(false)
                }
            }
            Err(_) => Ok(false),
        }
    }

    fn connection_stats(&self) -> &ConnectionStats {
        // Return our wrapper's stats combined with inner transport stats
        if let Some(ref transport) = self.inner {
            // In a real implementation, we might want to combine stats
            transport.connection_stats()
        } else {
            &self.stats
        }
    }
}
