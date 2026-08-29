//! Generic async UDP transport implementation with zero-cost abstractions.
//!
//! This module provides a runtime-agnostic UDP transport that works with any
//! socket type implementing the AsyncDatagram trait.

use crate::{
    transport::{
        async_io::AsyncDatagram, builder::TransportConfig, AddressingMode, AsyncTransport,
        HasTransportConfig, SendSemantics,
    },
    Error,
};

/// Generic UDP transport for async VISCA communication.
///
/// This transport uses native async functions without boxing and supports
/// both IPv4 and IPv6 addresses. It works with any socket type implementing
/// the AsyncDatagram trait (tokio, smol, etc.).
#[derive(Debug)]
pub struct Udp<S: AsyncDatagram> {
    socket: S,
    config: TransportConfig,
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
    pub fn new(socket: S, config: TransportConfig) -> Self {
        Self { socket, config }
    }

    /// Get the transport configuration.
    pub fn config(&self) -> &TransportConfig {
        &self.config
    }
}

impl<S: AsyncDatagram> AsyncTransport for Udp<S> {
    async fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        // Send directly - retry logic is handled at the runtime/scheduler level
        self.socket.send(data).await?;
        Ok(())
    }

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        // An empty UDP datagram is a valid packet, but `Ok(0)` is reserved for
        // stream EOF by the transport contract. Keep receiving until a packet
        // with payload arrives or the socket reports an error.
        loop {
            let n = self.socket.recv(dst).await?;
            if n > 0 {
                return Ok(n);
            }
            cooperative_yield().await;
        }
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        Some(AddressingMode::Ip)
    }

    fn send_semantics(&self) -> SendSemantics {
        // UDP sends are atomic at the datagram boundary - a failed send
        // does not affect the state for subsequent sends
        SendSemantics::Datagram
    }
}

impl<S: AsyncDatagram> HasTransportConfig for Udp<S> {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }

    fn standard_transport_kind(&self) -> Option<crate::camera::TransportKind> {
        Some(crate::camera::TransportKind::Udp)
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
    }

    impl AsyncDatagram for ScriptedDatagram {
        fn send(&self, buf: &[u8]) -> impl Future<Output = Result<usize, Error>> + Send {
            ready(Ok(buf.len()))
        }

        async fn recv(&self, buf: &mut [u8]) -> Result<usize, Error> {
            let datagram = self
                .receives
                .lock()
                .expect("scripted datagram queue is not poisoned")
                .pop_front()
                .expect("scripted datagram queue should contain a receive result");
            let datagram = datagram?;
            let len = datagram.len().min(buf.len());
            buf[..len].copy_from_slice(&datagram[..len]);
            Ok(len)
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
        let mut transport = Udp::new(socket, TransportConfig::default());
        let mut dst = [0; 16];

        let received = block_on(transport.recv_into(&mut dst)).expect("valid datagram");

        assert_eq!(received, 5);
        assert_eq!(&dst[..received], b"valid");
    }

    #[test]
    fn recv_yields_before_polling_after_empty_datagram() {
        let socket = ScriptedDatagram::new([Ok(Vec::new()), Ok(b"valid".to_vec())]);
        let mut transport = Udp::new(socket, TransportConfig::default());
        let mut dst = [0; 16];
        let counting_waker = Arc::new(CountingWaker::default());
        let waker = Waker::from(Arc::clone(&counting_waker));
        let mut context = Context::from_waker(&waker);

        let received = {
            let mut receive = std::pin::pin!(transport.recv_into(&mut dst));
            assert!(matches!(receive.as_mut().poll(&mut context), Poll::Pending));
            assert_eq!(counting_waker.wake_count(), 1);

            match receive.as_mut().poll(&mut context) {
                Poll::Ready(Ok(received)) => received,
                other => panic!("expected valid datagram after cooperative yield, got {other:?}"),
            }
        };
        assert_eq!(received, 5);
        assert_eq!(&dst[..received], b"valid");
    }

    #[test]
    fn recv_yields_after_each_empty_datagram() {
        let socket = ScriptedDatagram::new([Ok(Vec::new()), Ok(Vec::new()), Ok(b"valid".to_vec())]);
        let mut transport = Udp::new(socket, TransportConfig::default());
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
                Poll::Ready(Ok(received)) => received,
                other => panic!("expected valid datagram after cooperative yields, got {other:?}"),
            }
        };
        assert_eq!(received, 5);
        assert_eq!(&dst[..received], b"valid");
    }
}
