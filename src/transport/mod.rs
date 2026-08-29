//! Transport layer for VISCA communication.
//!
//! This module provides both blocking and async transport implementations.
//! The transport layer has been redesigned to use separate traits for blocking
//! and async transports, enabling zero-cost abstractions and better type safety.
//!
//! ## Architecture
//!
//! The transport layer now uses two separate traits:
//! - **AsyncTransport** - Native async functions for zero-cost async transports
//! - **BlockingTransport** - Blocking methods with OS-level timeout support
//! - Implementations: TCP and UDP for both blocking and async
//!
//! ## Send Semantics
//!
//! Transports are classified by their send semantics, which determines how
//! the runtime handles send failures:
//!
//! - **Stream** (TCP, Serial): A partial write can leave the byte stream in an
//!   unknown state. On send failure/timeout, the transport is "poisoned" and
//!   must be dropped to prevent protocol desynchronization.
//!
//! - **Datagram** (UDP): Each send is atomic at the datagram boundary. A failed
//!   send does not affect subsequent sends, so the transport can continue operating.
//!
//! ## Usage
//!
//! For blocking transports (using camera-first API):
//! ```rust,ignore
//! # #[cfg(feature = "blocking")]
//! use grafton_visca::{
//!     camera::{Connect, profiles::PtzOpticsG2},
//! };
//!
//! # #[cfg(feature = "blocking")]
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # #[cfg(feature = "blocking")]
//! let camera = Connect::open_tcp::<PtzOpticsG2>("192.168.0.110:5678")?;
//! # #[cfg(feature = "blocking")]
//! // Camera is ready to use with accessor pattern
//! # #[cfg(feature = "blocking")]
//! camera.power().on()?;
//! # #[cfg(feature = "blocking")]
//! camera.zoom().tele()?;
//! # Ok(())
//! # }
//! ```
//!
//! For async transports (with tokio):
//! ```rust,ignore
//! # #[cfg(feature = "runtime-tokio")]
//! use grafton_visca::{
//!     camera::{Connect, profiles::PtzOpticsG2},
//!     runtime::{Runtime, TokioRuntime},
//! };
//!
//! # #[cfg(feature = "runtime-tokio")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let runtime = TokioRuntime::from_current()?;
//! let camera = Connect::open_tcp::<PtzOpticsG2, _>(
//!     "192.168.0.110:5678",
//!     runtime
//! ).await?;
//! // Camera is ready to use with accessor pattern
//! camera.power().on().await?;
//! camera.zoom().tele().await?;
//! # Ok(())
//! # }
//! ```
/// Send semantics for transport classification.
///
/// This enum categorizes transports by how their send operations behave,
/// which determines the runtime's failure handling strategy:
///
/// - **Stream transports** (TCP, Serial): Byte-oriented, where a partial write
///   can leave the stream in an unknown state. On send failure or timeout,
///   the transport must be poisoned to prevent protocol desynchronization.
///
/// - **Datagram transports** (UDP): Message-oriented, where each send is atomic.
///   A failed send does not affect subsequent sends, so the transport can
///   continue operating.
///
/// # Usage
///
/// The runtime queries this via `send_semantics()` on send errors to decide
/// whether to poison the transport (Stream) or continue (Datagram).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SendSemantics {
    /// Byte-stream transport (TCP, Serial).
    ///
    /// A send failure/timeout can leave the stream in an unknown state
    /// (partial frame written). The transport must be poisoned on send
    /// failure to prevent protocol desynchronization.
    Stream,

    /// Datagram transport (UDP).
    ///
    /// Each send is atomic at the datagram boundary. A failed send
    /// does not affect subsequent sends, so the transport can continue
    /// operating after a send failure.
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
pub(crate) mod envelope;
#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
#[macro_use]
pub(crate) mod runtime_common;
#[cfg(any(feature = "transport-serial", feature = "transport-serial-tokio"))]
pub mod serial;
#[cfg(all(feature = "blocking", feature = "transport-serial"))]
pub(crate) mod serial_blocking;
#[cfg(all(feature = "async", feature = "runtime-smol"))]
pub(crate) mod smol;
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
pub(crate) mod tokio;

#[cfg(feature = "async")]
pub use async_transport::AsyncTransport;
#[cfg(feature = "blocking")]
pub use blocking_transport::BlockingTransportHandle;
pub use blocking_transport::{BlockingTransport, HasTransportConfig};
pub use buffer::BufferConfig;
pub use builder::{AddressingMode, TcpKeepaliveConfig, TransportConfig};
#[cfg(feature = "blocking")]
pub use builder::{NetTransportBuilder, Transport, TransportBuilderExt};
pub use envelope::{Envelope, FrameMeta, FrameSequence, RawVisca, SonyEncapsulated};
