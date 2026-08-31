//! Generic async UDP transport implementation with zero-cost abstractions.
//!
//! This module provides a runtime-agnostic UDP transport that works with any
//! socket type implementing the AsyncDatagram trait.

use crate::{
    transport::{
        async_io::AsyncDatagram, builder::TransportConfig, AddressingMode, AsyncTransport,
        HasTransportConfig, ReceiveOutcome, SendSemantics,
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
        match self.recv_into_with_outcome(dst).await? {
            ReceiveOutcome::Complete { bytes } => Ok(bytes),
            ReceiveOutcome::Truncated { .. } | ReceiveOutcome::PossiblyTruncated { .. } => {
                Err(Error::ResponseTooLarge {
                    max_size: dst.len(),
                })
            }
        }
    }

    async fn recv_into_with_outcome(&mut self, dst: &mut [u8]) -> Result<ReceiveOutcome, Error> {
        // An empty UDP datagram is a valid packet, but `Ok(0)` is reserved for
        // stream EOF by the transport contract. Keep receiving until a packet
        // with payload arrives or the socket reports an error.
        loop {
            let outcome = self.socket.recv_with_outcome(dst).await?;
            let copied = outcome.copied_len();
            if copied > dst.len() {
                return Err(Error::InvalidResponse {
                    expected: "datagram receive fitting the supplied buffer".into(),
                    actual: copied.to_le_bytes().to_vec(),
                });
            }
            // A non-complete result describes a datagram that the socket
            // already consumed. Return it immediately so the owner rejects
            // the whole datagram rather than treating its copied prefix as
            // a valid VISCA frame.
            if !outcome.is_complete() || copied > 0 {
                return Ok(outcome);
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

    #[derive(Debug)]
    struct LegacyDatagram {
        payload: Vec<u8>,
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

        async fn recv(&self, buf: &mut [u8]) -> Result<usize, Error> {
            Ok(self.receive(buf)?.copied_len())
        }

        async fn recv_with_outcome(&self, buf: &mut [u8]) -> Result<ReceiveOutcome, Error> {
            self.receive(buf)
        }
    }

    // Deliberately implements only the original `recv` requirement. This is
    // the source-compatible custom-socket shape; its exact-fill result must be
    // treated conservatively until it adopts `recv_with_outcome`.
    impl AsyncDatagram for LegacyDatagram {
        fn send(&self, buf: &[u8]) -> impl Future<Output = Result<usize, Error>> + Send {
            ready(Ok(buf.len()))
        }

        async fn recv(&self, buf: &mut [u8]) -> Result<usize, Error> {
            let copied = self.payload.len().min(buf.len());
            buf[..copied].copy_from_slice(&self.payload[..copied]);
            Ok(copied)
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

    #[test]
    fn recv_rejects_a_truncated_valid_frame_prefix() {
        let socket = ScriptedDatagram::new([Ok(vec![0x90, 0x41, 0xff, 0x00])]);
        let mut transport = Udp::new(socket, TransportConfig::default());
        let mut dst = [0; 3];

        let error = block_on(transport.recv_into(&mut dst)).expect_err("truncated datagram");

        assert!(matches!(error, Error::ResponseTooLarge { max_size: 3 }));
        assert_eq!(dst, [0x90, 0x41, 0xff]);
    }

    #[test]
    fn recv_accepts_a_valid_datagram_at_the_buffer_limit() {
        let socket = ScriptedDatagram::new([Ok(vec![0x90, 0x41, 0xff])]);
        let mut transport = Udp::new(socket, TransportConfig::default());
        let mut dst = [0; 3];

        let received = block_on(transport.recv_into(&mut dst)).expect("exact-fit datagram");

        assert_eq!(received, 3);
        assert_eq!(dst, [0x90, 0x41, 0xff]);
    }

    #[test]
    fn legacy_datagram_implementation_fails_closed_on_an_exact_fill() {
        let socket = LegacyDatagram {
            payload: vec![0x90, 0x41, 0xff],
        };
        let mut transport = Udp::new(socket, TransportConfig::default());
        let mut dst = [0; 3];

        let error = block_on(transport.recv_into(&mut dst))
            .expect_err("legacy datagram cannot certify an exact fit");

        assert!(matches!(error, Error::ResponseTooLarge { max_size: 3 }));
    }
}
