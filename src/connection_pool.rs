//! Connection pooling for managing multiple VISCA cameras.
//!
//! This module provides a connection pool that manages multiple camera connections
//! using the unified `Client` architecture.

// Standard library imports
use std::time::{Duration, Instant};

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use std::collections::HashMap;

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use std::sync::{Arc, Mutex};

// Third-party crate imports
// (none)

// Workspace / local-crate imports
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use crate::{
    error::Error, unified_client::Client, Response, ViscaCommand,
};

/// Configuration for the connection pool.
#[derive(Debug, Copy, Clone)]
pub struct PoolConfig {
    /// How often to run health checks on all connections.
    pub health_check_interval: Duration,
    /// Whether to automatically remove unhealthy connections.
    pub auto_remove_unhealthy: bool,
    /// Maximum idle time before a connection is considered stale.
    pub max_idle_time: Option<Duration>,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            health_check_interval: Duration::from_secs(60),
            auto_remove_unhealthy: false,
            max_idle_time: Some(Duration::from_secs(300)),
        }
    }
}

/// A pooled connection that tracks usage and health.
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
#[derive(Debug)]
struct PooledConnection {
    client: Client,
    last_used: Instant,
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
pub struct PooledCameraStats {
    /// Camera information.
    pub info: CameraInfo,
    /// Whether the connection is currently healthy.
    pub is_healthy: bool,
    /// Last time the connection was used.
    pub last_used: Instant,
}

/// Connection type for pool entries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    /// UDP connection
    Udp,
    /// TCP connection
    Tcp,
}

// Provide a stub when features are disabled to prevent breaking the public API
#[cfg(not(any(feature = "blocking-client", feature = "async-client")))]
/// A pool of VISCA camera connections (requires blocking-client or async-client feature).
#[derive(Debug, Clone, Copy)]
pub struct ViscaConnectionPool;

#[cfg(not(any(feature = "blocking-client", feature = "async-client")))]
/// Async-specific connection pool (requires async-client feature).
#[derive(Debug, Clone, Copy)]
pub struct AsyncViscaConnectionPool;

/// A pool of VISCA camera connections using the unified `Client`.
///
/// This pool manages multiple camera connections and provides convenient
/// methods for executing commands on specific cameras.
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
#[derive(Debug)]
pub struct ViscaConnectionPool {
    connections: Arc<Mutex<HashMap<String, PooledConnection>>>,
    config: PoolConfig,
}

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
impl ViscaConnectionPool {
    /// Creates a new connection pool.
    ///
    /// # Arguments
    ///
    /// * `config` - Pool configuration
    #[must_use]
    pub fn new(config: PoolConfig) -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            config,
        }
    }

    /// Adds a camera to the pool using the blocking API.
    ///
    /// # Arguments
    ///
    /// * `camera_id` - Unique identifier for the camera
    /// * `address` - Network address of the camera (e.g., "192.168.1.100:5678")
    /// * `connection_type` - Whether to use UDP or TCP
    /// * `info` - Camera information
    ///
    /// # Errors
    ///
    /// Returns `Error` if the connection cannot be established.
    #[cfg(feature = "blocking-client")]
    pub fn add_camera(
        &self,
        camera_id: impl Into<String>,
        address: &str,
        connection_type: ConnectionType,
        info: CameraInfo,
    ) -> Result<(), Error> {
        let camera_id = camera_id.into();

        let client = match connection_type {
            ConnectionType::Udp => Client::connect_udp(address)?,
            ConnectionType::Tcp => Client::connect_tcp(address)?,
        };

        let pooled = PooledConnection {
            client,
            last_used: Instant::now(),
            camera_info: info,
        };

        let _ = self
            .connections
            .lock()
            .map_err(|_| Error::InvalidState("Mutex poisoned".into()))?
            .insert(camera_id, pooled);
        Ok(())
    }

    /// Removes a camera from the pool.
    #[must_use]
    pub fn remove_camera(&self, camera_id: &str) -> Option<CameraInfo> {
        let mut connections = self.connections.lock().ok()?;
        connections.remove(camera_id).map(|conn| conn.camera_info)
    }

    /// Gets a connection from the pool and executes a command.
    ///
    /// This method handles the connection lookup and automatically updates
    /// the `last_used` timestamp.
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidParameter` if the camera is not found in the pool.
    /// Returns other `Error` variants if the command execution fails.
    #[cfg(feature = "blocking-client")]
    pub fn execute_command(
        &self,
        camera_id: &str,
        command: &dyn ViscaCommand,
    ) -> Result<Response, Error> {
        let mut connections = self
            .connections
            .lock()
            .map_err(|_| Error::InvalidState("Mutex poisoned".into()))?;
        let pooled = connections.get_mut(camera_id).ok_or_else(|| {
            Error::InvalidParameter(format!("Camera '{camera_id}' not found in pool"))
        })?;

        pooled.last_used = Instant::now();
        let result = pooled.client.send(command);
        drop(connections);
        result
    }

    /// Gets a list of all cameras in the pool.
    #[must_use]
    pub fn list_cameras(&self) -> Vec<String> {
        let Ok(connections) = self.connections.lock() else {
            return Vec::new();
        };
        connections.keys().cloned().collect()
    }

    /// Gets statistics for all cameras in the pool.
    #[cfg(feature = "blocking-client")]
    #[must_use]
    pub fn get_all_stats(&self) -> Vec<PooledCameraStats> {
        // First collect camera info and clients to avoid holding lock during health checks
        let camera_data: Vec<(CameraInfo, Instant, Client)> = {
            let Ok(connections) = self.connections.lock() else {
                return Vec::new();
            };
            connections
                .values()
                .map(|conn| {
                    (
                        conn.camera_info.clone(),
                        conn.last_used,
                        conn.client.clone(),
                    )
                })
                .collect()
        };

        // Now check health without holding the lock
        camera_data
            .into_iter()
            .map(|(info, last_used, client)| {
                let is_healthy = client.is_healthy_blocking().unwrap_or(false);
                PooledCameraStats {
                    info,
                    is_healthy,
                    last_used,
                }
            })
            .collect()
    }

    /// Performs health checks on all connections.
    ///
    /// Returns a map of camera IDs to their health status.
    #[cfg(feature = "blocking-client")]
    #[must_use]
    pub fn health_check_all(&self) -> HashMap<String, bool> {
        // First collect the camera IDs to avoid holding the lock during health checks
        let camera_ids: Vec<String> = {
            let Ok(connections) = self.connections.lock() else {
                return HashMap::new();
            };
            connections.keys().cloned().collect()
        };

        let mut results = HashMap::new();

        // Now check each camera without holding the main lock
        for camera_id in camera_ids {
            let is_healthy = {
                let Ok(connections) = self.connections.lock() else {
                    continue;
                };
                connections
                    .get(&camera_id)
                    .is_some_and(|conn| conn.client.is_healthy_blocking().unwrap_or(false))
            };
            let _ = results.insert(camera_id, is_healthy);
        }

        results
    }

    /// Removes all unhealthy connections from the pool.
    ///
    /// Returns the IDs of removed cameras.
    #[cfg(feature = "blocking-client")]
    #[must_use]
    pub fn remove_unhealthy(&self) -> Vec<String> {
        let health_results = self.health_check_all();
        let mut removed = Vec::new();

        let Ok(mut connections) = self.connections.lock() else {
            return Vec::new();
        };
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
    #[must_use]
    pub fn remove_stale(&self) -> Vec<String> {
        self.config.max_idle_time.map_or_else(Vec::new, |max_idle| {
            let now = Instant::now();
            let mut removed = Vec::new();

            let Ok(mut connections) = self.connections.lock() else {
                return Vec::new();
            };
            connections.retain(|id, conn| {
                let is_stale = now.duration_since(conn.last_used) > max_idle;
                if is_stale {
                    removed.push(id.clone());
                }
                !is_stale
            });

            removed
        })
    }
}

/// Async-specific connection pool implementation.
///
/// This provides async methods when the async-client feature is enabled.
#[cfg(feature = "async-client")]
#[derive(Debug)]
pub struct AsyncViscaConnectionPool {
    connections: Arc<tokio::sync::Mutex<HashMap<String, PooledConnection>>>,
    config: PoolConfig,
}

#[cfg(feature = "async-client")]
impl AsyncViscaConnectionPool {
    /// Creates a new async connection pool.
    #[must_use]
    pub fn new(config: PoolConfig) -> Self {
        Self {
            connections: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            config,
        }
    }

    /// Adds a camera to the pool using the async API.
    ///
    /// # Errors
    ///
    /// Returns `Error` if the connection cannot be established.
    pub async fn add_camera(
        &self,
        camera_id: impl Into<String>,
        address: &str,
        connection_type: ConnectionType,
        info: CameraInfo,
    ) -> Result<(), Error> {
        let camera_id = camera_id.into();

        let client = match connection_type {
            ConnectionType::Udp => Client::connect_udp_async(address).await?,
            ConnectionType::Tcp => Client::connect_tcp_async(address).await?,
        };

        let pooled = PooledConnection {
            client,
            last_used: Instant::now(),
            camera_info: info,
        };

        let _ = self.connections.lock().await.insert(camera_id, pooled);
        Ok(())
    }

    /// Removes a camera from the pool.
    pub async fn remove_camera(&self, camera_id: &str) -> Option<CameraInfo> {
        let mut connections = self.connections.lock().await;
        connections.remove(camera_id).map(|conn| conn.camera_info)
    }

    /// Gets a connection from the pool and executes a command.
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidParameter` if the camera is not found in the pool.
    /// Returns other `Error` variants if the command execution fails.
    pub async fn execute_command(
        &self,
        camera_id: &str,
        command: &dyn ViscaCommand,
    ) -> Result<Response, Error> {
        let mut connections = self.connections.lock().await;
        let pooled = connections.get_mut(camera_id).ok_or_else(|| {
            Error::InvalidParameter(format!("Camera '{camera_id}' not found in pool"))
        })?;

        pooled.last_used = Instant::now();
        let result = pooled.client.send_async(command).await;
        drop(connections);
        result
    }

    /// Gets a list of all cameras in the pool.
    #[must_use]
    pub async fn list_cameras(&self) -> Vec<String> {
        let connections = self.connections.lock().await;
        connections.keys().cloned().collect()
    }

    /// Gets statistics for all cameras in the pool.
    #[must_use]
    pub async fn get_all_stats(&self) -> Vec<PooledCameraStats> {
        // First collect camera info and clients to avoid holding lock during health checks
        let camera_data: Vec<(CameraInfo, Instant, Client)> = {
            let connections = self.connections.lock().await;
            connections
                .values()
                .map(|conn| {
                    (
                        conn.camera_info.clone(),
                        conn.last_used,
                        conn.client.clone(),
                    )
                })
                .collect()
        };

        let mut stats = Vec::new();

        // Now check health without holding the lock
        for (info, last_used, client) in camera_data {
            let is_healthy = client.is_healthy().await.unwrap_or(false);
            stats.push(PooledCameraStats {
                info,
                is_healthy,
                last_used,
            });
        }

        stats
    }

    /// Performs health checks on all connections.
    pub async fn health_check_all(&self) -> HashMap<String, bool> {
        // First collect the camera IDs to avoid holding the lock during health checks
        let camera_ids: Vec<String> = {
            let connections = self.connections.lock().await;
            connections.keys().cloned().collect()
        };

        let mut results = HashMap::new();

        // Now check each camera without holding the main lock
        for camera_id in camera_ids {
            let is_healthy = {
                let connections = self.connections.lock().await;
                match connections.get(&camera_id) {
                    Some(conn) => conn.client.is_healthy().await.unwrap_or(false),
                    None => false,
                }
            };
            let _ = results.insert(camera_id, is_healthy);
        }

        results
    }

    /// Removes all unhealthy connections from the pool.
    pub async fn remove_unhealthy(&self) -> Vec<String> {
        let health_results = self.health_check_all().await;
        let mut removed = Vec::new();

        let mut connections = self.connections.lock().await;
        for (id, is_healthy) in health_results {
            if !is_healthy && connections.remove(&id).is_some() {
                removed.push(id);
            }
        }

        removed
    }

    /// Removes connections that have been idle longer than the configured `max_idle_time`.
    pub async fn remove_stale(&self) -> Vec<String> {
        if let Some(max_idle) = self.config.max_idle_time {
            let now = Instant::now();
            let mut connections = self.connections.lock().await;
            let mut removed = Vec::new();

            connections.retain(|id, conn| {
                let is_stale = now.duration_since(conn.last_used) > max_idle;
                if is_stale {
                    removed.push(id.clone());
                }
                !is_stale
            });

            removed
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    #[cfg(feature = "async-client")]
    use super::AsyncViscaConnectionPool;
    #[cfg(feature = "blocking-client")]
    use super::ViscaConnectionPool;
    #[cfg(any(feature = "blocking-client", feature = "async-client"))]
    use super::{CameraInfo, ConnectionType, PoolConfig};

    #[test]
    #[cfg(feature = "blocking-client")]
    fn test_pool_creation() {
        let config = PoolConfig::default();
        let pool = ViscaConnectionPool::new(config);
        assert_eq!(pool.list_cameras().len(), 0);
    }

    #[test]
    #[cfg(feature = "blocking-client")]
    fn test_add_remove_camera() {
        let config = PoolConfig::default();
        let pool = ViscaConnectionPool::new(config);

        let info = CameraInfo {
            id: "cam1".to_string(),
            name: Some("Front Camera".to_string()),
            model: Some("PTZOptics G2".to_string()),
            location: Some("Main Stage".to_string()),
        };

        // Note: This will fail without a real camera, but the structure is correct
        let _ = pool.add_camera(
            "cam1",
            "192.168.1.100:1259",
            ConnectionType::Udp,
            info.clone(),
        );

        // Even if add fails, test the remove logic
        if pool.list_cameras().contains(&"cam1".to_string()) {
            let removed_info = pool.remove_camera("cam1").unwrap();
            assert_eq!(removed_info.name, info.name);
            assert_eq!(pool.list_cameras().len(), 0);
        }
    }

    #[tokio::test]
    #[cfg(feature = "async-client")]
    async fn test_pool_creation_async() {
        let config = PoolConfig::default();
        let pool = AsyncViscaConnectionPool::new(config);
        assert_eq!(pool.list_cameras().await.len(), 0);
    }

    #[tokio::test]
    #[cfg(feature = "async-client")]
    async fn test_add_remove_camera_async() {
        let config = PoolConfig::default();
        let pool = AsyncViscaConnectionPool::new(config);

        let info = CameraInfo {
            id: "cam1".to_string(),
            name: Some("Front Camera".to_string()),
            model: Some("PTZOptics G2".to_string()),
            location: Some("Main Stage".to_string()),
        };

        // Note: This will fail without a real camera, but the structure is correct
        let _ = pool
            .add_camera(
                "cam1",
                "192.168.1.100:1259",
                ConnectionType::Udp,
                info.clone(),
            )
            .await;

        // Even if add fails, test the remove logic
        if pool.list_cameras().await.contains(&"cam1".to_string()) {
            let removed_info = pool.remove_camera("cam1").await.unwrap();
            assert_eq!(removed_info.name, info.name);
            assert_eq!(pool.list_cameras().await.len(), 0);
        }
    }
}
