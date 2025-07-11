//! Tokio transport implementations for VISCA communication.
//!
//! This module provides async transport implementations using the tokio runtime.

// GAT-based implementations
pub mod tcp_gat;
pub mod udp_gat;

// Re-export for convenience
pub use tcp_gat::TcpGat;
pub use udp_gat::UdpGat;