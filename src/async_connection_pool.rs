//! Async connection pooling for managing multiple VISCA cameras.
//!
//! This module provides an async connection pool that manages multiple camera connections,
//! handles automatic reconnection, and provides health checking capabilities.

// Standard library imports
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

// Third-party imports
use tokio::sync::{Mutex, RwLock};

// Crate imports
use crate::{
    AsyncConnectionManagement, AsyncReconnectingTransport, AsyncViscaTransport, ReconnectionConfig,
    ViscaError,
};

/// Configuration for the async connection pool.
#[derive(Debug, Clone)]
pub struct AsyncPoolConfig {
    /// Configuration for automatic reconnection.
    pub reconnection_config: ReconnectionConfig,
    /// How often to run health checks on all connections.
    pub health_check_interval: Duration,
    /// Whether to automatically remove unhealthy connections.
    pub auto_remove_unhealthy: bool,
    /// Maximum idle time before a connection is considered stale.
    pub max_idle_time: Option<Duration>,
}

impl Default for AsyncPoolConfig {
    fn default() -> Self {
        Self {
            reconnection_config: ReconnectionConfig::default(),
            health_check_interval: Duration::from_secs(60),
            auto_remove_unhealthy: false,
            max_idle_time: Some(Duration::from_secs(300)),
        }
    }
}

/// A pooled async connection that tracks usage and health.
struct AsyncPooledConnection<T> {
    transport: Arc<Mutex<AsyncReconnectingTransport<T>>>,
    last_used: Arc<RwLock<Instant>>,
    camera_info: CameraInfo,
}

/// Information about a camera in the pool.
#[derive(Debug, Clone)]
pub struct CameraInfo {
    /// Unique identifier for this camera.
    pub id: String,
    /// Human-readable name for the camera.
    pub name: Option<String>,
    /// Camera model, if known.
    pub model: Option<String>,
    /// Camera location or description.
    pub location: Option<String>,
}

/// Statistics for a camera in the pool.
#[derive(Debug, Clone)]
pub struct AsyncPooledCameraStats {
    /// Camera information.
    pub info: CameraInfo,
    /// Whether the connection is currently healthy.
    pub is_healthy: bool,
    /// Last time the connection was used.
    pub last_used: Instant,
    /// Connection statistics from the underlying transport.
    pub connection_stats: crate::ConnectionStats,
}

/// Factory function type for creating async transports.
type AsyncTransportFactory<T> = Arc<
    dyn Fn(
            &str,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, ViscaError>> + Send>>
        + Send
        + Sync,
>;

/// An async pool of VISCA camera connections.
pub struct AsyncViscaConnectionPool<T>
where
    T: AsyncViscaTransport + AsyncConnectionManagement + Send + Sync + 'static,
{
    connections: Arc<RwLock<HashMap<String, AsyncPooledConnection<T>>>>,
    config: AsyncPoolConfig,
    create_transport: AsyncTransportFactory<T>,
}

impl<T> AsyncViscaConnectionPool<T>
where
    T: AsyncViscaTransport + AsyncConnectionManagement + Send + Sync + 'static,
{
    /// Creates a new async connection pool.
    ///
    /// # Arguments
    ///
    /// * `config` - Pool configuration
    /// * `create_transport` - Async factory function to create new transports
    pub fn new<F, Fut>(config: AsyncPoolConfig, create_transport: F) -> Self
    where
        F: Fn(&str) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<T, ViscaError>> + Send + 'static,
    {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            config,
            create_transport: Arc::new(move |addr| Box::pin(create_transport(addr))),
        }
    }

    /// Adds a camera to the pool.
    ///
    /// # Arguments
    ///
    /// * `camera_id` - Unique identifier for the camera
    /// * `address` - Network address of the camera (e.g., "192.168.1.100:5678")
    /// * `info` - Camera information
    pub async fn add_camera(
        &self,
        camera_id: impl Into<String>,
        address: &str,
        info: CameraInfo,
    ) -> Result<(), ViscaError> {
        let camera_id = camera_id.into();
        let address = address.to_string();
        let create_fn = self.create_transport.clone();

        let transport = AsyncReconnectingTransport::new(
            move || {
                let addr = address.clone();
                let create = create_fn.clone();
                async move { create(&addr).await }
            },
            self.config.reconnection_config.clone(),
        )
        .await?;

        let pooled = AsyncPooledConnection {
            transport: Arc::new(Mutex::new(transport)),
            last_used: Arc::new(RwLock::new(Instant::now())),
            camera_info: info,
        };

        let mut connections = self.connections.write().await;
        connections.insert(camera_id, pooled);

        Ok(())
    }

    /// Removes a camera from the pool.
    pub async fn remove_camera(&self, camera_id: &str) -> Option<CameraInfo> {
        let mut connections = self.connections.write().await;
        connections.remove(camera_id).map(|conn| conn.camera_info)
    }

    /// Gets a connection from the pool.
    ///
    /// Returns a guard that provides access to the transport and automatically
    /// updates the `last_used` timestamp when dropped.
    pub async fn get_connection(
        &self,
        camera_id: &str,
    ) -> Result<AsyncPooledConnectionGuard<T>, ViscaError> {
        let connections = self.connections.read().await;

        let pooled = connections.get(camera_id).ok_or_else(|| {
            ViscaError::InvalidParameter(format!("Camera '{}' not found in pool", camera_id))
        })?;

        Ok(AsyncPooledConnectionGuard {
            transport: pooled.transport.clone(),
            last_used: pooled.last_used.clone(),
            _camera_id: camera_id.to_string(),
        })
    }

    /// Gets a list of all cameras in the pool.
    pub async fn list_cameras(&self) -> Vec<String> {
        let connections = self.connections.read().await;
        connections.keys().cloned().collect()
    }

    /// Gets statistics for all cameras in the pool.
    pub async fn get_all_stats(&self) -> Vec<AsyncPooledCameraStats> {
        let connections = self.connections.read().await;
        let mut stats = Vec::new();

        for (_id, conn) in connections.iter() {
            let mut transport = conn.transport.lock().await;
            let is_healthy = transport.is_healthy().await.unwrap_or(false);
            let connection_stats = transport.combined_stats().await;
            let last_used = *conn.last_used.read().await;

            stats.push(AsyncPooledCameraStats {
                info: conn.camera_info.clone(),
                is_healthy,
                last_used,
                connection_stats,
            });
        }

        stats
    }

    /// Performs health checks on all connections.
    ///
    /// Returns a map of camera IDs to their health status.
    pub async fn health_check_all(&self) -> HashMap<String, bool> {
        let connections = self.connections.read().await;
        let mut results = HashMap::new();

        for (id, conn) in connections.iter() {
            let mut transport = conn.transport.lock().await;
            let is_healthy = transport.is_healthy().await.unwrap_or(false);
            results.insert(id.clone(), is_healthy);
        }

        results
    }

    /// Removes all unhealthy connections from the pool.
    ///
    /// Returns the IDs of removed cameras.
    pub async fn remove_unhealthy(&self) -> Vec<String> {
        let health_results = self.health_check_all().await;
        let mut removed = Vec::new();

        let mut connections = self.connections.write().await;
        for (id, is_healthy) in health_results {
            if !is_healthy && connections.remove(&id).is_some() {
                removed.push(id);
            }
        }

        removed
    }

    /// Removes connections that have been idle longer than the configured `max_idle_time`.
    ///
    /// Returns the IDs of removed cameras.
    pub async fn remove_stale(&self) -> Vec<String> {
        if let Some(max_idle) = self.config.max_idle_time {
            let now = Instant::now();
            let mut connections = self.connections.write().await;
            let mut removed = Vec::new();

            // Collect stale camera IDs first
            let mut stale_ids = Vec::new();
            for (id, conn) in connections.iter() {
                let last_used = *conn.last_used.read().await;
                if now.duration_since(last_used) > max_idle {
                    stale_ids.push(id.clone());
                }
            }

            // Remove stale connections
            for id in stale_ids {
                if connections.remove(&id).is_some() {
                    removed.push(id);
                }
            }

            removed
        } else {
            Vec::new()
        }
    }

    /// Starts a background task that periodically performs health checks and cleanup.
    ///
    /// Returns a handle that can be used to stop the background task.
    pub fn start_maintenance_task(&self) -> tokio::task::JoinHandle<()> {
        let pool = Arc::new(self.connections.clone());
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.health_check_interval);

            loop {
                interval.tick().await;

                // Perform health checks
                let connections = pool.read().await;
                for (_id, conn) in connections.iter() {
                    let mut transport = conn.transport.lock().await;
                    let _ = transport.is_healthy().await;
                }
                drop(connections);

                // Remove stale connections if configured
                if config.max_idle_time.is_some() {
                    let now = Instant::now();
                    let mut connections = pool.write().await;
                    connections.retain(|_, conn| {
                        let last_used = conn.last_used.try_read().unwrap();
                        now.duration_since(*last_used) <= config.max_idle_time.unwrap()
                    });
                }
            }
        })
    }
}

/// A guard that provides access to an async pooled connection.
///
/// Automatically updates the `last_used` timestamp when dropped.
pub struct AsyncPooledConnectionGuard<T> {
    transport: Arc<Mutex<AsyncReconnectingTransport<T>>>,
    last_used: Arc<RwLock<Instant>>,
    _camera_id: String,
}

impl<T> AsyncPooledConnectionGuard<T>
where
    T: AsyncViscaTransport + AsyncConnectionManagement,
{
    /// Gets access to the underlying transport.
    pub async fn transport(&self) -> tokio::sync::MutexGuard<'_, AsyncReconnectingTransport<T>> {
        self.transport.lock().await
    }
}

impl<T> Drop for AsyncPooledConnectionGuard<T> {
    fn drop(&mut self) {
        // Update last_used timestamp
        let last_used = self.last_used.clone();
        tokio::spawn(async move {
            *last_used.write().await = Instant::now();
        });
    }
}

// Tests are currently limited due to lifetime complexity with async closures and traits
// More comprehensive tests can be added when lifetime bounds are resolved
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_async_pool_basic() {
        // Basic test to ensure the module compiles
        let config = AsyncPoolConfig::default();
        assert_eq!(config.health_check_interval, Duration::from_secs(60));
    }
}
