//! Tokio's I/O primitives for the shared async transports.

use std::{future::Future, io, net::SocketAddr, time::Duration};

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
        async_udp::NetDatagram,
        datagram::recv_datagram,
        socket_options::{configured_tcp_socket, TcpConnectionConfig, UdpSocketConfig},
        ReceiveOutcome,
    },
    Error,
};

/// Tokio's connect primitives.
#[derive(Debug, Clone, Copy)]
pub struct TokioNet;

impl AsyncNet for TokioNet {
    type TcpStream = TcpStream;
    type UdpSocket = UdpSocket;

    async fn lookup(endpoint: String) -> io::Result<Vec<SocketAddr>> {
        Ok(tokio::net::lookup_host(endpoint).await?.collect())
    }

    async fn connect_tcp(
        address: SocketAddr,
        options: TcpConnectionConfig,
    ) -> io::Result<TcpStream> {
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

    async fn sleep(duration: Duration) {
        tokio::time::sleep(duration).await;
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

    async fn flush(&mut self) -> Result<(), Error> {
        Ok(self.stream.flush().await?)
    }
}

impl NetStream for TokioTcpStream {
    type Net = TokioNet;

    fn from_connected(stream: TcpStream) -> Self {
        Self { stream }
    }
}

impl NetDatagram for UdpSocket {
    type Net = TokioNet;
}

/// Connect TCP on an explicitly selected Tokio runtime.
///
/// `TokioRuntime::from_handle` has already selected a runtime, so its DNS,
/// timer and socket work must execute on that handle even if its future is
/// awaited elsewhere.
pub(crate) async fn connect_tcp_on(
    handle: &tokio::runtime::Handle,
    address: String,
    config: TcpConnectionConfig,
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
    address: String,
    config: UdpSocketConfig,
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
    struct AbortOnDrop<T>(tokio::task::JoinHandle<T>);

    impl<T> Drop for AbortOnDrop<T> {
        fn drop(&mut self) {
            self.0.abort();
        }
    }

    let mut task = AbortOnDrop(handle.spawn(future));
    (&mut task.0).await.map_err(|error| Error::InvalidState {
        reason: format!("selected Tokio runtime stopped while connecting {kind}: {error}").into(),
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
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

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
    fn full_accept_queue() -> (std::net::TcpListener, std::net::TcpStream) {
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
        let listener: std::net::TcpListener = listener.into();
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
        let mut config = crate::transport::TransportConfig::for_tcp();
        config.connect_timeout = Duration::from_secs(10);
        assert!(
            tokio::time::timeout(
                Duration::from_millis(200),
                runtime.connect_tcp(&address, config)
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
