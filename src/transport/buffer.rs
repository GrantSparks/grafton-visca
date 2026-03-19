//! Buffer management utilities for VISCA transport layer.
//!
//! This module provides unified buffer management across all transport implementations,
//! ensuring consistent buffer sizes and allocation strategies.

use bytes::BytesMut;

/// Default buffer size for most VISCA operations.
/// VISCA commands are typically small (< 20 bytes) and responses rarely exceed 64 bytes.
pub(crate) const DEFAULT_BUFFER_SIZE: usize = 128;

/// Buffer size for UDP transports.
/// UDP packets can be larger but VISCA over UDP still uses small messages.
pub(crate) const UDP_BUFFER_SIZE: usize = 1024;

/// Buffer size for Sony IP protocol.
/// Sony protocol adds headers requiring slightly larger buffers.
pub(crate) const SONY_BUFFER_SIZE: usize = 512;

/// Buffer size for raw IP protocol.
/// Raw IP protocol may batch multiple commands.
pub(crate) const RAW_IP_BUFFER_SIZE: usize = 256;

/// Buffer size for serial transports.
/// Updated to 256 bytes to match async serial's previous choice and
/// safely accommodate longer VISCA replies.
pub(crate) const SERIAL_BUFFER_SIZE: usize = 256;

/// Configuration for buffer management.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferConfig {
    /// Initial buffer capacity for receive operations.
    pub recv_buffer_size: usize,

    /// Initial buffer capacity for send operations.
    pub send_buffer_size: usize,

    /// Maximum buffer size to prevent unbounded growth.
    pub max_buffer_size: usize,
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            recv_buffer_size: DEFAULT_BUFFER_SIZE,
            send_buffer_size: DEFAULT_BUFFER_SIZE,
            max_buffer_size: 8192, // 8KB max
        }
    }
}

impl BufferConfig {
    /// Create a configuration for UDP transports.
    pub fn for_udp() -> Self {
        Self {
            recv_buffer_size: UDP_BUFFER_SIZE,
            send_buffer_size: UDP_BUFFER_SIZE,
            ..Default::default()
        }
    }

    /// Create a configuration for Sony IP protocol.
    pub fn for_sony_ip() -> Self {
        Self {
            recv_buffer_size: SONY_BUFFER_SIZE,
            send_buffer_size: SONY_BUFFER_SIZE,
            ..Default::default()
        }
    }

    /// Create a configuration for raw IP protocol.
    pub fn for_raw_ip() -> Self {
        Self {
            recv_buffer_size: RAW_IP_BUFFER_SIZE,
            send_buffer_size: RAW_IP_BUFFER_SIZE,
            ..Default::default()
        }
    }

    /// Create a configuration for serial transports.
    pub fn for_serial() -> Self {
        Self {
            recv_buffer_size: SERIAL_BUFFER_SIZE,
            send_buffer_size: SERIAL_BUFFER_SIZE,
            ..Default::default()
        }
    }
}

/// Manager for buffer allocation and lifecycle.
/// Available for all transport configurations.
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct BufferManager {
    config: BufferConfig,
}

impl BufferManager {
    /// Create a new buffer manager with the given configuration.
    pub fn new(config: BufferConfig) -> Self {
        Self { config }
    }

    /// Get the buffer configuration (crate-visible for runtime loop).
    #[inline]
    pub(crate) fn config(&self) -> BufferConfig {
        self.config
    }

    /// Allocate a new receive buffer.
    /// Only used by blocking transports that need BytesMut for receive operations.
    #[cfg(all(not(feature = "mode-async"), test))]
    pub fn alloc_recv_buffer(&self) -> BytesMut {
        BytesMut::with_capacity(self.config.recv_buffer_size)
    }

    /// Allocate a new send buffer.
    /// Available for all transport configurations that need BytesMut.
    /// Some transports don't need send buffers (they send data directly).
    pub fn alloc_send_buffer(&self) -> BytesMut {
        BytesMut::with_capacity(self.config.send_buffer_size)
    }

    /// Resize a buffer if needed, respecting max size limits.
    #[cfg(all(test, not(feature = "mode-async")))]
    pub fn resize_buffer(&self, buffer: &mut BytesMut, required_size: usize) {
        let new_size = required_size.min(self.config.max_buffer_size);
        if buffer.capacity() < new_size {
            // Reserve ensures total capacity is at least new_size
            buffer.reserve(new_size);
        }
    }

    /// Clear and reset a buffer for reuse.
    #[cfg(all(test, not(feature = "mode-async")))]
    pub fn reset_buffer(&self, buffer: &mut BytesMut) {
        buffer.clear();
        // Shrink if buffer has grown too large
        if buffer.capacity() > self.config.recv_buffer_size * 4 {
            buffer.resize(self.config.recv_buffer_size, 0);
            buffer.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_config_defaults() {
        let config = BufferConfig::default();
        assert_eq!(config.recv_buffer_size, DEFAULT_BUFFER_SIZE);
        assert_eq!(config.send_buffer_size, DEFAULT_BUFFER_SIZE);
        assert_eq!(config.max_buffer_size, 8192);
    }

    #[test]
    fn test_buffer_config_for_udp() {
        let config = BufferConfig::for_udp();
        assert_eq!(config.recv_buffer_size, UDP_BUFFER_SIZE);
        assert_eq!(config.send_buffer_size, UDP_BUFFER_SIZE);
    }

    #[test]
    fn test_buffer_config_for_sony_ip() {
        let config = BufferConfig::for_sony_ip();
        assert_eq!(config.recv_buffer_size, SONY_BUFFER_SIZE);
        assert_eq!(config.send_buffer_size, SONY_BUFFER_SIZE);
    }

    // These tests rely on BytesMut and blocking-only allocation helpers
    #[cfg(not(feature = "mode-async"))]
    #[test]
    fn test_buffer_manager_allocation() {
        let manager = BufferManager::new(BufferConfig::default());

        let recv_buf = manager.alloc_recv_buffer();
        assert_eq!(recv_buf.capacity(), DEFAULT_BUFFER_SIZE);

        let send_buf = manager.alloc_send_buffer();
        assert_eq!(send_buf.capacity(), DEFAULT_BUFFER_SIZE);
    }

    #[cfg(not(feature = "mode-async"))]
    #[test]
    fn test_buffer_resize() {
        let manager = BufferManager::new(BufferConfig::default());
        let mut buffer = manager.alloc_recv_buffer();

        // Initial capacity should be DEFAULT_BUFFER_SIZE (128)
        assert_eq!(buffer.capacity(), DEFAULT_BUFFER_SIZE);

        // Test resize within limits
        manager.resize_buffer(&mut buffer, 256);
        // Buffer should be resized to at least 256
        assert!(
            buffer.capacity() >= 256,
            "Buffer capacity {} should be >= 256",
            buffer.capacity()
        );

        // Test resize beyond max limit
        manager.resize_buffer(&mut buffer, 10000);
        // Buffer should not exceed max size
        assert!(buffer.capacity() <= 8192);
    }

    #[cfg(not(feature = "mode-async"))]
    #[test]
    fn test_buffer_reset() {
        let manager = BufferManager::new(BufferConfig::default());
        let mut buffer = manager.alloc_recv_buffer();

        // Add some data
        buffer.extend_from_slice(b"test data");
        assert!(!buffer.is_empty());

        // Reset should clear the buffer
        manager.reset_buffer(&mut buffer);
        assert!(buffer.is_empty());
    }
}
