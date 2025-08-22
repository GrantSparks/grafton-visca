//! Runtime-specific transport adapters.
//!
//! This module provides transport implementations for specific async runtimes.
//! Each runtime's transports are feature-gated to prevent unwanted dependencies.

#[cfg(feature = "rt-tokio")]
/// Tokio runtime transport adapters.
///
/// These transports are optimized for use with the Tokio runtime and require
/// the `rt-tokio` feature to be enabled.
///
/// # Example
///
/// ```rust,no_run
/// # #[cfg(feature = "rt-tokio")]
/// use grafton_visca::runtime_adapters::tokio::{TcpTransport, UdpTransport};
/// use grafton_visca::transport::AsyncTransport;
///
/// # #[cfg(feature = "rt-tokio")]
/// # #[tokio::main]
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let tcp = TcpTransport::connect("192.168.0.110:5678").await?;
/// let udp = UdpTransport::connect("192.168.0.110:1259").await?;
/// # Ok(())
/// # }
/// ```
pub mod tokio {

    pub use crate::transport::tokio::sony::{Tcp as SonyTcp, Udp as SonyUdp};
    pub use crate::transport::tokio::tcp::Tcp as TcpTransport;
    pub use crate::transport::tokio::udp::Udp as UdpTransport;
}

#[cfg(feature = "rt-async-std")]
/// async-std runtime transport adapters.
///
/// These transports are optimized for use with the async-std runtime and require
/// the `rt-async-std` feature to be enabled.
///
/// NOTE: These implementations are not yet available.
/// This module is reserved for future async-std transport support.
pub mod async_std {}

#[cfg(feature = "rt-smol")]
/// smol runtime transport adapters.
///
/// These transports are optimized for use with the smol runtime and require
/// the `rt-smol` feature to be enabled.
///
/// NOTE: These implementations are not yet available.
/// This module is reserved for future smol transport support.
pub mod smol {}
