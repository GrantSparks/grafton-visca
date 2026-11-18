//! Runtime abstraction that binds executor and transport connectors.
//!
//! This module provides a unified `Runtime` trait that ensures type-safe pairing
//! of executors and their corresponding transport implementations, preventing
//! cross-runtime mismatches at compile time.

#[cfg(feature = "async")]
use std::future::Future;

#[cfg(feature = "async")]
use crate::{
    executor::Executor,
    transport::{
        AsyncTransport, HasTransportConfig, IoBounds, NetworkEndpoint, ReceiveOutcome, TcpConnect,
        UdpConnect,
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
/// ```no_run
/// # #[cfg(feature = "runtime-tokio")]
/// use grafton_visca::{runtime::{Runtime, TokioRuntime}, transport::{IoBounds, NetworkEndpoint, TcpConnect}};
///
/// # #[cfg(feature = "runtime-tokio")]
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let runtime = TokioRuntime::from_current()?;
/// let tcp_transport = runtime
///     .connect_tcp(NetworkEndpoint::parse("192.168.0.110:5678", None)?, TcpConnect::default(), IoBounds::default())
///     .await?;
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
    /// [`crate::runtime::TransportHandle`] cannot be constructed with Tokio reactor I/O for
    /// the wrong runtime.
    #[cfg(feature = "transport-serial-tokio")]
    type SerialTransport: AsyncTransport + HasTransportConfig + Send + Sync + 'static;

    /// Connect to a checked TCP endpoint with the provided settings.
    ///
    /// Parse a textual endpoint with [`NetworkEndpoint::parse`]. Use the
    /// connector value for socket setup and report the I/O bounds from the
    /// returned transport.
    /// The returned future is [`Send`] so a session can move it to an executor
    /// task before polling.
    fn connect_tcp(
        &self,
        endpoint: NetworkEndpoint,
        options: TcpConnect,
        io_bounds: IoBounds,
    ) -> impl Future<Output = Result<Self::TcpTransport, Error>> + Send + '_;

    /// Connect to a checked UDP endpoint with the provided settings.
    fn connect_udp(
        &self,
        endpoint: NetworkEndpoint,
        options: UdpConnect,
        io_bounds: IoBounds,
    ) -> impl Future<Output = Result<Self::UdpTransport, Error>> + Send + '_;

    /// Connect to a serial port using checked device settings.
    ///
    /// A runtime without serial support returns [`Error::FeatureNotSupported`]
    /// before opening a device.
    #[cfg(feature = "transport-serial-tokio")]
    fn connect_serial(
        &self,
        config: crate::transport::SerialConfig,
        io_bounds: IoBounds,
    ) -> impl Future<Output = Result<Self::SerialTransport, Error>> + Send + '_;
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

    async fn recv_into(&mut self, _dst: &mut [u8]) -> Result<ReceiveOutcome, Error> {
        match *self {}
    }
}

#[cfg(all(feature = "async", feature = "transport-serial-tokio"))]
impl HasTransportConfig for std::convert::Infallible {
    fn identity(&self) -> crate::transport::TransportIdentity<'_> {
        match *self {}
    }

    fn io_bounds(&self) -> &IoBounds {
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
/// ```no_run
/// # #[cfg(feature = "runtime-tokio")]
/// use grafton_visca::{runtime::{Runtime, TokioRuntime, TransportHandle}, transport::{IoBounds, NetworkEndpoint, TcpConnect}};
///
/// # #[cfg(feature = "runtime-tokio")]
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let runtime = TokioRuntime::from_current()?;
/// let transport: TransportHandle<TokioRuntime> = TransportHandle::Tcp(
///     runtime
///         .connect_tcp(NetworkEndpoint::parse("192.168.0.110:5678", None)?, TcpConnect::default(), IoBounds::default())
///         .await?,
/// );
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

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<ReceiveOutcome, Error> {
        match self {
            TransportHandle::Tcp(transport) => transport.recv_into(dst).await,
            TransportHandle::Udp(transport) => transport.recv_into(dst).await,
            #[cfg(feature = "transport-serial-tokio")]
            TransportHandle::Serial(transport) => transport.recv_into(dst).await,
        }
    }
}

#[cfg(feature = "async")]
impl<R: Runtime> HasTransportConfig for TransportHandle<R> {
    fn identity(&self) -> crate::transport::TransportIdentity<'_> {
        match self {
            TransportHandle::Tcp(transport) => transport.identity(),
            TransportHandle::Udp(transport) => transport.identity(),
            #[cfg(feature = "transport-serial-tokio")]
            TransportHandle::Serial(transport) => transport.identity(),
        }
    }

    fn io_bounds(&self) -> &IoBounds {
        match self {
            TransportHandle::Tcp(transport) => transport.io_bounds(),
            TransportHandle::Udp(transport) => transport.io_bounds(),
            #[cfg(feature = "transport-serial-tokio")]
            TransportHandle::Serial(transport) => transport.io_bounds(),
        }
    }
}

// Tokio runtime implementation
/// Implements [`crate::runtime::Executor`] for a built-in runtime by delegating every item to
/// the executor it wraps in its `executor` field.
#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
macro_rules! delegate_executor {
    ($runtime:ty, $executor:ty) => {
        impl Executor for $runtime {
            type Timer = <$executor as Executor>::Timer;

            fn timer(&self, deadline: std::time::Instant) -> Self::Timer {
                self.executor.timer(deadline)
            }

            fn spawn_detached<F>(&self, fut: F)
            where
                F: Future<Output = ()> + Send + 'static,
            {
                self.executor.spawn_detached(fut);
            }

            fn sleep(&self, duration: std::time::Duration) -> impl Future<Output = ()> + Send + '_ {
                self.executor.sleep(duration)
            }

            fn now(&self) -> std::time::Instant {
                self.executor.now()
            }
        }
    };
}

#[cfg(feature = "runtime-tokio")]
mod tokio_impl {
    use super::*;
    use crate::{
        executor::TokioExecutor,
        transport::tokio::{TcpTransport, UdpTransport},
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
    delegate_executor!(TokioRuntime, TokioExecutor);

    impl Runtime for TokioRuntime {
        type TcpTransport = TcpTransport;
        type UdpTransport = UdpTransport;
        #[cfg(feature = "transport-serial-tokio")]
        type SerialTransport = crate::transport::tokio::serial::Serial;

        #[allow(clippy::manual_async_fn)]
        fn connect_tcp(
            &self,
            endpoint: NetworkEndpoint,
            options: TcpConnect,
            io_bounds: IoBounds,
        ) -> impl Future<Output = Result<Self::TcpTransport, Error>> + Send + '_ {
            let handle = self.executor.handle().clone();
            async move {
                // Own the connector on the selected runtime rather than the task's
                // ambient Tokio context, just as for the resulting session actor.
                let stream =
                    crate::transport::tokio::connectors::connect_tcp_on(&handle, endpoint, options)
                        .await?;
                Ok(TcpTransport::new(stream, io_bounds))
            }
        }

        #[allow(clippy::manual_async_fn)]
        fn connect_udp(
            &self,
            endpoint: NetworkEndpoint,
            options: UdpConnect,
            io_bounds: IoBounds,
        ) -> impl Future<Output = Result<Self::UdpTransport, Error>> + Send + '_ {
            let handle = self.executor.handle().clone();
            async move {
                let socket =
                    crate::transport::tokio::connectors::connect_udp_on(&handle, endpoint, options)
                        .await?;
                Ok(UdpTransport::new(socket, io_bounds))
            }
        }

        #[cfg(feature = "transport-serial-tokio")]
        fn connect_serial(
            &self,
            config: crate::transport::SerialConfig,
            io_bounds: IoBounds,
        ) -> impl Future<Output = Result<Self::SerialTransport, Error>> + Send + '_ {
            let handle = self.executor.handle().clone();
            crate::transport::tokio::serial::Serial::connect_on(handle, config, io_bounds)
        }
    }

    #[cfg(test)]
    #[allow(clippy::expect_used)]
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
            let endpoint = NetworkEndpoint::parse(&listener.local_addr()?.to_string(), None)?;
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
                    .connect_tcp(endpoint, TcpConnect::default(), IoBounds::default())
                    .await
                    ?;
                assert_eq!(accepted.await??, selected_for_accept.id());
                let udp = runtime
                    .connect_udp(NetworkEndpoint::parse("127.0.0.1:9", None)?, UdpConnect::default(), IoBounds::default())
                    .await
                    ?;
                drop((tcp, udp));

                let (task, completed) = flume::bounded(1);
                runtime.spawn_detached(async move {
                    let _ = task.send(tokio::runtime::Handle::current().id());
                });
                assert_eq!(completed.recv_async().await?, selected_handle.id());

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

                let timeout = crate::timeout::within(future::pending::<()>(), runtime.sleep(Duration::from_secs(1)));
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
        transport::smol::{TcpTransport, UdpTransport},
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
    delegate_executor!(SmolRuntime, SmolExecutor);

    impl Runtime for SmolRuntime {
        type TcpTransport = TcpTransport;
        type UdpTransport = UdpTransport;
        #[cfg(feature = "transport-serial-tokio")]
        type SerialTransport = std::convert::Infallible;

        #[allow(clippy::manual_async_fn)]
        fn connect_tcp(
            &self,
            endpoint: NetworkEndpoint,
            options: TcpConnect,
            io_bounds: IoBounds,
        ) -> impl Future<Output = Result<Self::TcpTransport, Error>> + Send + '_ {
            async move { TcpTransport::connect_endpoint(&endpoint, options, io_bounds).await }
        }

        #[allow(clippy::manual_async_fn)]
        fn connect_udp(
            &self,
            endpoint: NetworkEndpoint,
            options: UdpConnect,
            io_bounds: IoBounds,
        ) -> impl Future<Output = Result<Self::UdpTransport, Error>> + Send + '_ {
            async move { UdpTransport::connect_endpoint(&endpoint, options, io_bounds).await }
        }

        #[cfg(feature = "transport-serial-tokio")]
        #[allow(clippy::manual_async_fn)]
        fn connect_serial(
            &self,
            _config: crate::transport::SerialConfig,
            _io_bounds: IoBounds,
        ) -> impl Future<Output = Result<Self::SerialTransport, Error>> + Send + '_ {
            async {
                Err(Error::FeatureNotSupported {
                    feature: "serial transport for this runtime",
                })
            }
        }
    }

    #[cfg(all(test, feature = "transport-serial-tokio"))]
    #[allow(clippy::expect_used)]
    mod tests {
        use super::*;

        #[test]
        fn unsupported_serial_runtime_returns_a_send_future_without_device_io() {
            fn connect<R: Runtime>(
                runtime: &R,
                config: crate::transport::SerialConfig,
            ) -> impl Future<Output = Result<R::SerialTransport, Error>> + Send + '_ {
                runtime.connect_serial(config, IoBounds::default())
            }

            let runtime = SmolRuntime::new();
            let config = crate::transport::SerialConfig::new(
                "grafton-visca-unsupported-runtime-serial-device",
            );
            let result = smol::block_on(connect(&runtime, config));
            assert!(matches!(
                result,
                Err(Error::FeatureNotSupported {
                    feature: "serial transport for this runtime"
                })
            ));
        }
    }
}

#[cfg(feature = "runtime-smol")]
pub use smol_impl::SmolRuntime;
