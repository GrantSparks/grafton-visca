//! Tokio-specific implementations of unified async I/O connectors.

use std::time::Instant;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpStream, UdpSocket},
};

use crate::{
    timeout::Deadline,
    transport::{
        address::AddressResolver,
        async_io::{
            AsyncDatagram, AsyncReadExt as AsyncReadExtTrait, AsyncWriteExt as AsyncWriteExtTrait,
        },
        socket_options::{apply_tcp_socket_options, TcpConnectionConfig, UdpSocketConfig},
    },
    Error,
};

/// Wrapper around tokio's BufReader to implement our AsyncReadExt trait.
#[derive(Debug)]
pub struct TokioBufferedReader<R> {
    inner: BufReader<R>,
}

impl<R: AsyncReadExt + Unpin> TokioBufferedReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            inner: BufReader::new(reader),
        }
    }
}

impl<R: AsyncReadExt + Unpin + Send> AsyncReadExtTrait for TokioBufferedReader<R> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        Ok(self.inner.read(buf).await?)
    }
}

/// Wrapper around tokio streams to implement our AsyncWriteExt trait.
#[derive(Debug)]
pub struct TokioWriter<W> {
    inner: W,
}

impl<W> TokioWriter<W> {
    pub fn new(writer: W) -> Self {
        Self { inner: writer }
    }
}

impl<W: AsyncWriteExt + Unpin + Send> AsyncWriteExtTrait for TokioWriter<W> {
    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Error> {
        Ok(self.inner.write_all(buf).await?)
    }

    async fn flush(&mut self) -> Result<(), Error> {
        Ok(self.inner.flush().await?)
    }
}

/// Combined TCP stream wrapper that implements both read and write traits.
#[derive(Debug)]
pub struct TokioTcpStream {
    pub reader: TokioBufferedReader<tokio::net::tcp::OwnedReadHalf>,
    pub writer: TokioWriter<tokio::net::tcp::OwnedWriteHalf>,
}

impl AsyncReadExtTrait for TokioTcpStream {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        self.reader.read(buf).await
    }
}

impl AsyncWriteExtTrait for TokioTcpStream {
    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Error> {
        self.writer.write_all(buf).await
    }

    async fn flush(&mut self) -> Result<(), Error> {
        self.writer.flush().await
    }
}

/// Create a configured TCP connection using unified helpers.
pub async fn connect_tcp(
    address: &str,
    config: TcpConnectionConfig,
) -> Result<TokioTcpStream, Error> {
    // Connect with timeout
    let stream = tokio::time::timeout(config.connect_timeout, TcpStream::connect(address))
        .await
        .map_err(|_| Error::Timeout)??;

    apply_tcp_socket_options(&stream, config)?;

    // Split and wrap
    let (read_half, write_half) = stream.into_split();

    Ok(TokioTcpStream {
        reader: TokioBufferedReader::new(read_half),
        writer: TokioWriter::new(write_half),
    })
}

/// Create a configured UDP socket using unified helpers.
///
/// Uses a single end-to-end deadline for the entire connect operation,
/// ensuring that the total time spent on DNS resolution + socket connect
/// does not exceed `config.connect_timeout`.
pub async fn connect_udp(address: &str, config: UdpSocketConfig) -> Result<UdpSocket, Error> {
    // Create a single deadline for the entire operation
    let deadline = Deadline::from_timeout(config.connect_timeout)?;

    // Perform async DNS resolution with remaining budget
    let remaining = deadline.remaining_at(Instant::now());
    if remaining.is_zero() {
        return Err(Error::Timeout);
    }

    let target_addr = tokio::time::timeout(remaining, tokio::net::lookup_host(address))
        .await
        .map_err(|_| Error::Timeout)??
        .next()
        .ok_or_else(|| Error::InvalidAddress {
            reason: "No addresses resolved".into(),
        })?;

    // Bind to the appropriate unspecified address based on target family
    let resolver = AddressResolver::new();
    let bind_addr = resolver.bind_address_for(&target_addr);

    let socket = UdpSocket::bind(bind_addr).await?;

    // Connect with remaining budget
    let remaining = deadline.remaining_at(Instant::now());
    if remaining.is_zero() {
        return Err(Error::Timeout);
    }

    tokio::time::timeout(remaining, socket.connect(target_addr))
        .await
        .map_err(|_| Error::Timeout)??;

    // Apply socket configuration
    if let Some(ttl) = config.ttl {
        socket.set_ttl(ttl)?;
    }

    Ok(socket)
}

/// Implement AsyncDatagram for tokio's UdpSocket
impl AsyncDatagram for UdpSocket {
    async fn send(&self, buf: &[u8]) -> Result<usize, Error> {
        Ok(UdpSocket::send(self, buf).await?)
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<usize, Error> {
        Ok(UdpSocket::recv(self, buf).await?)
    }
}

// Serial port adapters are defined in the serial_async module where tokio_serial is available

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Test that verifies deadline budget consumption across sequential steps.
    ///
    /// This test validates the "single budget across steps" property:
    /// - Creates a deadline with timeout T
    /// - Step A sleeps for ~T * 0.6 and must succeed
    /// - Step B sleeps for ~T * 0.6 and must fail with Timeout because only ~T * 0.4 remains
    #[tokio::test]
    async fn test_deadline_budget_consumption() {
        let total_timeout = Duration::from_millis(200);
        let step_duration = Duration::from_millis(120); // 60% of total

        let deadline = Deadline::from_timeout(total_timeout).expect("finite test timeout");

        // Step A: Should succeed with ~60% of budget
        let remaining = deadline.remaining_at(Instant::now());
        assert!(
            !remaining.is_zero(),
            "Should have remaining time before step A"
        );

        let step_a_result = tokio::time::timeout(remaining, async {
            tokio::time::sleep(step_duration).await;
            Ok::<_, Error>(())
        })
        .await;

        assert!(
            step_a_result.is_ok(),
            "Step A should complete within remaining budget"
        );

        // Step B: Should fail because only ~40% of budget remains but needs 60%
        let remaining = deadline.remaining_at(Instant::now());
        // After step A took 60%, we have ~40% left which is less than the 60% step B needs
        // However, due to timing variations, we check that step B times out

        let step_b_result = tokio::time::timeout(remaining, async {
            tokio::time::sleep(step_duration).await;
            Ok::<_, Error>(())
        })
        .await;

        assert!(
            step_b_result.is_err(),
            "Step B should timeout because remaining budget is insufficient"
        );
    }

    /// Test that an already-expired deadline returns zero remaining time.
    #[tokio::test]
    async fn test_deadline_expired_returns_zero() {
        let timeout = Duration::from_millis(10);
        let deadline = Deadline::from_timeout(timeout).expect("finite test timeout");

        // Wait for deadline to expire
        tokio::time::sleep(Duration::from_millis(20)).await;

        let remaining = deadline.remaining_at(Instant::now());
        assert!(
            remaining.is_zero(),
            "Expired deadline should return zero remaining time"
        );
    }

    /// Test that deadline correctly tracks remaining time across multiple checks.
    #[tokio::test]
    async fn test_deadline_remaining_decreases() {
        let timeout = Duration::from_millis(100);
        let deadline = Deadline::from_timeout(timeout).expect("finite test timeout");

        let remaining_before = deadline.remaining_at(Instant::now());

        tokio::time::sleep(Duration::from_millis(30)).await;

        let remaining_after = deadline.remaining_at(Instant::now());

        assert!(
            remaining_after < remaining_before,
            "Remaining time should decrease after sleep"
        );
        assert!(
            remaining_after <= Duration::from_millis(75),
            "Remaining time should be roughly 70ms or less after 30ms sleep"
        );
    }
}
