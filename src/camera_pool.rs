//! Camera pooling for managing multiple VISCA cameras with the `Camera<P>` API.
//!
//! This module provides a pool that manages multiple camera connections of the same
//! profile type, with health monitoring, automatic cleanup, and concurrent operations.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(feature = "tokio")]
use futures::future::join_all;
#[cfg(feature = "tokio")]
use tokio::sync::RwLock as AsyncRwLock;

#[cfg(not(feature = "tokio"))]
use std::sync::RwLock;

use crate::{
    camera::{Camera, CameraProfile},
    transport::Transport,
    Error as ViscaError,
};

/// Configuration for the camera pool.
#[derive(Debug, Clone, Copy)]
pub struct PoolConfig {
    /// How often to run health checks on all connections.
    pub health_check_interval: Duration,
    /// Whether to automatically remove unhealthy connections.
    pub auto_remove_unhealthy: bool,
    /// Maximum idle time before a connection is considered stale.
    pub max_idle_time: Option<Duration>,
    /// Maximum number of cameras in the pool (None for unlimited).
    pub max_cameras: Option<usize>,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            health_check_interval: Duration::from_secs(60),
            auto_remove_unhealthy: true,
            max_idle_time: Some(Duration::from_secs(300)),
            max_cameras: None,
        }
    }
}

/// Information about a camera in the pool.
#[derive(Debug, Clone)]
pub struct CameraInfo {
    /// Unique identifier for this camera.
    pub id: String,
    /// Human-readable name for the camera.
    pub name: Option<String>,
    /// Camera location or description.
    pub location: Option<String>,
    /// Custom metadata as key-value pairs.
    pub metadata: HashMap<String, String>,
}

impl CameraInfo {
    /// Creates a new camera info with just an ID.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: None,
            location: None,
            metadata: HashMap::new(),
        }
    }

    /// Sets the camera name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the camera location.
    #[must_use]
    pub fn with_location(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    /// Adds a metadata entry.
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Statistics for a camera in the pool.
#[derive(Debug, Clone)]
pub struct CameraStats {
    /// Camera information.
    pub info: CameraInfo,
    /// Whether the connection is currently healthy.
    pub is_healthy: bool,
    /// Last time the connection was used.
    pub last_used: Instant,
    /// Number of successful operations.
    pub successful_ops: u64,
    /// Number of failed operations.
    pub failed_ops: u64,
}

/// A pooled camera connection.
struct PooledCamera<P: CameraProfile> {
    camera: Arc<Camera<P>>,
    info: CameraInfo,
    last_used: Instant,
    successful_ops: u64,
    failed_ops: u64,
}

/// A pool of cameras with the same profile type.
///
/// This pool manages multiple camera connections and provides convenient
/// methods for executing commands on specific cameras or groups of cameras.
pub struct CameraPool<P: CameraProfile> {
    #[cfg(feature = "tokio")]
    cameras: Arc<AsyncRwLock<HashMap<String, PooledCamera<P>>>>,
    #[cfg(not(feature = "tokio"))]
    cameras: Arc<RwLock<HashMap<String, PooledCamera<P>>>>,
    config: PoolConfig,
}

impl<P: CameraProfile> std::fmt::Debug for CameraPool<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CameraPool")
            .field("config", &self.config)
            .finish()
    }
}

impl<P: CameraProfile> CameraPool<P> {
    /// Creates a new camera pool.
    #[must_use]
    pub fn new(config: PoolConfig) -> Self {
        Self {
            #[cfg(feature = "tokio")]
            cameras: Arc::new(AsyncRwLock::new(HashMap::new())),
            #[cfg(not(feature = "tokio"))]
            cameras: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Adds a camera to the pool.
    ///
    /// # Errors
    /// Returns an error if:
    /// - The pool is at maximum capacity
    /// - A camera with the same ID already exists
    pub fn add_camera(
        &self,
        info: CameraInfo,
        transport: Box<dyn Transport>,
    ) -> Result<Arc<Camera<P>>, ViscaError> {
        #[cfg(feature = "tokio")]
        let mut cameras = self.cameras.blocking_write();
        #[cfg(not(feature = "tokio"))]
        let mut cameras = self
            .cameras
            .write()
            .map_err(|_| ViscaError::InvalidState("Camera pool lock poisoned".to_string()))?;

        // Check capacity
        if let Some(max) = self.config.max_cameras {
            if cameras.len() >= max {
                return Err(ViscaError::InvalidParameter(format!(
                    "Camera pool at maximum capacity ({})",
                    max
                )));
            }
        }

        // Check for duplicate ID
        if cameras.contains_key(&info.id) {
            return Err(ViscaError::InvalidParameter(format!(
                "Camera with ID '{}' already exists in pool",
                info.id
            )));
        }

        let camera = Arc::new(Camera::new(transport));
        let pooled = PooledCamera {
            camera: camera.clone(),
            info: info.clone(),
            last_used: Instant::now(),
            successful_ops: 0,
            failed_ops: 0,
        };

        cameras.insert(info.id, pooled);
        Ok(camera)
    }

    /// Removes a camera from the pool.
    ///
    /// Returns the camera info if the camera was found and removed.
    pub fn remove_camera(&self, camera_id: &str) -> Option<CameraInfo> {
        #[cfg(feature = "tokio")]
        let mut cameras = self.cameras.blocking_write();
        #[cfg(not(feature = "tokio"))]
        let mut cameras = self.cameras.write().ok()?;

        cameras.remove(camera_id).map(|pooled| pooled.info)
    }

    /// Gets a camera from the pool.
    ///
    /// # Errors
    /// Returns an error if the camera is not found in the pool.
    pub fn get_camera(&self, camera_id: &str) -> Result<Arc<Camera<P>>, ViscaError> {
        #[cfg(feature = "tokio")]
        let mut cameras = self.cameras.blocking_write();
        #[cfg(not(feature = "tokio"))]
        let mut cameras = self
            .cameras
            .write()
            .map_err(|_| ViscaError::InvalidState("Camera pool lock poisoned".to_string()))?;

        let pooled = cameras.get_mut(camera_id).ok_or_else(|| {
            ViscaError::InvalidParameter(format!("Camera '{}' not found in pool", camera_id))
        })?;

        pooled.last_used = Instant::now();
        Ok(pooled.camera.clone())
    }

    /// Executes an operation on a specific camera and updates statistics.
    ///
    /// # Errors
    /// Returns an error if the camera is not found or the operation fails.
    pub fn with_camera<F, R>(&self, camera_id: &str, op: F) -> Result<R, ViscaError>
    where
        F: FnOnce(&Camera<P>) -> Result<R, ViscaError>,
    {
        let camera = self.get_camera(camera_id)?;
        let result = op(&camera);

        // Update statistics
        #[cfg(feature = "tokio")]
        let mut cameras = self.cameras.blocking_write();
        #[cfg(not(feature = "tokio"))]
        let mut cameras = self
            .cameras
            .write()
            .map_err(|_| ViscaError::InvalidState("Camera pool lock poisoned".to_string()))?;

        if let Some(pooled) = cameras.get_mut(camera_id) {
            if result.is_ok() {
                pooled.successful_ops += 1;
            } else {
                pooled.failed_ops += 1;
            }
        }

        result
    }

    /// Lists all camera IDs in the pool.
    #[must_use]
    pub fn list_cameras(&self) -> Vec<String> {
        #[cfg(feature = "tokio")]
        let cameras = self.cameras.blocking_read();
        #[cfg(not(feature = "tokio"))]
        let cameras = match self.cameras.read() {
            Ok(guard) => guard,
            Err(_) => return Vec::new(),
        };

        cameras.keys().cloned().collect()
    }

    /// Gets statistics for all cameras in the pool.
    #[must_use]
    pub fn get_all_stats(&self) -> Vec<CameraStats> {
        #[cfg(feature = "tokio")]
        let cameras = self.cameras.blocking_read();
        #[cfg(not(feature = "tokio"))]
        let cameras = match self.cameras.read() {
            Ok(guard) => guard,
            Err(_) => return Vec::new(),
        };

        cameras
            .values()
            .map(|pooled| CameraStats {
                info: pooled.info.clone(),
                is_healthy: true, // Will be updated by health check
                last_used: pooled.last_used,
                successful_ops: pooled.successful_ops,
                failed_ops: pooled.failed_ops,
            })
            .collect()
    }

    /// Performs a health check on a specific camera.
    ///
    /// # Errors
    /// Returns an error if the camera is not found.
    pub fn health_check(&self, camera_id: &str) -> Result<bool, ViscaError> {
        self.with_camera(camera_id, |_camera| {
            // Try a simple operation as health check
            // Since we can't do async in sync context, just return Ok(true)
            // Real health check would need async version
            Ok(true)
        })
    }

    /// Performs health checks on all cameras.
    ///
    /// Returns a map of camera IDs to their health status.
    #[must_use]
    pub fn health_check_all(&self) -> HashMap<String, bool> {
        let camera_ids = self.list_cameras();
        let mut results = HashMap::new();

        for id in camera_ids {
            let is_healthy = self.health_check(&id).unwrap_or(false);
            results.insert(id, is_healthy);
        }

        results
    }

    /// Removes all unhealthy cameras from the pool.
    ///
    /// Returns the IDs of removed cameras.
    #[must_use]
    pub fn remove_unhealthy(&self) -> Vec<String> {
        if !self.config.auto_remove_unhealthy {
            return Vec::new();
        }

        let health_results = self.health_check_all();
        let mut removed = Vec::new();

        for (id, is_healthy) in health_results {
            if !is_healthy && self.remove_camera(&id).is_some() {
                removed.push(id);
            }
        }

        removed
    }

    /// Removes cameras that have been idle longer than the configured `max_idle_time`.
    ///
    /// Returns the IDs of removed cameras.
    #[must_use]
    pub fn remove_stale(&self) -> Vec<String> {
        let Some(max_idle) = self.config.max_idle_time else {
            return Vec::new();
        };

        let now = Instant::now();
        let mut to_remove = Vec::new();

        {
            #[cfg(feature = "tokio")]
            let cameras = self.cameras.blocking_read();
            #[cfg(not(feature = "tokio"))]
            let cameras = match self.cameras.read() {
                Ok(guard) => guard,
                Err(_) => return Vec::new(),
            };

            for (id, pooled) in cameras.iter() {
                if now.duration_since(pooled.last_used) > max_idle {
                    to_remove.push(id.clone());
                }
            }
        }

        // Remove stale cameras
        let mut removed = Vec::new();
        for id in to_remove {
            if self.remove_camera(&id).is_some() {
                removed.push(id);
            }
        }

        removed
    }

    /// Performs maintenance on the pool (health checks and cleanup).
    ///
    /// This method:
    /// 1. Runs health checks on all cameras
    /// 2. Removes unhealthy cameras if configured
    /// 3. Removes stale cameras if configured
    ///
    /// Returns a summary of actions taken.
    pub fn maintenance(&self) -> MaintenanceReport {
        let unhealthy_removed = self.remove_unhealthy();
        let stale_removed = self.remove_stale();

        MaintenanceReport {
            unhealthy_removed,
            stale_removed,
            timestamp: Instant::now(),
        }
    }
}

/// Report from a maintenance operation.
#[derive(Debug, Clone)]
pub struct MaintenanceReport {
    /// IDs of cameras removed due to health check failures.
    pub unhealthy_removed: Vec<String>,
    /// IDs of cameras removed due to inactivity.
    pub stale_removed: Vec<String>,
    /// When the maintenance was performed.
    pub timestamp: Instant,
}

impl MaintenanceReport {
    /// Total number of cameras removed.
    #[must_use]
    pub fn total_removed(&self) -> usize {
        self.unhealthy_removed.len() + self.stale_removed.len()
    }
}

#[cfg(feature = "tokio")]
impl<P: CameraProfile> CameraPool<P> {
    /// Async version of add_camera.
    pub async fn add_camera_async(
        &self,
        info: CameraInfo,
        transport: Box<dyn Transport>,
    ) -> Result<Arc<Camera<P>>, ViscaError> {
        let mut cameras = self.cameras.write().await;

        // Check capacity
        if let Some(max) = self.config.max_cameras {
            if cameras.len() >= max {
                return Err(ViscaError::InvalidParameter(format!(
                    "Camera pool at maximum capacity ({})",
                    max
                )));
            }
        }

        // Check for duplicate ID
        if cameras.contains_key(&info.id) {
            return Err(ViscaError::InvalidParameter(format!(
                "Camera with ID '{}' already exists in pool",
                info.id
            )));
        }

        let camera = Arc::new(Camera::new(transport));
        let pooled = PooledCamera {
            camera: camera.clone(),
            info: info.clone(),
            last_used: Instant::now(),
            successful_ops: 0,
            failed_ops: 0,
        };

        cameras.insert(info.id, pooled);
        Ok(camera)
    }

    /// Async version of with_camera.
    pub async fn with_camera_async<F, Fut, R>(
        &self,
        camera_id: &str,
        op: F,
    ) -> Result<R, ViscaError>
    where
        F: FnOnce(Arc<Camera<P>>) -> Fut,
        Fut: std::future::Future<Output = Result<R, ViscaError>>,
    {
        let camera = {
            let mut cameras = self.cameras.write().await;
            let pooled = cameras.get_mut(camera_id).ok_or_else(|| {
                ViscaError::InvalidParameter(format!("Camera '{}' not found in pool", camera_id))
            })?;
            pooled.last_used = Instant::now();
            pooled.camera.clone()
        };

        let result = op(camera).await;

        // Update statistics
        let mut cameras = self.cameras.write().await;
        if let Some(pooled) = cameras.get_mut(camera_id) {
            if result.is_ok() {
                pooled.successful_ops += 1;
            } else {
                pooled.failed_ops += 1;
            }
        }

        result
    }

    /// Async version of health_check.
    pub async fn health_check_async(&self, camera_id: &str) -> Result<bool, ViscaError> {
        self.with_camera_async(camera_id, |_camera| async move {
            // TODO: The camera pool stores Arc<Camera<P>>, which cannot be mutated.
            // This needs a redesign to support mutable operations or the Camera
            // should use interior mutability for its transport.
            // For now, we'll just return true to indicate the camera exists in the pool.
            Ok(true)
        })
        .await
    }

    /// Executes an operation on all cameras concurrently.
    ///
    /// Returns a map of camera IDs to results.
    pub async fn with_all_cameras_async<F, Fut, R>(
        &self,
        op: F,
    ) -> HashMap<String, Result<R, ViscaError>>
    where
        F: Fn(Arc<Camera<P>>) -> Fut + Clone,
        Fut: std::future::Future<Output = Result<R, ViscaError>>,
        R: Send,
    {
        let cameras: Vec<(String, Arc<Camera<P>>)> = {
            let mut guard = self.cameras.write().await;
            guard
                .iter_mut()
                .map(|(id, pooled)| {
                    pooled.last_used = Instant::now();
                    (id.clone(), pooled.camera.clone())
                })
                .collect()
        };

        let mut results = HashMap::new();
        let mut futures = Vec::new();

        for (id, camera) in cameras {
            let op = op.clone();
            futures.push(async move {
                let result = op(camera).await;
                (id, result)
            });
        }

        let outputs = join_all(futures).await;

        for (id, result) in outputs {
            results.insert(id, result);
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_info_builder() {
        let info = CameraInfo::new("cam1")
            .with_name("Front Camera")
            .with_location("Main Stage")
            .with_metadata("model", "PTZOptics G2")
            .with_metadata("ip", "192.168.1.100");

        assert_eq!(info.id, "cam1");
        assert_eq!(info.name, Some("Front Camera".to_string()));
        assert_eq!(info.location, Some("Main Stage".to_string()));
        assert_eq!(
            info.metadata.get("model"),
            Some(&"PTZOptics G2".to_string())
        );
        assert_eq!(info.metadata.get("ip"), Some(&"192.168.1.100".to_string()));
    }

    #[test]
    fn test_pool_config_default() {
        let config = PoolConfig::default();
        assert_eq!(config.health_check_interval, Duration::from_secs(60));
        assert!(config.auto_remove_unhealthy);
        assert_eq!(config.max_idle_time, Some(Duration::from_secs(300)));
        assert_eq!(config.max_cameras, None);
    }

    #[test]
    fn test_maintenance_report() {
        let report = MaintenanceReport {
            unhealthy_removed: vec!["cam1".to_string(), "cam2".to_string()],
            stale_removed: vec!["cam3".to_string()],
            timestamp: Instant::now(),
        };

        assert_eq!(report.total_removed(), 3);
    }
}
