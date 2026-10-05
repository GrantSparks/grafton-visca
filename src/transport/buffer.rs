//! Receive-side frame and retention limits shared by every transport.
//!
//! The per-transport presets are consumed by
//! [`TransportConfig::for_tcp`](crate::transport::TransportConfig::for_tcp),
//! [`TransportConfig::for_udp`](crate::transport::TransportConfig::for_udp) and
//! [`TransportConfig::for_serial`](crate::transport::TransportConfig::for_serial),
//! which are the defaults every built-in entry point uses.

/// Frame limit of the transport-neutral [`BufferConfig::default`].
const DEFAULT_FRAME_LIMIT: usize = 128;

/// Frame limit for one UDP datagram.
const UDP_FRAME_LIMIT: usize = 1024;

/// Frame limit for raw VISCA over a TCP byte stream.
const RAW_IP_FRAME_LIMIT: usize = 256;

/// Frame limit for VISCA over a serial byte stream.
const SERIAL_FRAME_LIMIT: usize = 256;

/// Stream retention bound shared by every preset.
const DEFAULT_RETENTION_LIMIT: usize = 8192;

/// Receive-side limits: the largest accepted frame and the stream retention
/// bound.
///
/// # Semantics
///
/// - [`recv_buffer_size`](Self::recv_buffer_size) is the **largest reply
///   frame** a session accepts — a raw VISCA frame including its `0xFF`
///   terminator, a Sony header plus its payload, or one UDP datagram — and the
///   most bytes a single transport read requests. A larger datagram is
///   discarded; a larger stream frame is a framing failure that ends the
///   session. The largest standard VISCA reply is 16 bytes (24 with a Sony
///   header), so every preset leaves ample headroom.
/// - [`max_buffer_size`](Self::max_buffer_size) bounds the **stream input
///   carried between reads**: an incomplete frame, or complete frames the
///   owner has not yet delivered. The framer always has room for one more full
///   read on top of it, so a healthy stream whose replies are split across
///   reads can never overflow, while the memory a byte stream can retain is
///   bounded by `max_buffer_size + recv_buffer_size`.
///
/// [`TransportConfig::validate`](crate::transport::TransportConfig) requires
/// both to be non-zero and `recv_buffer_size <= max_buffer_size`, so every
/// partial frame (at most `recv_buffer_size - 1` bytes) fits within the
/// retention bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct BufferConfig {
    /// Largest accepted reply frame or datagram, and the size of one read.
    pub recv_buffer_size: usize,

    /// Stream input that may be carried between reads.
    pub max_buffer_size: usize,
}

impl Default for BufferConfig {
    /// Transport-neutral limits (128-byte frames) for custom transports.
    fn default() -> Self {
        Self::with_frame_limit(DEFAULT_FRAME_LIMIT)
    }
}

impl BufferConfig {
    const fn with_frame_limit(recv_buffer_size: usize) -> Self {
        Self {
            recv_buffer_size,
            max_buffer_size: DEFAULT_RETENTION_LIMIT,
        }
    }

    /// Limits for UDP transports: one datagram of up to 1024 bytes.
    pub fn for_udp() -> Self {
        Self::with_frame_limit(UDP_FRAME_LIMIT)
    }

    /// Limits for raw VISCA over TCP: frames of up to 256 bytes.
    pub fn for_raw_ip() -> Self {
        Self::with_frame_limit(RAW_IP_FRAME_LIMIT)
    }

    /// Limits for serial transports: frames of up to 256 bytes.
    pub fn for_serial() -> Self {
        Self::with_frame_limit(SERIAL_FRAME_LIMIT)
    }

    /// The most bytes a stream framer may hold at once: the retention bound
    /// plus one full read.
    #[cfg(any(feature = "async", feature = "blocking", test))]
    pub(crate) const fn framer_capacity(self) -> usize {
        self.max_buffer_size.saturating_add(self.recv_buffer_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_differ_only_in_their_frame_limit() {
        for (config, frame_limit) in [
            (BufferConfig::default(), DEFAULT_FRAME_LIMIT),
            (BufferConfig::for_udp(), UDP_FRAME_LIMIT),
            (BufferConfig::for_raw_ip(), RAW_IP_FRAME_LIMIT),
            (BufferConfig::for_serial(), SERIAL_FRAME_LIMIT),
        ] {
            assert_eq!(config.recv_buffer_size, frame_limit);
            assert_eq!(config.max_buffer_size, DEFAULT_RETENTION_LIMIT);
        }
    }

    #[test]
    fn framer_capacity_leaves_room_for_one_full_read() {
        let config = BufferConfig {
            recv_buffer_size: 64,
            max_buffer_size: 64,
        };
        assert_eq!(config.framer_capacity(), 128);
    }
}
