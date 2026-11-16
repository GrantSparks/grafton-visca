//! Blocking transport trait for VISCA communication.
//!
//! This trait provides blocking methods for transports,
//! with built-in timeout support using OS-level socket timeouts.

use core::time::Duration;

use crate::{
    transport::{builder::TransportConfig, AddressingMode, ReceiveOutcome, SendSemantics},
    Error,
};

/// Private test handle exercising the concrete blocking transports through
/// their shared timeout and identity contracts.
#[cfg(all(test, feature = "blocking"))]
#[derive(Debug)]
#[non_exhaustive]
pub(crate) enum BlockingTransportHandle {
    /// TCP transport for blocking mode.
    Tcp(crate::transport::blocking::Tcp),
    /// UDP transport for blocking mode.
    Udp(crate::transport::blocking::Udp),
    /// Serial transport for blocking mode.
    #[cfg(feature = "transport-serial")]
    Serial(crate::transport::serial_blocking::SerialTransport),
}

#[cfg(all(test, feature = "blocking"))]
impl BlockingTransport for BlockingTransportHandle {
    fn send_with_timeout(&mut self, bytes: &[u8], timeout: Duration) -> Result<(), Error> {
        match self {
            BlockingTransportHandle::Tcp(transport) => transport.send_with_timeout(bytes, timeout),
            BlockingTransportHandle::Udp(transport) => transport.send_with_timeout(bytes, timeout),
            #[cfg(feature = "transport-serial")]
            BlockingTransportHandle::Serial(transport) => {
                transport.send_with_timeout(bytes, timeout)
            }
        }
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        timeout: Duration,
    ) -> Result<ReceiveOutcome, Error> {
        match self {
            BlockingTransportHandle::Tcp(transport) => {
                transport.recv_into_with_timeout(dst, timeout)
            }
            BlockingTransportHandle::Udp(transport) => {
                transport.recv_into_with_timeout(dst, timeout)
            }
            #[cfg(feature = "transport-serial")]
            BlockingTransportHandle::Serial(transport) => {
                transport.recv_into_with_timeout(dst, timeout)
            }
        }
    }
}

#[cfg(all(test, feature = "blocking"))]
impl HasTransportConfig for BlockingTransportHandle {
    fn identity(&self) -> TransportIdentity<'_> {
        match self {
            BlockingTransportHandle::Tcp(transport) => transport.identity(),
            BlockingTransportHandle::Udp(transport) => transport.identity(),
            #[cfg(feature = "transport-serial")]
            BlockingTransportHandle::Serial(transport) => transport.identity(),
        }
    }

    fn transport_config(&self) -> &TransportConfig {
        match self {
            BlockingTransportHandle::Tcp(transport) => transport.transport_config(),
            BlockingTransportHandle::Udp(transport) => transport.transport_config(),
            #[cfg(feature = "transport-serial")]
            BlockingTransportHandle::Serial(transport) => transport.transport_config(),
        }
    }
}

/// Blocking transport for VISCA communication.
///
/// This trait provides blocking methods for transports that block
/// the current thread. It includes built-in timeout support that should
/// be implemented using OS-level socket timeouts where possible.
///
/// Transports are stream/datagram-level adapters that read/write raw bytes.
/// Framing and retry logic are handled by the runtime layer.
///
/// # Example
///
/// ```rust
/// use core::time::Duration;
/// use grafton_visca::{
///     transport::{BlockingTransport, ReceiveOutcome},
///     Error,
/// };
///
/// struct MyBlockingTransport {/* ... */}
///
/// impl BlockingTransport for MyBlockingTransport {
///     fn send_with_timeout(&mut self, bytes: &[u8], timeout: Duration) -> Result<(), Error> {
///         // Send implementation bounded by `timeout`.
///         Ok(())
///     }
///
///     fn recv_into_with_timeout(
///         &mut self,
///         dst: &mut [u8],
///         timeout: Duration,
///     ) -> Result<ReceiveOutcome, Error> {
///         // Receive implementation bounded by `timeout`.
///         Err(Error::io_timeout())
///     }
/// }
/// ```
/// # Session callbacks
///
/// A transport must not call into its own session from a read or write.
/// Calls that would wait for the owner fail immediately with
/// [`Error::InvalidState`], since the worker cannot answer its own request.
/// Calling `close` also requests shutdown before returning that error; an
/// external caller can then join the worker and release the transport.
pub trait BlockingTransport: Send {
    /// Send framed bytes to the device with a deadline (blocking).
    ///
    /// This method must block until all bytes have been written, an error is
    /// known, or `timeout` expires. Implementations must arrange an OS/device
    /// write timeout or an equivalent bounded operation; the blocking owner
    /// cannot safely preempt an arbitrary synchronous implementation. Report
    /// expiry as [`Error::io_timeout`]. The owner supplies the complete framed packet.
    ///
    /// # Arguments
    ///
    /// * `bytes` - The raw VISCA command bytes to send
    /// * `timeout` - Maximum time to complete the whole write
    ///
    /// # Error contract
    ///
    /// What a send failure costs is decided by
    /// [`TransportIdentity::send_semantics`], not by the error value:
    ///
    /// - [`SendSemantics::Stream`]: the byte-stream position is now unknowable,
    ///   so the runtime poisons the session. Every request in flight fails with
    ///   [`Error::StreamPoisoned`] carrying this error as its `source()`.
    /// - [`SendSemantics::Datagram`]: only the request being written fails and
    ///   the session keeps running. Because the session survives, the runtime
    ///   normalizes a session-fatal error value (any error for which
    ///   [`Error::requires_new_session`] is true, such as
    ///   [`Error::ConnectionClosed`]) into a plain per-request
    ///   [`Error::TransportError`]: a caller must never be told to open a new
    ///   session by an error raised on one that is still running. Report a
    ///   genuinely dead socket from the receive side, which is the side the
    ///   runtime treats as authoritative about session death.
    fn send_with_timeout(&mut self, bytes: &[u8], timeout: Duration) -> Result<(), Error>;

    /// Read raw bytes with a timeout (blocking), reporting whether everything
    /// received fitted in `dst`.
    ///
    /// This method must block until some data is available, an error is known,
    /// or `timeout` expires. Implementations must use an OS/device timeout or
    /// an equivalent bounded operation; they must not synthesize an immediate
    /// idle result for a positive timeout. Report expiry as [`Error::io_timeout`].
    ///
    /// # Arguments
    ///
    /// * `dst` - The buffer to read data into
    /// * `timeout` - Maximum time to wait for data
    ///
    /// # Receive outcome
    ///
    /// The [`ReceiveOutcome`] is the transport's own statement about what it
    /// copied; the runtime never infers it from the byte count, and applies
    /// the same rule as the async facade's receive.
    ///
    /// - A byte stream always reports [`ReceiveOutcome::complete`]: a short
    ///   read is routine and the runtime buffers the remainder.
    /// - A datagram transport reports [`ReceiveOutcome::complete`] only when
    ///   the whole datagram fitted, [`ReceiveOutcome::truncated`] when its OS
    ///   reported a discarded tail, and [`ReceiveOutcome::possibly_truncated`]
    ///   when it filled `dst` and cannot tell. The runtime discards a datagram
    ///   that is not complete as one malformed input and keeps the session; it
    ///   never decodes its prefix. A byte stream that reports one ends the
    ///   session, because bytes it consumed were lost.
    /// - A datagram transport never reports zero bytes for an empty datagram
    ///   (zero bytes is end of stream): it skips the empty datagram and keeps
    ///   reading within the same deadline. One that reports
    ///   [`ReceiveOutcome::possibly_truncated`] for every exact fill needs a
    ///   `recv_buffer_size` of at least
    ///   [`BufferConfig::MIN_RECV_BUFFER_SIZE`](crate::transport::BufferConfig::MIN_RECV_BUFFER_SIZE)
    ///   ` + 1`; see [`ReceiveOutcome`].
    ///
    /// # Error contract
    ///
    /// The runtime never guesses: the value this method returns decides whether
    /// the session lives, and whether every command still waiting for its ACK is
    /// retransmitted. A failed read must consume nothing, so that the runtime's
    /// framing state stays intact.
    ///
    /// - `Ok(outcome)` with a non-zero [`ReceiveOutcome::copied_len`] — bytes
    ///   were read. Returning fewer bytes than a whole frame is normal and is
    ///   not an error.
    /// - `Ok(ReceiveOutcome::complete(0))` — end of stream: the peer closed.
    ///   The runtime ends the session with [`Error::ConnectionClosed`]. Never
    ///   report zero bytes to mean "no data yet"; that is the one signal
    ///   reserved for EOF.
    /// - `Err(Error::io_timeout())` — `timeout` expired and no bytes arrived. The
    ///   runtime treats it as "no data": the session lives, framing state is
    ///   untouched, and no request's retry budget is spent. The raw I/O
    ///   spellings [`std::io::ErrorKind::WouldBlock`] and
    ///   [`std::io::ErrorKind::Interrupted`] wrapped in [`Error::Io`] are
    ///   normalized to the same meaning. Any [`Error::Timeout`] is read the same
    ///   way. Use [`Error::io_timeout`], not a raw
    ///   [`std::io::ErrorKind::TimedOut`], for an application-owned idle timer:
    ///   a connected TCP socket can report the latter when OS keepalive
    ///   exhausts, and the runtime treats that as session death.
    /// - A session-fatal error — any error for which
    ///   [`Error::requires_new_session`] is true, runtime shutdown, or a raw
    ///   I/O error listed in [`crate::transport::FATAL_RECEIVE_IO_KINDS`].
    ///   That shared table defines the fatal kinds for both transport traits.
    ///   The runtime ends the session as [`Error::ConnectionClosed`] and retains
    ///   the original cause. A failed read consumed nothing and does not lose
    ///   the stream's framing position.
    /// - Any other error is a *transient* fault: the read failed but the
    ///   connection may still be usable — a UDP `recv` reporting ECONNREFUSED
    ///   after an ICMP port-unreachable is the canonical case. The engine may
    ///   retransmit sequence-correlated Sony commands still waiting for their
    ///   ACK under each command's retry policy. A raw command awaiting ACK has
    ///   no sequence key, so it is left to its own ACK deadline and is never
    ///   replayed merely because of this fault. Do not use this class for idle
    ///   timeouts.
    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        timeout: Duration,
    ) -> Result<ReceiveOutcome, Error>;
}

/// The outcome of serial Address Set on one bus: the port and the number of
/// cameras the chain reported, which then hold addresses `1..=cameras`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct AddressedBus {
    /// The serial port path the bus is attached to.
    pub port: String,
    /// The number of cameras Address Set addressed.
    pub cameras: u8,
}

impl AddressedBus {
    /// Reports that Address Set on `port` addressed `cameras` cameras.
    #[must_use]
    pub fn new(port: impl Into<String>, cameras: u8) -> Self {
        Self {
            port: port.into(),
            cameras,
        }
    }
}

/// Closed identity of a transport, including its framing and failure semantics.
///
/// Standard transports derive all policy from their variant. A caller-owned
/// transport explicitly declares its semantics and VISCA addressing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransportIdentity<'a> {
    /// TCP byte stream with IP addressing.
    Tcp,
    /// UDP datagrams with IP addressing.
    Udp,
    /// Serial byte stream, optionally carrying the Address Set bus report.
    Serial {
        /// The bus addressed during startup, when Address Set ran.
        bus: Option<&'a AddressedBus>,
    },
    /// Caller-owned transport with explicitly declared protocol facts.
    Custom {
        /// Whether failed writes can corrupt the byte stream.
        semantics: SendSemantics,
        /// How camera addresses are represented.
        addressing: AddressingMode,
    },
}

impl<'a> TransportIdentity<'a> {
    /// Whether a failed write can corrupt the stream.
    #[must_use]
    pub const fn send_semantics(self) -> SendSemantics {
        match self {
            Self::Udp => SendSemantics::Datagram,
            Self::Tcp | Self::Serial { .. } => SendSemantics::Stream,
            Self::Custom { semantics, .. } => semantics,
        }
    }
    /// The transport's VISCA addressing mode.
    #[must_use]
    pub const fn addressing(self) -> AddressingMode {
        match self {
            Self::Tcp | Self::Udp => AddressingMode::Ip,
            Self::Serial { .. } => AddressingMode::Serial,
            Self::Custom { addressing, .. } => addressing,
        }
    }
    /// The standard profile-matrix kind, or the explicit custom escape hatch.
    #[must_use]
    pub const fn standard_transport_kind(self) -> Option<crate::camera::TransportKind> {
        match self {
            Self::Tcp => Some(crate::camera::TransportKind::Tcp),
            Self::Udp => Some(crate::camera::TransportKind::Udp),
            Self::Serial { .. } => Some(crate::camera::TransportKind::Serial),
            Self::Custom { .. } => None,
        }
    }
    /// The serial bus addressed during startup, if Address Set ran.
    #[must_use]
    pub const fn addressed_bus(self) -> Option<&'a AddressedBus> {
        match self {
            Self::Serial { bus } => bus,
            _ => None,
        }
    }
    #[cfg(any(feature = "blocking", feature = "async"))]
    pub(crate) const fn engine_kind(self) -> crate::runtime::engine::TransportKind {
        match self {
            Self::Udp
            | Self::Custom {
                semantics: SendSemantics::Datagram,
                ..
            } => crate::runtime::engine::TransportKind::Datagram,
            _ => crate::runtime::engine::TransportKind::Stream,
        }
    }
}

/// Trait for types that carry transport configuration.
///
/// This trait provides a unified interface for accessing the transport configuration
/// from various transport types, enabling the runtime to apply consistent settings
/// (buffers, timeouts) across all transport implementations.
///
/// # Example
///
/// ```rust
/// use grafton_visca::transport::HasTransportConfig;
///
/// // Access configuration from any transport
/// fn inspect_config(transport: &impl HasTransportConfig) {
///     let config = transport.transport_config();
///     let read_timeout = config.read_timeout();
///     let buffer_config = config.buffer_config();
/// }
/// ```
pub trait HasTransportConfig {
    /// Declare the transport identity. Wrappers must forward the inner identity.
    fn identity(&self) -> TransportIdentity<'_>;

    /// Get the transport configuration.
    fn transport_config(&self) -> &TransportConfig;
}

// Blanket implementation for references, enabling HRTB bounds like `for<'a> &'a T: HasTransportConfig`
impl<T: HasTransportConfig + ?Sized> HasTransportConfig for &T {
    fn identity(&self) -> TransportIdentity<'_> {
        (*self).identity()
    }

    #[inline]
    fn transport_config(&self) -> &TransportConfig {
        (*self).transport_config()
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    use crate::camera::TransportKind;

    #[test]
    fn identity_projects_one_consistent_standard_transport_contract() {
        let bus = AddressedBus::new("/dev/visca", 3);
        for (identity, semantics, addressing, kind, addressed) in [
            (
                TransportIdentity::Tcp,
                SendSemantics::Stream,
                AddressingMode::Ip,
                Some(TransportKind::Tcp),
                None,
            ),
            (
                TransportIdentity::Udp,
                SendSemantics::Datagram,
                AddressingMode::Ip,
                Some(TransportKind::Udp),
                None,
            ),
            (
                TransportIdentity::Serial { bus: None },
                SendSemantics::Stream,
                AddressingMode::Serial,
                Some(TransportKind::Serial),
                None,
            ),
            (
                TransportIdentity::Serial { bus: Some(&bus) },
                SendSemantics::Stream,
                AddressingMode::Serial,
                Some(TransportKind::Serial),
                Some(&bus),
            ),
        ] {
            assert_eq!(identity.send_semantics(), semantics);
            assert_eq!(identity.addressing(), addressing);
            assert_eq!(identity.standard_transport_kind(), kind);
            assert_eq!(identity.addressed_bus(), addressed);
        }
    }

    #[test]
    fn custom_identity_explicitly_preserves_each_semantics_and_addressing_pair() {
        for semantics in [SendSemantics::Stream, SendSemantics::Datagram] {
            for addressing in [AddressingMode::Ip, AddressingMode::Serial] {
                let identity = TransportIdentity::Custom {
                    semantics,
                    addressing,
                };
                assert_eq!(identity.send_semantics(), semantics);
                assert_eq!(identity.addressing(), addressing);
                assert_eq!(identity.standard_transport_kind(), None);
                assert_eq!(identity.addressed_bus(), None);
            }
        }
    }
}
