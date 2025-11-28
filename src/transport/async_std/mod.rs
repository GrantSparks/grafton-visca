//! async-std transport implementations.
//!
//! This module provides transport implementations optimized for the async-std runtime.
//! All types in this module require the `runtime-async-std` feature to be enabled.

pub(crate) mod connectors;

// Use the macro to generate TCP and UDP transport implementations
crate::declare_net_transport!(
    runtime = "async_std",
    tcp_stream = crate::transport::async_std::connectors::AsyncStdTcpStream,
    udp_socket = async_std::net::UdpSocket,
    tcp_connect = crate::transport::async_std::connectors::connect_tcp,
    udp_connect = crate::transport::async_std::connectors::connect_udp,
    tcp_split = clone
);
