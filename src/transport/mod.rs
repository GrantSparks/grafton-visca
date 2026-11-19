//! VISCA connections over TCP, UDP, serial, or a caller-owned transport.
//!
//! Configure a standard connection with [`crate::config::CameraConfig`]. To
//! attach an existing connection with `Session::open`, implement [`BlockingTransport`]
//! or `AsyncTransport` and [`HasTransportConfig`]. The transport identity declares
//! addressing and stream or datagram semantics. Reads report [`ReceiveOutcome`] so the
//! decoder can discard an incomplete datagram.
//!
//! For a working adapter, see the [caller-owned UDP example](https://github.com/GrantSparks/grafton-visca/blob/main/examples/builder_api.rs).

/// How writes are delivered, as reported by [`TransportIdentity::send_semantics`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SendSemantics {
    /// TCP or serial byte stream. A failed write can leave a partial frame,
    /// so the session must be reopened after a transport write failure.
    Stream,

    /// UDP-style atomic datagrams. A failed send preserves the framing of
    /// subsequent datagrams, though delivery of the failed write can be uncertain.
    Datagram,
}

#[cfg(any(
    feature = "async",
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol",
    test
))]
pub(crate) mod address;
#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
pub(crate) mod async_connect;
#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
pub(crate) mod async_io;
#[cfg(feature = "transport-serial-tokio")]
pub(crate) mod async_serial;
#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
pub(crate) mod async_tcp;
#[cfg(feature = "async")]
pub(crate) mod async_transport;
#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
pub(crate) mod async_udp;
#[cfg(feature = "blocking")]
pub(crate) mod blocking;
pub(crate) mod blocking_transport;
pub(crate) mod buffer;
pub(crate) mod builder;
#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
pub(crate) mod connect;
pub(crate) mod datagram;
#[cfg(test)]
mod entry_point_tests;
pub(crate) mod envelope;
mod kind;
pub(crate) use kind::NetworkTransport;
pub use kind::StandardTransport;
#[path = "serial/config.rs"]
pub(crate) mod serial_config;
pub use serial_config::{SerialConfig, SerialStartup};

#[cfg(any(feature = "transport-serial", feature = "transport-serial-tokio"))]
pub(crate) mod serial;
#[cfg(all(feature = "blocking", feature = "transport-serial"))]
pub(crate) mod serial_blocking;
#[cfg(all(feature = "async", feature = "runtime-smol"))]
pub mod smol;
#[cfg(all(
    any(unix, windows),
    any(
        feature = "blocking",
        feature = "runtime-tokio",
        feature = "runtime-smol"
    )
))]
pub(crate) mod socket_options;
#[cfg(all(feature = "async", feature = "runtime-tokio"))]
pub mod tokio;

#[cfg(any(feature = "blocking", feature = "async"))]
pub use address::NetworkEndpoint;
#[cfg(feature = "async")]
pub use async_transport::AsyncTransport;
pub use blocking_transport::{
    AddressedBus, BlockingTransport, HasTransportConfig, TransportIdentity,
};
pub use buffer::BufferConfig;
pub use builder::{AddressingMode, IoBounds, TcpConnect, TcpKeepaliveConfig, UdpConnect};
pub use datagram::ReceiveOutcome;
#[doc(hidden)]
pub use datagram::ScriptedReceive;
pub use envelope::{Envelope, FrameMeta, RawVisca, SonyEncapsulated};

/// The one end-of-stream rule shared by the built-in byte-stream transports.
///
/// A zero-byte read from a stream means the peer closed it. Every built-in
/// stream transport reports that as [`crate::Error::ConnectionClosed`] with
/// a typed `UnexpectedEof` source, so closure classification does not depend on
/// the transport or facade.
#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
pub(crate) fn stream_read(read: usize, closed: &'static str) -> crate::Result<usize> {
    if read == 0 {
        Err(crate::Error::connection_closed(Some(std::sync::Arc::new(
            crate::Error::from(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                closed,
            )),
        ))))
    } else {
        Ok(read)
    }
}

mod receive_errors;
pub use receive_errors::FATAL_RECEIVE_IO_KINDS;

#[cfg(all(
    test,
    not(feature = "blocking"),
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
#[path = "blocking/tests/issue_843_udp_transient.rs"]
mod issue_843_udp_transient_tests;
