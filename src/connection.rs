use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Statistics about a VISCA transport connection
#[derive(Debug)]
pub struct ConnectionStats {
    /// When the connection was established
    connected_since: Arc<RwLock<Option<Instant>>>,
    /// Total bytes sent
    bytes_sent: AtomicU64,
    /// Total bytes received
    bytes_received: AtomicU64,
    /// Total commands sent
    commands_sent: AtomicU64,
    /// Total responses received
    responses_received: AtomicU64,
    /// Total errors encountered
    error_count: AtomicU64,
    /// Last successful activity timestamp
    last_activity: Arc<RwLock<Option<Instant>>>,
    /// Last error timestamp
    last_error: Arc<RwLock<Option<Instant>>>,
    /// Last health check timestamp
    last_health_check: Arc<RwLock<Option<Instant>>>,
    /// Last health check result
    last_health_result: Arc<RwLock<Option<bool>>>,
}

/// A snapshot of connection statistics at a point in time
#[derive(Debug, Clone)]
pub struct ConnectionStatsSnapshot {
    /// When the connection was established
    pub connected_since: Option<Instant>,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Total commands sent
    pub commands_sent: u64,
    /// Total responses received
    pub responses_received: u64,
    /// Total errors encountered
    pub error_count: u64,
    /// Last successful activity timestamp
    pub last_activity: Option<Instant>,
    /// Last error timestamp
    pub last_error: Option<Instant>,
}

impl ConnectionStats {
    /// Create new connection statistics starting now
    pub fn new() -> Self {
        Self {
            connected_since: Arc::new(RwLock::new(Some(Instant::now()))),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            commands_sent: AtomicU64::new(0),
            responses_received: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            last_activity: Arc::new(RwLock::new(None)),
            last_error: Arc::new(RwLock::new(None)),
            last_health_check: Arc::new(RwLock::new(None)),
            last_health_result: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the connection uptime
    pub fn uptime(&self) -> Option<Duration> {
        self.connected_since
            .read()
            .ok()?
            .as_ref()
            .map(|since| since.elapsed())
    }

    /// Get the time since last activity
    pub fn idle_time(&self) -> Option<Duration> {
        self.last_activity
            .read()
            .ok()?
            .as_ref()
            .map(|last| last.elapsed())
    }

    /// Record a sent command
    pub fn record_sent(&self, bytes: usize) {
        self.bytes_sent.fetch_add(bytes as u64, Ordering::Relaxed);
        self.commands_sent.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut last_activity) = self.last_activity.write() {
            *last_activity = Some(Instant::now());
        }
    }

    /// Record a received response
    pub fn record_received(&self, bytes: usize) {
        self.bytes_received
            .fetch_add(bytes as u64, Ordering::Relaxed);
        self.responses_received.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut last_activity) = self.last_activity.write() {
            *last_activity = Some(Instant::now());
        }
    }

    /// Record an error
    pub fn record_error(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut last_error) = self.last_error.write() {
            *last_error = Some(Instant::now());
        }
    }

    /// Reset statistics (useful after reconnection)
    pub fn reset(&self) {
        if let Ok(mut connected_since) = self.connected_since.write() {
            *connected_since = Some(Instant::now());
        }
        self.bytes_sent.store(0, Ordering::Relaxed);
        self.bytes_received.store(0, Ordering::Relaxed);
        self.commands_sent.store(0, Ordering::Relaxed);
        self.responses_received.store(0, Ordering::Relaxed);
        self.error_count.store(0, Ordering::Relaxed);
        if let Ok(mut last_activity) = self.last_activity.write() {
            *last_activity = None;
        }
        if let Ok(mut last_error) = self.last_error.write() {
            *last_error = None;
        }
        if let Ok(mut last_health_check) = self.last_health_check.write() {
            *last_health_check = None;
        }
        if let Ok(mut last_health_result) = self.last_health_result.write() {
            *last_health_result = None;
        }
    }

    /// Get a consistent snapshot of all statistics
    pub fn snapshot(&self) -> ConnectionStatsSnapshot {
        ConnectionStatsSnapshot {
            connected_since: self.connected_since.read().ok().and_then(|g| *g),
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            commands_sent: self.commands_sent.load(Ordering::Relaxed),
            responses_received: self.responses_received.load(Ordering::Relaxed),
            error_count: self.error_count.load(Ordering::Relaxed),
            last_activity: self.last_activity.read().ok().and_then(|g| *g),
            last_error: self.last_error.read().ok().and_then(|g| *g),
        }
    }

    /// Record a health check result
    pub fn record_health_check(&self, healthy: bool) {
        if let Ok(mut last_health_check) = self.last_health_check.write() {
            *last_health_check = Some(Instant::now());
        }
        if let Ok(mut last_health_result) = self.last_health_result.write() {
            *last_health_result = Some(healthy);
        }
    }

    /// Get the last health check result if still valid (within 5 seconds)
    pub fn get_cached_health(&self) -> Option<bool> {
        if let (Ok(last_check), Ok(last_result)) = (
            self.last_health_check.read(),
            self.last_health_result.read(),
        ) {
            if let (Some(check_time), Some(result)) = (last_check.as_ref(), last_result.as_ref()) {
                if check_time.elapsed() < Duration::from_secs(5) {
                    return Some(*result);
                }
            }
        }
        None
    }
}

// Implement Clone manually due to Arc<RwLock<>> fields
impl Clone for ConnectionStats {
    fn clone(&self) -> Self {
        Self {
            connected_since: Arc::clone(&self.connected_since),
            bytes_sent: AtomicU64::new(self.bytes_sent.load(Ordering::Relaxed)),
            bytes_received: AtomicU64::new(self.bytes_received.load(Ordering::Relaxed)),
            commands_sent: AtomicU64::new(self.commands_sent.load(Ordering::Relaxed)),
            responses_received: AtomicU64::new(self.responses_received.load(Ordering::Relaxed)),
            error_count: AtomicU64::new(self.error_count.load(Ordering::Relaxed)),
            last_activity: Arc::clone(&self.last_activity),
            last_error: Arc::clone(&self.last_error),
            last_health_check: Arc::clone(&self.last_health_check),
            last_health_result: Arc::clone(&self.last_health_result),
        }
    }
}

// Implement Default for ConnectionStats
impl Default for ConnectionStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension trait for VISCA transports to add connection management capabilities
pub trait ConnectionManagement {
    /// Check if the connection is healthy by sending a simple inquiry
    fn is_healthy(&mut self) -> Result<bool, crate::ViscaError>;

    /// Get connection statistics
    fn connection_stats(&self) -> &ConnectionStats;
}

#[cfg(feature = "async")]
use crate::async_transport::TransportFuture;

/// Async version of the connection management trait
#[cfg(feature = "async")]
pub trait AsyncConnectionManagement: Send + Sync {
    /// Check if the connection is healthy by sending a simple inquiry
    fn is_healthy(&mut self) -> TransportFuture<'_, Result<bool, crate::ViscaError>>;

    /// Get connection statistics
    fn connection_stats(&self) -> &ConnectionStats;
}
