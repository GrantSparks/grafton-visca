//! Unified serial port configuration for VISCA communication.
//!
//! This module provides a single configuration type used by both
//! blocking and async serial implementations, eliminating duplication
//! and ensuring consistency.

use std::time::Duration;

use crate::transport::{buffer::BufferConfig, RetryConfig};

/// Serial port configuration for VISCA communication.
///
/// This unified configuration is used by both blocking and async serial
/// transports, ensuring consistent behavior across different runtime modes.
#[derive(Debug, Clone)]
pub struct Config {
    /// Serial port path (e.g., "/dev/ttyUSB0" on Unix, "COM1" on Windows).
    pub port: String,
    /// Baud rate (typically 9600 or 38400 for VISCA).
    pub baud_rate: u32,
    /// Camera address (1-7 for RS-232, 1-112 for RS-422).
    pub camera_address: u8,
    /// Whether to perform I/F Clear on connect.
    pub if_clear_on_connect: bool,
    /// Whether to perform Address Set on connect.
    pub address_set_on_connect: bool,
    /// Read timeout for serial operations.
    pub read_timeout: Duration,
    /// Write timeout for serial operations.
    pub write_timeout: Duration,
    /// Retry configuration for serial operations.
    pub retry_config: RetryConfig,
    /// Buffer configuration for managing buffers.
    pub buffer_config: BufferConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: "/dev/ttyUSB0".to_string(),
            baud_rate: 9600,
            camera_address: 1,
            if_clear_on_connect: true,
            address_set_on_connect: false,
            read_timeout: Duration::from_millis(100),
            write_timeout: Duration::from_millis(100),
            retry_config: RetryConfig::default(),
            buffer_config: BufferConfig::default(),
        }
    }
}

impl Config {
    /// Create a new serial configuration with default settings.
    pub fn new(port: impl Into<String>) -> Self {
        Self {
            port: port.into(),
            ..Default::default()
        }
    }

    /// Set the baud rate.
    pub fn baud_rate(mut self, baud_rate: u32) -> Self {
        self.baud_rate = baud_rate;
        self
    }

    /// Set the camera address.
    pub fn camera_address(mut self, address: u8) -> Self {
        self.camera_address = address;
        self
    }

    /// Enable or disable I/F Clear on connect.
    pub fn if_clear_on_connect(mut self, enable: bool) -> Self {
        self.if_clear_on_connect = enable;
        self
    }

    /// Enable or disable Address Set on connect.
    pub fn address_set_on_connect(mut self, enable: bool) -> Self {
        self.address_set_on_connect = enable;
        self
    }

    /// Set the read timeout.
    pub fn read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = timeout;
        self
    }

    /// Set the write timeout.
    pub fn write_timeout(mut self, timeout: Duration) -> Self {
        self.write_timeout = timeout;
        self
    }

    /// Set the retry configuration.
    pub fn retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    /// Set the buffer configuration.
    pub fn buffer_config(mut self, config: BufferConfig) -> Self {
        self.buffer_config = config;
        self
    }
}
