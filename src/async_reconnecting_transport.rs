//! Async auto-reconnecting transport wrapper.

#[cfg(feature = "async")]
use crate::{
    async_transport::{AsyncViscaTransport, TransportFuture},
    connection::{AsyncConnectionManagement, ConnectionStats},
    reconnecting_transport::ReconnectionConfig,
    ViscaCommand, ViscaError, ViscaResponse,
};
#[cfg(feature = "async")]
use std::future::Future;
#[cfg(feature = "async")]
use std::pin::Pin;
#[cfg(feature = "async")]
use std::sync::Arc;
#[cfg(feature = "async")]
use std::time::Instant;
#[cfg(feature = "async")]
use tokio::sync::Mutex;
#[cfg(feature = "async")]
use tokio::time::{sleep, Duration};

#[cfg(feature = "async")]
type CreateTransportFn<T> =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<T, ViscaError>> + Send>> + Send + Sync>;

/// Async version of the reconnecting transport wrapper.
#[cfg(feature = "async")]
pub struct AsyncReconnectingTransport<T> {
    /// The underlying transport (None when disconnected)
    inner: Arc<Mutex<Option<T>>>,
    /// Configuration for reconnection behavior
    config: ReconnectionConfig,
    /// Function to create a new transport instance
    create_transport: CreateTransportFn<T>,
    /// Last successful operation time (for health checks)
    last_successful_operation: Arc<Mutex<Option<Instant>>>,
    /// Current retry count
    current_retry_count: Arc<Mutex<usize>>,
    /// Connection statistics
    stats: Arc<Mutex<ConnectionStats>>,
}

#[cfg(feature = "async")]
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

        Ok(Self {
            inner: Arc::new(Mutex::new(Some(transport))),
            config,
            create_transport: create_fn,
            last_successful_operation: Arc::new(Mutex::new(Some(Instant::now()))),
            current_retry_count: Arc::new(Mutex::new(0)),
            stats: Arc::new(Mutex::new(ConnectionStats::new())),
        })
    }

    /// Ensures that we have a healthy connection, reconnecting if necessary.
    async fn ensure_connected(&self) -> Result<(), ViscaError> {
        // Check if we need a health check
        if let Some(interval) = self.config.health_check_interval {
            let last_op = *self.last_successful_operation.lock().await;
            if let Some(last_op_time) = last_op {
                if last_op_time.elapsed() > interval {
                    // Perform health check
                    let mut inner_guard = self.inner.lock().await;
                    if let Some(ref mut transport) = *inner_guard {
                        match transport.is_healthy().await {
                            Ok(true) => {
                                drop(inner_guard);
                                *self.last_successful_operation.lock().await = Some(Instant::now());
                                return Ok(());
                            }
                            Ok(false) | Err(_) => {
                                log::warn!("Health check failed, reconnecting...");
                                *inner_guard = None;
                            }
                        }
                    }
                }
            }
        }

        // If we have a connection and no health check is needed, we're done
        if self.inner.lock().await.is_some() {
            return Ok(());
        }

        // Try to reconnect
        self.reconnect().await
    }

    /// Attempts to reconnect with exponential backoff.
    async fn reconnect(&self) -> Result<(), ViscaError> {
        *self.current_retry_count.lock().await = 0;
        let mut delay = self.config.initial_delay;

        loop {
            let retry_count = *self.current_retry_count.lock().await;
            log::info!(
                "Attempting to reconnect (attempt {}/{})",
                retry_count + 1,
                self.config.max_retries
            );

            match (self.create_transport)().await {
                Ok(transport) => {
                    log::info!("Successfully reconnected");
                    *self.inner.lock().await = Some(transport);
                    *self.last_successful_operation.lock().await = Some(Instant::now());
                    *self.current_retry_count.lock().await = 0;
                    self.stats.lock().await.record_error();
                    return Ok(());
                }
                Err(e) => {
                    *self.current_retry_count.lock().await += 1;
                    let retry_count = *self.current_retry_count.lock().await;

                    if retry_count >= self.config.max_retries {
                        log::error!("Max reconnection attempts reached");
                        return Err(ViscaError::ConnectionLost {
                            reason: "Max reconnection attempts reached".to_string(),
                        });
                    }

                    log::warn!(
                        "Reconnection attempt {} failed: {}, waiting {:?} before retry",
                        retry_count,
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

    /// Executes an operation with automatic retry on connection errors.
    async fn with_retry<'a, F, Fut, R>(&'a self, operation: F) -> Result<R, ViscaError>
    where
        F: Fn(&'a mut T) -> Fut,
        Fut: Future<Output = Result<R, ViscaError>> + 'a,
    {
        for attempt in 0..self.config.max_retries {
            // Ensure we have a connection
            self.ensure_connected().await?;

            // Try the operation
            let mut inner_guard = self.inner.lock().await;
            if let Some(ref mut transport) = *inner_guard {
                match operation(transport).await {
                    Ok(result) => {
                        drop(inner_guard);
                        *self.last_successful_operation.lock().await = Some(Instant::now());
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
                            *inner_guard = None;
                            drop(inner_guard);
                            self.stats.lock().await.record_error();

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

#[cfg(feature = "async")]
impl<T> AsyncViscaTransport for AsyncReconnectingTransport<T>
where
    T: AsyncViscaTransport + AsyncConnectionManagement + Send + Sync + 'static,
{
    fn send_command<'a>(&'a mut self, command: &'a dyn ViscaCommand) -> TransportFuture<'a, ()> {
        Box::pin(async move {
            let self_ref = unsafe { &*(self as *const _) };
            self_ref
                .with_retry(|transport| async move { transport.send_command(command).await })
                .await
        })
    }

    fn receive_response(&mut self) -> TransportFuture<'_, Vec<Vec<u8>>> {
        Box::pin(async move {
            let self_ref = unsafe { &*(self as *const _) };
            self_ref
                .with_retry(|transport| async move { transport.receive_response().await })
                .await
        })
    }

    fn send_and_wait<'a>(
        &'a mut self,
        command: &'a dyn ViscaCommand,
    ) -> TransportFuture<'a, ViscaResponse> {
        Box::pin(async move {
            let self_ref = unsafe { &*(self as *const _) };
            self_ref
                .with_retry(|transport| async move { transport.send_and_wait(command).await })
                .await
        })
    }
}

#[cfg(feature = "async")]
impl<T> AsyncConnectionManagement for AsyncReconnectingTransport<T>
where
    T: AsyncViscaTransport + AsyncConnectionManagement + Send + Sync + 'static,
{
    fn is_healthy(&mut self) -> TransportFuture<'_, bool> {
        Box::pin(async move {
            match self.ensure_connected().await {
                Ok(()) => {
                    let mut inner_guard = self.inner.lock().await;
                    if let Some(ref mut transport) = *inner_guard {
                        transport.is_healthy().await
                    } else {
                        Ok(false)
                    }
                }
                Err(_) => Ok(false),
            }
        })
    }

    fn connection_stats(&self) -> &ConnectionStats {
        // This is a limitation - we can't easily return a reference to stats inside Arc<Mutex<>>
        // In a real implementation, we might need to redesign this API
        unimplemented!("Stats access needs redesign for async wrapper")
    }
}
