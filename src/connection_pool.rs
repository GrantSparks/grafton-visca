//! Connection pooling for managing multiple VISCA cameras.
//!
//! This module provides a connection pool that manages multiple camera connections,
//! handles automatic reconnection, and provides health checking capabilities.

#![allow(deprecated)]

use crate::{
    ConnectionManagement, ReconnectingTransport, ReconnectionConfig, ViscaError, ViscaTransport,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Configuration for the connection pool.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Configuration for automatic reconnection.
    pub reconnection_config: ReconnectionConfig,
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
            reconnection_config: ReconnectionConfig::default(),
            health_check_interval: Duration::from_secs(60),
            auto_remove_unhealthy: false,
            max_idle_time: Some(Duration::from_secs(300)),
        }
    }
}

/// A pooled connection that tracks usage and health.
struct PooledConnection<T> {
    transport: Arc<Mutex<ReconnectingTransport<T>>>,
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
    /// Connection statistics from the underlying transport.
    pub connection_stats: crate::ConnectionStats,
}

/// A pool of VISCA camera connections.
/// Factory function type for creating transports.
type TransportFactory<T> = Arc<dyn Fn(&str) -> Result<T, ViscaError> + Send + Sync>;

/// A pool of VISCA camera connections.
pub struct ViscaConnectionPool<T>
where
    T: ViscaTransport + ConnectionManagement + Send + 'static,
{
    connections: Arc<Mutex<HashMap<String, PooledConnection<T>>>>,
    config: PoolConfig,
    create_transport: TransportFactory<T>,
}

impl<T> ViscaConnectionPool<T>
where
    T: ViscaTransport + ConnectionManagement + Send + 'static,
{
    /// Creates a new connection pool.
    ///
    /// # Arguments
    ///
    /// * `config` - Pool configuration
    /// * `create_transport` - Factory function to create new transports
    pub fn new<F>(config: PoolConfig, create_transport: F) -> Self
    where
        F: Fn(&str) -> Result<T, ViscaError> + Send + Sync + 'static,
    {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            config,
            create_transport: Arc::new(create_transport),
        }
    }

    /// Adds a camera to the pool.
    ///
    /// # Arguments
    ///
    /// * `camera_id` - Unique identifier for the camera
    /// * `address` - Network address of the camera (e.g., "192.168.1.100:5678")
    /// * `info` - Camera information
    pub fn add_camera(
        &self,
        camera_id: impl Into<String>,
        address: &str,
        info: CameraInfo,
    ) -> Result<(), ViscaError> {
        let camera_id = camera_id.into();
        let create_fn = self.create_transport.clone();
        let address = address.to_string();

        let transport = ReconnectingTransport::new(
            move || create_fn(&address),
            self.config.reconnection_config.clone(),
        )?;

        let pooled = PooledConnection {
            transport: Arc::new(Mutex::new(transport)),
            last_used: Instant::now(),
            camera_info: info,
        };

        let mut connections = self.connections.lock().unwrap();
        connections.insert(camera_id, pooled);

        Ok(())
    }

    /// Removes a camera from the pool.
    pub fn remove_camera(&self, camera_id: &str) -> Option<CameraInfo> {
        let mut connections = self.connections.lock().unwrap();
        connections.remove(camera_id).map(|conn| conn.camera_info)
    }

    /// Gets a connection from the pool.
    ///
    /// Returns a guard that provides access to the transport and automatically
    /// updates the `last_used` timestamp when dropped.
    pub fn get_connection(&self, camera_id: &str) -> Result<PooledConnectionGuard<T>, ViscaError> {
        let connections = self.connections.lock().unwrap();

        let pooled = connections.get(camera_id).ok_or_else(|| {
            ViscaError::InvalidParameter(format!("Camera '{}' not found in pool", camera_id))
        })?;

        Ok(PooledConnectionGuard {
            transport: pooled.transport.clone(),
            camera_id: camera_id.to_string(),
            pool: self.connections.clone(),
        })
    }

    /// Gets a list of all cameras in the pool.
    pub fn list_cameras(&self) -> Vec<String> {
        let connections = self.connections.lock().unwrap();
        connections.keys().cloned().collect()
    }

    /// Gets statistics for all cameras in the pool.
    pub fn get_all_stats(&self) -> Vec<PooledCameraStats> {
        let connections = self.connections.lock().unwrap();

        connections
            .iter()
            .map(|(_id, conn)| {
                let mut transport = conn.transport.lock().unwrap();
                let is_healthy = transport.is_healthy().unwrap_or(false);
                let stats = transport.combined_stats();

                PooledCameraStats {
                    info: conn.camera_info.clone(),
                    is_healthy,
                    last_used: conn.last_used,
                    connection_stats: stats,
                }
            })
            .collect()
    }

    /// Performs health checks on all connections.
    ///
    /// Returns a map of camera IDs to their health status.
    pub fn health_check_all(&self) -> HashMap<String, bool> {
        let connections = self.connections.lock().unwrap();
        let mut results = HashMap::new();

        for (id, conn) in connections.iter() {
            let mut transport = conn.transport.lock().unwrap();
            let is_healthy = transport.is_healthy().unwrap_or(false);
            results.insert(id.clone(), is_healthy);
        }

        results
    }

    /// Removes all unhealthy connections from the pool.
    ///
    /// Returns the IDs of removed cameras.
    pub fn remove_unhealthy(&self) -> Vec<String> {
        let health_results = self.health_check_all();
        let mut removed = Vec::new();

        let mut connections = self.connections.lock().unwrap();
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
    pub fn remove_stale(&self) -> Vec<String> {
        self.config.max_idle_time.map_or_else(Vec::new, |max_idle| {
            let now = Instant::now();
            let mut connections = self.connections.lock().unwrap();
            let mut removed = Vec::new();

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

/// A guard that provides access to a pooled connection.
///
/// Automatically updates the `last_used` timestamp when dropped.
pub struct PooledConnectionGuard<T> {
    transport: Arc<Mutex<ReconnectingTransport<T>>>,
    camera_id: String,
    pool: Arc<Mutex<HashMap<String, PooledConnection<T>>>>,
}

impl<T> PooledConnectionGuard<T>
where
    T: ViscaTransport + ConnectionManagement,
{
    /// Gets access to the underlying transport.
    pub fn transport(&self) -> std::sync::MutexGuard<'_, ReconnectingTransport<T>> {
        self.transport.lock().unwrap()
    }
}

impl<T> Drop for PooledConnectionGuard<T> {
    fn drop(&mut self) {
        // Update last_used timestamp
        if let Ok(mut connections) = self.pool.lock() {
            if let Some(conn) = connections.get_mut(&self.camera_id) {
                conn.last_used = Instant::now();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UdpTransport;

    fn create_test_transport(addr: &str) -> Result<UdpTransport, ViscaError> {
        UdpTransport::new(addr).map_err(ViscaError::Io)
    }

    #[test]
    fn test_pool_creation() {
        let config = PoolConfig::default();
        let pool: ViscaConnectionPool<UdpTransport> =
            ViscaConnectionPool::new(config, create_test_transport);

        assert_eq!(pool.list_cameras().len(), 0);
    }

    #[test]
    fn test_add_remove_camera() {
        let config = PoolConfig::default();
        let pool = ViscaConnectionPool::new(config, create_test_transport);

        let info = CameraInfo {
            id: "cam1".to_string(),
            name: Some("Front Camera".to_string()),
            model: Some("PTZOptics G2".to_string()),
            location: Some("Main Stage".to_string()),
        };

        // Add camera
        pool.add_camera("cam1", "192.168.1.100:1259", info.clone())
            .unwrap();
        assert_eq!(pool.list_cameras().len(), 1);

        // Remove camera
        let removed_info = pool.remove_camera("cam1").unwrap();
        assert_eq!(removed_info.name, info.name);
        assert_eq!(pool.list_cameras().len(), 0);
    }

    #[test]
    fn test_get_connection() {
        let config = PoolConfig::default();
        let pool = ViscaConnectionPool::new(config, create_test_transport);

        let info = CameraInfo {
            id: "cam1".to_string(),
            name: None,
            model: None,
            location: None,
        };

        pool.add_camera("cam1", "192.168.1.100:1259", info).unwrap();

        // Get connection
        {
            let _guard = pool.get_connection("cam1").unwrap();
            // Connection is now in use
        }
        // Connection guard dropped, last_used updated

        // Try to get non-existent camera
        assert!(pool.get_connection("cam2").is_err());
    }

    #[test]
    fn test_multiple_cameras() {
        let config = PoolConfig::default();
        let pool = ViscaConnectionPool::new(config, create_test_transport);

        // Add multiple cameras
        for i in 1..=3 {
            let info = CameraInfo {
                id: format!("cam{}", i),
                name: Some(format!("Camera {}", i)),
                model: None,
                location: None,
            };
            pool.add_camera(
                format!("cam{}", i),
                &format!("192.168.1.10{}:1259", i),
                info,
            )
            .unwrap();
        }

        let cameras = pool.list_cameras();
        assert_eq!(cameras.len(), 3);

        // Get stats for all
        let stats = pool.get_all_stats();
        assert_eq!(stats.len(), 3);
    }
}
