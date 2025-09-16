//! Runtime-specific transport adapters.
//!
//! This module provides transport implementations for specific async runtimes.
//! Each runtime's transports are feature-gated to prevent unwanted dependencies.

#[cfg(feature = "runtime-tokio")]
/// Tokio runtime transport adapters.
///
/// These transports are optimized for use with the Tokio runtime and require
/// the `rt-tokio` feature to be enabled.
///
/// # Example
///
/// ```rust,no_run
/// # #[cfg(feature = "runtime-tokio")]
/// use grafton_visca::runtime_adapters::tokio::{TcpTransport, UdpTransport};
/// use grafton_visca::transport::AsyncTransport;
///
/// # #[cfg(feature = "runtime-tokio")]
/// # #[tokio::main]
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let tcp = TcpTransport::connect("192.168.0.110:5678").await?;
/// let udp = UdpTransport::connect("192.168.0.110:1259").await?;
/// # Ok(())
/// # }
/// ```
pub mod tokio {
    #[cfg(feature = "transport-serial-tokio")]
    pub use crate::transport::serial::Config as SerialConfig;
    #[cfg(feature = "transport-serial-tokio")]
    pub use crate::transport::tokio::serial::Serial as SerialTransport;
    pub use crate::transport::tokio::tcp::Tcp as TcpTransport;
    pub use crate::transport::tokio::udp::Udp as UdpTransport;
}

#[cfg(feature = "runtime-async-std")]
/// async-std runtime transport adapters.
///
/// These transports are optimized for use with the async-std runtime and require
/// the `rt-async-std` feature to be enabled.
///
/// # Example
///
/// ```rust,no_run
/// # #[cfg(feature = "runtime-async-std")]
/// use grafton_visca::runtime_adapters::async_std::{TcpTransport, UdpTransport};
/// use grafton_visca::transport::AsyncTransport;
///
/// # #[cfg(feature = "runtime-async-std")]
/// # async_std::task::block_on(async {
/// let tcp = TcpTransport::connect("192.168.0.110:5678").await?;
/// let udp = UdpTransport::connect("192.168.0.110:1259").await?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// # }).unwrap();
/// ```
pub mod async_std {
    pub use crate::transport::async_std::tcp::Tcp as TcpTransport;
    pub use crate::transport::async_std::udp::Udp as UdpTransport;
}

#[cfg(feature = "runtime-smol")]
/// smol runtime transport adapters.
///
/// These transports are optimized for use with the smol runtime and require
/// the `rt-smol` feature to be enabled.
///
/// # Example
///
/// ```rust,no_run
/// # #[cfg(feature = "runtime-smol")]
/// use grafton_visca::runtime_adapters::smol::{TcpTransport, UdpTransport};
/// use grafton_visca::transport::AsyncTransport;
///
/// # #[cfg(feature = "runtime-smol")]
/// # smol::block_on(async {
/// let tcp = TcpTransport::connect("192.168.0.110:5678").await?;
/// let udp = UdpTransport::connect("192.168.0.110:1259").await?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// # }).unwrap();
/// ```
pub mod smol {
    pub use crate::transport::smol::tcp::Tcp as TcpTransport;
    pub use crate::transport::smol::udp::Udp as UdpTransport;
}
