//! Clean tokio transport implementations that work directly with AsyncTransport.
//!
//! This module provides simplified transport implementations that work directly
//! with the AsyncTransport trait, eliminating the need for adapters and complex
//! trait hierarchies.

mod tcp;
mod udp;

pub use tcp::TcpTransport;
pub use udp::UdpTransport;
