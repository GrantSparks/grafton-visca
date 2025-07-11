//! Clean tokio transport implementations that work directly with AsyncTransport.
//!
//! This module provides simplified transport implementations that work directly
//! with the AsyncTransport trait, eliminating the need for adapters and complex
//! trait hierarchies.

mod tcp;
mod udp;

// GAT-based implementations
pub mod tcp_gat;
pub mod udp_gat;

pub use tcp::Tcp;
pub use udp::Udp;
