//! Blocking transport implementations for VISCA communication.
//!
//! This module provides synchronous I/O for VISCA camera control,
//! designed as the primary API for most use cases.

// GAT-based implementations
pub mod tcp_gat;
pub mod udp_gat;

// Re-exports
pub use tcp_gat::TcpGat;
pub use udp_gat::UdpGat;
