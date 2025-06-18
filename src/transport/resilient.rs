//! Resilient transport wrapper that provides automatic reconnection and retry logic.
//!
//! This module implements a wrapper around any `Transport` that automatically
//! handles connection failures, reconnection with configurable retry policies,
//! and transparent recovery from network issues.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(feature = "tokio")]
use tokio::sync::Mutex as AsyncMutex;

#[cfg(not(feature = "tokio"))]
use std::sync::Mutex;

use crate::{transport::Transport, types::SocketId, Command, Error as ViscaError};

/// Configuration for resilient transport behavior.
#[derive(Debug, Clone, Copy)]
pub struct ResilienceConfig {
    /// Maximum number of retry attempts for a single operation
    pub max_retries: usize,
    /// Initial delay between retry attempts
    pub initial_retry_delay: Duration,
    /// Maximum delay between retry attempts
    pub max_retry_delay: Duration,
    /// Factor by which to increase the delay after each failed attempt
    pub backoff_factor: f64,
    /// Maximum number of reconnection attempts
    pub max_reconnect_attempts: usize,
    /// Delay between reconnection attempts
    pub reconnect_delay: Duration,
    /// Interval for periodic health checks (None to disable)
    pub health_check_interval: Option<Duration>,
    /// Timeout for individual operations
    pub operation_timeout: Duration,
}

impl Default for ResilienceConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_retry_delay: Duration::from_millis(100),
            max_retry_delay: Duration::from_secs(5),
            backoff_factor: 2.0,
            max_reconnect_attempts: 5,
            reconnect_delay: Duration::from_secs(1),
            health_check_interval: Some(Duration::from_secs(30)),
            operation_timeout: Duration::from_secs(5),
        }
    }
}

/// Events that can occur during resilient transport operation.
#[derive(Debug, Clone)]
pub enum ResilienceEvent {
    /// Operation succeeded after retries
    OperationSucceeded {
        /// Number of retries that were needed
        retries: usize,
    },
    /// Operation failed after all retries
    OperationFailed {
        /// Total number of attempts made
        attempts: usize,
        /// Final error
        error: String,
    },
    /// Transport reconnected successfully
    Reconnected {
        /// Number of reconnection attempts that were needed
        attempts: usize,
    },
    /// Transport reconnection failed
    ReconnectionFailed {
        /// Total number of attempts made
        attempts: usize,
        /// Final error
        error: String,
    },
    /// Health check succeeded
    HealthCheckPassed,
    /// Health check failed
    HealthCheckFailed {
        /// Error from health check
        error: String,
    },
}

/// Callback for resilience events
pub type EventCallback = Arc<dyn Fn(ResilienceEvent) + Send + Sync>;

/// Statistics for the resilient transport
#[derive(Debug, Clone, Copy, Default)]
pub struct ResilienceStats {
    /// Total number of operations attempted
    pub total_operations: u64,
    /// Number of operations that succeeded on first try
    pub first_try_successes: u64,
    /// Number of operations that succeeded after retries
    pub retry_successes: u64,
    /// Number of operations that failed after all retries
    pub failures: u64,
    /// Total number of retries performed
    pub total_retries: u64,
    /// Number of successful reconnections
    pub successful_reconnections: u64,
    /// Number of failed reconnection attempts
    pub failed_reconnections: u64,
    /// Last successful operation time
    pub last_success: Option<Instant>,
    /// Last failure time
    pub last_failure: Option<Instant>,
}

impl ResilienceStats {
    /// Calculate the success rate as a percentage
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 100.0;
        }
        let successes = self.first_try_successes + self.retry_successes;
        (successes as f64 / self.total_operations as f64) * 100.0
    }

    /// Calculate the average retries per operation
    #[must_use]
    pub fn average_retries(&self) -> f64 {
        if self.total_operations == 0 {
            return 0.0;
        }
        self.total_retries as f64 / self.total_operations as f64
    }
}

/// Internal state for the resilient transport
struct TransportState<T> {
    /// The underlying transport
    #[allow(dead_code)]
    inner: Option<T>,
    /// Last successful operation time
    #[allow(dead_code)]
    last_success: Option<Instant>,
    /// Statistics
    stats: ResilienceStats,
}

/// Future type for async transport factory
pub type TransportFactoryFuture<T> = Pin<Box<dyn Future<Output = Result<T, ViscaError>> + Send>>;

/// Factory for creating transport instances
pub enum TransportFactory<T> {
    /// Synchronous factory function
    Sync(Arc<dyn Fn() -> Result<T, ViscaError> + Send + Sync>),
    /// Asynchronous factory function
    Async(Arc<dyn Fn() -> TransportFactoryFuture<T> + Send + Sync>),
}

impl<T> std::fmt::Debug for TransportFactory<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportFactory::Sync(_) => write!(f, "TransportFactory::Sync"),
            TransportFactory::Async(_) => write!(f, "TransportFactory::Async"),
        }
    }
}

/// A transport wrapper that provides resilience through retries and reconnection.
pub struct ResilientTransport<T: Transport> {
    /// Protected state
    #[cfg(feature = "tokio")]
    state: Arc<AsyncMutex<TransportState<T>>>,
    #[cfg(not(feature = "tokio"))]
    state: Arc<Mutex<TransportState<T>>>,
    /// Configuration
    config: ResilienceConfig,
    /// Factory for creating new transport instances
    #[cfg(feature = "tokio")]
    factory: TransportFactory<T>,
    /// Optional event callback
    event_callback: Option<EventCallback>,
}

impl<T: Transport> std::fmt::Debug for ResilientTransport<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResilientTransport")
            .field("config", &self.config)
            .field("has_event_callback", &self.event_callback.is_some())
            .finish()
    }
}

impl<T: Transport + Send + Sync + 'static> ResilientTransport<T> {
    /// Creates a new resilient transport wrapper without reconnection support.
    ///
    /// This constructor is available when tokio is not enabled. It provides retry
    /// logic but not reconnection capabilities.
    ///
    /// # Arguments
    /// * `transport` - The initial transport instance
    /// * `config` - Resilience configuration
    #[cfg(not(feature = "tokio"))]
    pub fn new(transport: T, config: ResilienceConfig) -> Self {
        let state = TransportState {
            inner: Some(transport),
            last_success: Some(Instant::now()),
            stats: ResilienceStats::default(),
        };

        Self {
            state: Arc::new(Mutex::new(state)),
            config,
            event_callback: None,
        }
    }

    /// Creates a new resilient transport wrapper with a synchronous factory.
    ///
    /// # Arguments
    /// * `transport` - The initial transport instance
    /// * `factory` - Function to create new transport instances for reconnection
    /// * `config` - Resilience configuration
    #[cfg(feature = "tokio")]
    pub fn new<F>(transport: T, factory: F, config: ResilienceConfig) -> Self
    where
        F: Fn() -> Result<T, ViscaError> + Send + Sync + 'static,
    {
        let state = TransportState {
            inner: Some(transport),
            last_success: Some(Instant::now()),
            stats: ResilienceStats::default(),
        };

        Self {
            state: Arc::new(AsyncMutex::new(state)),
            config,
            factory: TransportFactory::Sync(Arc::new(factory)),
            event_callback: None,
        }
    }

    /// Creates a new resilient transport wrapper with an asynchronous factory.
    ///
    /// # Arguments
    /// * `transport` - The initial transport instance
    /// * `factory` - Async function to create new transport instances for reconnection
    /// * `config` - Resilience configuration
    #[cfg(feature = "tokio")]
    pub fn new_async<F, Fut>(transport: T, factory: F, config: ResilienceConfig) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T, ViscaError>> + Send + 'static,
    {
        let state = TransportState {
            inner: Some(transport),
            last_success: Some(Instant::now()),
            stats: ResilienceStats::default(),
        };

        Self {
            state: Arc::new(AsyncMutex::new(state)),
            config,
            factory: TransportFactory::Async(Arc::new(move || Box::pin(factory()))),
            event_callback: None,
        }
    }

    /// Sets an event callback for monitoring resilience events.
    pub fn set_event_callback(&mut self, callback: EventCallback) {
        self.event_callback = Some(callback);
    }

    /// Gets a snapshot of the current statistics.
    #[cfg(feature = "tokio")]
    pub async fn stats(&self) -> ResilienceStats {
        let state = self.state.lock().await;
        state.stats
    }

    /// Gets a snapshot of the current statistics.
    #[cfg(not(feature = "tokio"))]
    #[must_use]
    pub fn stats(&self) -> ResilienceStats {
        let state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.stats
    }

    /// Notifies about an event if a callback is set.
    #[allow(dead_code)]
    fn notify_event(&self, event: ResilienceEvent) {
        if let Some(ref callback) = self.event_callback {
            callback(event);
        }
    }

    /// Attempts to reconnect the transport (async).
    #[cfg(feature = "tokio")]
    async fn reconnect_async(&self) -> Result<(), ViscaError> {
        for attempt in 1..=self.config.max_reconnect_attempts {
            log::info!(
                "Attempting to reconnect (attempt {}/{})",
                attempt,
                self.config.max_reconnect_attempts
            );

            let result = match &self.factory {
                TransportFactory::Sync(factory) => factory(),
                TransportFactory::Async(factory) => factory().await,
            };

            match result {
                Ok(new_transport) => {
                    let mut state = self.state.lock().await;
                    // Take ownership of the old transport (dropping it)
                    let _ = state.inner.take();
                    state.inner = Some(new_transport);
                    state.stats.successful_reconnections += 1;
                    drop(state);

                    self.notify_event(ResilienceEvent::Reconnected { attempts: attempt });
                    log::info!("Reconnection successful");
                    return Ok(());
                }
                Err(e) => {
                    log::error!("Reconnection attempt {} failed: {}", attempt, e);

                    if attempt < self.config.max_reconnect_attempts {
                        tokio::time::sleep(self.config.reconnect_delay).await;
                    }
                }
            }
        }

        let mut state = self.state.lock().await;
        state.stats.failed_reconnections += 1;

        let error = "Failed to reconnect after maximum attempts".to_string();
        self.notify_event(ResilienceEvent::ReconnectionFailed {
            attempts: self.config.max_reconnect_attempts,
            error: error.clone(),
        });

        Err(ViscaError::ConnectionLost { reason: error })
    }
}

impl<T: Transport + Send + Sync + 'static> Transport for ResilientTransport<T> {
    fn send_command<'a>(
        &'a mut self,
        command: &'a dyn Command,
        socket_id: SocketId,
    ) -> crate::transport::TransportFuture<'a, ()> {
        Box::pin(async move {
            #[cfg(not(feature = "tokio"))]
            {
                let _ = command; // Silence unused warning
                let _ = socket_id; // Silence unused warning
                Err(ViscaError::InvalidState(
                    "Async transport requires tokio feature".to_string(),
                ))
            }

            #[cfg(feature = "tokio")]
            {
                let mut delay = self.config.initial_retry_delay;
                let mut last_error = None;

                // Update stats for operation attempt
                {
                    let mut state = self.state.lock().await;
                    state.stats.total_operations += 1;
                }

                for attempt in 0..=self.config.max_retries {
                    // Try the operation
                    let result = {
                        let mut state = self.state.lock().await;
                        if let Some(ref mut transport) = state.inner {
                            transport.send_command(command, socket_id).await
                        } else {
                            Err(ViscaError::ConnectionLost {
                                reason: "Transport not connected".to_string(),
                            })
                        }
                    };

                    match result {
                        Ok(()) => {
                            // Update success stats
                            let mut state = self.state.lock().await;
                            state.last_success = Some(Instant::now());
                            state.stats.last_success = Some(Instant::now());

                            if attempt == 0 {
                                state.stats.first_try_successes += 1;
                            } else {
                                state.stats.retry_successes += 1;
                                state.stats.total_retries += attempt as u64;
                            }

                            if attempt > 0 {
                                self.notify_event(ResilienceEvent::OperationSucceeded {
                                    retries: attempt,
                                });
                            }

                            return Ok(());
                        }
                        Err(e) => {
                            // Check if this is a retryable error
                            let is_retryable = matches!(
                                &e,
                                ViscaError::Io(_)
                                    | ViscaError::ConnectionLost { .. }
                                    | ViscaError::Timeout
                            );

                            if is_retryable {
                                if attempt < self.config.max_retries {
                                    log::warn!(
                                        "Operation failed (attempt {}), retrying: {}",
                                        attempt + 1,
                                        e
                                    );

                                    // Try to reconnect if it's a connection error
                                    if matches!(&e, ViscaError::ConnectionLost { .. }) {
                                        let _ = self.reconnect_async().await;
                                    }

                                    last_error = Some(e);

                                    // Wait before retry with exponential backoff
                                    tokio::time::sleep(delay).await;

                                    delay = Duration::from_secs_f64(
                                        (delay.as_secs_f64() * self.config.backoff_factor)
                                            .min(self.config.max_retry_delay.as_secs_f64()),
                                    );
                                }
                            } else {
                                // Non-retryable error, fail immediately
                                last_error = Some(e);
                                break;
                            }
                        }
                    }
                }

                // All retries exhausted
                let mut state = self.state.lock().await;
                state.stats.failures += 1;
                state.stats.last_failure = Some(Instant::now());
                state.stats.total_retries += self.config.max_retries as u64;

                let error = last_error
                    .unwrap_or_else(|| ViscaError::InvalidState("No error recorded".to_string()));

                self.notify_event(ResilienceEvent::OperationFailed {
                    attempts: self.config.max_retries + 1,
                    error: error.to_string(),
                });

                Err(error)
            }
        })
    }

    fn receive_response(&mut self) -> crate::transport::TransportFuture<'_, (SocketId, Vec<u8>)> {
        Box::pin(async move {
            #[cfg(not(feature = "tokio"))]
            return Err(ViscaError::InvalidState(
                "Async transport requires tokio feature".to_string(),
            ));

            #[cfg(feature = "tokio")]
            {
                let mut delay = self.config.initial_retry_delay;
                let mut last_error = None;

                for attempt in 0..=self.config.max_retries {
                    // Try the operation
                    let result = {
                        let mut state = self.state.lock().await;
                        if let Some(ref mut transport) = state.inner {
                            transport.receive_response().await
                        } else {
                            Err(ViscaError::ConnectionLost {
                                reason: "Transport not connected".to_string(),
                            })
                        }
                    };

                    match result {
                        Ok(response) => {
                            // Update success stats
                            let mut state = self.state.lock().await;
                            state.last_success = Some(Instant::now());
                            return Ok(response);
                        }
                        Err(e) => {
                            // Check if this is a retryable error
                            let is_retryable = matches!(
                                &e,
                                ViscaError::Io(_)
                                    | ViscaError::ConnectionLost { .. }
                                    | ViscaError::Timeout
                            );

                            if is_retryable {
                                if attempt < self.config.max_retries {
                                    log::warn!(
                                        "Receive failed (attempt {}), retrying: {}",
                                        attempt + 1,
                                        e
                                    );

                                    last_error = Some(e);

                                    // Wait before retry
                                    tokio::time::sleep(delay).await;

                                    delay = Duration::from_secs_f64(
                                        (delay.as_secs_f64() * self.config.backoff_factor)
                                            .min(self.config.max_retry_delay.as_secs_f64()),
                                    );
                                }
                            } else {
                                // Non-retryable error, fail immediately
                                last_error = Some(e);
                                break;
                            }
                        }
                    }
                }

                Err(last_error
                    .unwrap_or_else(|| ViscaError::InvalidState("No error recorded".to_string())))
            }
        })
    }
}

impl<T: Transport> Clone for ResilientTransport<T> {
    fn clone(&self) -> Self {
        #[cfg(feature = "tokio")]
        {
            Self {
                state: Arc::clone(&self.state),
                config: self.config,
                factory: match &self.factory {
                    TransportFactory::Sync(f) => TransportFactory::Sync(Arc::clone(f)),
                    TransportFactory::Async(f) => TransportFactory::Async(Arc::clone(f)),
                },
                event_callback: self.event_callback.as_ref().map(Arc::clone),
            }
        }
        #[cfg(not(feature = "tokio"))]
        {
            Self {
                state: Arc::clone(&self.state),
                config: self.config,
                event_callback: self.event_callback.as_ref().map(Arc::clone),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resilience_config_default() {
        let config = ResilienceConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_retry_delay, Duration::from_millis(100));
        assert_eq!(config.backoff_factor, 2.0);
    }

    #[test]
    fn test_resilience_stats() {
        let stats = ResilienceStats {
            total_operations: 100,
            first_try_successes: 80,
            retry_successes: 15,
            failures: 5,
            total_retries: 25,
            ..Default::default()
        };

        assert_eq!(stats.success_rate(), 95.0);
        assert_eq!(stats.average_retries(), 0.25);
    }

    #[test]
    fn test_stats_edge_cases() {
        let stats = ResilienceStats::default();
        assert_eq!(stats.success_rate(), 100.0);
        assert_eq!(stats.average_retries(), 0.0);
    }
}
