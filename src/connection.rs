// Standard library imports
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, RwLock,
    },
    time::{Duration, Instant},
};

/// Statistics about a VISCA transport connection
#[derive(Debug)]
pub struct ConnectionStats {
    /// Shared state between clones
    inner: Arc<ConnectionStatsInner>,
}

#[derive(Debug)]
struct ConnectionStatsInner {
    /// When the connection was established
    connected_since: RwLock<Option<Instant>>,
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
    last_activity: RwLock<Option<Instant>>,
    /// Last error timestamp
    last_error: RwLock<Option<Instant>>,
    /// Last health check timestamp
    last_health_check: Mutex<Option<Instant>>,
    /// Last health check result
    last_health_result: Mutex<Option<bool>>,
    /// Cache validity flag
    cache_valid: AtomicBool,
}

/// A snapshot of connection statistics at a point in time
#[derive(Debug, Copy, Clone)]
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
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ConnectionStatsInner {
                connected_since: RwLock::new(Some(Instant::now())),
                bytes_sent: AtomicU64::new(0),
                bytes_received: AtomicU64::new(0),
                commands_sent: AtomicU64::new(0),
                responses_received: AtomicU64::new(0),
                error_count: AtomicU64::new(0),
                last_activity: RwLock::new(None),
                last_error: RwLock::new(None),
                last_health_check: Mutex::new(None),
                last_health_result: Mutex::new(None),
                cache_valid: AtomicBool::new(false),
            }),
        }
    }

    /// Get the connection uptime
    #[must_use]
    pub fn uptime(&self) -> Option<Duration> {
        Self::read_instant(&self.inner.connected_since).map(|since| since.elapsed())
    }

    /// Get the time since last activity
    #[must_use]
    pub fn idle_time(&self) -> Option<Duration> {
        Self::read_instant(&self.inner.last_activity).map(|last| last.elapsed())
    }

    /// Record a sent command
    pub fn record_sent(&self, bytes: usize) {
        let _ = self
            .inner
            .bytes_sent
            .fetch_add(bytes as u64, Ordering::Relaxed);
        let _ = self.inner.commands_sent.fetch_add(1, Ordering::Relaxed);
        Self::write_instant(&self.inner.last_activity, Some(Instant::now()));
    }

    /// Record a received response
    pub fn record_received(&self, bytes: usize) {
        let _ = self
            .inner
            .bytes_received
            .fetch_add(bytes as u64, Ordering::Relaxed);
        let _ = self
            .inner
            .responses_received
            .fetch_add(1, Ordering::Relaxed);
        Self::write_instant(&self.inner.last_activity, Some(Instant::now()));
    }

    /// Record an error
    pub fn record_error(&self) {
        let _ = self.inner.error_count.fetch_add(1, Ordering::Relaxed);
        Self::write_instant(&self.inner.last_error, Some(Instant::now()));
    }

    /// Reset statistics (useful after reconnection)
    pub fn reset(&self) {
        Self::write_instant(&self.inner.connected_since, Some(Instant::now()));
        self.inner.bytes_sent.store(0, Ordering::Relaxed);
        self.inner.bytes_received.store(0, Ordering::Relaxed);
        self.inner.commands_sent.store(0, Ordering::Relaxed);
        self.inner.responses_received.store(0, Ordering::Relaxed);
        self.inner.error_count.store(0, Ordering::Relaxed);
        Self::write_instant(&self.inner.last_activity, None);
        Self::write_instant(&self.inner.last_error, None);
        if let Ok(mut last_health_check) = self.inner.last_health_check.lock() {
            *last_health_check = None;
        }
        if let Ok(mut last_health_result) = self.inner.last_health_result.lock() {
            *last_health_result = None;
        }
        self.inner.cache_valid.store(false, Ordering::Release);
    }

    /// Get a consistent snapshot of all statistics
    #[must_use]
    pub fn snapshot(&self) -> ConnectionStatsSnapshot {
        ConnectionStatsSnapshot {
            connected_since: Self::read_instant(&self.inner.connected_since),
            bytes_sent: self.inner.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.inner.bytes_received.load(Ordering::Relaxed),
            commands_sent: self.inner.commands_sent.load(Ordering::Relaxed),
            responses_received: self.inner.responses_received.load(Ordering::Relaxed),
            error_count: self.inner.error_count.load(Ordering::Relaxed),
            last_activity: Self::read_instant(&self.inner.last_activity),
            last_error: Self::read_instant(&self.inner.last_error),
        }
    }

    /// Record a health check result
    pub fn record_health_check(&self, healthy: bool) {
        let now = Instant::now();
        if let Ok(mut last_health_check) = self.inner.last_health_check.lock() {
            *last_health_check = Some(now);
        }
        if let Ok(mut last_health_result) = self.inner.last_health_result.lock() {
            *last_health_result = Some(healthy);
        }
        self.inner.cache_valid.store(true, Ordering::Release);
    }

    /// Get the last health check result if still valid (within 5 seconds)
    #[must_use]
    pub fn get_cached_health(&self) -> Option<bool> {
        // Check cache validity flag first for fast path
        if !self.inner.cache_valid.load(Ordering::Acquire) {
            return None;
        }

        // Lock both values together to ensure consistency
        let (check_time, result) = {
            let last_check = self.inner.last_health_check.lock().ok()?;
            let last_result = self.inner.last_health_result.lock().ok()?;
            match (last_check.as_ref(), last_result.as_ref()) {
                (Some(time), Some(res)) => (*time, *res),
                _ => return None,
            }
        };

        // Check if still within validity period
        if check_time.elapsed() < Duration::from_secs(5) {
            Some(result)
        } else {
            // Invalidate cache for next check
            self.inner.cache_valid.store(false, Ordering::Release);
            None
        }
    }

    /// Helper method to read instant from `RwLock` with poison recovery
    fn read_instant(lock: &RwLock<Option<Instant>>) -> Option<Instant> {
        match lock.read() {
            Ok(guard) => *guard,
            Err(poisoned) => {
                // Recover from poison by reading the data anyway
                *poisoned.into_inner()
            }
        }
    }

    /// Helper method to write instant to `RwLock` with poison recovery
    fn write_instant(lock: &RwLock<Option<Instant>>, value: Option<Instant>) {
        match lock.write() {
            Ok(mut guard) => *guard = value,
            Err(poisoned) => {
                // Recover from poison by writing the data anyway
                let mut guard = poisoned.into_inner();
                *guard = value;
            }
        }
    }
}

// Clone is now trivial since all state is shared via Arc
impl Clone for ConnectionStats {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
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
    ///
    /// # Errors
    ///
    /// Returns a `ViscaError` if the health check command fails to send or receive a response
    fn is_healthy(&mut self) -> Result<bool, crate::ViscaError>;

    /// Get connection statistics
    fn connection_stats(&self) -> &ConnectionStats;
}

#[cfg(feature = "async-client")]
use crate::async_transport::TransportFuture;

/// Async version of the connection management trait
#[cfg(feature = "async-client")]
pub trait AsyncConnectionManagement: Send + Sync {
    /// Check if the connection is healthy by sending a simple inquiry
    fn is_healthy(&mut self) -> TransportFuture<'_, bool>;

    /// Get connection statistics
    fn connection_stats(&self) -> &ConnectionStats;
}
