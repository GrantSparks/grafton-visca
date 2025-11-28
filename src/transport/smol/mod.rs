//! smol runtime transport implementations.
//!
//! This module provides transport implementations for the smol runtime.

pub(crate) mod connectors;

crate::declare_net_transport!(
    runtime = "smol",
    tcp_stream = crate::transport::smol::connectors::SmolTcpStream,
    udp_socket = smol::net::UdpSocket,
    tcp_connect = crate::transport::smol::connectors::connect_tcp,
    udp_connect = crate::transport::smol::connectors::connect_udp,
    tcp_split = clone
);
