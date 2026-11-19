//! Tokio's I/O primitives for the shared async transports.

use std::{
    future::Future,
    io,
    net::{SocketAddr, ToSocketAddrs},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, UdpSocket},
};

use crate::{
    transport::{
        async_connect::{self, AsyncNet},
        async_io::{
            AsyncDatagram, AsyncReadExt as AsyncReadExtTrait, AsyncWriteExt as AsyncWriteExtTrait,
        },
        async_tcp::NetStream,
        datagram::recv_datagram,
        socket_options::configured_tcp_socket,
        ReceiveOutcome, TcpConnect, UdpConnect,
    },
    Error,
};

/// Tokio's connect primitives.
#[derive(Debug, Clone, Copy)]
pub struct TokioNet;

tokio::task_local! {
    static CONNECT_ABANDONED: Arc<AtomicBool>;
}

struct AbortOnDrop<T> {
    task: tokio::task::JoinHandle<T>,
    abandoned: Arc<AtomicBool>,
}

impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        self.abandoned.store(true, Ordering::Release);
        self.task.abort();
    }
}

async fn lookup_hostname(
    endpoint: String,
    resolve: impl FnOnce(String) -> io::Result<Vec<SocketAddr>> + Send + 'static,
) -> io::Result<Vec<SocketAddr>> {
    let abandoned = CONNECT_ABANDONED
        .try_with(Arc::clone)
        .unwrap_or_else(|_| Arc::new(AtomicBool::new(false)));
    let worker_abandoned = Arc::clone(&abandoned);
    let mut lookup = AbortOnDrop {
        task: tokio::task::spawn_blocking(move || {
            // The connect future can be dropped before Tokio polls the
            // selected-runtime task's cancellation.
            if worker_abandoned.load(Ordering::Acquire) {
                return Err(io::ErrorKind::Interrupted.into());
            }
            resolve(endpoint)
        }),
        abandoned,
    };
    let result = (&mut lookup.task).await.map_err(io::Error::from)?;
    if lookup.abandoned.load(Ordering::Acquire) {
        return Err(io::ErrorKind::Interrupted.into());
    }
    result
}

impl AsyncNet for TokioNet {
    type TcpStream = TcpStream;
    type UdpSocket = UdpSocket;

    async fn lookup(endpoint: String) -> io::Result<Vec<SocketAddr>> {
        #[cfg(test)]
        if let Ok(probe) = tests::LOOKUP_PROBE.try_with(Arc::clone) {
            return probe.lookup(endpoint).await;
        }
        lookup_hostname(endpoint, |endpoint| {
            Ok(endpoint.to_socket_addrs()?.collect())
        })
        .await
    }

    async fn connect_tcp(address: SocketAddr, options: TcpConnect) -> io::Result<TcpStream> {
        let socket = configured_tcp_socket(address, options)?;
        socket.set_nonblocking(true)?;
        tokio::net::TcpSocket::from_std_stream(socket.into())
            .connect(address)
            .await
    }

    async fn bind_udp(address: SocketAddr) -> io::Result<UdpSocket> {
        UdpSocket::bind(address).await
    }

    async fn connect_udp(socket: &UdpSocket, address: SocketAddr) -> io::Result<()> {
        socket.connect(address).await
    }

    #[cfg(test)]
    fn tcp_socket(stream: &TcpStream) -> socket2::SockRef<'_> {
        socket2::SockRef::from(stream)
    }

    fn udp_socket(socket: &UdpSocket) -> socket2::SockRef<'_> {
        socket2::SockRef::from(socket)
    }

    fn now() -> Instant {
        tokio::time::Instant::now().into_std()
    }

    async fn sleep_until(deadline: Instant) {
        tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
    }
}

/// Tokio TCP stream read and written directly through the whole transport.
#[derive(Debug)]
pub struct TokioTcpStream {
    stream: TcpStream,
}

impl AsyncReadExtTrait for TokioTcpStream {
    async fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> Result<usize, Error> {
        Ok(self.stream.read(buf).await?)
    }
}

impl AsyncWriteExtTrait for TokioTcpStream {
    async fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> Result<(), Error> {
        Ok(self.stream.write_all(buf).await?)
    }
}

impl NetStream for TokioTcpStream {
    type Net = TokioNet;

    fn from_connected(stream: TcpStream) -> Self {
        Self { stream }
    }
}

/// Connect TCP on an explicitly selected Tokio runtime.
///
/// `TokioRuntime::from_handle` has already selected a runtime, so its DNS,
/// timer and socket work must execute on that handle even if its future is
/// awaited elsewhere.
pub(crate) async fn connect_tcp_on(
    handle: &tokio::runtime::Handle,
    address: crate::transport::NetworkEndpoint,
    config: TcpConnect,
) -> Result<TokioTcpStream, Error> {
    connect_on(handle, "TCP", async move {
        async_connect::connect_tcp::<TokioNet>(&address, config)
            .await
            .map(TokioTcpStream::from_connected)
    })
    .await
}

/// Connect UDP on an explicitly selected Tokio runtime.
///
/// See [`connect_tcp_on`] for why runtime-bound callers use this rather than
/// the ambient connector directly.
pub(crate) async fn connect_udp_on(
    handle: &tokio::runtime::Handle,
    address: crate::transport::NetworkEndpoint,
    config: UdpConnect,
) -> Result<UdpSocket, Error> {
    connect_on(handle, "UDP", async move {
        async_connect::connect_udp::<TokioNet>(&address, config).await
    })
    .await
}

/// Keep selected-runtime connector work owned by the caller's future.
async fn connect_on<T: Send + 'static>(
    handle: &tokio::runtime::Handle,
    kind: &'static str,
    future: impl Future<Output = Result<T, Error>> + Send + 'static,
) -> Result<T, Error> {
    let abandoned = Arc::new(AtomicBool::new(false));
    let mut connector = AbortOnDrop {
        task: handle.spawn(CONNECT_ABANDONED.scope(Arc::clone(&abandoned), future)),
        abandoned,
    };
    (&mut connector.task)
        .await
        .map_err(|error| Error::InvalidState {
            reason: format!("selected Tokio runtime stopped while connecting {kind}: {error}")
                .into(),
        })?
}

/// Implement AsyncDatagram for tokio's UdpSocket
impl AsyncDatagram for UdpSocket {
    async fn send(&self, buf: &[u8]) -> Result<usize, Error> {
        Ok(UdpSocket::send(self, buf).await?)
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<ReceiveOutcome, Error> {
        Ok(self
            .async_io(
                tokio::io::Interest::READABLE | tokio::io::Interest::ERROR,
                || recv_datagram(self, buf),
            )
            .await?)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::{
        net::TcpListener,
        sync::{atomic::AtomicUsize, mpsc, Mutex},
        time::Duration,
    };
    use tokio::sync::{oneshot, Notify};

    type Resolver = Box<dyn FnOnce(String) -> io::Result<Vec<SocketAddr>> + Send>;

    pub(super) struct LookupProbe {
        resolver: Mutex<Option<Resolver>>,
        queued: Notify,
    }

    tokio::task_local! {
        pub(super) static LOOKUP_PROBE: Arc<LookupProbe>;
    }

    impl LookupProbe {
        fn new(
            resolver: impl FnOnce(String) -> io::Result<Vec<SocketAddr>> + Send + 'static,
        ) -> Arc<Self> {
            Arc::new(Self {
                resolver: Mutex::new(Some(Box::new(resolver))),
                queued: Notify::new(),
            })
        }

        pub(super) async fn lookup(&self, endpoint: String) -> io::Result<Vec<SocketAddr>> {
            let resolve = self.resolver.lock().unwrap().take().unwrap();
            let mut lookup = std::pin::pin!(lookup_hostname(endpoint, resolve));
            let ready = futures_lite::future::poll_once(lookup.as_mut()).await;
            self.queued.notify_one();
            match ready {
                Some(result) => result,
                None => lookup.await,
            }
        }

        async fn connect(
            self: Arc<Self>,
            endpoint: String,
            timeout: Duration,
        ) -> Result<TcpStream, Error> {
            let endpoint = crate::transport::NetworkEndpoint::parse(&endpoint, None).unwrap();
            let config = TcpConnect::default().with_connect_timeout(timeout).unwrap();
            connect_on(
                &tokio::runtime::Handle::current(),
                "TCP",
                LOOKUP_PROBE.scope(self, async move {
                    async_connect::connect_tcp::<TokioNet>(&endpoint, config).await
                }),
            )
            .await
        }
    }

    #[test]
    fn dropped_and_expired_connects_skip_queued_hostname_resolution() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .max_blocking_threads(1)
            .enable_all()
            .start_paused(true)
            .build()
            .unwrap();
        runtime.block_on(async {
            let (busy, worker_started) = oneshot::channel();
            let (release_worker, release) = mpsc::channel::<()>();
            let worker = tokio::task::spawn_blocking(move || {
                busy.send(()).unwrap();
                let _ = release.recv();
            });
            worker_started.await.unwrap();

            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let hostname = format!("camera.invalid:{}", address.port());
            let calls = Arc::new(AtomicUsize::new(0));
            let new_probe = || {
                let calls = Arc::clone(&calls);
                LookupProbe::new(move |_| {
                    calls.fetch_add(1, Ordering::Relaxed);
                    Ok(vec![address])
                })
            };
            let short = Duration::from_millis(100);
            let long = Duration::from_secs(10);
            let expired = new_probe();
            let connector = tokio::spawn(Arc::clone(&expired).connect(hostname.clone(), short));
            expired.queued.notified().await;
            tokio::time::advance(short).await;
            assert!(matches!(
                connector.await.unwrap(),
                Err(Error::Timeout { .. })
            ));

            let live = new_probe();
            let connector = tokio::spawn(Arc::clone(&live).connect(hostname.clone(), long));
            live.queued.notified().await;

            let dropped = new_probe();
            let mut dropped_connector = Box::pin(Arc::clone(&dropped).connect(hostname, long));
            assert!(futures_lite::future::poll_once(dropped_connector.as_mut())
                .await
                .is_none());
            dropped.queued.notified().await;
            drop(dropped_connector);

            // Drain the blocking queue before Tokio can poll the aborted
            // connector. Dropping it must signal its queued job immediately.
            let (barrier, drained) = mpsc::channel();
            let barrier = tokio::task::spawn_blocking(move || barrier.send(()).unwrap());
            release_worker.send(()).unwrap();
            drained.recv_timeout(Duration::from_secs(5)).unwrap();
            assert_eq!(calls.load(Ordering::Relaxed), 1);
            barrier.await.unwrap();
            worker.await.unwrap();
            assert!(connector.await.unwrap().is_ok());
            assert_eq!(calls.load(Ordering::Relaxed), 1);

            assert!(new_probe().connect(address.to_string(), long).await.is_ok());
            assert_eq!(calls.load(Ordering::Relaxed), 1);
        });
    }

    #[test]
    fn dropping_connect_discards_an_executing_lookup_result() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .max_blocking_threads(1)
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let address = listener.local_addr().unwrap();
            let (started, resolver_started) = oneshot::channel();
            let (release_resolver, resume) = mpsc::channel::<()>();
            let probe = LookupProbe::new(move |_| {
                started.send(()).unwrap();
                resume.recv().unwrap();
                Ok(vec![address])
            });
            let handle = tokio::runtime::Handle::current();
            let baseline = handle.metrics().num_alive_tasks();
            let mut connector = Box::pin(probe.connect(
                format!("camera.invalid:{}", address.port()),
                Duration::from_secs(10),
            ));
            assert!(futures_lite::future::poll_once(connector.as_mut())
                .await
                .is_none());
            resolver_started.await.unwrap();
            drop(connector);
            release_resolver.send(()).unwrap();
            tokio::task::spawn_blocking(|| {}).await.unwrap();
            wait_for_task_cleanup(&handle, baseline).await;
            assert!(matches!(listener.accept(), Err(e) if e.kind() == io::ErrorKind::WouldBlock));
        });
    }

    struct Dropped(Arc<AtomicBool>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    async fn wait_for_task_cleanup(handle: &tokio::runtime::Handle, baseline: usize) {
        // A completed task may publish its result before scheduler cleanup
        // updates this metric. Observe cleanup without spawning a barrier task
        // that would itself race the same instantaneous sample.
        let cleaned_up = tokio::time::timeout(Duration::from_secs(1), async {
            while handle.metrics().num_alive_tasks() != baseline {
                tokio::task::yield_now().await;
            }
        })
        .await;
        assert!(
            cleaned_up.is_ok(),
            "connector task cleanup exceeded its bound: {} alive, expected {baseline}",
            handle.metrics().num_alive_tasks(),
        );
    }

    #[tokio::test]
    async fn dropping_connector_future_aborts_its_selected_runtime_task() {
        let dropped = Arc::new(AtomicBool::new(false));
        let guard = Dropped(Arc::clone(&dropped));
        let handle = tokio::runtime::Handle::current();
        let baseline = handle.metrics().num_alive_tasks();
        let mut connector = Box::pin(connect_on(&handle, "test", async move {
            let _guard = guard;
            std::future::pending::<Result<(), Error>>().await
        }));
        assert!(futures_lite::future::poll_once(connector.as_mut())
            .await
            .is_none());
        tokio::task::yield_now().await;
        assert_eq!(handle.metrics().num_alive_tasks(), baseline + 1);
        drop(connector);
        wait_for_task_cleanup(&handle, baseline).await;
        assert!(dropped.load(Ordering::SeqCst));
    }
    #[cfg(target_os = "linux")]
    fn full_accept_queue() -> (TcpListener, std::net::TcpStream) {
        let listener = socket2::Socket::new(
            socket2::Domain::IPV4,
            socket2::Type::STREAM,
            Some(socket2::Protocol::TCP),
        )
        .unwrap();
        listener
            .bind(&"127.0.0.1:0".parse::<SocketAddr>().unwrap().into())
            .unwrap();
        listener.listen(0).unwrap();
        let listener: TcpListener = listener.into();
        let filler = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        listener.set_nonblocking(true).unwrap();
        (listener, filler)
    }

    #[cfg(target_os = "linux")]
    async fn assert_no_abandoned_connection<R: crate::runtime::Runtime>(
        runtime: &R,
        selected: Option<(&tokio::runtime::Handle, usize)>,
    ) {
        let (listener, _filler) = full_accept_queue();
        let address = listener.local_addr().unwrap().to_string();
        let config = TcpConnect::default()
            .with_connect_timeout(Duration::from_secs(10))
            .unwrap();
        assert!(
            tokio::time::timeout(
                Duration::from_millis(200),
                runtime.connect_tcp(
                    crate::transport::NetworkEndpoint::parse(&address, None).unwrap(),
                    config,
                    crate::transport::IoBounds::default(),
                )
            )
            .await
            .is_err(),
            "the full queue keeps the connector pending"
        );
        if let Some((handle, baseline)) = selected {
            wait_for_task_cleanup(handle, baseline).await;
        }
        let (_accepted_filler, _) = listener.accept().unwrap();
        // An abandoned live socket retransmits its SYN after about one second.
        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert!(
            matches!(listener.accept(), Err(error) if error.kind() == io::ErrorKind::WouldBlock),
            "a dropped connect must never reach the listener after the queue is freed"
        );
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn abandoned_tokio_connect_does_not_reach_the_camera() {
        let selected = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let handle = selected.handle().clone();
        let baseline = handle.metrics().num_alive_tasks();
        let runtime = crate::runtime::TokioRuntime::from_handle(handle.clone());
        assert_no_abandoned_connection(&runtime, Some((&handle, baseline))).await;
        wait_for_task_cleanup(&handle, baseline).await;
        selected.shutdown_background();
    }

    #[cfg(all(target_os = "linux", feature = "runtime-smol"))]
    #[tokio::test]
    async fn abandoned_smol_connect_does_not_reach_the_camera() {
        assert_no_abandoned_connection(&crate::runtime::SmolRuntime::new(), None).await;
    }
}
