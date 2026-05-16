//! Runtime abstraction that binds executor and transport connectors.
//!
//! This module provides a unified `Runtime` trait that ensures type-safe pairing
//! of executors and their corresponding transport implementations, preventing
//! cross-runtime mismatches at compile time.

#[cfg(feature = "mode-async")]
use std::time::Instant;

#[cfg(feature = "mode-async")]
use crate::{
    executor::Executor,
    transport::{builder::TransportConfig, AsyncTransport, HasTransportConfig},
    Error,
};

/// Runtime trait that binds executor and transport connectors at the type level.
///
/// This trait provides a cohesive runtime unit that includes both an executor
/// for task scheduling and transport connectors for network I/O. By binding
/// these at the type level, we ensure that incompatible runtime combinations
/// (e.g., Tokio transport with smol executor) are impossible to create.
///
/// # Example
///
/// ```rust,ignore
/// # #[cfg(feature = "runtime-tokio")]
/// use grafton_visca::{Runtime, TokioRuntime};
///
/// # #[cfg(feature = "runtime-tokio")]
/// # #[tokio::main]
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let runtime = TokioRuntime::from_current()?;
/// let tcp_transport = runtime.connect_tcp("192.168.0.110:5678", Default::default()).await?;
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "mode-async")]
pub trait Runtime: Executor + Clone + Send + Sync + 'static {
    /// TCP transport type for this runtime.
    type TcpTransport: AsyncTransport + HasTransportConfig + Send + Sync + 'static;

    /// UDP transport type for this runtime.
    type UdpTransport: AsyncTransport + HasTransportConfig + Send + Sync + 'static;

    /// Connect to a TCP endpoint.
    ///
    /// This method creates a TCP transport using the runtime's specific
    /// implementation. The transport is configured with the provided settings.
    async fn connect_tcp(
        &self,
        addr: &str,
        cfg: TransportConfig,
    ) -> Result<Self::TcpTransport, Error>;

    /// Connect to a UDP endpoint.
    ///
    /// This method creates a UDP transport using the runtime's specific
    /// implementation. The transport is configured with the provided settings.
    async fn connect_udp(
        &self,
        addr: &str,
        cfg: TransportConfig,
    ) -> Result<Self::UdpTransport, Error>;

    /// Get the current time according to this runtime.
    ///
    /// Default implementation delegates to the executor's `now()` method.
    /// This allows test runtimes to provide virtual time.
    fn now(&self) -> Instant {
        <Self as Executor>::now(self)
    }
}

/// Runtime trait extension for serial transport support.
///
/// This sub-trait extends the base Runtime trait with serial-specific connectivity.
/// It's kept separate to avoid forcing all runtimes to implement serial support
/// when the serialport feature is enabled.
///
/// Currently only implemented for Tokio runtime as it has tokio-serial integration.
#[cfg(all(feature = "mode-async", feature = "transport-serial-tokio"))]
pub trait RuntimeSerial: Runtime {
    /// Serial transport type for this runtime.
    type SerialTransport: AsyncTransport + HasTransportConfig + Send + Sync + 'static;

    /// Connect to a serial port.
    ///
    /// This method creates a serial transport using the runtime's specific
    /// implementation. The transport is configured with the provided settings.
    async fn connect_serial(
        &self,
        cfg: crate::transport::serial::Config,
    ) -> Result<Self::SerialTransport, Error>;
}

/// Transport handle that wraps TCP, UDP, or Serial transport for a specific runtime.
///
/// This enum is parameterized over a specific runtime, ensuring type safety and
/// reducing enum variants from "runtime × transport" to just "TCP | UDP | Serial".
/// Each variant is monomorphized per runtime, eliminating cross-runtime bloat and
/// dynamic dispatch.
///
/// This unified approach allows downstream users to implement traits uniformly across
/// all transport types, resolving trait coherence issues that occurred when Serial
/// was handled separately.
///
/// # Example
///
/// ```rust,ignore
/// # #[cfg(feature = "runtime-tokio")]
/// use grafton_visca::{Runtime, TokioRuntime, TransportHandle};
///
/// # #[cfg(feature = "runtime-tokio")]
/// # #[tokio::main]
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let runtime = TokioRuntime::from_current()?;
/// let transport: TransportHandle<TokioRuntime> =
///     TransportHandle::Tcp(runtime.connect_tcp("192.168.0.110:5678", Default::default()).await?);
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "mode-async")]
#[derive(Debug)]
pub enum TransportHandle<R: Runtime> {
    /// TCP transport for this runtime.
    Tcp(R::TcpTransport),
    /// UDP transport for this runtime.
    Udp(R::UdpTransport),
    /// Serial transport (only available for Tokio runtime with transport-serial-tokio feature).
    ///
    /// Note: This uses the concrete Tokio serial type directly to avoid requiring
    /// RuntimeSerial bound on all uses of TransportHandle.
    #[cfg(feature = "transport-serial-tokio")]
    Serial(Box<crate::transport::tokio::serial::Serial>),
}

#[cfg(feature = "mode-async")]
impl<R: Runtime> AsyncTransport for TransportHandle<R> {
    async fn send(&mut self, bytes: &[u8]) -> Result<(), Error> {
        match self {
            TransportHandle::Tcp(transport) => transport.send(bytes).await,
            TransportHandle::Udp(transport) => transport.send(bytes).await,
            #[cfg(feature = "transport-serial-tokio")]
            TransportHandle::Serial(transport) => transport.send(bytes).await,
        }
    }

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        match self {
            TransportHandle::Tcp(transport) => transport.recv_into(dst).await,
            TransportHandle::Udp(transport) => transport.recv_into(dst).await,
            #[cfg(feature = "transport-serial-tokio")]
            TransportHandle::Serial(transport) => transport.recv_into(dst).await,
        }
    }
}

#[cfg(feature = "mode-async")]
impl<R: Runtime> HasTransportConfig for TransportHandle<R> {
    fn transport_config(&self) -> &TransportConfig {
        match self {
            TransportHandle::Tcp(transport) => transport.transport_config(),
            TransportHandle::Udp(transport) => transport.transport_config(),
            #[cfg(feature = "transport-serial-tokio")]
            TransportHandle::Serial(transport) => transport.transport_config(),
        }
    }

    fn standard_transport_kind(&self) -> Option<crate::camera::TransportKind> {
        Some(match self {
            TransportHandle::Tcp(_) => crate::camera::TransportKind::Tcp,
            TransportHandle::Udp(_) => crate::camera::TransportKind::Udp,
            #[cfg(feature = "transport-serial-tokio")]
            TransportHandle::Serial(_) => crate::camera::TransportKind::Serial,
        })
    }
}

// Tokio runtime implementation
#[cfg(feature = "runtime-tokio")]
mod tokio_impl {
    use super::*;
    use crate::{
        executor::TokioExecutor,
        runtime_adapters::tokio::{TcpTransport, UdpTransport},
    };

    use std::future::Future;

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

        type Detach = <TokioExecutor as Executor>::Detach;

        fn spawn_with_detach<F>(&self, fut: F) -> (Self::Join<F::Output>, Self::Detach)
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            self.executor.spawn_with_detach(fut)
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

        fn now(&self) -> Instant {
            self.executor.now()
        }
    }

    impl Runtime for TokioRuntime {
        type TcpTransport = TcpTransport;
        type UdpTransport = UdpTransport;

        async fn connect_tcp(
            &self,
            addr: &str,
            cfg: TransportConfig,
        ) -> Result<Self::TcpTransport, Error> {
            // Timeout is enforced at the connector layer (single source of truth)
            TcpTransport::connect_with_config(addr, cfg).await
        }

        async fn connect_udp(
            &self,
            addr: &str,
            cfg: TransportConfig,
        ) -> Result<Self::UdpTransport, Error> {
            // Timeout is enforced at the connector layer (single source of truth)
            UdpTransport::connect_with_config(addr, cfg).await
        }
    }

    // Implement RuntimeSerial for TokioRuntime when tokio-serial feature is enabled
    #[cfg(feature = "transport-serial-tokio")]
    impl RuntimeSerial for TokioRuntime {
        type SerialTransport = crate::transport::tokio::serial::Serial;

        async fn connect_serial(
            &self,
            cfg: crate::transport::serial::Config,
        ) -> Result<Self::SerialTransport, Error> {
            // Use the unified Config directly (it's now the same type)
            crate::transport::tokio::serial::Serial::connect(cfg).await
        }
    }
}

#[cfg(feature = "runtime-tokio")]
pub use tokio_impl::TokioRuntime;

// smol runtime implementation
#[cfg(feature = "runtime-smol")]
mod smol_impl {
    use super::*;
    use crate::{
        executor::SmolExecutor,
        runtime_adapters::smol::{TcpTransport, UdpTransport},
    };

    use std::future::Future;

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

        type Detach = <SmolExecutor as Executor>::Detach;

        fn spawn_with_detach<F>(&self, fut: F) -> (Self::Join<F::Output>, Self::Detach)
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            self.executor.spawn_with_detach(fut)
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

        fn now(&self) -> Instant {
            self.executor.now()
        }
    }

    impl Runtime for SmolRuntime {
        type TcpTransport = TcpTransport;
        type UdpTransport = UdpTransport;

        async fn connect_tcp(
            &self,
            addr: &str,
            cfg: TransportConfig,
        ) -> Result<Self::TcpTransport, Error> {
            // Timeout is enforced at the connector layer (single source of truth)
            TcpTransport::connect_with_config(addr, cfg).await
        }

        async fn connect_udp(
            &self,
            addr: &str,
            cfg: TransportConfig,
        ) -> Result<Self::UdpTransport, Error> {
            // Timeout is enforced at the connector layer (single source of truth)
            UdpTransport::connect_with_config(addr, cfg).await
        }
    }
}

#[cfg(feature = "runtime-smol")]
pub use smol_impl::SmolRuntime;
