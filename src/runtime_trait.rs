//! Runtime abstraction that binds executor and transport connectors.
//!
//! This module provides a unified `Runtime` trait that ensures type-safe pairing
//! of executors and their corresponding transport implementations, preventing
//! cross-runtime mismatches at compile time.

#[cfg(feature = "async")]
use core::future::Future;
#[cfg(feature = "async")]
use std::time::Instant;

#[cfg(feature = "async")]
use crate::{
    executor::Executor, transport::builder::TransportConfig, transport::AsyncTransport, Error,
};

/// Runtime trait that binds executor and transport connectors at the type level.
///
/// This trait provides a cohesive runtime unit that includes both an executor
/// for task scheduling and transport connectors for network I/O. By binding
/// these at the type level, we ensure that incompatible runtime combinations
/// (e.g., Tokio transport with async-std executor) are impossible to create.
///
/// # Example
///
/// ```rust,ignore
/// # #[cfg(feature = "rt-tokio")]
/// use grafton_visca::{Runtime, TokioRuntime};
///
/// # #[cfg(feature = "rt-tokio")]
/// # #[tokio::main]
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let runtime = TokioRuntime::from_current()?;
/// let tcp_transport = runtime.connect_tcp("192.168.0.110:5678", Default::default()).await?;
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "async")]
pub trait Runtime: Executor + Clone + Send + Sync + 'static {
    /// TCP transport type for this runtime.
    type TcpTransport: AsyncTransport + Send + 'static;

    /// UDP transport type for this runtime.
    type UdpTransport: AsyncTransport + Send + 'static;

    /// Connect to a TCP endpoint.
    ///
    /// This method creates a TCP transport using the runtime's specific
    /// implementation. The transport is configured with the provided settings.
    fn connect_tcp(
        addr: &str,
        cfg: TransportConfig,
    ) -> impl Future<Output = Result<Self::TcpTransport, Error>> + Send;

    /// Connect to a UDP endpoint.
    ///
    /// This method creates a UDP transport using the runtime's specific
    /// implementation. The transport is configured with the provided settings.
    fn connect_udp(
        addr: &str,
        cfg: TransportConfig,
    ) -> impl Future<Output = Result<Self::UdpTransport, Error>> + Send;

    /// Get the current time according to this runtime.
    ///
    /// Default implementation delegates to the executor's `now()` method.
    /// This allows test runtimes to provide virtual time.
    fn now(&self) -> Instant {
        <Self as Executor>::now(self)
    }
}

/// Transport handle that wraps either TCP or UDP transport for a specific runtime.
///
/// This enum replaces `AnyTransport` by being parameterized over a specific runtime,
/// ensuring type safety and reducing enum variants from "runtime × transport" to just
/// "TCP | UDP". Each variant is monomorphized per runtime, eliminating cross-runtime
/// bloat and dynamic dispatch.
///
/// # Example
///
/// ```rust,ignore
/// # #[cfg(feature = "rt-tokio")]
/// use grafton_visca::{Runtime, TokioRuntime, TransportHandle};
///
/// # #[cfg(feature = "rt-tokio")]
/// # #[tokio::main]
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let runtime = TokioRuntime::from_current()?;
/// let transport: TransportHandle<TokioRuntime> =
///     TransportHandle::Tcp(runtime.connect_tcp("192.168.0.110:5678", Default::default()).await?);
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "async")]
#[derive(Debug)]
pub enum TransportHandle<R: Runtime> {
    /// TCP transport for this runtime.
    Tcp(R::TcpTransport),
    /// UDP transport for this runtime.
    Udp(R::UdpTransport),
}

#[cfg(feature = "async")]
impl<R: Runtime> AsyncTransport for TransportHandle<R> {
    async fn send(&mut self, bytes: &[u8]) -> Result<(), Error> {
        match self {
            TransportHandle::Tcp(transport) => transport.send(bytes).await,
            TransportHandle::Udp(transport) => transport.send(bytes).await,
        }
    }

    async fn recv(&mut self) -> Result<bytes::Bytes, Error> {
        match self {
            TransportHandle::Tcp(transport) => transport.recv().await,
            TransportHandle::Udp(transport) => transport.recv().await,
        }
    }
}

// Tokio runtime implementation
#[cfg(feature = "rt-tokio")]
mod tokio_impl {
    use super::*;
    use crate::executor::TokioExecutor;
    use crate::runtime_adapters::tokio::{TcpTransport, UdpTransport};

    /// Tokio runtime implementation.
    ///
    /// This runtime binds the Tokio executor with Tokio-specific TCP and UDP
    /// transport implementations, ensuring type-safe runtime consistency.
    #[derive(Debug, Clone)]
    pub struct TokioRuntime {
        executor: TokioExecutor,
    }

    impl TokioRuntime {
        /// Create a runtime from the current Tokio runtime.
        ///
        /// # Errors
        ///
        /// Returns an error if no Tokio runtime is active.
        pub fn from_current() -> Result<Self, Error> {
            Ok(Self {
                executor: TokioExecutor::from_current()?,
            })
        }

        /// Create a runtime from a specific Tokio handle.
        pub fn from_handle(handle: tokio::runtime::Handle) -> Self {
            Self {
                executor: TokioExecutor::from_handle(handle),
            }
        }
    }

    // Implement Executor trait by delegating to inner executor
    impl Executor for TokioRuntime {
        type Join<T>
            = <TokioExecutor as Executor>::Join<T>
        where
            T: Send + 'static;

        type LocalJoin<T>
            = <TokioExecutor as Executor>::LocalJoin<T>
        where
            T: 'static;

        fn spawn<F>(&self, fut: F) -> Self::Join<F::Output>
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            self.executor.spawn(fut)
        }

        fn spawn_local<F>(&self, fut: F) -> Self::LocalJoin<F::Output>
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            self.executor.spawn_local(fut)
        }

        fn block_on<F: Future>(&self, fut: F) -> F::Output {
            self.executor.block_on(fut)
        }

        fn sleep(&self, duration: std::time::Duration) -> impl Future<Output = ()> + Send + '_ {
            self.executor.sleep(duration)
        }

        fn timeout<'a, F, T>(
            &'a self,
            duration: std::time::Duration,
            fut: F,
        ) -> impl Future<Output = Result<T, Error>> + Send + 'a
        where
            F: Future<Output = T> + Send + 'a,
            T: Send + 'a,
        {
            self.executor.timeout(duration, fut)
        }

        fn timeout_owned<T>(
            &self,
            duration: std::time::Duration,
            fut: impl Future<Output = T> + Send + 'static,
        ) -> std::pin::Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'static>>
        where
            T: Send + 'static,
        {
            self.executor.timeout_owned(duration, fut)
        }

        fn now(&self) -> Instant {
            self.executor.now()
        }
    }

    impl Runtime for TokioRuntime {
        type TcpTransport = TcpTransport;
        type UdpTransport = UdpTransport;

        async fn connect_tcp(
            addr: &str,
            cfg: TransportConfig,
        ) -> Result<Self::TcpTransport, Error> {
            TcpTransport::connect_with_config(addr, cfg).await
        }

        async fn connect_udp(
            addr: &str,
            cfg: TransportConfig,
        ) -> Result<Self::UdpTransport, Error> {
            UdpTransport::connect_with_config(addr, cfg).await
        }
    }
}

#[cfg(feature = "rt-tokio")]
pub use tokio_impl::TokioRuntime;

// async-std runtime implementation
#[cfg(feature = "rt-async-std")]
mod async_std_impl {
    use super::*;
    use crate::executor::AsyncStdExecutor;
    use crate::runtime_adapters::async_std::{TcpTransport, UdpTransport};

    /// async-std runtime implementation.
    ///
    /// This runtime binds the async-std executor with async-std-specific TCP and UDP
    /// transport implementations, ensuring type-safe runtime consistency.
    #[derive(Debug, Clone, Copy)]
    pub struct AsyncStdRuntime {
        executor: AsyncStdExecutor,
    }

    impl AsyncStdRuntime {
        /// Create a new async-std runtime.
        pub fn new() -> Self {
            Self {
                executor: AsyncStdExecutor::new(),
            }
        }
    }

    impl Default for AsyncStdRuntime {
        fn default() -> Self {
            Self::new()
        }
    }

    // Implement Executor trait by delegating to inner executor
    impl Executor for AsyncStdRuntime {
        type Join<T>
            = <AsyncStdExecutor as Executor>::Join<T>
        where
            T: Send + 'static;

        type LocalJoin<T>
            = <AsyncStdExecutor as Executor>::LocalJoin<T>
        where
            T: 'static;

        fn spawn<F>(&self, fut: F) -> Self::Join<F::Output>
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            self.executor.spawn(fut)
        }

        fn spawn_local<F>(&self, fut: F) -> Self::LocalJoin<F::Output>
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            self.executor.spawn_local(fut)
        }

        fn block_on<F: Future>(&self, fut: F) -> F::Output {
            self.executor.block_on(fut)
        }

        fn sleep(&self, duration: std::time::Duration) -> impl Future<Output = ()> + Send + '_ {
            self.executor.sleep(duration)
        }

        fn timeout<'a, F, T>(
            &'a self,
            duration: std::time::Duration,
            fut: F,
        ) -> impl Future<Output = Result<T, Error>> + Send + 'a
        where
            F: Future<Output = T> + Send + 'a,
            T: Send + 'a,
        {
            self.executor.timeout(duration, fut)
        }

        fn timeout_owned<T>(
            &self,
            duration: std::time::Duration,
            fut: impl Future<Output = T> + Send + 'static,
        ) -> std::pin::Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'static>>
        where
            T: Send + 'static,
        {
            self.executor.timeout_owned(duration, fut)
        }

        fn now(&self) -> Instant {
            self.executor.now()
        }
    }

    impl Runtime for AsyncStdRuntime {
        type TcpTransport = TcpTransport;
        type UdpTransport = UdpTransport;

        async fn connect_tcp(
            addr: &str,
            cfg: TransportConfig,
        ) -> Result<Self::TcpTransport, Error> {
            TcpTransport::connect_with_config(addr, cfg).await
        }

        async fn connect_udp(
            addr: &str,
            cfg: TransportConfig,
        ) -> Result<Self::UdpTransport, Error> {
            UdpTransport::connect_with_config(addr, cfg).await
        }
    }
}

#[cfg(feature = "rt-async-std")]
pub use async_std_impl::AsyncStdRuntime;

// smol runtime implementation
#[cfg(feature = "rt-smol")]
mod smol_impl {
    use super::*;
    use crate::executor::SmolExecutor;
    use crate::runtime_adapters::smol::{TcpTransport, UdpTransport};

    /// smol runtime implementation.
    ///
    /// This runtime binds the smol executor with smol-specific TCP and UDP
    /// transport implementations, ensuring type-safe runtime consistency.
    #[derive(Debug, Clone, Copy)]
    pub struct SmolRuntime {
        executor: SmolExecutor,
    }

    impl SmolRuntime {
        /// Create a new smol runtime.
        pub fn new() -> Self {
            Self {
                executor: SmolExecutor::new(),
            }
        }
    }

    impl Default for SmolRuntime {
        fn default() -> Self {
            Self::new()
        }
    }

    // Implement Executor trait by delegating to inner executor
    impl Executor for SmolRuntime {
        type Join<T>
            = <SmolExecutor as Executor>::Join<T>
        where
            T: Send + 'static;

        type LocalJoin<T>
            = <SmolExecutor as Executor>::LocalJoin<T>
        where
            T: 'static;

        fn spawn<F>(&self, fut: F) -> Self::Join<F::Output>
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            self.executor.spawn(fut)
        }

        fn spawn_local<F>(&self, fut: F) -> Self::LocalJoin<F::Output>
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            self.executor.spawn_local(fut)
        }

        fn block_on<F: Future>(&self, fut: F) -> F::Output {
            self.executor.block_on(fut)
        }

        fn sleep(&self, duration: std::time::Duration) -> impl Future<Output = ()> + Send + '_ {
            self.executor.sleep(duration)
        }

        fn timeout<'a, F, T>(
            &'a self,
            duration: std::time::Duration,
            fut: F,
        ) -> impl Future<Output = Result<T, Error>> + Send + 'a
        where
            F: Future<Output = T> + Send + 'a,
            T: Send + 'a,
        {
            self.executor.timeout(duration, fut)
        }

        fn timeout_owned<T>(
            &self,
            duration: std::time::Duration,
            fut: impl Future<Output = T> + Send + 'static,
        ) -> std::pin::Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'static>>
        where
            T: Send + 'static,
        {
            self.executor.timeout_owned(duration, fut)
        }

        fn now(&self) -> Instant {
            self.executor.now()
        }
    }

    impl Runtime for SmolRuntime {
        type TcpTransport = TcpTransport;
        type UdpTransport = UdpTransport;

        async fn connect_tcp(
            addr: &str,
            cfg: TransportConfig,
        ) -> Result<Self::TcpTransport, Error> {
            TcpTransport::connect_with_config(addr, cfg).await
        }

        async fn connect_udp(
            addr: &str,
            cfg: TransportConfig,
        ) -> Result<Self::UdpTransport, Error> {
            UdpTransport::connect_with_config(addr, cfg).await
        }
    }
}

#[cfg(feature = "rt-smol")]
pub use smol_impl::SmolRuntime;
