//! Common utilities for examples
//!
//! This module provides shared transport implementations that can be used
//! across all examples. Since the core library no longer includes concrete
//! transport implementations, examples need to provide their own.

#[cfg(feature = "blocking-client")]
#[allow(unused_imports)]
pub mod blocking {
    pub use super::tcp_transport::TcpTransport;
    pub use super::udp_transport::UdpTransport;
}

#[cfg(feature = "async-client")]
#[allow(unused_imports)]
pub mod r#async {
    pub use super::tcp_transport::AsyncTcpTransport;
    pub use super::udp_transport::AsyncUdpTransport;
}

// Re-export the transport implementations from the example files
#[path = "../tcp_transport.rs"]
#[allow(dead_code)]
mod tcp_transport;

#[path = "../udp_transport.rs"]
#[allow(dead_code)]
mod udp_transport;
