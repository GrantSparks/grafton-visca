//! Receive-side frame and retention limits shared by every transport.
//!
//! The per-transport presets are consumed by
//! [`TransportConfig::for_tcp`](crate::transport::TransportConfig::for_tcp),
//! [`TransportConfig::for_udp`](crate::transport::TransportConfig::for_udp) and
//! [`TransportConfig::for_serial`](crate::transport::TransportConfig::for_serial),
//! which are the defaults every built-in entry point uses.

/// Validate the byte count a transport reported before indexing its buffer.
#[cfg(any(feature = "async", feature = "blocking", test))]
pub(crate) fn checked_read(reported: usize, capacity: usize) -> crate::Result<usize> {
    if reported > capacity {
        Err(crate::Error::buffer_too_small(reported, capacity))
    } else {
        Ok(reported)
    }
}

/// Frame limit of the transport-neutral [`BufferConfig::default`].
const DEFAULT_FRAME_LIMIT: usize = 128;

/// Frame limit for one UDP datagram.
const UDP_FRAME_LIMIT: usize = 1024;

/// Frame limit for raw VISCA over a TCP byte stream.
const TCP_FRAME_LIMIT: usize = 256;

/// Frame limit for VISCA over a serial byte stream.
const SERIAL_FRAME_LIMIT: usize = 256;

/// Stream retention bound shared by every preset.
const DEFAULT_RETENTION_LIMIT: usize = 8192;

/// The largest documented reply currently supported is sixteen bytes.
/// Request capacity is separate: FR7 position commands need seventeen bytes.
const MAX_VISCA_MESSAGE: usize = 16;

/// Sony VISCA-over-IP prefixes each message with an 8-byte header.
const SONY_HEADER: usize = 8;

/// Receive-side limits: the largest accepted frame and the stream retention
/// bound.
///
/// # Semantics
///
/// - [`recv_buffer_size`](Self::recv_buffer_size) is the **largest reply
///   frame** a session accepts — a raw VISCA frame including its `0xFF`
///   terminator, a Sony header plus its payload, or one UDP datagram — and the
///   most bytes a single transport read requests. A larger datagram is
///   discarded. A larger raw stream frame that ends in `0xFF` is discarded
///   and counted as malformed input; the terminator preserves the next frame's
///   boundary. An oversized or undecodable Sony header ends a stream session because its
///   next boundary is unknowable; a datagram is independently discarded and
///   counted as malformed input. The receive floor reserves 16 payload bytes
///   plus a Sony header ([`BufferConfig::MIN_RECV_BUFFER_SIZE`]); every preset
///   leaves ample headroom above it.
/// - [`max_buffer_size`](Self::max_buffer_size) bounds the **stream input
///   carried between reads**: an incomplete frame, or complete frames the
///   owner has not yet delivered. The framer always has room for one more full
///   read on top of it, so a healthy stream whose replies are split across
///   reads can never overflow, while the memory a byte stream can retain is
///   bounded by `max_buffer_size + recv_buffer_size`. Exceeding that sum is a
///   framing-position loss that ends the stream session, including when input
///   has no terminating boundary.
///
/// Every session open validates `recv_buffer_size >=`
/// [`BufferConfig::MIN_RECV_BUFFER_SIZE`], a non-zero `max_buffer_size` and
/// `recv_buffer_size <= max_buffer_size`, so every valid reply fits one read
/// and every partial frame (at most `recv_buffer_size - 1` bytes) fits within
/// the retention bound. The receive buffer is limited to 65,535 bytes and
/// the retention bound to 1,048,576 bytes (1 MiB). These ceilings keep input
/// memory bounded even for caller-selected limits; all transport presets are
/// well below them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub struct BufferConfig {
    /// Largest accepted reply frame or datagram, and the size of one read.
    pub(crate) recv_buffer_size: usize,

    /// Stream input that may be carried between reads.
    pub(crate) max_buffer_size: usize,
}

impl Default for BufferConfig {
    /// Transport-neutral limits (128-byte frames) for custom transports.
    fn default() -> Self {
        Self::with_frame_limit(DEFAULT_FRAME_LIMIT)
    }
}

impl BufferConfig {
    /// Returns the largest accepted reply frame or datagram.
    #[must_use]
    pub const fn recv_buffer_size(&self) -> usize {
        self.recv_buffer_size
    }

    /// Selects the reply frame limit. Opening validates `24..=65_535` and
    /// requires it not to exceed the stream retention bound.
    #[must_use]
    pub const fn with_recv_buffer_size(mut self, size: usize) -> Self {
        self.recv_buffer_size = size;
        self
    }

    /// Returns the stream input retention bound.
    #[must_use]
    pub const fn max_buffer_size(&self) -> usize {
        self.max_buffer_size
    }

    /// Selects the stream retention bound. Opening requires it to hold at
    /// least one reply frame and not to exceed 1,048,576 bytes.
    #[must_use]
    pub const fn with_max_buffer_size(mut self, size: usize) -> Self {
        self.max_buffer_size = size;
        self
    }

    #[cfg(any(feature = "async", feature = "blocking", test))]
    pub(crate) const MAX_RECV_BUFFER_SIZE: usize = 65_535;

    #[cfg(any(feature = "async", feature = "blocking", test))]
    pub(crate) const MAX_RETENTION_SIZE: usize = 1_048_576;

    /// The smallest accepted [`recv_buffer_size`](Self::recv_buffer_size):
    /// the largest valid VISCA reply, a 16-byte message behind Sony's 8-byte
    /// header. A smaller limit would reject a valid reply as oversized.
    pub const MIN_RECV_BUFFER_SIZE: usize = SONY_HEADER + MAX_VISCA_MESSAGE;

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
    pub fn for_tcp() -> Self {
        Self::with_frame_limit(TCP_FRAME_LIMIT)
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
    fn read_counts_are_checked_before_buffer_access() {
        assert_eq!(checked_read(0, 0).ok(), Some(0));
        assert_eq!(checked_read(4, 4).ok(), Some(4));
        assert!(matches!(
            checked_read(5, 4),
            Err(crate::Error::BufferTooSmall {
                required: 5,
                actual: 4
            })
        ));
    }

    #[test]
    fn presets_differ_only_in_their_frame_limit() {
        for (config, frame_limit) in [
            (BufferConfig::default(), DEFAULT_FRAME_LIMIT),
            (BufferConfig::for_udp(), UDP_FRAME_LIMIT),
            (BufferConfig::for_tcp(), TCP_FRAME_LIMIT),
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
