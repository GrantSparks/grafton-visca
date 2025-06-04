use std::time::{Duration, Instant};

/// Statistics about a VISCA transport connection
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
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
            connected_since: Some(Instant::now()),
            ..Default::default()
        }
    }

    /// Get the connection uptime
    pub fn uptime(&self) -> Option<Duration> {
        self.connected_since.map(|since| since.elapsed())
    }

    /// Get the time since last activity
    pub fn idle_time(&self) -> Option<Duration> {
        self.last_activity.map(|last| last.elapsed())
    }

    /// Record a sent command
    pub fn record_sent(&mut self, bytes: usize) {
        self.bytes_sent += bytes as u64;
        self.commands_sent += 1;
        self.last_activity = Some(Instant::now());
    }

    /// Record a received response
    pub fn record_received(&mut self, bytes: usize) {
        self.bytes_received += bytes as u64;
        self.responses_received += 1;
        self.last_activity = Some(Instant::now());
    }

    /// Record an error
    pub fn record_error(&mut self) {
        self.error_count += 1;
        self.last_error = Some(Instant::now());
    }

    /// Reset statistics (useful after reconnection)
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

/// Extension trait for VISCA transports to add connection management capabilities
pub trait ConnectionManagement {
    /// Check if the connection is healthy by sending a simple inquiry
    fn is_healthy(&mut self) -> bool;

    /// Get connection statistics
    fn connection_stats(&self) -> &ConnectionStats;

    /// Get mutable connection statistics
    fn connection_stats_mut(&mut self) -> &mut ConnectionStats;
}

#[cfg(feature = "async")]
use crate::async_transport::TransportFuture;

/// Async version of the connection management trait
#[cfg(feature = "async")]
pub trait AsyncConnectionManagement: Send + Sync {
    /// Check if the connection is healthy by sending a simple inquiry
    fn is_healthy(&mut self) -> TransportFuture<'_, bool>;

    /// Get connection statistics
    fn connection_stats(&self) -> &ConnectionStats;

    /// Get mutable connection statistics  
    fn connection_stats_mut(&mut self) -> &mut ConnectionStats;
}
