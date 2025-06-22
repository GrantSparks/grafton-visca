//! Simplified async transport trait for runtime-agnostic transport implementations.
//!
//! This module provides a minimal transport abstraction that allows users to
//! implement transports for different async runtimes without complex trait hierarchies.

use crate::error::Error;
use std::future::Future;

/// Async transport trait for implementing custom VISCA transports.
///
/// This trait defines the minimal interface needed for async I/O operations.
/// It's designed to be runtime-agnostic, allowing implementations for any
/// async runtime (tokio, async-std, smol, etc.).
///
/// The trait uses `&self` methods with interior mutability, enabling natural
/// concurrent usage patterns in async code.
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
/// use std::sync::Arc;
/// use tokio::sync::Mutex;
///
/// pub struct TokioTcpTransport {
///     stream: Arc<Mutex<TcpStream>>,
/// }
///
/// impl AsyncTransport for TokioTcpTransport {
///     type SendFuture<'a> = Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>>;
///     type ReceiveFuture<'a> = Pin<Box<dyn Future<Output = Result<Vec<u8>, Error>> + Send + 'a>>;
///
///     fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFuture<'a> {
///         Box::pin(async move {
///             let mut stream = self.stream.lock().await;
///             stream.write_all(data).await?;
///             stream.flush().await?;
///             Ok(())
///         })
///     }
///
///     fn receive(&self) -> Self::ReceiveFuture<'_> {
///         Box::pin(async move {
///             let mut stream = self.stream.lock().await;
///             let mut buf = vec![0; 1024];
///             let n = stream.read(&mut buf).await?;
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
/// use std::sync::Arc;
/// use async_std::sync::Mutex;
///
/// pub struct AsyncStdTcpTransport {
///     stream: Arc<Mutex<TcpStream>>,
/// }
///
/// impl AsyncTransport for AsyncStdTcpTransport {
///     type SendFuture<'a> = Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>>;
///     type ReceiveFuture<'a> = Pin<Box<dyn Future<Output = Result<Vec<u8>, Error>> + Send + 'a>>;
///
///     fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFuture<'a> {
///         Box::pin(async move {
///             let mut stream = self.stream.lock().await;
///             stream.write_all(data).await?;
///             stream.flush().await?;
///             Ok(())
///         })
///     }
///
///     fn receive(&self) -> Self::ReceiveFuture<'_> {
///         Box::pin(async move {
///             let mut stream = self.stream.lock().await;
///             let mut buf = vec![0; 1024];
///             let n = stream.read(&mut buf).await?;
///             buf.truncate(n);
///             Ok(buf)
///         })
///     }
/// }
/// ```
pub trait AsyncTransport: Send + Sync + std::fmt::Debug {
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
    fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFuture<'a>;

    /// Receive data from the transport.
    ///
    /// Implementations should read available data and return it as a `Vec<u8>`.
    /// The exact amount of data read depends on the transport implementation
    /// and what's available.
    fn receive(&self) -> Self::ReceiveFuture<'_>;
}
