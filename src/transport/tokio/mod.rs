//! Tokio transport implementations for VISCA communication.
//!
//! This module provides async transport implementations using the tokio runtime.

pub(crate) mod connectors;
#[cfg(feature = "transport-serial-tokio")]
pub mod serial;

// Use the macro to generate TCP and UDP transport implementations
crate::declare_net_transport!(
    runtime = "tokio",
    tcp_stream = crate::transport::tokio::connectors::TokioTcpStream,
    udp_socket = tokio::net::UdpSocket,
    tcp_connect = crate::transport::tokio::connectors::connect_tcp,
    udp_connect = crate::transport::tokio::connectors::connect_udp,
    tcp_split = owned {
        reader: crate::transport::tokio::connectors::TokioBufferedReader<tokio::net::tcp::OwnedReadHalf>,
        writer: crate::transport::tokio::connectors::TokioWriter<tokio::net::tcp::OwnedWriteHalf>
    }
);
