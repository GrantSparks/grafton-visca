//! Simplified async transport trait for runtime-agnostic transport implementations.
//!
//! This module provides a minimal transport abstraction that allows users to
//! implement transports for different async runtimes without complex trait hierarchies.

use super::{RawTransport, TransportFuture};
use crate::error::Error;
use std::fmt::Debug;
use std::future::Future;

/// Simple async transport trait that defines the minimal interface needed
/// for VISCA communication over different async runtimes.
///
/// This trait is intentionally minimal to avoid complexity and allow easy
/// implementation for different runtimes (tokio, async-std, smol, etc.).
///
/// # Examples
///
/// ## Implementing for tokio
/// ```rust,ignore
/// use grafton_visca::transport::AsyncTransport;
/// use grafton_visca::error::Error;
/// use tokio::net::TcpStream;
/// use tokio::io::{AsyncReadExt, AsyncWriteExt};
/// use std::future::Future;
/// use std::pin::Pin;
///
/// pub struct TokioTcpTransport {
///     stream: TcpStream,
/// }
///
/// impl AsyncTransport for TokioTcpTransport {
///     type SendFuture<'a> = Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>>;
///     type ReceiveFuture<'a> = Pin<Box<dyn Future<Output = Result<Vec<u8>, Error>> + Send + 'a>>;
///
///     fn send<'a>(&'a mut self, data: &'a [u8]) -> Self::SendFuture<'a> {
///         Box::pin(async move {
///             self.stream.write_all(data).await?;
///             self.stream.flush().await?;
///             Ok(())
///         })
///     }
///
///     fn receive(&mut self) -> Self::ReceiveFuture<'_> {
///         Box::pin(async move {
///             let mut buf = vec![0; 1024];
///             let n = self.stream.read(&mut buf).await?;
///             buf.truncate(n);
///             Ok(buf)
///         })
///     }
/// }
/// ```
///
/// ## Implementing for async-std
/// ```rust,ignore
/// use grafton_visca::transport::AsyncTransport;
/// use grafton_visca::error::Error;
/// use async_std::net::TcpStream;
/// use async_std::io::prelude::*;
/// use std::future::Future;
/// use std::pin::Pin;
///
/// pub struct AsyncStdTcpTransport {
///     stream: TcpStream,
/// }
///
/// impl AsyncTransport for AsyncStdTcpTransport {
///     type SendFuture<'a> = Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>>;
///     type ReceiveFuture<'a> = Pin<Box<dyn Future<Output = Result<Vec<u8>, Error>> + Send + 'a>>;
///
///     fn send<'a>(&'a mut self, data: &'a [u8]) -> Self::SendFuture<'a> {
///         Box::pin(async move {
///             self.stream.write_all(data).await?;
///             self.stream.flush().await?;
///             Ok(())
///         })
///     }
///
///     fn receive(&mut self) -> Self::ReceiveFuture<'_> {
///         Box::pin(async move {
///             let mut buf = vec![0; 1024];
///             let n = self.stream.read(&mut buf).await?;
///             buf.truncate(n);
///             Ok(buf)
///         })
///     }
/// }
/// ```
pub trait AsyncTransport: Send + Sync {
    /// The future type returned by the send method.
    type SendFuture<'a>: Future<Output = Result<(), Error>> + Send + 'a
    where
        Self: 'a;

    /// The future type returned by the receive method.
    type ReceiveFuture<'a>: Future<Output = Result<Vec<u8>, Error>> + Send + 'a
    where
        Self: 'a;

    /// Send data over the transport.
    ///
    /// Implementations should ensure all data is sent (e.g., using write_all)
    /// and flushed before returning.
    fn send<'a>(&'a mut self, data: &'a [u8]) -> Self::SendFuture<'a>;

    /// Receive data from the transport.
    ///
    /// Implementations should read available data and return it as a Vec<u8>.
    /// The exact amount of data read depends on the transport implementation
    /// and what's available.
    fn receive(&mut self) -> Self::ReceiveFuture<'_>;
}

/// Adapter that implements RawTransport for any type implementing AsyncTransport.
///
/// This allows the simplified AsyncTransport implementations to work with the
/// existing ViscaTransport wrapper.
#[derive(Debug)]
pub struct AsyncTransportAdapter<T> {
    inner: T,
    description: String,
}

impl<T: AsyncTransport + Debug> AsyncTransportAdapter<T> {
    /// Create a new adapter wrapping an AsyncTransport.
    pub fn new(inner: T, description: String) -> Self {
        Self { inner, description }
    }
}

impl<T: AsyncTransport + Debug + 'static> RawTransport for AsyncTransportAdapter<T> {
    fn send<'a>(&'a mut self, data: &'a [u8]) -> TransportFuture<'a, ()> {
        Box::pin(self.inner.send(data))
    }

    fn receive(&mut self) -> TransportFuture<'_, Vec<u8>> {
        Box::pin(self.inner.receive())
    }

    fn is_connected(&self) -> bool {
        true // Simplified transports are always considered connected
    }

    fn description(&self) -> &str {
        &self.description
    }
}
