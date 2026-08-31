//! Runtime abstraction that binds executor and transport connectors.
//!
//! This module provides a unified `Runtime` trait that ensures type-safe pairing
//! of executors and their corresponding transport implementations, preventing
//! cross-runtime mismatches at compile time.

#[cfg(feature = "async")]
use std::time::Instant;

#[cfg(feature = "async")]
use crate::{
    executor::Executor,
    transport::{
        builder::TransportConfig, AddressingMode, AsyncTransport, HasTransportConfig,
        ReceiveOutcome,
    },
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
#[cfg(feature = "async")]
pub trait Runtime: Executor + Clone + Send + Sync + 'static {
    /// TCP transport type for this runtime.
    type TcpTransport: AsyncTransport + HasTransportConfig + Send + Sync + 'static;

    /// UDP transport type for this runtime.
    type UdpTransport: AsyncTransport + HasTransportConfig + Send + Sync + 'static;

    /// Serial transport type for this runtime.
    ///
    /// This is present only with Tokio serial support. Runtimes that do not
    /// support Tokio serial must use an uninhabited type, so a
    /// [`TransportHandle`] cannot be constructed with Tokio reactor I/O for
    /// the wrong runtime.
    #[cfg(feature = "transport-serial-tokio")]
    type SerialTransport: AsyncTransport + HasTransportConfig + Send + Sync + 'static;

    /// Connect to a TCP endpoint.
    ///
    /// This method creates a TCP transport using the runtime's specific
    /// implementation. The transport is configured with the provided settings.
    /// Runtime connectors have no profile default port, so `addr` must include
    /// an explicit port. Explicit IPv6 ports require brackets.
    async fn connect_tcp(
        &self,
        addr: &str,
        cfg: TransportConfig,
    ) -> Result<Self::TcpTransport, Error>;

    /// Connect to a UDP endpoint.
    ///
    /// This method creates a UDP transport using the runtime's specific
    /// implementation. The transport is configured with the provided settings.
    /// Runtime connectors have no profile default port, so `addr` must include
    /// an explicit port. Explicit IPv6 ports require brackets.
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
/// [`Runtime::SerialTransport`] stays on the base trait so [`TransportHandle`]
/// remains runtime-paired, while runtimes without serial support use an
/// uninhabited associated type and do not implement this connector trait.
///
/// Currently only implemented for Tokio runtime as it has tokio-serial integration.
#[cfg(all(feature = "async", feature = "transport-serial-tokio"))]
pub trait RuntimeSerial: Runtime {
    /// Connect to a serial port.
    ///
    /// This method creates a serial transport using the runtime's specific
    /// implementation. The transport is configured with the provided settings.
    async fn connect_serial(
        &self,
        cfg: crate::transport::serial::Config,
    ) -> Result<Self::SerialTransport, Error>;
}

// A runtime that has no Tokio serial integration still needs an associated
// serial type while the `transport-serial-tokio` feature is enabled. Using an
// uninhabited type keeps `TransportHandle<R>` uniformly shaped without giving
// a non-Tokio runtime a safe way to construct its `Serial` variant.
#[cfg(all(feature = "async", feature = "transport-serial-tokio"))]
impl AsyncTransport for std::convert::Infallible {
    async fn send(&mut self, _bytes: &[u8]) -> Result<(), Error> {
        match *self {}
    }

    async fn recv_into(&mut self, _dst: &mut [u8]) -> Result<usize, Error> {
        match *self {}
    }
}

#[cfg(all(feature = "async", feature = "transport-serial-tokio"))]
impl HasTransportConfig for std::convert::Infallible {
    fn transport_config(&self) -> &TransportConfig {
        match *self {}
    }
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
#[cfg(feature = "async")]
#[derive(Debug)]
#[non_exhaustive]
pub enum TransportHandle<R: Runtime> {
    /// TCP transport for this runtime.
    Tcp(R::TcpTransport),
    /// UDP transport for this runtime.
    Udp(R::UdpTransport),
    /// Serial transport paired with this runtime.
    ///
    /// Tokio uses its Tokio-serial transport here. Runtimes without Tokio
    /// serial support use an uninhabited associated type, making this variant
    /// impossible to construct safely for them.
    #[cfg(feature = "transport-serial-tokio")]
    Serial(Box<R::SerialTransport>),
}

#[cfg(feature = "async")]
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

    async fn recv_into_with_outcome(&mut self, dst: &mut [u8]) -> Result<ReceiveOutcome, Error> {
        match self {
            TransportHandle::Tcp(transport) => transport.recv_into_with_outcome(dst).await,
            TransportHandle::Udp(transport) => transport.recv_into_with_outcome(dst).await,
            #[cfg(feature = "transport-serial-tokio")]
            TransportHandle::Serial(transport) => transport.recv_into_with_outcome(dst).await,
        }
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        match self {
            TransportHandle::Tcp(transport) => transport.addressing_mode_hint(),
            TransportHandle::Udp(transport) => transport.addressing_mode_hint(),
            #[cfg(feature = "transport-serial-tokio")]
            TransportHandle::Serial(transport) => transport.addressing_mode_hint(),
        }
    }

    fn send_semantics(&self) -> crate::transport::SendSemantics {
        // Forward to the wrapped transport rather than inheriting the trait
        // default (`SendSemantics::Stream`). The inner transport is the sole
        // authority on its send semantics: `Udp` reports `Datagram`, so a UDP
        // session opened through this wrapper must be governed by datagram
        // rules (a failed `send_to` or a malformed datagram fails one command,
        // it does not poison the session). Omitting this forward silently
        // demoted every UDP session built via `TransportHandle::Udp` to the
        // stream-poison policy. This mirrors `BlockingTransportHandle`.
        match self {
            TransportHandle::Tcp(transport) => transport.send_semantics(),
            TransportHandle::Udp(transport) => transport.send_semantics(),
            #[cfg(feature = "transport-serial-tokio")]
            TransportHandle::Serial(transport) => transport.send_semantics(),
        }
    }
}

#[cfg(feature = "async")]
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
        transport::{
            address::canonicalize_endpoint,
            socket_options::{TcpConnectionConfig, UdpSocketConfig},
        },
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
        #[cfg(feature = "transport-serial-tokio")]
        type SerialTransport = crate::transport::tokio::serial::Serial;

        async fn connect_tcp(
            &self,
            addr: &str,
            cfg: TransportConfig,
        ) -> Result<Self::TcpTransport, Error> {
            // Run the connector on this runtime's handle rather than the
            // ambient task's Tokio context. The resulting stream is then
            // owned by the actor this same runtime spawns.
            let address = canonicalize_endpoint(addr, None)?;
            let stream = crate::transport::tokio::connectors::connect_tcp_on(
                self.executor.handle(),
                address,
                TcpConnectionConfig::from(cfg),
            )
            .await?;
            Ok(TcpTransport::new(stream, cfg))
        }

        async fn connect_udp(
            &self,
            addr: &str,
            cfg: TransportConfig,
        ) -> Result<Self::UdpTransport, Error> {
            // As with TCP, DNS, timer and socket work belongs to the selected
            // runtime even when this future is polled by another Tokio runtime.
            let address = canonicalize_endpoint(addr, None)?;
            let socket = crate::transport::tokio::connectors::connect_udp_on(
                self.executor.handle(),
                address,
                UdpSocketConfig::from(cfg),
            )
            .await?;
            Ok(UdpTransport::new(socket, cfg))
        }
    }

    // Implement RuntimeSerial for TokioRuntime when tokio-serial feature is enabled
    #[cfg(feature = "transport-serial-tokio")]
    impl RuntimeSerial for TokioRuntime {
        async fn connect_serial(
            &self,
            cfg: crate::transport::serial::Config,
        ) -> Result<Self::SerialTransport, Error> {
            crate::transport::tokio::serial::Serial::connect_on(self.executor.handle(), cfg).await
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::{future, time::Duration};

        /// `from_handle` is allowed to be constructed and awaited while a
        /// different Tokio runtime is current. The ambient runtime below has a
        /// timer but deliberately no I/O driver: successful TCP/UDP setup
        /// therefore proves connector work used `selected`; advancing only
        /// the ambient clock must not fire a timer bound to `selected`.
        #[test]
        fn from_handle_binds_connectors_timers_and_spawn_to_selected_runtime(
        ) -> Result<(), Box<dyn std::error::Error>> {
            let selected = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()?;
            let listener =
                selected.block_on(async { tokio::net::TcpListener::bind("127.0.0.1:0").await })?;
            let address = listener.local_addr()?.to_string();
            let selected_handle = selected.handle().clone();

            let ambient = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()?;
            ambient.block_on(async move {
                tokio::time::pause();
                let runtime = TokioRuntime::from_handle(selected_handle.clone());
                let selected_for_accept = selected_handle.clone();
                let accepted = selected_handle.spawn(async move {
                    let _ = listener.accept().await?;
                    Ok::<_, std::io::Error>(tokio::runtime::Handle::current().id())
                });

                let tcp = runtime
                    .connect_tcp(&address, TransportConfig::default())
                    .await
                    ?;
                assert_eq!(accepted.await??, selected_for_accept.id());
                let udp = runtime
                    .connect_udp("127.0.0.1:9", TransportConfig::default())
                    .await
                    ?;
                drop((tcp, udp));

                let actor_task = runtime.spawn(async {
                    tokio::runtime::Handle::current().id()
                });
                assert_eq!(actor_task.await?, selected_handle.id());

                let before = Executor::now(&runtime);
                let sleep = runtime.sleep(Duration::from_secs(1));
                tokio::pin!(sleep);
                tokio::select! {
                    _ = &mut sleep => panic!("the selected runtime timer fired before its real deadline"),
                    _ = tokio::task::yield_now() => {}
                }
                tokio::time::advance(Duration::from_secs(3_600)).await;
                tokio::select! {
                    _ = &mut sleep => panic!("ambient time advanced a timer bound to the selected runtime"),
                    _ = tokio::task::yield_now() => {}
                }
                assert!(
                    Executor::now(&runtime).duration_since(before) < Duration::from_secs(30)
                );

                let timeout = runtime.timeout(Duration::from_secs(1), future::pending::<()>());
                tokio::pin!(timeout);
                tokio::select! {
                    result = &mut timeout => panic!("selected timer fired before advance: {result:?}"),
                    _ = tokio::task::yield_now() => {}
                }
                tokio::time::advance(Duration::from_secs(3_600)).await;
                tokio::select! {
                    result = &mut timeout => panic!("ambient time advanced a selected-runtime timeout: {result:?}"),
                    _ = tokio::task::yield_now() => {}
                }
                Ok::<(), Box<dyn std::error::Error>>(())
            })?;
            Ok(())
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
        #[cfg(feature = "transport-serial-tokio")]
        type SerialTransport = std::convert::Infallible;

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
