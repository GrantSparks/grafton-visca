//! async-std-specific implementations of unified async I/O connectors.

use std::time::Instant;

use async_std::{
    io::prelude::*,
    net::{TcpStream, ToSocketAddrs, UdpSocket},
};

use crate::{
    timeout::Deadline,
    transport::{
        address::AddressResolver,
        async_io::{
            AsyncDatagram, AsyncReadExt as AsyncReadExtTrait, AsyncWriteExt as AsyncWriteExtTrait,
            TcpConnectionConfig, UdpSocketConfig,
        },
    },
    Error,
};

/// Combined TCP stream wrapper that implements both read and write traits.
#[derive(Debug)]
pub struct AsyncStdTcpStream {
    stream: TcpStream,
}

impl AsyncStdTcpStream {
    pub fn new(stream: TcpStream) -> Self {
        Self { stream }
    }

    pub fn clone_stream(&self) -> TcpStream {
        self.stream.clone()
    }
}

impl AsyncReadExtTrait for AsyncStdTcpStream {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        Ok(self.stream.read(buf).await?)
    }
}

impl AsyncWriteExtTrait for AsyncStdTcpStream {
    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Error> {
        Ok(self.stream.write_all(buf).await?)
    }

    async fn flush(&mut self) -> Result<(), Error> {
        Ok(self.stream.flush().await?)
    }
}

/// Create a configured TCP connection using unified helpers.
pub async fn connect_tcp(
    address: &str,
    config: TcpConnectionConfig,
) -> Result<AsyncStdTcpStream, Error> {
    // Connect with timeout
    let stream = async_std::future::timeout(config.connect_timeout, TcpStream::connect(address))
        .await
        .map_err(|_| Error::Timeout)??;

    // Apply socket configuration
    if let Some(nodelay) = config.nodelay {
        stream.set_nodelay(nodelay)?;
    } else {
        stream.set_nodelay(true)?; // Default to low latency
    }

    if let Some(ttl) = config.ttl {
        stream.set_ttl(ttl)?;
    }

    Ok(AsyncStdTcpStream::new(stream))
}

/// Create a configured UDP socket using unified helpers.
///
/// Uses a single end-to-end deadline for the entire connect operation,
/// ensuring that the total time spent on DNS resolution + socket connect
/// does not exceed `config.connect_timeout`.
pub async fn connect_udp(address: &str, config: UdpSocketConfig) -> Result<UdpSocket, Error> {
    // Create a single deadline for the entire operation
    let deadline = Deadline::from_timeout(config.connect_timeout);

    // Perform async DNS resolution with remaining budget
    let remaining = deadline.remaining_at(Instant::now());
    if remaining.is_zero() {
        return Err(Error::Timeout);
    }

    let mut addrs = async_std::future::timeout(remaining, address.to_socket_addrs())
        .await
        .map_err(|_| Error::Timeout)??;
    let target_addr = addrs.next().ok_or_else(|| Error::InvalidAddress {
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

    async_std::future::timeout(remaining, socket.connect(target_addr))
        .await
        .map_err(|_| Error::Timeout)??;

    // Apply socket options
    if let Some(ttl) = config.ttl {
        socket.set_ttl(ttl)?;
    }

    Ok(socket)
}

/// Implement AsyncDatagram for async-std's UdpSocket
impl AsyncDatagram for UdpSocket {
    async fn send(&self, buf: &[u8]) -> Result<usize, Error> {
        Ok(UdpSocket::send(self, buf).await?)
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<usize, Error> {
        Ok(UdpSocket::recv(self, buf).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Test that verifies deadline budget consumption across sequential steps.
    ///
    /// This test validates the "single budget across steps" property:
    /// - Creates a deadline with timeout T
    /// - Step A sleeps for ~T * 0.6 and must succeed
    /// - Step B sleeps for ~T * 0.6 and must fail with Timeout because only ~T * 0.4 remains
    #[async_std::test]
    async fn test_deadline_budget_consumption() {
        let total_timeout = Duration::from_millis(200);
        let step_duration = Duration::from_millis(120); // 60% of total

        let deadline = Deadline::from_timeout(total_timeout);

        // Step A: Should succeed with ~60% of budget
        let remaining = deadline.remaining_at(Instant::now());
        assert!(
            !remaining.is_zero(),
            "Should have remaining time before step A"
        );

        let step_a_result = async_std::future::timeout(remaining, async {
            async_std::task::sleep(step_duration).await;
            Ok::<_, Error>(())
        })
        .await;

        assert!(
            step_a_result.is_ok(),
            "Step A should complete within remaining budget"
        );

        // Step B: Should fail because only ~40% of budget remains but needs 60%
        let remaining = deadline.remaining_at(Instant::now());

        let step_b_result = async_std::future::timeout(remaining, async {
            async_std::task::sleep(step_duration).await;
            Ok::<_, Error>(())
        })
        .await;

        assert!(
            step_b_result.is_err(),
            "Step B should timeout because remaining budget is insufficient"
        );
    }

    /// Test that an already-expired deadline returns zero remaining time.
    #[async_std::test]
    async fn test_deadline_expired_returns_zero() {
        let timeout = Duration::from_millis(10);
        let deadline = Deadline::from_timeout(timeout);

        // Wait for deadline to expire
        async_std::task::sleep(Duration::from_millis(20)).await;

        let remaining = deadline.remaining_at(Instant::now());
        assert!(
            remaining.is_zero(),
            "Expired deadline should return zero remaining time"
        );
    }

    /// Test that deadline correctly tracks remaining time across multiple checks.
    #[async_std::test]
    async fn test_deadline_remaining_decreases() {
        let timeout = Duration::from_millis(100);
        let deadline = Deadline::from_timeout(timeout);

        let remaining_before = deadline.remaining_at(Instant::now());

        async_std::task::sleep(Duration::from_millis(30)).await;

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
