//! Generic async UDP transport, written once for every async runtime.
//!
//! A runtime supplies its connected socket. This module applies the shared
//! datagram receive policy from [`crate::transport::datagram`].

#[cfg(feature = "runtime-smol")]
use crate::transport::async_connect::AsyncNet;

use crate::{
    transport::{
        async_io::AsyncDatagram, builder::IoBounds, datagram::deliverable, AsyncTransport,
        HasTransportConfig, ReceiveOutcome,
    },
    Error,
};

/// A runtime's UDP socket as used by [`Udp`].
#[cfg(feature = "runtime-smol")]
pub trait NetDatagram: AsyncDatagram + Sized {
    /// The runtime providing the connect primitives.
    type Net: AsyncNet<UdpSocket = Self>;
}

/// Generic UDP transport for async VISCA communication.
///
/// This transport supports both IPv4 and IPv6 addresses and works with any
/// socket type implementing [`AsyncDatagram`].
#[derive(Debug)]
pub struct Udp<S: AsyncDatagram> {
    socket: S,
    config: IoBounds,
}

/// Yield once to the current executor without depending on a runtime.
async fn cooperative_yield() {
    let mut yielded = false;
    std::future::poll_fn(move |context| {
        if yielded {
            std::task::Poll::Ready(())
        } else {
            yielded = true;
            context.waker().wake_by_ref();
            std::task::Poll::Pending
        }
    })
    .await;
}

impl<S: AsyncDatagram> Udp<S> {
    /// Create a new UDP transport from a connected socket.
    ///
    /// The socket should already be connected to the remote endpoint.
    pub(crate) fn new(socket: S, config: IoBounds) -> Self {
        Self { socket, config }
    }
}

#[cfg(feature = "runtime-smol")]
impl<S: NetDatagram> Udp<S> {
    pub(crate) async fn connect_endpoint(
        endpoint: &crate::transport::NetworkEndpoint,
        options: crate::transport::UdpConnect,
        io_bounds: IoBounds,
    ) -> Result<Self, Error> {
        let socket =
            crate::transport::async_connect::connect_udp::<S::Net>(endpoint, options).await?;
        Ok(Self::new(socket, io_bounds))
    }
}

impl<S: AsyncDatagram> AsyncTransport for Udp<S> {
    async fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        // Send directly - retry logic is handled at the runtime/scheduler level
        self.socket.send(data).await?;
        Ok(())
    }

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<ReceiveOutcome, Error> {
        // Empty datagrams are skipped (see `transport::datagram`), yielding
        // before polling again. A non-complete outcome describes a datagram
        // the socket already consumed: return it so the owner discards the
        // whole datagram rather than decoding its prefix.
        loop {
            let outcome = self.socket.recv(dst).await?;
            if let Some(outcome) = deliverable(outcome, dst.len())? {
                return Ok(outcome);
            }
            cooperative_yield().await;
        }
    }
}

impl<S: AsyncDatagram> HasTransportConfig for Udp<S> {
    fn identity(&self) -> crate::transport::TransportIdentity<'_> {
        crate::transport::TransportIdentity::Udp
    }

    fn io_bounds(&self) -> &IoBounds {
        &self.config
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use std::{
        collections::VecDeque,
        future::{ready, Future},
        sync::{atomic::AtomicUsize, Arc, Mutex},
        task::{Context, Poll, Wake, Waker},
    };

    use futures_lite::future::block_on;

    use super::*;

    type ReceiveQueue = Arc<Mutex<VecDeque<Result<Vec<u8>, Error>>>>;

    #[derive(Debug)]
    struct ScriptedDatagram {
        receives: ReceiveQueue,
    }

    impl ScriptedDatagram {
        fn new(receives: impl IntoIterator<Item = Result<Vec<u8>, Error>>) -> Self {
            Self {
                receives: Arc::new(Mutex::new(receives.into_iter().collect())),
            }
        }

        fn receive(&self, buf: &mut [u8]) -> Result<ReceiveOutcome, Error> {
            let datagram = self
                .receives
                .lock()
                .expect("scripted datagram queue is not poisoned")
                .pop_front()
                .expect("scripted datagram queue should contain a receive result");
            let datagram = datagram?;
            let copied = datagram.len().min(buf.len());
            buf[..copied].copy_from_slice(&datagram[..copied]);
            Ok(if datagram.len() > buf.len() {
                ReceiveOutcome::Truncated { copied }
            } else {
                ReceiveOutcome::Complete { bytes: copied }
            })
        }
    }

    impl AsyncDatagram for ScriptedDatagram {
        fn send(&self, buf: &[u8]) -> impl Future<Output = Result<usize, Error>> + Send {
            ready(Ok(buf.len()))
        }

        async fn recv(&self, buf: &mut [u8]) -> Result<ReceiveOutcome, Error> {
            self.receive(buf)
        }
    }

    #[derive(Debug, Default)]
    struct CountingWaker {
        wakes: AtomicUsize,
    }

    impl CountingWaker {
        fn wake_count(&self) -> usize {
            self.wakes.load(std::sync::atomic::Ordering::Relaxed)
        }
    }

    impl Wake for CountingWaker {
        fn wake(self: Arc<Self>) {
            self.wakes
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.wakes
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    #[test]
    fn recv_discards_empty_datagram_before_valid_datagram() {
        let socket = ScriptedDatagram::new([Ok(Vec::new()), Ok(b"valid".to_vec())]);
        let mut transport = Udp::new(socket, IoBounds::default());
        let mut dst = [0; 16];

        let received = block_on(transport.recv_into(&mut dst)).expect("valid datagram");

        assert_eq!(received, ReceiveOutcome::complete(5));
        assert_eq!(&dst[..5], b"valid");
    }

    #[test]
    fn recv_yields_before_polling_after_empty_datagram() {
        let socket = ScriptedDatagram::new([Ok(Vec::new()), Ok(b"valid".to_vec())]);
        let mut transport = Udp::new(socket, IoBounds::default());
        let mut dst = [0; 16];
        let counting_waker = Arc::new(CountingWaker::default());
        let waker = Waker::from(Arc::clone(&counting_waker));
        let mut context = Context::from_waker(&waker);

        let received = {
            let mut receive = std::pin::pin!(transport.recv_into(&mut dst));
            assert!(matches!(receive.as_mut().poll(&mut context), Poll::Pending));
            assert_eq!(counting_waker.wake_count(), 1);

            match receive.as_mut().poll(&mut context) {
                Poll::Ready(Ok(received)) => received.copied_len(),
                other => panic!("expected valid datagram after cooperative yield, got {other:?}"),
            }
        };
        assert_eq!(received, 5);
        assert_eq!(&dst[..received], b"valid");
    }

    #[test]
    fn recv_yields_after_each_empty_datagram() {
        let socket = ScriptedDatagram::new([Ok(Vec::new()), Ok(Vec::new()), Ok(b"valid".to_vec())]);
        let mut transport = Udp::new(socket, IoBounds::default());
        let mut dst = [0; 16];
        let counting_waker = Arc::new(CountingWaker::default());
        let waker = Waker::from(Arc::clone(&counting_waker));
        let mut context = Context::from_waker(&waker);

        let received = {
            let mut receive = std::pin::pin!(transport.recv_into(&mut dst));
            assert!(matches!(receive.as_mut().poll(&mut context), Poll::Pending));
            assert!(matches!(receive.as_mut().poll(&mut context), Poll::Pending));
            assert_eq!(counting_waker.wake_count(), 2);

            match receive.as_mut().poll(&mut context) {
                Poll::Ready(Ok(received)) => received.copied_len(),
                other => panic!("expected valid datagram after cooperative yields, got {other:?}"),
            }
        };
        assert_eq!(received, 5);
        assert_eq!(&dst[..received], b"valid");
    }

    #[test]
    fn recv_rejects_a_truncated_valid_frame_prefix() {
        let socket = ScriptedDatagram::new([Ok(vec![0x90, 0x41, 0xff, 0x00])]);
        let mut transport = Udp::new(socket, IoBounds::default());
        let mut dst = [0; 3];

        let truncated = block_on(transport.recv_into(&mut dst)).expect("consumed datagram");

        assert_eq!(truncated, ReceiveOutcome::truncated(3));
        assert_eq!(dst, [0x90, 0x41, 0xff]);
    }

    #[test]
    fn recv_accepts_a_valid_datagram_at_the_buffer_limit() {
        let socket = ScriptedDatagram::new([Ok(vec![0x90, 0x41, 0xff])]);
        let mut transport = Udp::new(socket, IoBounds::default());
        let mut dst = [0; 3];

        let received = block_on(transport.recv_into(&mut dst)).expect("exact-fit datagram");

        assert_eq!(received, ReceiveOutcome::complete(3));
        assert_eq!(dst, [0x90, 0x41, 0xff]);
    }
}

/// The OS truncation policy of every async runtime's UDP socket, run as one
/// table through each runtime's `AsyncDatagram` implementation.
#[cfg(test)]
#[allow(clippy::expect_used)]
mod os_truncation {
    use super::*;

    /// (datagram, receive capacity, expected outcome)
    const CASES: [(&[u8], usize, ReceiveOutcome); 2] = [
        // A valid ACK prefix must not certify a larger datagram.
        (
            &[0x90, 0x41, 0xff, 0x00],
            3,
            ReceiveOutcome::Truncated { copied: 3 },
        ),
        // An exact-size datagram is complete.
        (
            &[0x90, 0x41, 0xff],
            3,
            ReceiveOutcome::Complete { bytes: 3 },
        ),
    ];

    /// A connected receiver (non-blocking, for a runtime) and its sender.
    fn std_pair() -> (std::net::UdpSocket, std::net::UdpSocket) {
        let receiver = std::net::UdpSocket::bind("127.0.0.1:0").expect("bind receiver");
        let sender = std::net::UdpSocket::bind("127.0.0.1:0").expect("bind sender");
        receiver
            .connect(sender.local_addr().expect("sender address"))
            .expect("connect receiver");
        sender
            .connect(receiver.local_addr().expect("receiver address"))
            .expect("connect sender");
        receiver
            .set_nonblocking(true)
            .expect("non-blocking receiver");
        (receiver, sender)
    }

    async fn check<S: AsyncDatagram>(wrap: impl Fn(std::net::UdpSocket) -> S) {
        for (datagram, capacity, expected) in CASES {
            let (receiver, sender) = std_pair();
            let receiver = wrap(receiver);
            sender.send(datagram).expect("send datagram");
            let mut destination = vec![0; capacity];
            let outcome = receiver
                .recv(&mut destination)
                .await
                .expect("receive datagram");
            assert_eq!(outcome, expected, "datagram {datagram:02X?}");
            assert_eq!(destination, datagram[..capacity]);
        }
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn tokio_udp_socket() {
        check(|socket| tokio::net::UdpSocket::from_std(socket).expect("tokio socket")).await;
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_udp_socket() {
        smol::block_on(check(|socket| {
            smol::net::UdpSocket::try_from(socket).expect("smol socket")
        }));
    }
}
