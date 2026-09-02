//! Canonical owner-backed async construction.
//!
//! Standard connections resolve configuration before creating a transport and
//! end in [`crate::Session::open`].

#![cfg(feature = "async")]

#[cfg(feature = "transport-serial-tokio")]
use crate::capabilities::SupportsSerial;
use crate::{
    camera::{CameraConfig, TransportOptions},
    profile::CompileTimeProfile,
};

/// One-line canonical async connection constructor.
#[derive(Debug, Clone, Copy)]
pub struct Connect;

impl Connect {
    /// Opens a standard TCP or UDP transport selected at runtime.
    ///
    /// Unlike [`Self::open_tcp`] and [`Self::open_udp`], this entry point only
    /// requires [`CompileTimeProfile`]. Transport compatibility and the
    /// profile's default port are resolved before any connector I/O.
    /// Serial transports use `Connect::open_serial`, and custom transports use
    /// [`crate::Session::open`].
    pub async fn open<P, R>(
        transport: TransportOptions,
        runtime: R,
    ) -> crate::Result<crate::CameraSession<P>>
    where
        P: CompileTimeProfile,
        R: crate::runtime::Runtime,
    {
        CameraConfig::<P>::new()
            .transport(transport)
            .open_async(runtime)
            .await
    }

    /// Opens one canonical owner-backed TCP camera session.
    pub async fn open_tcp<P, R>(
        address: impl Into<String>,
        runtime: R,
    ) -> crate::Result<crate::CameraSession<P>>
    where
        P: CompileTimeProfile + crate::capabilities::SupportsTcp,
        R: crate::runtime::Runtime,
    {
        CameraConfig::<P>::tcp(address).open_async(runtime).await
    }

    /// Opens one canonical owner-backed UDP camera session.
    pub async fn open_udp<P, R>(
        address: impl Into<String>,
        runtime: R,
    ) -> crate::Result<crate::CameraSession<P>>
    where
        P: CompileTimeProfile + crate::capabilities::SupportsUdp,
        R: crate::runtime::Runtime,
    {
        CameraConfig::<P>::udp(address).open_async(runtime).await
    }

    /// Open one canonical owner-backed Tokio serial session.
    #[cfg(feature = "transport-serial-tokio")]
    pub async fn open_serial<P, R>(
        port: impl Into<String>,
        baud_rate: u32,
        runtime: R,
    ) -> crate::Result<crate::Session>
    where
        P: CompileTimeProfile + SupportsSerial,
        R: crate::runtime::Runtime
            + crate::runtime::RuntimeSerial<SerialTransport = crate::transport::tokio::serial::Serial>,
    {
        CameraConfig::<P>::serial(port, baud_rate)
            .open_serial_async(runtime)
            .await
    }
}
