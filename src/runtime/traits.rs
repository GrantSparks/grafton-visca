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
        AsyncTransport, HasTransportConfig, NetworkEndpoint, ReceiveOutcome, TransportConfig,
        ValidatedTransportConfig,
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
/// use grafton_visca::{runtime::Runtime, runtime::TokioRuntime};
///
/// # #[cfg(feature = "runtime-tokio")]
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let runtime = TokioRuntime::from_current()?;
/// let tcp_transport = runtime
///     .connect_tcp("192.168.0.110:5678", Default::default())
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

    /// Connect to a TCP endpoint.
    ///
    /// This method creates a TCP transport using the runtime's specific
    /// implementation. The transport is configured with the provided settings.
    /// Runtime connectors have no profile default port, so `addr` must include
    /// an explicit port. Explicit IPv6 ports require brackets.
    ///
    /// The returned future is [`Send`] so standard construction can move it to
    /// an executor task before polling. Custom runtime implementations must
    /// preserve that guarantee.
    fn connect_tcp<'a>(
        &'a self,
        addr: &'a str,
        cfg: TransportConfig,
    ) -> impl Future<Output = Result<Self::TcpTransport, Error>> + Send + 'a;

    /// Connect to a UDP endpoint.
    ///
    /// This method creates a UDP transport using the runtime's specific
    /// implementation. The transport is configured with the provided settings.
    /// Runtime connectors have no profile default port, so `addr` must include
    /// an explicit port. Explicit IPv6 ports require brackets.
    ///
    /// The returned future is [`Send`] so standard construction can move it to
    /// an executor task before polling. Custom runtime implementations must
    /// preserve that guarantee.
    fn connect_udp<'a>(
        &'a self,
        addr: &'a str,
        cfg: TransportConfig,
    ) -> impl Future<Output = Result<Self::UdpTransport, Error>> + Send + 'a;

    /// Connect using an endpoint and I/O bounds already checked by the session plan.
    ///
    /// Custom runtimes can implement this hook to reuse the plan's checked values.
    /// The default delegates to the string connector for backward compatibility.
    #[allow(clippy::manual_async_fn)]
    fn connect_tcp_endpoint(
        &self,
        endpoint: NetworkEndpoint,
        config: ValidatedTransportConfig,
    ) -> impl Future<Output = Result<Self::TcpTransport, Error>> + Send + '_ {
        async move {
            self.connect_tcp(&endpoint.to_string(), *config.as_config())
                .await
        }
    }

    /// Connect using a UDP endpoint and I/O bounds already checked by the session plan.
    ///
    /// Custom runtimes can implement this hook to reuse the plan's checked values.
    /// The default delegates to the string connector for backward compatibility.
    #[allow(clippy::manual_async_fn)]
    fn connect_udp_endpoint(
        &self,
        endpoint: NetworkEndpoint,
        config: ValidatedTransportConfig,
    ) -> impl Future<Output = Result<Self::UdpTransport, Error>> + Send + '_ {
        async move {
            self.connect_udp(&endpoint.to_string(), *config.as_config())
                .await
        }
    }

    /// Connect to a serial port.
    ///
    /// Runtimes with serial support return their own serial transport. The
    /// default rejects the connection without opening a device, allowing every
    /// runtime to share the same connection entry point.
    ///
    /// The returned future is [`Send`] so standard construction can move it to
    /// an executor task before polling.
    #[cfg(feature = "transport-serial-tokio")]
    #[allow(clippy::manual_async_fn)]
    fn connect_serial(
        &self,
        _config: crate::transport::SerialConfig,
    ) -> impl Future<Output = Result<Self::SerialTransport, Error>> + Send + '_ {
        async {
            Err(Error::FeatureNotSupported {
                feature: "serial transport for this runtime",
            })
        }
    }

    /// Connect using serial device settings already checked by the session plan.
    #[cfg(feature = "transport-serial-tokio")]
    #[allow(clippy::manual_async_fn)]
    fn connect_validated_serial(
        &self,
        config: crate::transport::ValidatedSerialConfig,
    ) -> impl Future<Output = Result<Self::SerialTransport, Error>> + Send + '_ {
        async move { self.connect_serial(config.as_config().clone()).await }
    }
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

    fn transport_config(&self) -> &ValidatedTransportConfig {
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
/// use grafton_visca::{runtime::Runtime, runtime::TokioRuntime, runtime::TransportHandle};
///
/// # #[cfg(feature = "runtime-tokio")]
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let runtime = TokioRuntime::from_current()?;
/// let transport: TransportHandle<TokioRuntime> = TransportHandle::Tcp(
///     runtime
///         .connect_tcp("192.168.0.110:5678", Default::default())
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

    fn transport_config(&self) -> &ValidatedTransportConfig {
        match self {
            TransportHandle::Tcp(transport) => transport.transport_config(),
            TransportHandle::Udp(transport) => transport.transport_config(),
            #[cfg(feature = "transport-serial-tokio")]
            TransportHandle::Serial(transport) => transport.transport_config(),
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
            type Join<T>
                = <$executor as Executor>::Join<T>
            where
                T: Send + 'static;

            type Detach = <$executor as Executor>::Detach;

            type Timer = <$executor as Executor>::Timer;

            fn timer(&self, deadline: std::time::Instant) -> Self::Timer {
                self.executor.timer(deadline)
            }

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
        transport::{
            connect::preflight,
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
    delegate_executor!(TokioRuntime, TokioExecutor);

    impl Runtime for TokioRuntime {
        type TcpTransport = TcpTransport;
        type UdpTransport = UdpTransport;
        #[cfg(feature = "transport-serial-tokio")]
        type SerialTransport = crate::transport::tokio::serial::Serial;

        #[allow(clippy::manual_async_fn)]
        fn connect_tcp<'a>(
            &'a self,
            addr: &'a str,
            cfg: TransportConfig,
        ) -> impl Future<Output = Result<Self::TcpTransport, Error>> + Send + 'a {
            async move {
                let (endpoint, cfg) = preflight(addr, cfg)?;
                self.connect_tcp_endpoint(endpoint, cfg).await
            }
        }

        #[allow(clippy::manual_async_fn)]
        fn connect_udp<'a>(
            &'a self,
            addr: &'a str,
            cfg: TransportConfig,
        ) -> impl Future<Output = Result<Self::UdpTransport, Error>> + Send + 'a {
            async move {
                let (endpoint, cfg) = preflight(addr, cfg)?;
                self.connect_udp_endpoint(endpoint, cfg).await
            }
        }

        #[allow(clippy::manual_async_fn)]
        fn connect_tcp_endpoint(
            &self,
            endpoint: NetworkEndpoint,
            cfg: ValidatedTransportConfig,
        ) -> impl Future<Output = Result<Self::TcpTransport, Error>> + Send + '_ {
            let handle = self.executor.handle().clone();
            async move {
                // Own the connector on the selected runtime rather than the task's
                // ambient Tokio context, just as for the resulting session actor.
                let stream = crate::transport::tokio::connectors::connect_tcp_on(
                    &handle,
                    endpoint,
                    TcpConnectionConfig::from(cfg),
                )
                .await?;
                Ok(TcpTransport::new(stream, cfg))
            }
        }

        #[allow(clippy::manual_async_fn)]
        fn connect_udp_endpoint(
            &self,
            endpoint: NetworkEndpoint,
            cfg: ValidatedTransportConfig,
        ) -> impl Future<Output = Result<Self::UdpTransport, Error>> + Send + '_ {
            let handle = self.executor.handle().clone();
            async move {
                let socket = crate::transport::tokio::connectors::connect_udp_on(
                    &handle,
                    endpoint,
                    UdpSocketConfig::from(cfg),
                )
                .await?;
                Ok(UdpTransport::new(socket, cfg))
            }
        }

        #[cfg(feature = "transport-serial-tokio")]
        #[allow(clippy::manual_async_fn)]
        fn connect_serial(
            &self,
            config: crate::transport::SerialConfig,
        ) -> impl Future<Output = Result<Self::SerialTransport, Error>> + Send + '_ {
            async move { self.connect_validated_serial(config.validated()?).await }
        }

        #[cfg(feature = "transport-serial-tokio")]
        fn connect_validated_serial(
            &self,
            config: crate::transport::ValidatedSerialConfig,
        ) -> impl Future<Output = Result<Self::SerialTransport, Error>> + Send + '_ {
            let handle = self.executor.handle().clone();
            crate::transport::tokio::serial::Serial::connect_on(handle, config)
        }
    }

    #[cfg(test)]
    #[allow(clippy::expect_used)]
    mod tests {
        use super::*;
        use crate::transport::BufferConfig;
        use std::{future, time::Duration};

        fn invalid_buffer_config() -> TransportConfig {
            TransportConfig {
                buffer_config: BufferConfig {
                    recv_buffer_size: 65,
                    max_buffer_size: 64,
                },
                ..TransportConfig::default()
            }
        }

        fn assert_invalid_buffer_error<T>(result: Result<T, Error>) {
            assert!(matches!(
                result,
                Err(Error::InvalidRequest { reason: actual })
                    if actual.as_ref() == "transport receive buffer cannot exceed maximum buffer"
            ));
        }

        #[tokio::test]
        async fn invalid_runtime_config_returns_before_connector_io() {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind listener");
            let address = listener.local_addr().expect("listener address").to_string();
            let runtime = TokioRuntime::from_current().expect("runtime");

            let tcp = runtime.connect_tcp(&address, invalid_buffer_config()).await;
            let accepted =
                tokio::time::timeout(Duration::from_millis(100), listener.accept()).await;

            assert!(matches!(
                tcp,
                Err(Error::InvalidRequest { reason: actual })
                    if actual.as_ref() == "transport receive buffer cannot exceed maximum buffer"
            ));
            assert!(
                accepted.is_err(),
                "invalid configuration must be rejected before runtime TCP connect"
            );

            assert_invalid_buffer_error(
                runtime
                    .connect_udp("127.0.0.1:9", invalid_buffer_config())
                    .await,
            );

            // The malformed endpoint makes the buffer error's precedence over
            // runtime address parsing explicit.
            assert_invalid_buffer_error(runtime.connect_udp("[::1", invalid_buffer_config()).await);
        }

        #[cfg(feature = "transport-serial-tokio")]
        #[tokio::test]
        async fn invalid_runtime_serial_config_returns_before_device_open() {
            let runtime = TokioRuntime::from_current().expect("runtime");
            let config = crate::transport::SerialConfig::new(
                "grafton-visca-invalid-buffer-bounds-serial-device",
            )
            .with_buffer_config(BufferConfig {
                recv_buffer_size: 65,
                max_buffer_size: 64,
            });

            let result = Runtime::connect_serial(&runtime, config).await;

            assert!(matches!(
                result,
                Err(Error::InvalidRequest { reason: actual })
                    if actual.as_ref() == "transport receive buffer cannot exceed maximum buffer"
            ));
        }

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
        fn connect_tcp<'a>(
            &'a self,
            addr: &'a str,
            cfg: TransportConfig,
        ) -> impl Future<Output = Result<Self::TcpTransport, Error>> + Send + 'a {
            // The connector preflights, resolves and bounds the connect.
            TcpTransport::connect_with_config(addr, cfg)
        }

        #[allow(clippy::manual_async_fn)]
        fn connect_udp<'a>(
            &'a self,
            addr: &'a str,
            cfg: TransportConfig,
        ) -> impl Future<Output = Result<Self::UdpTransport, Error>> + Send + 'a {
            // The connector preflights, resolves and bounds the connect.
            UdpTransport::connect_with_config(addr, cfg)
        }

        #[allow(clippy::manual_async_fn)]
        fn connect_tcp_endpoint(
            &self,
            endpoint: NetworkEndpoint,
            cfg: ValidatedTransportConfig,
        ) -> impl Future<Output = Result<Self::TcpTransport, Error>> + Send + '_ {
            async move { TcpTransport::connect_endpoint(&endpoint, cfg).await }
        }

        #[allow(clippy::manual_async_fn)]
        fn connect_udp_endpoint(
            &self,
            endpoint: NetworkEndpoint,
            cfg: ValidatedTransportConfig,
        ) -> impl Future<Output = Result<Self::UdpTransport, Error>> + Send + '_ {
            async move { UdpTransport::connect_endpoint(&endpoint, cfg).await }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::transport::BufferConfig;

        fn invalid_buffer_config() -> TransportConfig {
            TransportConfig {
                buffer_config: BufferConfig {
                    recv_buffer_size: 65,
                    max_buffer_size: 64,
                },
                ..TransportConfig::default()
            }
        }

        #[test]
        fn invalid_runtime_config_is_rejected_before_connector_io() {
            smol::block_on(async {
                let runtime = SmolRuntime::new();

                let tcp = runtime
                    .connect_tcp("127.0.0.1:9", invalid_buffer_config())
                    .await;
                assert!(matches!(
                    tcp,
                    Err(Error::InvalidRequest { reason: actual })
                        if actual.as_ref()
                            == "transport receive buffer cannot exceed maximum buffer"
                ));

                // SmolRuntime forwards UDP construction to UdpTransport; the
                // transport's connector-spy test owns that shared setup seam.
                let udp = runtime
                    .connect_udp("127.0.0.1:9", invalid_buffer_config())
                    .await;
                assert!(matches!(
                    udp,
                    Err(Error::InvalidRequest { reason: actual })
                        if actual.as_ref()
                            == "transport receive buffer cannot exceed maximum buffer"
                ));
            });
        }

        #[cfg(feature = "transport-serial-tokio")]
        #[test]
        fn unsupported_serial_runtime_returns_a_send_future_without_device_io() {
            fn connect<R: Runtime>(
                runtime: &R,
            ) -> impl Future<Output = Result<R::SerialTransport, Error>> + Send + '_ {
                runtime.connect_serial(crate::transport::SerialConfig::new(
                    "grafton-visca-unsupported-runtime-serial-device",
                ))
            }

            let runtime = SmolRuntime::new();
            let result = smol::block_on(connect(&runtime));
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
