//! smol TCP transport implementation with zero-cost async using unified helpers.

use bytes::Bytes;

use std::time::Duration;

use crate::{
    transport::{
        async_io::{read_visca_frame, write_all_flush, TcpConnectionConfig},
        builder::TransportConfig,
        smol::connectors::{connect_tcp, SmolTcpStream},
        AsyncTransport,
    },
    Error,
};

/// TCP transport for async VISCA communication using smol.
///
/// This transport uses native async functions without boxing and unified helpers
/// to reduce code duplication across runtimes.
#[derive(Debug)]
pub struct Tcp {
    stream: SmolTcpStream,
}

impl Tcp {
    /// Connect to a TCP endpoint.
    ///
    /// This method resolves hostnames and supports both IPv4 and IPv6 addresses.
    pub async fn connect(address: &str) -> Result<Self, Error> {
        let config = TransportConfig::default();
        Self::connect_with_config(address, config).await
    }

    /// Connect with a custom timeout.
    ///
    /// This method resolves hostnames and supports both IPv4 and IPv6 addresses.
    pub async fn connect_timeout(address: &str, timeout: Duration) -> Result<Self, Error> {
        let config = TransportConfig {
            connect_timeout: timeout,
            ..Default::default()
        };
        Self::connect_with_config(address, config).await
    }

    /// Connect with a full configuration.
    ///
    /// This method provides full control over connection and socket parameters.
    pub async fn connect_with_config(
        address: &str,
        config: TransportConfig,
    ) -> Result<Self, Error> {
        let tcp_config = TcpConnectionConfig::from(config);
        let stream = connect_tcp(address, tcp_config).await?;
        Ok(Self { stream })
    }

    /// Split the TCP transport into separate reader and writer halves.
    ///
    /// This allows for concurrent reading and writing without needing mutable
    /// access to the entire transport. Useful for full-duplex communication patterns.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::runtime_adapters::smol::TcpTransport as Tcp;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let transport = Tcp::connect("192.168.1.100:5678").await?;
    /// let (mut reader, mut writer) = transport.split();
    ///
    /// // Can now read and write concurrently
    /// smol::spawn(async move {
    ///     // Use writer in one task
    ///     writer.send(&[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]).await.unwrap();
    /// }).detach();
    ///
    /// // Use reader in another task
    /// let response = reader.recv().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn split(self) -> (TcpReader, TcpWriter) {
        let cloned_stream = SmolTcpStream::new(self.stream.clone_stream());

        let reader = TcpReader {
            stream: self.stream,
        };

        let writer = TcpWriter {
            stream: cloned_stream,
        };

        (reader, writer)
    }
}

impl AsyncTransport for Tcp {
    async fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        write_all_flush(&mut self.stream, data).await
    }

    async fn recv(&mut self) -> Result<Bytes, Error> {
        read_visca_frame(&mut self.stream).await
    }
}

/// Reader half of a split TCP transport.
///
/// This type allows reading from an async TCP connection that has been split
/// into separate reader and writer halves.
#[derive(Debug)]
pub struct TcpReader {
    stream: SmolTcpStream,
}

impl TcpReader {
    /// Receive data from the TCP connection.
    pub async fn recv(&mut self) -> Result<Bytes, Error> {
        read_visca_frame(&mut self.stream).await
    }
}

/// Writer half of a split TCP transport.
///
/// This type allows writing to an async TCP connection that has been split
/// into separate reader and writer halves.
#[derive(Debug)]
pub struct TcpWriter {
    stream: SmolTcpStream,
}

impl TcpWriter {
    /// Send data over the TCP connection.
    pub async fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        write_all_flush(&mut self.stream, data).await
    }
}
