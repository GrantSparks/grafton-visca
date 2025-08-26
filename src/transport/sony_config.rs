//! Shared Sony VISCA-over-IP transport configuration.
//!
//! Exposes `SonyIpConfig` for both blocking and async transports to avoid
//! duplicating the struct across implementations.

use std::time::Duration;

/// Configuration for Sony encapsulated IP transports (shared by blocking and async).
#[derive(Debug, Clone)]
pub struct SonyIpConfig {
    /// Remote address and port.
    pub address: String,
    /// Connection timeout.
    pub connect_timeout: Duration,
    /// Read timeout.
    pub read_timeout: Duration,
    /// Write timeout.
    pub write_timeout: Duration,
    /// Maximum retries for failed commands.
    pub max_retries: u32,
    /// Timeout for waiting for a response.
    pub response_timeout: Duration,
    /// Whether to use TCP (true) or UDP (false).
    pub use_tcp: bool,
}

impl Default for SonyIpConfig {
    fn default() -> Self {
        Self {
            address: "192.168.0.110:52381".to_string(), // Sony default port
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_millis(100),
            write_timeout: Duration::from_millis(100),
            max_retries: 3,
            response_timeout: Duration::from_secs(2),
            use_tcp: true,
        }
    }
}
